# Contributing to Thesaurus

## Architecture

![Architecture diagram](docs/dev/architecture.svg)

**Components:**
- **TCP listener** — accepts incoming connections and spawns a handler task per client
- **Handler tasks** — decode RESP2 input, dispatch commands, write responses
- **Shared store** — an `Arc<RwLock<HashMap>>` shared across all handler tasks
- **TTL worker** — background task that evicts expired keys
- **AofWriter** — handles writes to AOF log
- **AOF log** — optional on-disk persistence via append-only file

Check the [Developer Guide](./docs/dev/README.md) for more information.

## Running tests and linting

```bash
cargo test
cargo clippy
cargo fmt
cargo audit     # requires: cargo install cargo-audit
```

## Making changes

- Open a PR against `main`
- Run all CI checks: `build`, `test`, `clippy`, `fmt`, `audit`
- PRs are merged via squash merge
- Commit messages must follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/)

## Using AI

The use of AI assistants to write code, issues or PRs is accepted. However, the human author is responsible for understanding the submitted code and should be able to comment on their submissions.
