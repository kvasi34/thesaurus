use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Shared in-memory key-value store.
///
/// Internally wraps an `Arc<Mutex<HashMap>>` so it can be cheaply cloned
/// and shared across handler tasks. All operations lock the mutex for the
/// duration of the call and release it immediately on return.
#[derive(Clone)]
pub(crate) struct Store {
    data: Arc<Mutex<HashMap<String, String>>>,
}

impl Store {
    /// Creates a new empty `Store`.
    pub fn new() -> Self {
        Store {
            data: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Returns the value for `key`, or `None` if the key does not exist.
    pub fn get(&self, key: &str) -> Option<String> {
        let guard = self.data.lock().unwrap();
        guard.get(key).cloned()
    }

    /// Inserts or overwrites `key` with `value`.
    pub fn set(&self, key: &str, value: String) {
        let mut guard = self.data.lock().unwrap();
        guard.insert(key.to_string(), value);
    }

    /// Removes `key` from the store. Returns `true` if the key existed.
    pub fn delete(&self, key: &str) -> bool {
        let mut guard = self.data.lock().unwrap();
        guard.remove(key).is_some()
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
}
