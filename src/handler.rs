use log::debug;
use tokio::{io::{self, BufReader}, net::TcpStream};
use uuid::Uuid;

use crate::resp2;

/// TCP socket connection handler.
#[derive(Debug)]
pub(crate) struct Handler {
    uuid: Uuid,
    socket: TcpStream,
}

impl Handler {
    /// Constructor for `Handler` struct.
    pub fn new(socket: TcpStream) -> Self {
        Handler {
            uuid: Uuid::new_v4(),
            socket,
        }
    }

    /// Continuously monitors the socket for RESP2 messages.
    pub async fn run_handler(self) -> io::Result<()> {
        debug!("Handler {} started", self.uuid);

        let mut stream = BufReader::new(self.socket);
        loop {
            let resp_value = resp2::decode(&mut stream).await?;
        }

        Ok(())
    }
}
