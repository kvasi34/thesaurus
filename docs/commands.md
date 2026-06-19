# Supported commands

| Command        | Signature                    | Supported options |
|----------------|------------------------------|-------------------|
| `PING`         | `PING [message]`             | —                 |
| `GET`          | `GET key`                    | —                 |
| `SET`          | `SET key value`              |                   |
| `DEL`          | `DEL key [key …]`            | —                 |
| `GETDEL`       | `GETDEL key`                 | —                 |
| `EXISTS`       | `EXISTS key [key …]`         | —                 |
| `EXPIRE`       | `EXPIRE key seconds`         |                   |
| `PEXPIRE`      | `PEXPIRE key milliseconds`   |                   |
| `EXPIREAT`     | `EXPIREAT key unix-secs`     |                   |
| `PEXPIREAT`    | `PEXPIREAT key unix-ms`      |                   |
| `TTL`          | `TTL key`                    | —                 |
| `EXPIRETIME`   | `EXPIRETIME key`             | —                 |
| `PEXPIRETIME`  | `PEXPIRETIME key`            | —                 |
| `PERSIST`      | `PERSIST key`                | —                 |
| `DBSIZE`       | `DBSIZE`                     | —                 |
| `SELECT`       | `SELECT index`               | —                 |
| `FLUSHDB`      | `FLUSHDB [ASYNC\|SYNC]`      | ASYNC, SYNC       |

`—` indicates Redis has no options for this command. Empty cells indicate Redis has options that are not yet supported.
