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

A `Semaphore` with capacity `--max-connections` (default: 100) caps concurrent tasks. If the semaphore is closed (shutdown), the accept loop exits. Active handlers finish naturally; there is no forced cancellation.

## Command Lifecycle

Every command travels through four stages:

```
TCP bytes
   │
   ▼  resp2::decode()          reads one RESP2 frame → RespValue
   │
   ▼  Command::from_resp2()    validates & parses → Command enum variant
   │
   ▼  Handler::run_handler()   matches variant → calls handle_*_response()
   │
   ▼  responses/*              talks to Store, builds reply → resp2::encode()
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

### 3. Dispatch and execution (`src/handler.rs`, `src/responses/`)

`Handler::run_handler()` matches on the `Command` variant and calls the corresponding function in `src/responses/`. Each response function:

1. Reads from or writes to the shared `Store`.
2. Constructs a `RespValue` reply.
3. Calls `resp2::encode()` and writes the bytes back to the socket.

### Error handling

| Error kind | Cause | Outcome |
|---|---|---|
| `UnknownCommand` | Unrecognised command name | `SimpleError` reply; connection stays open |
| Parse error (arity, type) | Malformed arguments | `SimpleError` reply; connection stays open |
| RESP2 decode error | Malformed frame | Handler loop exits, task ends |
| `UnexpectedEof` | Client disconnected | Clean exit |
