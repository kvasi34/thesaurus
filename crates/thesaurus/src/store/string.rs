use crate::errors::StoreError;

use super::{Store, StoreValue};

impl Store {
    /// Returns the string value for `key`.
    /// Returns `Ok(None)` if the key does not exist, `Err(StoreError::WrongType)` if it holds a non-string value.
    pub fn get_string(&self, key: &str) -> Result<Option<String>, StoreError> {
        match self.get(key) {
            Some(StoreValue::Str(s)) => Ok(Some(s)),
            Some(_) => Err(StoreError::WrongType),
            None => Ok(None),
        }
    }

    /// Inserts or overwrites `key` with the string `value`.
    /// Always succeeds regardless of the previous value type.
    pub fn set_string(&self, key: &str, value: &str) {
        self.set(key, StoreValue::Str(value.to_string()));
    }

    /// Returns the string value for `key` and immediately deletes the key.
    /// Returns `Ok(None)` if the key does not exist, `Err(StoreError::WrongType)` if it holds a non-string value.
    pub fn get_del_string(&self, key: &str) -> Result<Option<String>, StoreError> {
        match self.get_del(key) {
            Some(StoreValue::Str(s)) => Ok(Some(s)),
            Some(_) => Err(StoreError::WrongType),
            None => Ok(None),
        }
    }

    /// Returns the values of all specified keys. For every key that does not hold a string value
    /// or does not exist, `None` is returned. Because of this, the operation never fails.
    pub fn mget(&self, keys: &[String]) -> Vec<Option<String>> {
        let guard = self.inner.read().unwrap();
        keys.iter()
            .map(|key| match guard.get(key) {
                Some(StoreValue::Str(s)) => Some(s.clone()),
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::time::{Duration, Instant};

    use super::*;

    fn keys(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // mget
    #[test]
    fn test_mget_returns_values_in_requested_order() {
        let store = Store::new();
        store.set_string("a", "1");
        store.set_string("b", "2");
        assert_eq!(
            store.mget(&keys(&["b", "a"])),
            vec![Some("2".to_string()), Some("1".to_string())]
        );
    }

    #[test]
    fn test_mget_returns_none_for_missing_keys() {
        let store = Store::new();
        store.set_string("a", "1");
        assert_eq!(
            store.mget(&keys(&["a", "missing"])),
            vec![Some("1".to_string()), None]
        );
    }

    #[test]
    fn test_mget_returns_none_for_non_string_values() {
        let store = Store::new();
        store.set_string("a", "1");
        store.set("list", StoreValue::List(VecDeque::from(["x".to_string()])));
        assert_eq!(
            store.mget(&keys(&["a", "list"])),
            vec![Some("1".to_string()), None]
        );
    }

    #[test]
    fn test_mget_returns_none_for_expired_key() {
        let store = Store::new();
        store.set_string("a", "1");
        store.set_ttl("a", Instant::now() - Duration::from_secs(1));
        assert_eq!(store.mget(&keys(&["a"])), vec![None]);
    }

    #[test]
    fn test_mget_returns_value_for_key_with_future_expiry() {
        let store = Store::new();
        store.set_string("a", "1");
        store.set_ttl("a", Instant::now() + Duration::from_secs(60));
        assert_eq!(store.mget(&keys(&["a"])), vec![Some("1".to_string())]);
    }

    #[test]
    fn test_mget_repeats_value_for_duplicate_keys() {
        let store = Store::new();
        store.set_string("a", "1");
        assert_eq!(
            store.mget(&keys(&["a", "a"])),
            vec![Some("1".to_string()), Some("1".to_string())]
        );
    }

    #[test]
    fn test_mget_returns_empty_vec_for_no_keys() {
        let store = Store::new();
        store.set_string("a", "1");
        assert_eq!(store.mget(&[]), Vec::<Option<String>>::new());
    }
}
