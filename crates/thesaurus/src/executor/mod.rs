mod list;
mod set;
mod string;

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use log::trace;

use crate::command::{Command, FlushMode};
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
    lazyfree_lazy_user_flush: bool,
}

impl Executor {
    /// Creates a new `Executor` backed by `Store`.
    pub fn new(store: Store, lazyfree_lazy_user_flush: bool) -> Self {
        Executor {
            store,
            lazyfree_lazy_user_flush,
        }
    }

    /// Applies `cmd` to the store and returns the RESP2 response value.
    pub fn execute(&self, cmd: &Command) -> RespValue {
        match cmd {
            Command::Ping { message } => self.ping(message.as_deref()),
            Command::Get { key } => self.get(key),
            Command::Set {
                key,
                value,
                condition,
                expiry,
                get,
            } => self.set(key, value, condition, expiry, *get),
            Command::Delete { keys } => self.delete(keys),
            Command::GetDel { key } => self.get_del(key),
            Command::Exists { keys } => self.exists(keys),
            Command::LPush { key, elements } => self.lpush(key, elements),
            Command::RPush { key, elements } => self.rpush(key, elements),
            Command::LPushX { key, elements } => self.lpushx(key, elements),
            Command::RPushX { key, elements } => self.rpushx(key, elements),
            Command::LPop { key, count } => self.lpop(key, *count),
            Command::RPop { key, count } => self.rpop(key, *count),
            Command::LSet {
                key,
                index,
                element,
            } => self.lset(key, *index, element.clone()),
            Command::LLen { key } => self.llen(key),
            Command::LIndex { key, index } => self.lindex(key, *index),
            Command::LRange { key, start, stop } => self.lrange(key, *start, *stop),
            Command::SAdd { key, members } => self.sadd(key, members),
            Command::SMembers { key } => self.smembers(key),
            Command::SCard { key } => self.scard(key),
            Command::SPop { key, count } => self.spop(key, *count),
            Command::SRem { key, members } => self.srem(key, members),
            Command::Ttl { key } => self.ttl(key),
            Command::ExpireTime { key } => self.expire_time(key),
            Command::PExpireTime { key } => self.pexpire_time(key),
            Command::Persist { key } => self.persist(key),
            Command::Expire { key, seconds } => self.expire(key, *seconds),
            Command::PExpire { key, milliseconds } => self.pexpire(key, *milliseconds),
            Command::ExpireAt { key, deadline_secs } => self.expire_at(key, *deadline_secs),
            Command::PExpireAt { key, deadline_ms } => self.pexpire_at(key, *deadline_ms),
            Command::Digest { key } => self.digest(key),
            Command::Select { index } => self.select(*index),
            Command::DbSize => self.db_size(),
            Command::FlushDb { mode } => self.flush_db(mode.as_ref()),
        }
    }

    fn ping(&self, message: Option<&str>) -> RespValue {
        match message {
            None => RespValue::SimpleString("PONG".to_string()),
            Some(msg) => RespValue::BulkString(Some(msg.to_string())),
        }
    }

    fn delete(&self, keys: &[String]) -> RespValue {
        let count: i64 = keys.iter().map(|k| self.store.delete(k) as i64).sum();
        trace!("DEL {:?}: deleted {}", keys, count);
        RespValue::Integer(count)
    }

    fn exists(&self, keys: &[String]) -> RespValue {
        let count: i64 = keys.iter().map(|k| self.store.exists(k) as i64).sum();
        RespValue::Integer(count)
    }

    fn ttl(&self, key: &str) -> RespValue {
        self.resolve_expiry(key, "TTL", |r| r.as_secs() as i64)
    }

    fn expire_time(&self, key: &str) -> RespValue {
        self.resolve_expiry(key, "EXPIRETIME", |r| {
            let now_secs = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock is before Unix epoch")
                .as_secs() as i64;
            now_secs + r.as_secs() as i64
        })
    }

    fn pexpire_time(&self, key: &str) -> RespValue {
        self.resolve_expiry(key, "PEXPIRETIME", |r| {
            let now_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock is before Unix epoch")
                .as_millis() as i64;
            now_ms + r.as_millis() as i64
        })
    }

