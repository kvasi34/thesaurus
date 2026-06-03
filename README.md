# Thesaurus

A Redis-compatible in-memory key-value store written in Rust.

Speaks RESP2 over TCP, so it works with `redis-cli` and any standard Redis client out of the box.

> **Compatibility note:** Thesaurus implements a small subset of Redis commands and is not a drop-in replacement. Not all commands, options, or behaviours are supported.

**Features**

- Concurrent connections via async Tokio runtime
- TTL-based key expiry with a configurable background eviction loop
- Optional AOF persistence (append-only file)

## Getting started

Requires Rust 1.95+.

```bash
git clone https://github.com/kvasi34/thesaurus.git
cd thesaurus
cargo build
cargo run -p thesaurus
```

Connect with any Redis client:

```bash
redis-cli ping
redis-cli set foo bar
redis-cli get foo
```

## Options

| Flag       | Env var            | Default     |
|------------|--------------------|-------------|
| `--bind`   | `THESAURUS_BIND`   | `127.0.0.1` |
| `--port`   | `THESAURUS_PORT`   | `6379`      |
| `--config` | `THESAURUS_CONFIG` |             |

## Configuration

Server behaviour is configured via an INI file passed with `--config`. Environment variables prefixed with `THESAURUS_` override file values.

| Key              | Default        | Description                                   |
|------------------|----------------|-----------------------------------------------|
| `max_connections`| `100`          | Maximum concurrent TCP connections            |
| `hz`             | `100`          | TTL eviction interval in milliseconds         |
| `appendonly`     | `no`           | Enable AOF persistence                        |
| `appendfilename` | `appendonly.aof` | AOF file name                               |
| `appenddirname`  | `appendonlydir`| Directory for the AOF file                   |
| `appendfsync`    | `everysec`     | Fsync policy: `always`, `everysec`, or `no`  |

## Supported commands

See [docs/commands.md](docs/commands.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[GPL-3.0](LICENSE).
