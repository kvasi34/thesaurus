use thesaurus::{
    aof::{AofWriter, AppendFSyncMode},
    executor::Executor,
    handler::Handler,
    resp2,
    resp2::RespValue,
    store::Store,
};
use tokio::{
    io::{AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
};

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

    let response = resp2::decode_async(&mut client).await.unwrap();
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

    let response = resp2::decode_async(&mut client).await.unwrap();
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

    let response = resp2::decode_async(&mut client).await.unwrap();
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
    resp2::decode_async(&mut client).await.unwrap();

    client
        .write_all(b"*2\r\n$3\r\nGET\r\n$3\r\nfoo\r\n")
        .await
        .unwrap();

    let response = resp2::decode_async(&mut client).await.unwrap();
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

    let response = resp2::decode_async(&mut client).await.unwrap();
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
    resp2::decode_async(&mut client).await.unwrap();

    client
        .write_all(b"*2\r\n$3\r\nDEL\r\n$3\r\nfoo\r\n")
        .await
        .unwrap();

    let response = resp2::decode_async(&mut client).await.unwrap();
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

    let response = resp2::decode_async(&mut client).await.unwrap();
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
    resp2::decode_async(&mut client).await.unwrap();
    client
        .write_all(b"*3\r\n$3\r\nSET\r\n$3\r\nbaz\r\n$3\r\nqux\r\n")
        .await
        .unwrap();
    resp2::decode_async(&mut client).await.unwrap();

    // DEL two existing keys and one missing key — expect count 2
    client
        .write_all(b"*4\r\n$3\r\nDEL\r\n$3\r\nfoo\r\n$3\r\nbaz\r\n$7\r\nmissing\r\n")
        .await
        .unwrap();

    let response = resp2::decode_async(&mut client).await.unwrap();
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

    let response = resp2::decode_async(&mut client).await.unwrap();
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

    let response = resp2::decode_async(&mut client).await.unwrap();
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

    let response = resp2::decode_async(&mut client).await.unwrap();
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

    let response = resp2::decode_async(&mut client).await.unwrap();
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
    resp2::decode_async(&mut client).await.unwrap();

    client
        .write_all(b"*2\r\n$3\r\nTTL\r\n$3\r\nfoo\r\n")
        .await
        .unwrap();

    let response = resp2::decode_async(&mut client).await.unwrap();
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

    let response = resp2::decode_async(&mut client).await.unwrap();
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
    resp2::decode_async(&mut client).await.unwrap();

    client
        .write_all(b"*2\r\n$7\r\nPERSIST\r\n$3\r\nfoo\r\n")
        .await
        .unwrap();

    let response = resp2::decode_async(&mut client).await.unwrap();
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
    resp2::decode_async(&mut client).await.unwrap();

    client
        .write_all(b"*3\r\n$6\r\nEXPIRE\r\n$3\r\nfoo\r\n$2\r\n60\r\n")
        .await
        .unwrap();

    let response = resp2::decode_async(&mut client).await.unwrap();
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

    let response = resp2::decode_async(&mut client).await.unwrap();
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
    resp2::decode_async(&mut client).await.unwrap();

    client
        .write_all(b"*3\r\n$6\r\nEXPIRE\r\n$3\r\nfoo\r\n$2\r\n60\r\n")
        .await
        .unwrap();
    resp2::decode_async(&mut client).await.unwrap();

    client
        .write_all(b"*2\r\n$3\r\nTTL\r\n$3\r\nfoo\r\n")
        .await
        .unwrap();

    let response = resp2::decode_async(&mut client).await.unwrap();
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

    let response = resp2::decode_async(&mut client).await.unwrap();
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
    resp2::decode_async(&mut client).await.unwrap();

    client
        .write_all(b"*3\r\n$6\r\nEXPIRE\r\n$3\r\nfoo\r\n$2\r\n60\r\n")
        .await
        .unwrap();
    resp2::decode_async(&mut client).await.unwrap();

    client
        .write_all(b"*2\r\n$7\r\nPERSIST\r\n$3\r\nfoo\r\n")
        .await
        .unwrap();

    let response = resp2::decode_async(&mut client).await.unwrap();
    assert_eq!(response, RespValue::Integer(1));
}

