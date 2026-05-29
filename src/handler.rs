use log::{debug, error, warn};
use tokio::io::{self, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use uuid::Uuid;

use crate::aof::AofWriter;
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
    aof_writer: Option<AofWriter>,
}

impl Handler {
    /// Constructor for `Handler` struct.
    pub fn new(socket: TcpStream, executor: Executor, aof_writer: Option<AofWriter>) -> Self {
        Handler {
            uuid: Uuid::new_v4(),
            socket,
            executor,
            aof_writer,
        }
    }

    /// Reads and dispatches RESP2 commands from the client connection in a loop.
    ///
    /// Runs until the client closes the connection or an unrecoverable decode
    /// error occurs. Each iteration decodes one RESP2 message, parses it into
    /// a [`Command`], executes it via [`Executor`], and writes the RESP2
    /// response back to the socket.
    pub async fn run_handler(mut self) -> io::Result<()> {
        debug!("Handler {} started", self.uuid);

        let mut stream = BufReader::new(self.socket);
        loop {
            // Read from the socket and decode the RESP value into a `RespValue` struct
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

            // Convert the `RespValue` into a `Command` struct
            let cmd = Command::from_resp2(&resp_value);
            debug!("Handler {} received command: {:?}", self.uuid, cmd);

            // Capture the write flag before cmd is consumed by the match statement below
            let is_write_cmd = cmd.as_ref().is_ok_and(|c| c.is_write());

            // Execute the command using the `Executor` and generate the appropriate response
            // The `Executor` instance is responsible for both executing the command at the `Store` and generating a `RespValue` response
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

            // Call the AOF writer if the `appendonly` configuration is set to `on`
            if is_write_cmd
                && !matches!(response, RespValue::SimpleError(_))
                && let Some(writer) = self.aof_writer.as_mut()
                && let Err(e) = writer.append(&resp2::encode(&resp_value))
            {
                error!("Handler {} failed to write to AOF: {}", self.uuid, e);
            }

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
            Handler::new(socket, Executor::new(store), None)
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

    // Builds a PEXPIREAT command for `key` with a deadline `offset_ms` milliseconds from now.
    fn pexpireat_cmd(key: &str, offset_ms: i64) -> Vec<u8> {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let deadline = (now_ms + offset_ms).to_string();
        resp2::encode(&RespValue::Array(Some(vec![
            RespValue::BulkString(Some("PEXPIREAT".to_string())),
            RespValue::BulkString(Some(key.to_string())),
            RespValue::BulkString(Some(deadline)),
        ])))
    }

    #[tokio::test]
    async fn test_pexpireat_existing_key() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n")
            .await
            .unwrap();
        resp2::decode(&mut client).await.unwrap();

        client
            .write_all(&pexpireat_cmd("foo", 60_000))
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::Integer(1));
    }

    #[tokio::test]
    async fn test_pexpireat_missing_key() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(&pexpireat_cmd("missing", 60_000))
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::Integer(0));
    }

    #[tokio::test]
    async fn test_pexpireat_deadline_in_past() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        // Use a hardcoded timestamp from the past (1 second after the Unix epoch)
        client
            .write_all(b"*3\r\n$9\r\nPEXPIREAT\r\n$3\r\nfoo\r\n$4\r\n1000\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::Integer(0));
    }

    #[tokio::test]
    async fn test_ttl_after_pexpireat() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n")
            .await
            .unwrap();
        resp2::decode(&mut client).await.unwrap();

        // Set TTL to 60 seconds from now via PEXPIREAT
        client
            .write_all(&pexpireat_cmd("foo", 60_000))
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
    async fn test_select_ok() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*2\r\n$6\r\nSELECT\r\n$1\r\n0\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(response, RespValue::SimpleString("OK".to_string()));
    }

    #[tokio::test]
    async fn test_select_out_of_index() {
        let addr = start_handler().await;
        let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

        client
            .write_all(b"*2\r\n$6\r\nSELECT\r\n$1\r\n1\r\n")
            .await
            .unwrap();

        let response = resp2::decode(&mut client).await.unwrap();
        assert_eq!(
            response,
            RespValue::SimpleError("ERR DB index is out of range".to_string())
        );
    }
}
