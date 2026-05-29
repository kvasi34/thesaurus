mod aof;
mod command;
mod config;
mod errors;
mod executor;
mod handler;
mod resp2;
mod store;
mod ttl;

use std::io;
use std::sync::Arc;

use clap::Parser;
use log::{debug, info, trace, warn};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;

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
    debug!("Running with config: {:?}", cfg);

    // Initialize the store
    let store = store::Store::new();

    // Wrap the store in an executor; cloned cheaply into each connection handler
    let executor = executor::Executor::new(store.clone());

    let aof_writer = aof::open(
        cfg.appendonly,
        &cfg.appenddirname,
        &cfg.appendfilename,
        cfg.appendfsync,
    )?;

    // Define semaphore to limit the number of handlers
    let semaphore = Arc::new(Semaphore::new(cfg.max_connections));

    // Spawn the TTL eviction daemon task, which clears expired keys
    let daemon_store = store.clone();
    tokio::spawn(async move {
        ttl::TtlEvictionDaemon::spawn(cfg.hz, daemon_store).await;
    });

    // Start the TCP listener
    let address = format!("{}:{}", args.bind, args.port);
    info!("Starting TCP listener {}", address);
    let listener = TcpListener::bind(address).await?;
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
                tokio::spawn(async move {
                    if let Err(e) = handler.run_handler().await {
                        warn!("Error while running task at socket {:?}\n{}", socket_address, e);
                    }

                    drop(permit);
                });
            }
            // Handle Ctrl + C signals
            _ = tokio::signal::ctrl_c() => {
                info!("Shutting TCP listener down");
                semaphore.close();
                break;
            }
        }
    }

    Ok(())
}
