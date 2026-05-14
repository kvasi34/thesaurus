use log::debug;
use tokio::{
    io::{self, AsyncWriteExt, BufReader},
    net::TcpStream,
};
use uuid::Uuid;

use crate::{command::Command, errors::RespError, resp2};

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
            let resp_value = match resp2::decode(&mut stream).await {
                Ok(v) => v,
                Err(RespError::UnexpectedEof) => break,
                Err(e) => return Err(e.into()),
            };
            debug!("Handler {} received: {:?}", self.uuid, resp_value);
            let cmd = Command::from_resp2(&resp_value)?;

            match cmd {
                Command::Ping { message } => {
                    let reply = message.unwrap_or_default();
                    debug!("Handler {} sending: {:?}", self.uuid, reply);
                    let _ = stream.write(reply.as_bytes());
                    todo!("Encode and write RESP2 response");
                }
                _ => todo!("Handle GET, SET, DEL commands"),
            }
        }

        Ok(())
    }
}
