use log::{debug, warn};
use tokio::io::{self, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use uuid::Uuid;

use crate::{
    command::Command,
    errors::{HandlerError, RespError},
    executor::Executor,
    resp2::{self, RespValue},
};

/// TCP socket connection handler.
#[derive(Debug)]
pub(crate) struct Handler {
    uuid: Uuid,
    socket: TcpStream,
    executor: Executor,
}

impl Handler {
    /// Constructor for `Handler` struct.
    pub fn new(socket: TcpStream, executor: Executor) -> Self {
        Handler {
            uuid: Uuid::new_v4(),
            socket,
            executor,
        }
    }

    /// Reads and dispatches RESP2 commands from the client connection in a loop.
    ///
    /// Runs until the client closes the connection or an unrecoverable decode
    /// error occurs. Each iteration decodes one RESP2 message, parses it into
    /// a [`Command`], executes it via [`Executor`], and writes the RESP2
    /// response back to the socket.
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

            let cmd = Command::from_resp2(&resp_value);
            debug!("Handler {} received command: {:?}", self.uuid, cmd);

            let response = match cmd {
                Ok(cmd) => self.executor.execute(&cmd),
                Err(HandlerError::UnknownCommand(s)) => {
                    warn!("Handler {} received unknown command: {}", self.uuid, s);
                    RespValue::SimpleError(format!("ERR unknown command '{}'", s))
                }
                Err(e) => {
                    warn!("Handler {} command parse error: {}", self.uuid, e);
                    RespValue::SimpleError(e.to_string())
                }
            };

            send_response(&self.uuid, &mut stream, response).await?;
        }

        debug!("Handler {} stopped", self.uuid);
        Ok(())
    }
}

/// Encodes `response_value` and writes it to `stream`.
async fn send_response<W>(uuid: &Uuid, stream: &mut W, response_value: RespValue) -> io::Result<()>
where
    W: AsyncWrite + Unpin,
{
    let encoded = resp2::encode(&response_value);
    debug!("Handler {} sending: {:?}", uuid, response_value);
    stream.write_all(&encoded).await
}

#[cfg(test)]
mod tests {
    use tokio::{
        io::{AsyncWriteExt, BufReader},
        net::{TcpListener, TcpStream},
    };

    use super::*;
    use crate::{executor::Executor, resp2, store::Store};

    async fn start_handler() -> std::net::SocketAddr {
        start_handler_with_store(Store::new()).await
    }

    async fn start_handler_with_store(store: Store) -> std::net::SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            Handler::new(socket, Executor::new(store))
                .run_handler()
                .await
                .unwrap();
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

