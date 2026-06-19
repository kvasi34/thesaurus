use config::Config;
use log::error;

use crate::aof::AppendFSyncMode;

const DEFAULTS: &[(&str, &str)] = &[
    ("max_connections", "100"),
    ("hz", "100"),
    ("appendonly", "no"),
    ("appendfilename", "appendonly.aof"),
    ("appenddirname", "appendonlydir"),
    ("appendfsync", "everysec"),
    ("lazyfree_lazy_user_flush", "no"),
];

/// Returns the compile-time default string value for a config key from [`DEFAULTS`].
///
/// Panics if `key` is not present — callers must only use keys that exist in the slice.
fn default_val(key: &str) -> &'static str {
    DEFAULTS.iter().find(|(k, _)| *k == key).unwrap().1
}

/// Server behaviour configuration, loaded from an INI file.
///
/// Controls resource limits and background task tuning. Connection and
/// network settings (bind address, port) are handled separately by [`command::Cli`].
#[derive(Debug)]
pub struct ThesaurusConfig {
    /// Maximum number of simultaneous client connections.
    pub max_connections: usize,
    /// TTL eviction loop interval in milliseconds.
    pub hz: u64,

    // AOF configuration
    /// Whether AOF persistence is enabled.
    pub appendonly: bool,
    /// Filename of the AOF file (e.g. `appendonly.aof`).
    pub appendfilename: String,
    /// Directory where the AOF file is stored.
    pub appenddirname: String,
    /// fsync strategy for the AOF.
    pub appendfsync: AppendFSyncMode,

    /// When `true`, bare `FLUSHDB` (no modifier) frees memory asynchronously.
    pub lazyfree_lazy_user_flush: bool,
}

impl Default for ThesaurusConfig {
    fn default() -> Self {
        ThesaurusConfig {
            max_connections: default_val("max_connections").parse().unwrap(),
            hz: default_val("hz").parse().unwrap(),
            appendonly: yes_no_from_str(default_val("appendonly")).unwrap(),
            appendfilename: default_val("appendfilename").to_string(),
            appenddirname: default_val("appenddirname").to_string(),
            appendfsync: appendfsync_from_str(default_val("appendfsync")).unwrap(),
            lazyfree_lazy_user_flush: yes_no_from_str(default_val("lazyfree_lazy_user_flush"))
                .unwrap(),
        }
    }
}

/// Loads configuration from an INI file at `path`, with environment variable overrides.
///
/// Environment variables prefixed with `THESAURUS_` take precedence over file values.
/// Returns an error if the path does not point to an `.ini` file or if parsing fails.
pub fn load_config(path: &str) -> Result<ThesaurusConfig, String> {
    if !path.ends_with(".ini") {
        error!("Failed to read INI file at `{}`", path);
        return Err(format!("Expected an .ini file, got: {}", path));
    }
    // Set default values for any unspecified config properties
    let mut builder = config::Config::builder();
    for (key, value) in DEFAULTS {
        builder = builder
            .set_default(*key, *value)
            .map_err(|e| e.to_string())?;
    }
    let config = builder
        .add_source(config::File::with_name(path))
        .add_source(config::Environment::with_prefix("THESAURUS_"))
        .build()
        .map_err(|e| e.to_string())?;

    Ok(ThesaurusConfig {
        max_connections: config
            .get::<usize>("max_connections")
            .map_err(|e| e.to_string())?,
        hz: config.get::<u64>("hz").map_err(|e| e.to_string())?,
        appendonly: get_bool(&config, "appendonly")?,
        appendfilename: config.get("appendfilename").map_err(|e| e.to_string())?,
        appenddirname: config.get("appenddirname").map_err(|e| e.to_string())?,
        appendfsync: map_config_to_appendfsync_mode(&config)?,
        lazyfree_lazy_user_flush: get_bool(&config, "lazyfree-lazy-user-flush")?,
    })
}

/// Reads a `yes`/`no` config key and returns its `bool` value.
fn get_bool(config: &Config, key: &str) -> Result<bool, String> {
    let val: String = config.get(key).map_err(|e| e.to_string())?;
    yes_no_from_str(&val)
}

/// Parses a `"yes"` / `"no"` string into a `bool`.
fn yes_no_from_str(s: &str) -> Result<bool, String> {
    match s {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Err(format!("expected 'yes' or 'no', got '{}'", s)),
    }
}

/// Helper function that maps a `String` to a `AppendFSyncMode` value for `appendfsync`.
fn map_config_to_appendfsync_mode(config: &Config) -> Result<AppendFSyncMode, String> {
    let val: String = config.get("appendfsync").map_err(|e| e.to_string())?;
    appendfsync_from_str(&val)
}

/// Parses an `appendfsync` string value (`"always"`, `"everysec"`, or `"no"`) into an [`AppendFSyncMode`].
fn appendfsync_from_str(s: &str) -> Result<AppendFSyncMode, String> {
    match s {
        "always" => Ok(AppendFSyncMode::Always),
        "everysec" => Ok(AppendFSyncMode::EverySec),
        "no" => Ok(AppendFSyncMode::No),
        _ => Err(format!("{} is not a valid appendfsync mode", s)),
    }
}
