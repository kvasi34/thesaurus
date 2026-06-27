use std::time::{Duration, Instant};

use log::trace;
use xxhash_rust::xxh3::xxh3_64;

use crate::command::SetCondition::{self, IfDeq, IfDne, IfEq, IfNe, NX, XX};
use crate::command::SetExpiry::{self, Ex, ExAt, KeepTtl, Px, PxAt};
use crate::resp2::RespValue::{self, BulkString};
use crate::store::StoreValue;

use super::{Executor, WRONGTYPE_ERROR};

impl Executor {
    pub(super) fn get(&self, key: &str) -> RespValue {
        let value = self.store.get(key);
        trace!("GET {}: {:?}", key, value);
        match value {
            Some(StoreValue::Str(s)) => BulkString(Some(s)),
            Some(_) => RespValue::SimpleError(WRONGTYPE_ERROR.to_string()),
            None => BulkString(None),
        }
    }

    pub(super) fn set(
        &self,
        key: &str,
        value: &str,
        condition: &Option<SetCondition>,
        expiry: &Option<SetExpiry>,
        get: bool,
    ) -> RespValue {
        trace!("SET {} = {}", key, value);
        let prev = match self.store.get(key) {
            Some(StoreValue::Str(s)) => Some(s),
            Some(_) => return RespValue::SimpleError(WRONGTYPE_ERROR.to_string()),
            None => None,
        };

        // Handle SET command conditions
        let condition_met = match condition {
            None => true,
            Some(NX) => prev.is_none(),
            Some(XX) => prev.is_some(),
            Some(IfEq(s)) => prev.as_deref().is_some_and(|v| v == s),
            Some(IfNe(s)) => prev.as_deref().is_some_and(|v| v != s),
            Some(IfDeq(u)) => prev.as_deref().is_some_and(|v| Self::digest_value(v) == *u),
            Some(IfDne(u)) => prev.as_deref().is_some_and(|v| Self::digest_value(v) != *u),
        };

        if condition_met {
            let prev_ttl = if matches!(expiry, Some(KeepTtl)) {
                self.store.get_ttl(key)
            } else {
                None
            };

            self.store.set(key, StoreValue::Str(value.to_string()));

            // Handle SET command expiry arguments
            match expiry {
                None => {} // TTL already cleared by store.set()
                Some(Ex(secs)) => match Instant::now().checked_add(Duration::from_secs(*secs)) {
                    None => {
                        return RespValue::SimpleError(
                            "ERR invalid expire time in 'set' command".to_string(),
                        );
                    }
                    Some(deadline) => {
                        self.store.set_ttl(key, deadline);
                    }
                },
                Some(Px(millis)) => {
                    match Instant::now().checked_add(Duration::from_millis(*millis)) {
                        None => {
                            return RespValue::SimpleError(
                                "ERR invalid expire time in 'set' command".to_string(),
                            );
                        }
                        Some(deadline) => {
                            self.store.set_ttl(key, deadline);
                        }
                    }
                }
                Some(ExAt(deadline_secs)) => {
                    self.store
                        .set_ttl(key, Self::unix_secs_to_instant(*deadline_secs));
                }
                Some(PxAt(deadline_ms)) => {
                    self.store
                        .set_ttl(key, Self::unix_ms_to_instant(*deadline_ms));
                }
                Some(KeepTtl) => {
                    if let Some(ttl) = prev_ttl {
                        self.store.set_ttl(key, ttl);
                    }
                }
            }
        }

        if get {
            return RespValue::BulkString(prev);
        }

        if condition_met {
            RespValue::SimpleString("OK".to_string())
        } else {
            RespValue::BulkString(None)
        }
    }

    pub(super) fn get_del(&self, key: &str) -> RespValue {
        let value = match self.store.get_del(key) {
            Some(StoreValue::Str(s)) => Some(s),
            Some(_) => return RespValue::SimpleError(WRONGTYPE_ERROR.to_string()),
            None => None,
        };
        trace!("GETDEL {}: {:?}", key, value);
        RespValue::BulkString(value)
    }

    pub(super) fn digest(&self, key: &str) -> RespValue {
        match self.store.get(key) {
            Some(StoreValue::Str(s)) => {
                RespValue::BulkString(Some(format!("{:016x}", Self::digest_value(&s))))
            }
            Some(_) => RespValue::SimpleError(WRONGTYPE_ERROR.to_string()),
            None => RespValue::BulkString(None),
        }
    }

    fn digest_value(v: &str) -> u64 {
        xxh3_64(v.as_bytes())
    }
}
