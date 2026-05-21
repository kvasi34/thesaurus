mod command;
mod errors;
mod handler;
mod resp2;
mod responses;
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

    // Initialize the store
    let store = store::Store::new();

    // Spawn the TTL eviction daemon task, which clears expired keys
    let daemon_store = store.clone();
    tokio::spawn(async move {
        ttl::TtlEvictionDaemon::spawn(args.hz, daemon_store).await;
    });

    // Define semaphore to limit the number of Tokio tasks
    let semaphore = Arc::new(Semaphore::new(args.max_connections));

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
                let handler = handler::Handler::new(socket, store.clone());
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
