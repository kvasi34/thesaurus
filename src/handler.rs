use log::{debug, warn};
use tokio::{
    io::{self, AsyncWrite, AsyncWriteExt, BufReader},
    net::TcpStream,
};
use uuid::Uuid;

use crate::{
    command::Command,
    errors::RespError,
    resp2::{self, RespValue},
};

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

    /// Reads and dispatches RESP2 commands from the client connection in a loop.
    ///
    /// Runs until the client closes the connection or an unrecoverable decode
    /// error occurs. Each iteration decodes one RESP2 message, parses it into
    /// a [`Command`], and writes the appropriate RESP2 response back to the socket.
    pub async fn run_handler(self) -> io::Result<()> {
        debug!("Handler {} started", self.uuid);

        let mut stream = BufReader::new(self.socket);
        loop {
            let resp_value = match resp2::decode(&mut stream).await {
                Ok(v) => v,
                Err(RespError::UnexpectedEof) => {
                    debug!("Handler {} client disconnected", self.uuid);
                    break;
                }
                Err(e) => {
                    warn!("Handler {} decode error: {}", self.uuid, e);
                    return Err(e.into());
                }
            };
            let cmd = Command::from_resp2(&resp_value)?;
            debug!("Handler {} received command: {:?}", self.uuid, cmd);

            match cmd {
                Command::Ping { message } => {
                    Handler::handle_ping_response(&self.uuid, message, &mut stream).await;
                }
                _ => todo!("Handle GET, SET, DEL commands"),
            }
        }

        debug!("Handler {} stopped", self.uuid);
        Ok(())
    }

    /*
    Handles replies to PING commands.

    If PING has an argument, the argument is returned as a bulk string.
    Otherwise, PONG is returned as a simple string.
    */
    async fn handle_ping_response<R>(uuid: &Uuid, message: Option<String>, stream: &mut R)
    where
        R: AsyncWrite + Unpin,
    {
        let reply = match message {
            Some(s) => RespValue::BulkString(Some(s)),
            None => RespValue::SimpleString("PONG".to_string()),
        };

        let encoded_reply = resp2::encode(&reply);
        debug!("Handler {} sending: {:?}", uuid, reply);
        let _ = stream.write(&encoded_reply).await;
    }
}

#[cfg(test)]
mod tests {
    use tokio::{
        io::{AsyncWriteExt, BufReader},
        net::{TcpListener, TcpStream},
    };

    use super::*;
    use crate::resp2;

    async fn start_handler() -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            Handler::new(socket).run_handler().await.unwrap();
        });
        addr
    }

    #[tokio::test]
    async fn test_ping_no_message() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client.write_all(b"*1\r\n$4\r\nPING\r\n").await.unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::SimpleString("PONG".to_string()));
    }

    #[tokio::test]
    async fn test_ping_with_message() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*2\r\n$4\r\nPING\r\n$5\r\nhello\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::BulkString(Some("hello".to_string())));
    }
}
