use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use log::trace;

use crate::command::Command;
use crate::resp2::RespValue;
use crate::store::Store;

/// Bridges [`Command`] to [`Store`]: the single place where commands are
/// applied to in-memory state.
///
/// The [`Handler`] decodes RESP2 frames from a TCP connection and dispatches
/// here.
///
/// [`execute`] is intentionally synchronous: store operations hold an
/// `std::sync::RwLock` internally and complete in-place; no network or
/// file I/O takes place here.
///
/// [`Handler`]: crate::handler::Handler
#[derive(Clone, Debug)]
pub struct Executor {
    store: Store,
}

impl Executor {
    /// Creates a new `Executor` backed by `Store`.
    pub fn new(store: Store) -> Self {
        Executor { store }
    }

    /// Applies `cmd` to the store and returns the RESP2 response value.
    pub fn execute(&self, cmd: &Command) -> RespValue {
        match cmd {
            Command::Ping { message } => match message {
                None => RespValue::SimpleString("PONG".to_string()),
                Some(msg) => RespValue::BulkString(Some(msg.clone())),
            },

            Command::Get { key } => {
                let value = self.store.get(key);
                trace!("GET {}: {:?}", key, value);
                RespValue::BulkString(value)
            }

            Command::Set { key, value } => {
                trace!("SET {} = {}", key, value);
                self.store.set(key, value.clone());
                RespValue::SimpleString("OK".to_string())
            }

            Command::Delete { keys } => {
                let count: i64 = keys.iter().map(|k| self.store.delete(k) as i64).sum();
                trace!("DEL {:?}: deleted {}", keys, count);
                RespValue::Integer(count)
            }

            Command::GetDel { key } => {
                let value = self.store.get_del(key);
                trace!("GETDEL {}: {:?}", key, value);
                RespValue::BulkString(value)
            }

            Command::Exists { keys } => {
                let count: i64 = keys.iter().map(|k| self.store.exists(k) as i64).sum();
                RespValue::Integer(count)
            }

            Command::Ttl { key } => {
                let expiry = self.store.get_ttl(key);
                trace!("TTL {}: expiry = {:?}", key, expiry);
                match expiry {
                    Some(instant) => match instant.checked_duration_since(Instant::now()) {
                        Some(remaining) => RespValue::Integer(remaining.as_secs() as i64),
                        // Expiry instant is in the past — treat as a missing key
                        None => RespValue::Integer(-2),
                    },
                    // No expiry entry: -1 if the key exists, -2 if it doesn't
                    None => RespValue::Integer(if self.store.exists(key) { -1 } else { -2 }),
                }
            }

            Command::Persist { key } => {
                let removed = self.store.persist(key);
                RespValue::Integer(removed as i64)
            }

            Command::Expire { key, seconds } => {
                trace!("EXPIRE {} {}", key, seconds);
                match Instant::now().checked_add(Duration::from_secs(*seconds)) {
                    // checked_add returns None on overflow — reject with an error
                    None => RespValue::SimpleError(
                        "ERR invalid expire time in 'expire' command".to_string(),
                    ),
                    Some(deadline) => RespValue::Integer(self.store.set_ttl(key, deadline) as i64),
                }
            }

            Command::PExpireAt { key, deadline_ms } => {
                trace!("PEXPIREAT {} {}", key, deadline_ms);
                let now_duration = SystemTime::now().duration_since(UNIX_EPOCH);
                if now_duration.is_err() {
                    return RespValue::Integer(0);
                }

                // Fail PExpireAt commands where the deadline is in the past
                let now_ms = now_duration.unwrap_or_default().as_millis() as u64;
                if *deadline_ms <= now_ms {
                    self.store.delete(key);
                    return RespValue::Integer(0);
                }

                let remaining_ms = deadline_ms.saturating_sub(now_ms);
                let deadline = Instant::now() + Duration::from_millis(remaining_ms);
                RespValue::Integer(self.store.set_ttl(key, deadline) as i64)
            }

            Command::Select { index } => {
                if *index != 0 {
                    return RespValue::SimpleError("ERR DB index is out of range".to_string());
                }

                RespValue::SimpleString("OK".to_string())
            }

            Command::DbSize => RespValue::Integer(self.store.size() as i64),
        }
    }
}
