# Thesaurus

A Redis-compatible in-memory key-value store written in Rust, using the RESP2 protocol over TCP.

## Workspace layout

The project is a Cargo workspace with two crates under `crates/`:

- `crates/thesaurus/` — library crate (+ binary entry point)
- `crates/thesaurus-integration-suite/` — integration test suite; tests that spin up real TCP handlers live here

## Architecture

All source files live under `crates/thesaurus/src/`:

- `lib.rs` — crate root; re-exports all public modules
- `main.rs` — binary entry point; binds the TCP listener, accepts connections, spawns handler tasks, handles shutdown
- `handler.rs` — per-connection handler; decodes RESP2 input, dispatches to `Executor`, writes responses
- `command/` — CLI argument parsing (via clap) and RESP2-to-command parsing; `mod.rs` holds the `Command` enum, dispatch, and generic parsers (also used for SADD, SMEMBERS, SCARD, SPOP, SREM), `set.rs` holds SMOVE, SISMEMBER, SMISMEMBER, and SRANDMEMBER parsing, `string.rs` holds `SetCondition`, `SetExpiry`, and SET parsing, `list.rs` holds LSET/LINDEX/LRANGE parsing
- `executor/` — bridges `Command` to `Store`; `mod.rs` holds the `Executor` struct and generic command handlers, `string.rs` holds string-type-specific handlers (`GET`, `SET`, `GETDEL`, `DIGEST`), `list.rs` holds list-type-specific handlers, `set.rs` holds set-type-specific handlers (`SADD`, `SMEMBERS`, `SISMEMBER`, `SMISMEMBER`, `SRANDMEMBER`, `SCARD`, `SMOVE`, `SPOP`, `SREM`)
- `config.rs` — INI config file loading via the `config` crate; exposes `ThesaurusConfig`
- `resp2.rs` — RESP2 protocol encoder and decoder (async for live connections, sync for AOF replay)
- `store/` — shared in-memory `HashMap` wrapped in `Arc<RwLock>` for concurrent access; `mod.rs` holds the `Store` struct, `StoreValue` enum, and generic key operations, `list.rs` holds list-type operations, `set.rs` holds set-type operations
- `aof.rs` — optional AOF persistence; appends write commands to disk and replays them on startup
- `errors.rs` — error types for the handler and RESP2 layers

## Commands

Supported: `PING`, `GET`, `SET` (NX, XX, IFEQ, IFNE, IFDEQ, IFDNE, GET, EX, PX, EXAT, PXAT, KEEPTTL), `DEL`, `GETDEL`, `DIGEST`, `EXISTS`, `LPUSH`, `RPUSH`, `LPUSHX`, `RPUSHX`, `LPOP`, `RPOP`, `LLEN`, `LINDEX`, `LSET`, `LRANGE`, `SADD`, `SMEMBERS`, `SISMEMBER`, `SMISMEMBER`, `SRANDMEMBER`, `SCARD`, `SMOVE`, `SPOP`, `SREM`, `EXPIRE`, `PEXPIRE`, `EXPIREAT`, `PEXPIREAT`, `TTL`, `EXPIRETIME`, `PEXPIRETIME`, `PERSIST`, `DBSIZE`, `SELECT`, `FLUSHDB`

## Running

```bash
cargo run -p thesaurus -- [OPTIONS]

Options:
  --bind <BIND>     [env: THESAURUS_BIND]   [default: 127.0.0.1]
  --port <PORT>     [env: THESAURUS_PORT]   [default: 6379]
  --config <CONFIG> [env: THESAURUS_CONFIG]
```

Server behaviour (connection limits, TTL eviction rate) is configured via an INI file passed with `--config`. See `config.ini` for available keys and their defaults. Environment variables prefixed with `THESAURUS_` override file values.

## Development commands

```bash
cargo build
cargo test
cargo clippy -- -D warnings
cargo fmt
cargo fmt --check   # CI mode — reports without fixing
cargo audit         # requires: cargo install cargo-audit
```

## Conventions

- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)
- All changes go through a PR; squash merge into `main`
- CI runs five jobs in parallel: `build`, `test`, `clippy`, `fmt`, `audit`