    #[tokio::test]
    async fn test_set() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::SimpleString("OK".to_string()));
    }

    #[tokio::test]
    async fn test_get_existing_key() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n")
            .await
            .unwrap();
        resp2::decode(&mut client).await.unwrap();

        client
            .write_all(b"*2\r\n$3\r\nGET\r\n$3\r\nfoo\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::BulkString(Some("bar".to_string())));
    }

    #[tokio::test]
    async fn test_get_missing_key() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*2\r\n$3\r\nGET\r\n$7\r\nmissing\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::BulkString(None));
    }

    #[tokio::test]
    async fn test_del_existing_key() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n")
            .await
            .unwrap();
        resp2::decode(&mut client).await.unwrap();

        client
            .write_all(b"*2\r\n$3\r\nDEL\r\n$3\r\nfoo\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::Integer(1));
    }

    #[tokio::test]
    async fn test_del_missing_key() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*2\r\n$3\r\nDEL\r\n$7\r\nmissing\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::Integer(0));
    }

    #[tokio::test]
    async fn test_del_multiple_keys() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n")
            .await
            .unwrap();
        resp2::decode(&mut client).await.unwrap();
        client
            .write_all(b"*3\r\n$3\r\nSET\r\n$3\r\nbaz\r\n$3\r\nqux\r\n")
            .await
            .unwrap();
        resp2::decode(&mut client).await.unwrap();

        // DEL two existing keys and one missing key — expect count 2
        client
            .write_all(b"*4\r\n$3\r\nDEL\r\n$3\r\nfoo\r\n$3\r\nbaz\r\n$7\r\nmissing\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::Integer(2));
    }

    #[tokio::test]
    async fn test_exists_existing_key() {
        let store = Store::new();
        store.set("key1", "Hello".to_string());

        let addr = start_handler_with_store(store).await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*2\r\n$6\r\nEXISTS\r\n$4\r\nkey1\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::Integer(1));
    }

    #[tokio::test]
    async fn test_exists_missing_key() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*2\r\n$6\r\nEXISTS\r\n$9\r\nnosuchkey\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::Integer(0));
    }

    #[tokio::test]
    async fn test_exists_multiple_key() {
        let store = Store::new();
        store.set("key1", "Hello".to_string());
        store.set("key2", "World".to_string());

        let addr = start_handler_with_store(store).await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        // EXISTS key1 key2 nosuchkey — expect 2
        client
            .write_all(b"*4\r\n$6\r\nEXISTS\r\n$4\r\nkey1\r\n$4\r\nkey2\r\n$9\r\nnosuchkey\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::Integer(2));
    }

    #[tokio::test]
    async fn test_ttl_missing_key() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*2\r\n$3\r\nTTL\r\n$7\r\nmissing\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::Integer(-2));
    }

    #[tokio::test]
    async fn test_ttl_key_without_expiry() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n")
            .await
            .unwrap();
        resp2::decode(&mut client).await.unwrap();

        client
            .write_all(b"*2\r\n$3\r\nTTL\r\n$3\r\nfoo\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::Integer(-1));
    }

    #[tokio::test]
    async fn test_persist_missing_key() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*2\r\n$7\r\nPERSIST\r\n$7\r\nmissing\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::Integer(0));
    }

    #[tokio::test]
    async fn test_persist_key_without_ttl() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n")
            .await
            .unwrap();
        resp2::decode(&mut client).await.unwrap();

        client
            .write_all(b"*2\r\n$7\r\nPERSIST\r\n$3\r\nfoo\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::Integer(0));
    }

    #[tokio::test]
    async fn test_expire_existing_key() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n")
            .await
            .unwrap();
        resp2::decode(&mut client).await.unwrap();

        client
            .write_all(b"*3\r\n$6\r\nEXPIRE\r\n$3\r\nfoo\r\n$2\r\n60\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::Integer(1));
    }

    #[tokio::test]
    async fn test_expire_missing_key() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*3\r\n$6\r\nEXPIRE\r\n$7\r\nmissing\r\n$2\r\n60\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::Integer(0));
    }

    #[tokio::test]
    async fn test_ttl_key_with_expiry() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n")
            .await
            .unwrap();
        resp2::decode(&mut client).await.unwrap();

        client
            .write_all(b"*3\r\n$6\r\nEXPIRE\r\n$3\r\nfoo\r\n$2\r\n60\r\n")
            .await
            .unwrap();
        resp2::decode(&mut client).await.unwrap();

        client
            .write_all(b"*2\r\n$3\r\nTTL\r\n$3\r\nfoo\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        match response {
            RespValue::Integer(secs) => assert!(secs > 0 && secs <= 60),
            _ => panic!("expected integer response"),
        }
    }

    #[tokio::test]
    async fn test_ttl_expired_key() {
        use std::time::{Duration, Instant};
        let store = Store::new();
        store.set("foo", "bar".to_string());
        store.set_ttl("foo", Instant::now() - Duration::from_secs(1));

        let addr = start_handler_with_store(store).await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*2\r\n$3\r\nTTL\r\n$3\r\nfoo\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::Integer(-2));
    }

    #[tokio::test]
    async fn test_persist_key_with_ttl() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n")
            .await
            .unwrap();
        resp2::decode(&mut client).await.unwrap();

        client
            .write_all(b"*3\r\n$6\r\nEXPIRE\r\n$3\r\nfoo\r\n$2\r\n60\r\n")
            .await
            .unwrap();
        resp2::decode(&mut client).await.unwrap();

        client
            .write_all(b"*2\r\n$7\r\nPERSIST\r\n$3\r\nfoo\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::Integer(1));
    }
}
