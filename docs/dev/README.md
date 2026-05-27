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
   └─ Ctrl+C → semaphore.close(), break│
```

A `Semaphore` with capacity `max_connections` (default: 100, configured via `config.ini`) caps concurrent tasks. If the semaphore is closed (shutdown), the accept loop exits. Active handlers finish naturally; there is no forced cancellation.

## Command Lifecycle

Every command travels through four stages:

```
TCP bytes
   │
   ▼  resp2::decode()          reads one RESP2 frame → RespValue
   │
   ▼  Command::from_resp2()    validates & parses → Command enum variant
   │
   ▼  Executor::execute()      applies command to Store → RespValue
   │
   ▼  resp2::encode()          serialises reply → Handler writes to socket
   │
TCP bytes (response)
```

### 1. RESP2 framing (`src/resp2.rs`)

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

### 2. Command parsing (`src/command.rs`)

`Command::from_resp2()` validates the decoded value:

- Must be a `RespValue::Array` — otherwise `HandlerError::UnexpectedType`.
- Every element must be a `RespValue::BulkString` — otherwise `HandlerError::UnexpectedType`.
- First element is the command name. Dispatches to a per-command parser that checks arity and extracts typed fields.
- Unknown names return `HandlerError::UnknownCommand`.

The result is a typed `Command` variant:

```rust
Command::Set { key: "foo", value: "bar" }
```

### 3. Dispatch and execution (`src/handler.rs`, `src/executor.rs`)

`Handler::run_handler()` passes the parsed `Command` to `Executor::execute()`, which applies it to the shared `Store` and returns a `RespValue`. The handler then encodes and writes that value back to the socket.

`Executor` is the single place where commands mutate or read state. It is created once in `main.rs` and cloned cheaply (via the inner `Arc`) into each connection handler. This separation means the same execution path can be driven by AOF replay without a socket being involved.
