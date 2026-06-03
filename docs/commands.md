# Supported commands

| Command     | Signature               | Supported options |
|-------------|-------------------------|-------------------|
| `PING`      | `PING [message]`        | —                 |
| `GET`       | `GET key`               | —                 |
| `SET`       | `SET key value`         |                   |
| `DEL`       | `DEL key [key …]`       | —                 |
| `EXISTS`    | `EXISTS key [key …]`    | —                 |
| `EXPIRE`    | `EXPIRE key seconds`    |                   |
| `TTL`       | `TTL key`               | —                 |
| `PERSIST`   | `PERSIST key`           | —                 |
| `PEXPIREAT` | `PEXPIREAT key unix-ms` |                   |
| `SELECT`    | `SELECT index`          | —                 |

`—` indicates Redis has no options for this command. Empty cells indicate Redis has options that are not yet supported.
