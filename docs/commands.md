# Supported commands

| Command        | Signature                                      | Supported options                                              |
|----------------|------------------------------------------------|----------------------------------------------------------------|
| `PING`         | `PING [message]`                               | —                                                              |
| `GET`          | `GET key`                                      | —                                                              |
| `SET`          | `SET key value [condition] [GET] [expiry]`     | `NX`, `XX`, `IFEQ`, `IFNE`, `IFDEQ`, `IFDNE`, `GET`, `EX`, `PX`, `EXAT`, `PXAT`, `KEEPTTL` |
| `DEL`          | `DEL key [key …]`                              | —                                                              |
| `GETDEL`       | `GETDEL key`                                   | —                                                              |
| `EXISTS`       | `EXISTS key [key …]`                           | —                                                              |
| `EXPIRE`       | `EXPIRE key seconds`                           |                                                                |
| `PEXPIRE`      | `PEXPIRE key milliseconds`                     |                                                                |
| `EXPIREAT`     | `EXPIREAT key unix-secs`                       |                                                                |
| `PEXPIREAT`    | `PEXPIREAT key unix-ms`                        |                                                                |
| `TTL`          | `TTL key`                                      | —                                                              |
| `EXPIRETIME`   | `EXPIRETIME key`                               | —                                                              |
| `PEXPIRETIME`  | `PEXPIRETIME key`                              | —                                                              |
| `PERSIST`      | `PERSIST key`                                  | —                                                              |
| `DBSIZE`       | `DBSIZE`                                       | —                                                              |
| `SELECT`       | `SELECT index`                                 | —                                                              |
| `FLUSHDB`      | `FLUSHDB [ASYNC\|SYNC]`                        | `ASYNC`, `SYNC`                                                |

`—` indicates Redis has no options for this command. Empty cells indicate Redis has options that are not yet supported.

### SET option groups

`condition` and `expiry` are mutually exclusive within each group. At most one from each group may be specified.

**Condition** (write only if condition holds):

| Option      | Description                                                        |
|-------------|--------------------------------------------------------------------|
| `NX`        | Set if the key does **not** exist                                  |
| `XX`        | Set if the key **already** exists                                  |
| `IFEQ val`  | Set if the current value **equals** `val`                          |
| `IFNE val`  | Set if the current value **does not equal** `val`                  |
| `IFDEQ hash`| Set if the XXH3 digest of the current value **equals** `hash`      |
| `IFDNE hash`| Set if the XXH3 digest of the current value **does not equal** `hash` |

**Expiry:**

| Option           | Description                                        |
|------------------|----------------------------------------------------|
| `EX seconds`     | Expire after `seconds` seconds                     |
| `PX milliseconds`| Expire after `milliseconds` milliseconds           |
| `EXAT unix-secs` | Expire at the given Unix timestamp (seconds)       |
| `PXAT unix-ms`   | Expire at the given Unix timestamp (milliseconds)  |
| `KEEPTTL`        | Retain the existing TTL (no TTL if none was set)   |

**Return value:**

| Flag  | Description                                                     |
|-------|-----------------------------------------------------------------|
| `GET` | Return the previous value before the write (or `nil` if absent) |
