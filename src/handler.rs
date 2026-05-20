use std::time::{Duration, Instant};

use log::{debug, trace, warn};
use tokio::{
    io::{self, AsyncWrite, AsyncWriteExt, BufReader},
    net::TcpStream,
};
use uuid::Uuid;

use crate::{
    command::Command,
    errors::{HandlerError, RespError},
    resp2::{self, RespValue},
    store::Store,
};

/// TCP socket connection handler.
#[derive(Debug)]
pub(crate) struct Handler {
    uuid: Uuid,
    socket: TcpStream,
    store: Store,
}

impl Handler {
    /// Constructor for `Handler` struct.
    pub fn new(socket: TcpStream, store: Store) -> Self {
        Handler {
            uuid: Uuid::new_v4(),
            socket,
            store,
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
            let cmd = Command::from_resp2(&resp_value);
            debug!("Handler {} received command: {:?}", self.uuid, cmd);

            match cmd {
                Ok(Command::Ping { message }) => {
                    Handler::handle_ping_response(&self.uuid, message, &mut stream).await;
                }
                Ok(Command::Get { key }) => {
                    Handler::handle_get_response(&self.uuid, key, &mut stream, &self.store).await;
                }
                Ok(Command::Set { key, value }) => {
                    Handler::handle_set_response(&self.uuid, key, value, &mut stream, &self.store)
                        .await;
                }
                Ok(Command::Delete { keys }) => {
                    Handler::handle_del_response(&self.uuid, keys, &mut stream, &self.store).await;
                }
                Ok(Command::Ttl { key }) => {
                    Handler::handle_ttl_response(&self.uuid, key, &mut stream, &self.store).await;
                }
                Ok(Command::Persist { key }) => {
                    Handler::handle_persist_response(&self.uuid, key, &mut stream, &self.store)
                        .await;
                }
                Ok(Command::Expire { key, seconds }) => {
                    Handler::handle_expire_response(
                        &self.uuid,
                        key,
                        seconds,
                        &mut stream,
                        &self.store,
                    )
                    .await?;
                }
                Err(HandlerError::UnknownCommand(s)) => {
                    Handler::handle_unknown_command_response(&self.uuid, s, &mut stream).await;
                }
                Err(e) => {
                    warn!("Handler {} command parse error: {}", self.uuid, e);
                    Handler::send_response(&self.uuid, &mut stream, RespValue::SimpleError(e.to_string())).await;
                }
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
        let response_value = match message {
            Some(s) => RespValue::BulkString(Some(s)),
            None => RespValue::SimpleString("PONG".to_string()),
        };

        Handler::send_response(uuid, stream, response_value).await;
    }

    /*
    Handles replies to GET commands. Replies with a bulk string containing the value corresponsing to the key.
    Replies with a NULL bulk string, if the key does not exist in the store.
    */
    async fn handle_get_response<R>(uuid: &Uuid, key: String, stream: &mut R, store: &Store)
    where
        R: AsyncWrite + Unpin,
    {
        trace!("Handler {} getting key {}", uuid, key);
        let stored_value = store.get(&key);
        trace!(
            "Handler {} got key {} with value {:?}",
            uuid, key, stored_value
        );

        let response_value = RespValue::BulkString(stored_value);
        Handler::send_response(uuid, stream, response_value).await;
    }

    // Handles replies to SET commands. Inserts the key-value pair into the store and replies with a simple string: OK.
    async fn handle_set_response<R>(
        uuid: &Uuid,
        key: String,
        value: String,
        stream: &mut R,
        store: &Store,
    ) where
        R: AsyncWrite + Unpin,
    {
        debug!("Handler {} writing key {}: {}", uuid, key, value);
        store.set(&key, value);

        let response_value = RespValue::SimpleString("OK".to_string());
        Handler::send_response(uuid, stream, response_value).await;
    }

    /*
    Handles replies to DEL commands.
    Deletes all given keys from the store and replies with an integer indicating the number deleted keys.
    */
    async fn handle_del_response<R>(uuid: &Uuid, keys: Vec<String>, stream: &mut R, store: &Store)
    where
        R: AsyncWrite + Unpin,
    {
        debug!("Handler {} deleting keys: {:?}", uuid, keys);
        let mut deleted_key_count: i64 = 0;
        for key in keys.iter() {
            deleted_key_count += store.delete(key) as i64;
        }

        let response_value = RespValue::Integer(deleted_key_count);
        Handler::send_response(uuid, stream, response_value).await;
    }

    /*
    Handles replies to TTL commands.
    Returns the remaining time to live of a key that has a timeout in seconds.
    The command returns -2 if the key does not exist.
    The command returns -1 if the key exists but has no associated expire.
    */
    async fn handle_ttl_response<R>(uuid: &Uuid, key: String, stream: &mut R, store: &Store)
    where
        R: AsyncWrite + Unpin,
    {
        trace!("Handler {} getting TTL for key {}", uuid, key);
        let expiry_instant = store.get_ttl(&key);
        trace!(
            "Handler {} got TTL for key {} with expiry {:?}",
            uuid, key, expiry_instant
        );

        let response_value = match expiry_instant {
            Some(expiry) => {
                RespValue::Integer(expiry.duration_since(Instant::now()).as_secs() as i64)
            }
            None => RespValue::Integer(if store.exists(&key) { -1 } else { -2 }),
        };
        Handler::send_response(uuid, stream, response_value).await;
    }

    /*
    Handles replies to PERSIST commands.
    Removes the existing timeout on a key.
    Replies with 0 if key does not exist or does not have an associated timeout, or 1 if the timeout has been removed.
    */
    async fn handle_persist_response<R>(uuid: &Uuid, key: String, stream: &mut R, store: &Store)
    where
        R: AsyncWrite + Unpin,
    {
        trace!("Handler {} persisting key {}", uuid, key);
        let removed = store.persist(&key);
        let response_value = RespValue::Integer(removed as i64);
        Handler::send_response(uuid, stream, response_value).await;
    }

    /*
    Handles replies to EXPIRE commands.
    Replies with 1 if the timeout was set, or 0 if the timeout was not set.
    */
    async fn handle_expire_response<R>(
        uuid: &Uuid,
        key: String,
        seconds: u64,
        stream: &mut R,
        store: &Store,
    ) -> Result<(), HandlerError>
    where
        R: AsyncWrite + Unpin,
    {
        debug!(
            "Handler {} expiring key {} with {} seconds",
            uuid, key, seconds
        );

        let expiration = Instant::now().checked_add(Duration::from_secs(seconds));
        if expiration.is_none() {
            Handler::send_response(uuid, stream, RespValue::Integer(0)).await;
            return Err(HandlerError::ExpireOverflow(seconds));
        }

        let response_value = RespValue::Integer(store.set_ttl(&key, expiration.unwrap()) as i64);
        Handler::send_response(uuid, stream, response_value).await;

        Ok(())
    }

    // Handles unknown command errors by replying with a custom error message
    async fn handle_unknown_command_response<R>(uuid: &Uuid, e: String, stream: &mut R)
    where
        R: AsyncWrite + Unpin,
    {
        warn!("Handler {} received unknown command: {}", uuid, e);
        let response_value = RespValue::SimpleError(format!("ERR unknown command '{}'", e));
        Handler::send_response(uuid, stream, response_value).await;
    }

    // Helper function to simply the response process
    async fn send_response<R>(uuid: &Uuid, stream: &mut R, response_value: RespValue)
    where
        R: AsyncWrite + Unpin,
    {
        let encoded_response = resp2::encode(&response_value);
        debug!("Handler {} sending: {:?}", uuid, response_value);
        let _ = stream.write(&encoded_response).await;
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
            Handler::new(socket, Store::new())
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
