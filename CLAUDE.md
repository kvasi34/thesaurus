# Thesaurus

A Redis-compatible in-memory key-value store written in Rust, using the RESP2 protocol over TCP.

## Architecture

- `main.rs` — entry point; binds the TCP listener, accepts connections, spawns handler tasks, handles shutdown
- `handler.rs` — per-connection handler; decodes RESP2 input, dispatches commands, writes responses
- `command.rs` — CLI argument parsing (via clap) and RESP2-to-command parsing
- `resp2.rs` — RESP2 protocol encoder and decoder
- `store.rs` — shared in-memory `HashMap` wrapped in `Arc<RwLock>` for concurrent access
- `errors.rs` — error types for the handler and RESP2 layers

## Commands

Supported: `PING`, `GET`, `SET`, `DEL`

## Running

```bash
cargo run -- [OPTIONS]

Options:
  --bind <BIND>                       [env: THESAURUS_BIND] [default: 127.0.0.1]
  --port <PORT>                       [env: THESAURUS_PORT] [default: 6379]
  --max-connections <MAX_CONNECTIONS> [env: THESAURUS_MAX_CONNECTIONS] [default: 100]
```

## Development commands

```bash
cargo build
cargo test
cargo clippy
cargo fmt
cargo fmt --check   # CI mode — reports without fixing
cargo audit         # requires: cargo install cargo-audit
```

## Conventions

- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)
- All changes go through a PR; squash merge into `main`
- CI runs five jobs in parallel: `build`, `test`, `clippy`, `fmt`, `audit`
