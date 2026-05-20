use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Shared in-memory key-value store.
///
/// Internally wraps an `Arc<Mutex<HashMap>>` so it can be cheaply cloned
/// and shared across handler tasks. All operations lock the mutex for the
/// duration of the call and release it immediately on return.
#[derive(Clone, Debug)]
pub(crate) struct Store {
    inner: Arc<RwLock<StoreInner>>,
}

#[derive(Clone, Debug)]
struct StoreInner {
    data: HashMap<String, String>,
    expiry_index: HashMap<String, Instant>,
}

impl Store {
    /// Creates a new empty `Store`.
    pub fn new() -> Self {
        Store {
            inner: Arc::new(RwLock::new(StoreInner {
                data: HashMap::new(),
                expiry_index: HashMap::new(),
            })),
        }
    }

    /// Returns the value for `key`, or `None` if the key does not exist.
    pub fn get(&self, key: &str) -> Option<String> {
        let guard = self.inner.read().unwrap();
        let expiry_entry = guard.expiry_index.get(key);
        if expiry_entry.is_none_or(|v| Instant::now() < *v) {
            return guard.data.get(key).cloned();
        }

        None
    }

    /// Inserts or overwrites `key` with `value`.
    pub fn set(&self, key: &str, value: String) {
        let mut guard = self.inner.write().unwrap();
        guard.expiry_index.remove(key);
        guard.data.insert(key.to_string(), value);
    }

    /// Removes `key` from the store. Returns `true` if the `key` existed.
    pub fn delete(&self, key: &str) -> bool {
        let mut guard = self.inner.write().unwrap();
        guard.expiry_index.remove(key);
        guard.data.remove(key).is_some()
    }

    /// Checks if a `key` exists in the store.
    pub fn exists(&self, key: &str) -> bool {
        let guard = self.inner.read().unwrap();
        guard.data.contains_key(key)
    }

    /// Returns the TTL value for `key`, or `None` if the key does not exist in the expiry index.
    pub fn get_ttl(&self, key: &str) -> Option<Instant> {
        let guard = self.inner.read().unwrap();
        guard.expiry_index.get(key).cloned()
    }

    /// Sets or overwrites TTL in the expiry log for `key`. Returns `false` if the `key` does not exist in the store.
    pub fn set_ttl(&self, key: &str, ttl: Instant) -> bool {
        let mut guard = self.inner.write().unwrap();
        if guard.data.contains_key(key) {
            guard.expiry_index.insert(key.to_string(), ttl);
            return true;
        }

        false
    }

