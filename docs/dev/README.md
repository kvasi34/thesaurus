# Developer Guide

Internals reference for contributors. For a bird's-eye view, see the [architecture diagram](architecture.svg).

## TCP Listener

`main.rs` binds a `TcpListener` at startup, then drives a `tokio::select!` loop:

```
TcpListener::bind(addr)
        │
   ┌────┴──────────────────────────────┐
   │ loop (tokio::select!)             │
   ├─ listener.accept()                │
   │       │                           │
   │  acquire semaphore permit         │
   │       │                           │
   │  tokio::spawn(Handler::run) ──────┤
   │  (permit dropped on task exit)    │
   │                                   │
   └─ SIGINT/SIGTERM → semaphore.close(), break│
```

A `Semaphore` with capacity `max_connections` (default: 100, configured via `config.ini`) caps concurrent tasks. If the semaphore is closed (shutdown), the accept loop exits. Active handlers are given 10 seconds to finish; any still running after that are forcibly aborted via `handler_set.abort_all()`.

## Command Lifecycle

Every command travels through four stages:

```
TCP bytes
   │
   ▼  resp2::decode_async()    reads one RESP2 frame → RespValue
   │
   ▼  Command::from_resp2()    validates & parses → Command enum variant
   │
   ▼  Executor::execute()      applies command to Store → RespValue
   │
   ▼  resp2::encode()          serialises reply → Handler writes to socket
   │
TCP bytes (response)
```

For write commands (`SET`, `DEL`, `EXPIRE`, `PERSIST`, `PEXPIREAT`), the handler also appends the command to the AOF after a successful execution (see [AOF persistence](#aof-persistence) below). `EXPIRE` is stored as `PEXPIREAT` with an absolute Unix millisecond deadline so TTLs survive restarts correctly.

### 1. RESP2 framing (`crates/thesaurus/src/resp2.rs`)

`resp2.rs` exposes two decoders:

- `decode_async` — used by the connection handler for live client traffic (async, reads from a `tokio::io::BufReader`)
- `decode` — used by AOF replay at startup (sync, reads from a `std::io::BufReader`)

Clients speak [RESP2](https://redis.io/docs/latest/develop/reference/protocol-spec/). Commands arrive as arrays of bulk strings:

```
SET foo bar
────────────────────────
*3\r\n         ← array of 3 elements
$3\r\nSET\r\n
$3\r\nfoo\r\n
$3\r\nbar\r\n
```

`resp2::decode()` reads exactly one frame per call. The handler calls it in a loop, processing one command per iteration until the client disconnects (`UnexpectedEof`) or a malformed frame causes a hard error (loop exits, task ends).

### 2. Command parsing (`crates/thesaurus/src/command.rs`)

`Command::from_resp2()` validates the decoded value:

- Must be a `RespValue::Array` — otherwise `HandlerError::UnexpectedType`.
- Every element must be a `RespValue::BulkString` — otherwise `HandlerError::UnexpectedType`.
- First element is the command name. Dispatches to a per-command parser that checks arity and extracts typed fields.
- Unknown names return `HandlerError::UnknownCommand`.

The result is a typed `Command` variant:

```rust
Command::Set { key: "foo", value: "bar" }
```

### 3. Dispatch and execution (`crates/thesaurus/src/handler.rs`, `crates/thesaurus/src/executor.rs`)

`Handler::run_handler()` passes the parsed `Command` to `Executor::execute()`, which applies it to the shared `Store` and returns a `RespValue`. The handler then encodes and writes that value back to the socket.

`Executor` is the single place where commands mutate or read state. It is created once in `main.rs` and cloned cheaply (via the inner `Arc`) into each connection handler. This separation means the same execution path can be driven by AOF replay without a socket being involved.

## AOF persistence

AOF is opt-in (`appendonly = yes` in `config.ini`). When enabled:

**Write path** — after a write command succeeds, `Handler` appends the RESP2-encoded command to an `AofWriter`. `EXPIRE` is rewritten as `PEXPIREAT` with an absolute Unix millisecond deadline before being appended, so replaying the file after a restart sets the correct remaining TTL rather than resetting the clock.

**Startup replay** — before the TCP listener opens, `aof::sync_store_with_aof` reads the AOF with the sync `resp2::decode`, parses each entry via `Command::from_resp2`, and drives it through the same `Executor`. The store is fully restored before any client can connect.

**Fsync modes** — controlled by `appendfsync`:

| Mode       | Behaviour |
|------------|-----------|
| `always`   | `fdatasync` after every append — zero data loss, highest I/O cost |
| `everysec` | background task fsyncs once per second — at most ~1 s of data loss |
| `no`       | OS decides when to flush — fastest, up to ~30 s of potential loss |

**AofWriter** — a cheaply cloneable `Arc<Mutex<Writer>>` shared across handler tasks, where `Writer` wraps a `BufWriter<File>`. The buffer is always flushed to the kernel page cache on every append; the fsync mode only controls when it reaches physical disk.