/// Builds a PEXPIREAT command for `key` with a deadline `offset_ms` milliseconds from now.
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
    resp2::decode_async(&mut client).await.unwrap();

    client
        .write_all(&pexpireat_cmd("foo", 60_000))
        .await
        .unwrap();

    let response = resp2::decode_async(&mut client).await.unwrap();
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

    let response = resp2::decode_async(&mut client).await.unwrap();
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

    let response = resp2::decode_async(&mut client).await.unwrap();
    assert_eq!(response, RespValue::Integer(0));
}

#[tokio::test]
async fn test_pexpireat_deadline_in_past_existing_key() {
    let addr = start_handler().await;
    let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

    client
        .write_all(b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n")
        .await
        .unwrap();
    resp2::decode_async(&mut client).await.unwrap();

    // Deadline 60 seconds in the past — should return 0, not 1
    client
        .write_all(&pexpireat_cmd("foo", -60_000))
        .await
        .unwrap();

    let response = resp2::decode_async(&mut client).await.unwrap();
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
    resp2::decode_async(&mut client).await.unwrap();

    // Set TTL to 60 seconds from now via PEXPIREAT
    client
        .write_all(&pexpireat_cmd("foo", 60_000))
        .await
        .unwrap();
    resp2::decode_async(&mut client).await.unwrap();

    client
        .write_all(b"*2\r\n$3\r\nTTL\r\n$3\r\nfoo\r\n")
        .await
        .unwrap();

    let response = resp2::decode_async(&mut client).await.unwrap();
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

    let response = resp2::decode_async(&mut client).await.unwrap();
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

    let response = resp2::decode_async(&mut client).await.unwrap();
    assert_eq!(
        response,
        RespValue::SimpleError("ERR DB index is out of range".to_string())
    );
}

#[tokio::test]
async fn test_expire_written_as_pexpireat_to_aof() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let dir = tempfile::tempdir().unwrap();
    let aof_path = dir.path().join("appendonly.aof");

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let aof_writer = AofWriter::new(&aof_path, AppendFSyncMode::No).unwrap();

    let handle = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        Handler::new(socket, Executor::new(Store::new()), Some(aof_writer))
            .run_handler()
            .await
            .unwrap();
    });

    let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

    client
        .write_all(b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n")
        .await
        .unwrap();
    resp2::decode_async(&mut client).await.unwrap();

    client
        .write_all(b"*3\r\n$6\r\nEXPIRE\r\n$3\r\nfoo\r\n$4\r\n3600\r\n")
        .await
        .unwrap();
    resp2::decode_async(&mut client).await.unwrap();

    // Close the connection so the handler reaches EOF and exits cleanly
    drop(client);
    handle.await.unwrap();

    let aof_contents = std::fs::read_to_string(&aof_path).unwrap();

    // The AOF must store PEXPIREAT with an absolute deadline, not the raw EXPIRE
    assert!(
        aof_contents.contains("PEXPIREAT"),
        "AOF should contain PEXPIREAT"
    );
    assert!(
        !aof_contents.contains("EXPIRE\r\n"),
        "AOF should not contain raw EXPIRE"
    );

    // The deadline should be approximately now + 3600s (in ms)
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let expected_deadline_ms = now_ms + 3600 * 1000;
    // Extract the deadline from the AOF by finding the PEXPIREAT command's third bulk string
    let aof_bytes = std::fs::read(&aof_path).unwrap();
    let mut reader = std::io::BufReader::new(aof_bytes.as_slice());
    resp2::decode(&mut reader).unwrap(); // consume the SET command
    let pexpireat = resp2::decode(&mut reader).unwrap();
    if let RespValue::Array(Some(args)) = pexpireat {
        if let RespValue::BulkString(Some(deadline_str)) = &args[2] {
            let deadline_ms: u64 = deadline_str.parse().unwrap();
            let delta = deadline_ms.abs_diff(expected_deadline_ms);
            assert!(
                delta < 5_000,
                "PEXPIREAT deadline should be within 5s of expected: got {deadline_ms}, expected ~{expected_deadline_ms}"
            );
        } else {
            panic!("expected bulk string deadline");
        }
    } else {
        panic!("expected PEXPIREAT array");
    }
}

