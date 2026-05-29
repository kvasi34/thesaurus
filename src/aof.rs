use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use log::{debug, error};

/// Appends write commands to the AOF file so the dataset can be reconstructed on restart.
///
/// Cheaply cloneable — all clones share the same underlying file handle via [`Arc`].
#[derive(Clone, Debug)]
pub(crate) struct AofWriter {
    writer: Arc<Mutex<Writer>>,
    fsync_mode: AppendFSyncMode,
}

#[derive(Debug)]
struct Writer {
    file: io::BufWriter<File>,
}

/// Controls how often the AOF file is fsynced to disk.
///
/// Higher durability means lower throughput — `Always` is the safest but slowest,
/// `No` is the fastest but risks losing up to ~30 seconds of writes on a crash.
#[derive(Clone, Debug)]
pub(crate) enum AppendFSyncMode {
    /// fsync after every write command. At most zero data loss; highest I/O cost.
    Always,
    /// fsync once per second in the background. At most one second of data loss.
    EverySec,
    /// Never fsync explicitly — let the OS decide. Fastest; loss window is OS-dependent (~30s).
    No,
}

impl AofWriter {
    /// Opens or creates the AOF file at `path`, creating any missing parent directories.
    pub fn new(path: &Path, fsync_mode: AppendFSyncMode) -> io::Result<Self> {
        // Create any intermidiate directories in the complete path
        // e.g. path = /foo/bar/appendonly.aof, foo will be created if it doesn't exist
        //
        // filter(|p| !p.as_os_str().is_empty()) guards against create_dir_all("") when
        // the path has no directory component (e.g. a bare filename)
        if let Some(dir) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }

        let file = OpenOptions::new().create(true).append(true).open(path)?;

        Ok(AofWriter {
            writer: Arc::new(Mutex::new(Writer {
                file: io::BufWriter::new(file),
            })),
            fsync_mode,
        })
    }

    /// Appends a RESP2-encoded command to the AOF file.
    ///
    /// Always flushes to the kernel page cache. Fsyncs to disk immediately for
    /// [`AppendFSyncMode::Always`]; the `everysec` task handles the periodic fsync otherwise.
    pub fn append(&mut self, cmd_bytes: &[u8]) -> io::Result<()> {
        debug!("Writing to AOF: {:?}", cmd_bytes);
        let mut guard = self.writer.lock().unwrap();
        guard.file.write_all(cmd_bytes)?;
        guard.file.flush()?;

        // sync_data pushes the kernel buffer to physical disk; when to do that is mode-specific
        if matches!(self.fsync_mode, AppendFSyncMode::Always) {
            guard.file.get_ref().sync_data()?;
        }

        Ok(())
    }

    /// Spawns a background task that fsyncs the AOF file to disk once per second.
    ///
    /// No-op if the mode is not [`AppendFSyncMode::EverySec`]. The task runs until
    /// the tokio runtime shuts down. For a clean final fsync on shutdown, see the
    /// linked issue for cancellation token support.
    pub fn spawn_fsync_task(&self) {
        if !matches!(self.fsync_mode, AppendFSyncMode::EverySec) {
            return;
        }

        let writer = Arc::clone(&self.writer);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                let guard = writer.lock().unwrap();
                if let Err(e) = guard.file.get_ref().sync_data() {
                    error!("AOF everysec fsync failed: {}", e);
                }
            }
        });
    }
}

/// Opens the AOF file and starts the background fsync task if needed.
///
/// Returns `None` when `enabled` is `false` so callers stay unaware of AOF internals.
pub(crate) fn open(
    enabled: bool,
    dirname: &str,
    filename: &str,
    fsync_mode: AppendFSyncMode,
) -> io::Result<Option<AofWriter>> {
    if !enabled {
        return Ok(None);
    }
    let path = resolve_aof_path(dirname, filename);
    let writer = AofWriter::new(&path, fsync_mode).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!("Failed to open AOF file at '{}': {}", path.display(), e),
        )
    })?;
    writer.spawn_fsync_task();
    Ok(Some(writer))
}

/// Builds the full path to the AOF file from the configured directory and filename.
pub(crate) fn resolve_aof_path(dirname: &str, filename: &str) -> PathBuf {
    Path::new(dirname).join(filename)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_append_writes_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("appendonly.aof");
        let mut writer = AofWriter::new(&path, AppendFSyncMode::No).unwrap();

        writer
            .append(b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n")
            .unwrap();

        assert_eq!(
            fs::read(&path).unwrap(),
            b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n"
        );
    }

    #[test]
    fn test_append_accumulates_multiple_writes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("appendonly.aof");
        let mut writer = AofWriter::new(&path, AppendFSyncMode::No).unwrap();

        writer
            .append(b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n")
            .unwrap();
        writer.append(b"*2\r\n$3\r\nDEL\r\n$3\r\nfoo\r\n").unwrap();

        assert_eq!(
            fs::read(&path).unwrap(),
            b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n*2\r\n$3\r\nDEL\r\n$3\r\nfoo\r\n"
        );
    }

    #[test]
    fn test_creates_missing_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("dir").join("appendonly.aof");

        AofWriter::new(&path, AppendFSyncMode::No).unwrap();

        assert!(path.exists());
    }

    #[test]
    fn test_always_mode_does_not_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("appendonly.aof");
        let mut writer = AofWriter::new(&path, AppendFSyncMode::Always).unwrap();

        let result = writer.append(b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n");
        assert!(result.is_ok());
    }

    #[test]
    fn test_resolve_aof_path() {
        let path = resolve_aof_path("appendonlydir", "appendonly.aof");
        assert_eq!(path, PathBuf::from("appendonlydir/appendonly.aof"));
    }
}
