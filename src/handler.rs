use log::debug;
use tokio::{io, net::TcpStream};
use uuid::Uuid;

/// TCP socket connection handler.
#[derive(Debug)]
pub(crate) struct Handler {
    uuid: Uuid,
    socket: TcpStream,
}

impl Handler {
    /// Contructor for `Handler` struct.
    pub fn new(socket: TcpStream) -> Self {
        Handler {
            uuid: Uuid::new_v4(),
            socket,
        }
    }

    /// Continuously monitors the socket for RESP2 messages
    pub async fn run_handler(self) -> io::Result<()> {
        debug!("Handler {} started", self.uuid);

        Ok(())
    }
}