/// Helper function
/// Creates a TCP listener and a spawns a Handler task with an empty [`thesaurus::store::Store`] instance.
/// Returns the socket address and the Handler task's [`tokio::task::JoinHandle`].
async fn start_handler_with_aof(
    aof_writer: AofWriter,
) -> (std::net::SocketAddr, tokio::task::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handle = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        Handler::new(socket, Executor::new(Store::new()), Some(aof_writer))
            .run_handler()
            .await
            .unwrap();
    });
    (addr, handle)
}

#[tokio::test]
async fn test_noop_write_cmd_not_written_to_aof() {
    let dir = tempfile::tempdir().unwrap();
    let aof_path = dir.path().join("appendonly.aof");
    let aof_writer = AofWriter::new(&aof_path, AppendFSyncMode::No).unwrap();
    let (addr, handle) = start_handler_with_aof(aof_writer).await;

    let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

    // DEL on a missing key returns Integer(0) — should not be written to the AOF
    client
        .write_all(b"*2\r\n$3\r\nDEL\r\n$7\r\nmissing\r\n")
        .await
        .unwrap();
    let response = resp2::decode_async(&mut client).await.unwrap();
    assert_eq!(response, RespValue::Integer(0));

    // EXPIRE on a missing key also returns Integer(0) — also should not be written
    client
        .write_all(b"*3\r\n$6\r\nEXPIRE\r\n$7\r\nmissing\r\n$2\r\n60\r\n")
        .await
        .unwrap();
    let response = resp2::decode_async(&mut client).await.unwrap();
    assert_eq!(response, RespValue::Integer(0));

    drop(client);
    handle.await.unwrap();

    assert_eq!(
        std::fs::read(&aof_path).unwrap(),
        b"",
        "AOF should be empty — no-op commands must not be persisted"
    );
}

#[tokio::test]
async fn test_successful_write_cmd_written_to_aof() {
    let dir = tempfile::tempdir().unwrap();
    let aof_path = dir.path().join("appendonly.aof");
    let aof_writer = AofWriter::new(&aof_path, AppendFSyncMode::No).unwrap();
    let (addr, handle) = start_handler_with_aof(aof_writer).await;

    let mut client = BufReader::new(TcpStream::connect(addr).await.unwrap());

    // SET returns SimpleString("OK") — must be written to AOF
    client
        .write_all(b"*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$3\r\nbar\r\n")
        .await
        .unwrap();
    resp2::decode_async(&mut client).await.unwrap();

    // DEL on an existing key returns Integer(1) — must be written to AOF
    client
        .write_all(b"*2\r\n$3\r\nDEL\r\n$3\r\nfoo\r\n")
        .await
        .unwrap();
    let response = resp2::decode_async(&mut client).await.unwrap();
    assert_eq!(response, RespValue::Integer(1));

    drop(client);
    handle.await.unwrap();

    let aof_bytes = std::fs::read(&aof_path).unwrap();
    let mut reader = std::io::BufReader::new(aof_bytes.as_slice());
    assert!(resp2::decode(&mut reader).is_ok(), "AOF should contain SET");
    assert!(resp2::decode(&mut reader).is_ok(), "AOF should contain DEL");
}