    /// Removes the TTL entry for `key`. Returns `true` if the `key` there was a key to delete, otherwise `false`.
    pub fn persist(&self, key: &str) -> bool {
        let mut guard = self.inner.write().unwrap();
        guard.expiry_index.remove(key).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get() {
        let store = Store::new();
        store.set("foo", "bar".to_string());
        assert_eq!(store.get("foo"), Some("bar".to_string()));
    }

    #[test]
    fn test_get_missing_key() {
        let store = Store::new();
        assert_eq!(store.get("missing"), None);
    }

    #[test]
    fn test_delete_existing_key() {
        let store = Store::new();
        store.set("foo", "bar".to_string());
        assert!(store.delete("foo"));
        assert_eq!(store.get("foo"), None);
    }

    #[test]
    fn test_delete_missing_key() {
        let store = Store::new();
        assert!(!store.delete("missing"));
    }

    #[test]
    fn test_set_overwrites_existing_key() {
        let store = Store::new();
        store.set("foo", "bar".to_string());
        store.set("foo", "baz".to_string());
        assert_eq!(store.get("foo"), Some("baz".to_string()));
    }

    #[test]
    fn test_clones_share_same_data() {
        let store_a = Store::new();
        let store_b = store_a.clone();
        let store_c = store_a.clone();

        store_a.set("foo", "bar".to_string());
        assert_eq!(store_b.get("foo"), Some("bar".to_string()));

        store_b.set("foo", "baz".to_string());
        assert_eq!(store_c.get("foo"), Some("baz".to_string()));

        store_c.delete("foo");
        assert_eq!(store_a.get("foo"), None);
    }

    #[test]
    fn test_exists_present_key() {
        let store = Store::new();
        store.set("foo", "bar".to_string());
        assert!(store.exists("foo"));
    }

    #[test]
    fn test_exists_missing_key() {
        let store = Store::new();
        assert!(!store.exists("missing"));
    }

    #[test]
    fn test_exists_deleted_key() {
        let store = Store::new();
        store.set("foo", "bar".to_string());
        store.delete("foo");
        assert!(!store.exists("foo"));
    }

    #[test]
    fn test_get_ttl_no_expiry() {
        let store = Store::new();
        store.set("foo", "bar".to_string());
        assert_eq!(store.get_ttl("foo"), None);
    }

    #[test]
    fn test_get_ttl_with_expiry() {
        use std::time::Duration;
        let store = Store::new();
        store.set("foo", "bar".to_string());
        let expiry = Instant::now() + Duration::from_secs(60);
        store.set_ttl("foo", expiry);
        assert_eq!(store.get_ttl("foo"), Some(expiry));
    }

    #[test]
    fn test_set_ttl_existing_key() {
        use std::time::Duration;
        let store = Store::new();
        store.set("foo", "bar".to_string());
        let expiry = Instant::now() + Duration::from_secs(60);
        assert!(store.set_ttl("foo", expiry));
    }

    #[test]
    fn test_set_ttl_missing_key() {
        use std::time::Duration;
        let store = Store::new();
        let expiry = Instant::now() + Duration::from_secs(60);
        assert!(!store.set_ttl("missing", expiry));
    }

    #[test]
    fn test_set_ttl_overwrites_existing_ttl() {
        use std::time::Duration;
        let store = Store::new();
        store.set("foo", "bar".to_string());
        let first_expiry = Instant::now() + Duration::from_secs(60);
        let second_expiry = Instant::now() + Duration::from_secs(120);
        store.set_ttl("foo", first_expiry);
        store.set_ttl("foo", second_expiry);
        assert_eq!(store.get_ttl("foo"), Some(second_expiry));
    }

    #[test]
    fn test_persist_key_with_ttl() {
        use std::time::Duration;
        let store = Store::new();
        store.set("foo", "bar".to_string());
        store.set_ttl("foo", Instant::now() + Duration::from_secs(60));
        assert!(store.persist("foo"));
    }

    #[test]
    fn test_persist_removes_ttl() {
        use std::time::Duration;
        let store = Store::new();
        store.set("foo", "bar".to_string());
        store.set_ttl("foo", Instant::now() + Duration::from_secs(60));
        store.persist("foo");
        assert_eq!(store.get_ttl("foo"), None);
    }

    #[test]
    fn test_persist_key_without_ttl() {
        let store = Store::new();
        store.set("foo", "bar".to_string());
        assert!(!store.persist("foo"));
    }

    #[test]
    fn test_persist_missing_key() {
        let store = Store::new();
        assert!(!store.persist("missing"));
    }

    #[test]
    fn test_get_returns_none_for_expired_key() {
        use std::time::Duration;
        let store = Store::new();
        store.set("foo", "bar".to_string());
        // Set an expiry that is already in the past
        store.set_ttl("foo", Instant::now() - Duration::from_secs(1));
        assert_eq!(store.get("foo"), None);
    }

    #[test]
    fn test_get_returns_value_for_key_with_future_expiry() {
        use std::time::Duration;
        let store = Store::new();
        store.set("foo", "bar".to_string());
        store.set_ttl("foo", Instant::now() + Duration::from_secs(60));
        assert_eq!(store.get("foo"), Some("bar".to_string()));
    }
}