    fn persist(&self, key: &str) -> RespValue {
        let removed = self.store.persist(key);
        RespValue::Integer(removed as i64)
    }

    fn expire(&self, key: &str, seconds: u64) -> RespValue {
        trace!("EXPIRE {} {}", key, seconds);
        match Instant::now().checked_add(Duration::from_secs(seconds)) {
            // checked_add returns None on overflow — reject with an error
            None => {
                RespValue::SimpleError("ERR invalid expire time in 'expire' command".to_string())
            }
            Some(deadline) => RespValue::Integer(self.store.set_ttl(key, deadline) as i64),
        }
    }

    fn pexpire(&self, key: &str, milliseconds: u64) -> RespValue {
        trace!("PEXPIRE {} {}", key, milliseconds);
        match Instant::now().checked_add(Duration::from_millis(milliseconds)) {
            // checked_add returns None on overflow — reject with an error
            None => {
                RespValue::SimpleError("ERR invalid expire time in 'expire' command".to_string())
            }
            Some(deadline) => RespValue::Integer(self.store.set_ttl(key, deadline) as i64),
        }
    }

    fn expire_at(&self, key: &str, deadline_secs: u64) -> RespValue {
        trace!("EXPIREAT {} {}", key, deadline_secs);
        // Fail ExpireAt commands where the deadline is in the past
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before Unix epoch")
            .as_secs();
        if deadline_secs <= now_secs {
            self.store.delete(key);
            return RespValue::Integer(0);
        }

        RespValue::Integer(
            self.store
                .set_ttl(key, Self::unix_secs_to_instant(deadline_secs)) as i64,
        )
    }

    fn pexpire_at(&self, key: &str, deadline_ms: u64) -> RespValue {
        trace!("PEXPIREAT {} {}", key, deadline_ms);
        // Fail PExpireAt commands where the deadline is in the past
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before Unix epoch")
            .as_millis() as u64;
        if deadline_ms <= now_ms {
            self.store.delete(key);
            return RespValue::Integer(0);
        }

        RespValue::Integer(
            self.store
                .set_ttl(key, Self::unix_ms_to_instant(deadline_ms)) as i64,
        )
    }

    fn select(&self, index: u8) -> RespValue {
        if index != 0 {
            return RespValue::SimpleError("ERR DB index is out of range".to_string());
        }

        RespValue::SimpleString("OK".to_string())
    }

    fn db_size(&self) -> RespValue {
        RespValue::Integer(self.store.size() as i64)
    }

    fn flush_db(&self, mode: Option<&FlushMode>) -> RespValue {
        // Prioritize command argument over config value
        if let Some(mode) = mode {
            match mode {
                FlushMode::Sync => self.store.clear(),
                FlushMode::Async => self.store.clear_async(),
            }
        } else {
            match self.lazyfree_lazy_user_flush {
                true => self.store.clear_async(),
                false => self.store.clear(),
            }
        }

        RespValue::SimpleString("OK".to_string())
    }

    fn unix_secs_to_instant(deadline_secs: u64) -> Instant {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before Unix epoch")
            .as_secs();
        Instant::now() + Duration::from_secs(deadline_secs.saturating_sub(now_secs))
    }

    fn unix_ms_to_instant(deadline_ms: u64) -> Instant {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before Unix epoch")
            .as_millis() as u64;
        Instant::now() + Duration::from_millis(deadline_ms.saturating_sub(now_ms))
    }

    /// Looks up the expiry for `key` and maps the remaining [`Duration`] to an integer response
    /// via `f`. Returns `-2` if the key is expired or missing, `-1` if it exists with no expiry.
    fn resolve_expiry(&self, key: &str, cmd: &str, f: impl FnOnce(Duration) -> i64) -> RespValue {
        let expiry = self.store.get_ttl(key);
        trace!("{} {}: expiry = {:?}", cmd, key, expiry);
        match expiry {
            Some(instant) => match instant.checked_duration_since(Instant::now()) {
                Some(remaining) => RespValue::Integer(f(remaining)),
                // Expiry instant is in the past — treat as a missing key
                None => RespValue::Integer(-2),
            },
            // No expiry entry: -1 if the key exists, -2 if it doesn't
            None => RespValue::Integer(if self.store.exists(key) { -1 } else { -2 }),
        }
    }
}
