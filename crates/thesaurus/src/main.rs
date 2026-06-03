use std::io;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use log::{debug, error, info, trace, warn};
use thesaurus::{aof, command, config, executor, handler, store, ttl};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use command::Cli;

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init();

    let args = Cli::parse();
    debug!("Parsed command: {:?}", args);

    let cfg = match args.config.as_deref() {
        Some(path) => {
            config::load_config(path).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?
        }
        None => {
            debug!("No config file specified. Using defaults.");
            config::ThesaurusConfig::default()
        }
    };
    info!("Running with config: {:?}", cfg);

    // Initialize the store
    let store = store::Store::new();

    // Wrap the store in an executor; cloned cheaply into each connection handler
    let executor = executor::Executor::new(store.clone());

    if let Err(e) = aof::sync_store_with_aof(
        cfg.appendonly,
        &cfg.appenddirname,
        &cfg.appendfilename,
        executor.clone(),
    ) {
        error!(
            "Sync with AOF {} failed: {}",
            aof::resolve_aof_path(&cfg.appenddirname, &cfg.appendfilename).display(),
            e
        );
        return Err(io::Error::new(e.kind(), e));
    }
    let aof_writer = aof::open(
        cfg.appendonly,
        &cfg.appenddirname,
        &cfg.appendfilename,
        cfg.appendfsync,
    )?;

    let shutdown_token = CancellationToken::new();

    // Spawn the fsync task explicitly so we can hold the handle for a final fsync on shutdown
    let fsync_handle = aof_writer
        .as_ref()
        .and_then(|w| w.spawn_fsync_task(shutdown_token.clone()));

    // Define semaphore to limit the number of handlers
    let semaphore = Arc::new(Semaphore::new(cfg.max_connections));

    // Spawn the TTL eviction daemon task, which clears expired keys
    let daemon_store = store.clone();
    let ttl_shutdown_token = shutdown_token.clone();
    tokio::spawn(async move {
        ttl::TtlEvictionDaemon::spawn(cfg.hz, daemon_store, ttl_shutdown_token).await;
    });

    // Start the TCP listener
    let address = format!("{}:{}", args.bind, args.port);
    info!("Starting TCP listener {}", address);
    let listener = TcpListener::bind(address).await?;
    let mut handler_set: JoinSet<()> = JoinSet::new();
    loop {
        tokio::select! {
            result = listener.accept() => {
                let (socket, socket_address) = result?;
                trace!("Received connection from {:?} socket address: {:?}", socket, socket_address);

                // Acquire the semaphore if available. This is a defensive measure guarding against future changes.
                let Ok(permit) = Arc::clone(&semaphore).acquire_owned().await else {
                    break;
                };

                // Spawn handler instance and pass the socket connection
                let handler = handler::Handler::new(socket, executor.clone(), aof_writer.clone());
                handler_set.spawn(async move {
                    if let Err(e) = handler.run_handler().await {
                        warn!("Error while running task at socket {:?}\n{}", socket_address, e);
                    }

                    drop(permit);
                });
            }
            _ = shutdown_signal() => {
                info!("Shutdown signal received — stopping listener");
                semaphore.close();
                break;
            }
        }
    }

    // Drain in-flight connections; handlers must finish before the final AOF fsync
    info!(
        "Draining {} in-flight connection(s) (10s timeout)",
        handler_set.len()
    );
    let drained = tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(result) = handler_set.join_next().await {
            if let Err(e) = result {
                warn!("Handler task panicked: {:?}", e);
            }
        }
    })
    .await;
    if drained.is_err() {
        warn!("Drain timeout elapsed — aborting remaining connections");
        handler_set.abort_all();
    }

    // Cancel background tasks; this triggers the final fsync in the EverySec task
    shutdown_token.cancel();

    // Wait for the final AOF fsync to complete before exiting
    if let Some(handle) = fsync_handle
        && let Err(e) = handle.await
    {
        warn!("AOF fsync task failed on shutdown: {:?}", e);
    }

    info!("Shutdown complete");
    Ok(())
}

/// Resolves when the process receives SIGINT (Ctrl+C) or, on Unix, SIGTERM.
async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();

    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut sigterm =
            signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
        tokio::select! {
            _ = ctrl_c => {}
            _ = sigterm.recv() => {}
        }
    }

    #[cfg(not(unix))]
    ctrl_c.await.expect("failed to listen for ctrl_c");
}
