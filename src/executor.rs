use std::time::{Duration, Instant};

use log::{debug, trace};

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
pub(crate) struct Executor {
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
                debug!("SET {} = {}", key, value);
                self.store.set(key, value.clone());
                RespValue::SimpleString("OK".to_string())
            }

            Command::Delete { keys } => {
                let count: i64 = keys.iter().map(|k| self.store.delete(k) as i64).sum();
                debug!("DEL {:?}: deleted {}", keys, count);
                RespValue::Integer(count)
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
                match Instant::now().checked_add(Duration::from_secs(*seconds)) {
                    // checked_add returns None on overflow — reject with an error
                    None => RespValue::SimpleError(
                        "ERR invalid expire time in 'expire' command".to_string(),
                    ),
                    Some(deadline) => RespValue::Integer(self.store.set_ttl(key, deadline) as i64),
                }
            }
        }
    }
}
