mod list;
mod set;
mod string;

use std::collections::{HashMap, HashSet, VecDeque};
use std::mem;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use rand::seq::IteratorRandom;

use crate::pattern;

/// Wrapper for `Store` data structures.
#[derive(Clone, Debug, PartialEq)]
enum StoreValue {
    Str(String),
    List(VecDeque<String>),
    Set(HashSet<String>),
}

/// Shared in-memory key-value store.
///
/// Internally wraps an `Arc<RwLock<HashMap>>` so it can be cheaply cloned
/// and shared across handler tasks. All operations lock for the duration of
/// the call and release immediately on return.
#[derive(Clone, Debug)]
pub struct Store {
    inner: Arc<RwLock<StoreInner>>,
}

#[derive(Clone, Debug)]
struct StoreInner {
    data: HashMap<String, StoreValue>,
    expiry_index: HashMap<String, Instant>,
}

impl StoreInner {
    /// Returns a reference to the value for `key`, or `None` if the key does not exist or has
    /// expired.
    fn get(&self, key: &str) -> Option<&StoreValue> {
        if self
            .expiry_index
            .get(key)
            .is_some_and(|v| Instant::now() >= *v)
        {
            return None;
        }

        self.data.get(key)
    }

    /// Returns a mutable reference to the value for `key`, or `None` if the key does not exist or
    /// has expired.
    fn get_mut(&mut self, key: &str) -> Option<&mut StoreValue> {
        if self
            .expiry_index
            .get(key)
            .is_some_and(|v| Instant::now() >= *v)
        {
            return None;
        }

        self.data.get_mut(key)
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
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
    fn get(&self, key: &str) -> Option<StoreValue> {
        let guard = self.inner.read().unwrap();
        let expiry_entry = guard.expiry_index.get(key);
        if expiry_entry.is_none_or(|v| Instant::now() < *v) {
            return guard.data.get(key).cloned();
        }

        None
    }

    /// Inserts or overwrites `key` with `value`.
    fn set(&self, key: &str, value: StoreValue) {
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

    /// Gets the value for `key` and immediately delete the `key`.
    fn get_del(&self, key: &str) -> Option<StoreValue> {
        let mut guard = self.inner.write().unwrap();
        let expiry_entry = guard.expiry_index.get(key);
        if expiry_entry.is_none_or(|v| Instant::now() < *v) {
            guard.expiry_index.remove(key);
            return guard.data.remove(key);
        }

        None
    }

    /// Checks if a `key` exists in the store.
    pub fn exists(&self, key: &str) -> bool {
        let guard = self.inner.read().unwrap();
        guard.data.contains_key(key)
            && guard
                .expiry_index
                .get(key)
                .is_none_or(|v| Instant::now() < *v)
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

    /// Samples `n` pairs from the eviction index at random.
    pub fn sample_eviction_index(&self, n: usize) -> Vec<(String, Instant)> {
        let mut rng = rand::rng();
        let guard = self.inner.read().unwrap();
        guard
            .expiry_index
            .iter()
            .sample(&mut rng, n)
            .into_iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect()
    }

    /// Deletes all the keys in the `keys`. Skips keys that do not exist.
    pub fn delete_bulk(&self, keys: &Vec<String>) {
        let mut guard = self.inner.write().unwrap();
        for key in keys {
            guard.expiry_index.remove(key);
            guard.data.remove(key);
        }
    }

    /// Returns every key matching the glob-style `pattern`, skipping keys that have expired but
    /// have not been evicted yet. No pattern is ever rejected as malformed — an odd one simply
    /// matches few keys or none. O(N) time complexity over the keyspace.
    pub fn keys(&self, pattern: &[u8]) -> Vec<String> {
        // Redis short-circuits a pattern of exactly one star instead of consulting the matcher,
        // which is why `KEYS *` reports the empty key even though `*` alone does not match it.
        let all_keys = pattern == b"*";

        let guard = self.inner.read().unwrap();
        let now = Instant::now();
        guard
            .data
            .keys()
            .filter(|key| {
                (all_keys || pattern::string_match(key, pattern))
                    && guard
                        .expiry_index
                        .get(key.as_str())
                        .is_none_or(|v| now < *v)
            })
            .cloned()
            .collect()
    }

    /// Returns the number of keys in the store. It may slightly overcount because expired
    /// but un-evicted keys are also included. This approach guarantees O(1) time complexity.
    pub fn size(&self) -> usize {
        let guard = self.inner.read().unwrap();
        guard.data.len()
    }

    /// Removes all entries from the store and the expiry index.
    pub fn clear(&self) {
        let mut guard = self.inner.write().unwrap();
        guard.expiry_index.clear();
        guard.data.clear();
    }

    /// Creates a new store and expiry index and deletes old ones. O(1) time complexity.
    pub fn clear_async(&self) {
        let old_data;
        let old_expiry_index;

        {
            let mut guard = self.inner.write().unwrap();
            old_data = mem::take(&mut guard.data);
            old_expiry_index = mem::take(&mut guard.expiry_index);
        }

        tokio::spawn(async move {
            drop(old_data);
            drop(old_expiry_index);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // get
    #[test]
    fn test_get_missing_key() {
        let store = Store::new();
        assert_eq!(store.get("missing"), None);
    }

    #[test]
    fn test_get_returns_none_for_expired_key() {
        use std::time::Duration;
        let store = Store::new();
        store.set("foo", StoreValue::Str("bar".to_string()));
        store.set_ttl("foo", Instant::now() - Duration::from_secs(1));
        assert_eq!(store.get("foo"), None);
    }

    #[test]
    fn test_get_returns_value_for_key_with_future_expiry() {
        use std::time::Duration;
        let store = Store::new();
        store.set("foo", StoreValue::Str("bar".to_string()));
        store.set_ttl("foo", Instant::now() + Duration::from_secs(60));
        assert_eq!(store.get("foo"), Some(StoreValue::Str("bar".to_string())));
    }

    // set
    #[test]
    fn test_set_and_get() {
        let store = Store::new();
        store.set("foo", StoreValue::Str("bar".to_string()));
        assert_eq!(store.get("foo"), Some(StoreValue::Str("bar".to_string())));
    }

    #[test]
    fn test_set_overwrites_existing_key() {
        let store = Store::new();
        store.set("foo", StoreValue::Str("bar".to_string()));
        store.set("foo", StoreValue::Str("baz".to_string()));
        assert_eq!(store.get("foo"), Some(StoreValue::Str("baz".to_string())));
    }

    #[test]
    fn test_clones_share_same_data() {
        let store_a = Store::new();
        let store_b = store_a.clone();
        let store_c = store_a.clone();

        store_a.set("foo", StoreValue::Str("bar".to_string()));
        assert_eq!(store_b.get("foo"), Some(StoreValue::Str("bar".to_string())));

        store_b.set("foo", StoreValue::Str("baz".to_string()));
        assert_eq!(store_c.get("foo"), Some(StoreValue::Str("baz".to_string())));

        store_c.delete("foo");
        assert_eq!(store_a.get("foo"), None);
    }

    // delete
    #[test]
    fn test_delete_existing_key() {
        let store = Store::new();
        store.set("foo", StoreValue::Str("bar".to_string()));
        assert!(store.delete("foo"));
        assert_eq!(store.get("foo"), None);
    }

    #[test]
    fn test_delete_missing_key() {
        let store = Store::new();
        assert!(!store.delete("missing"));
    }

    // get_del
    #[test]
    fn test_get_del_existing_key() {
        let store = Store::new();
        store.set("foo", StoreValue::Str("bar".to_string()));
        assert_eq!(
            store.get_del("foo"),
            Some(StoreValue::Str("bar".to_string()))
        );
    }

    #[test]
    fn test_get_del_missing_key() {
        let store = Store::new();
        assert_eq!(store.get_del("missing"), None);
    }

    #[test]
    fn test_get_del_removes_key() {
        let store = Store::new();
        store.set("foo", StoreValue::Str("bar".to_string()));
        store.get_del("foo");
        assert_eq!(store.get("foo"), None);
    }

    #[test]
    fn test_get_del_removes_expiry() {
        use std::time::Duration;
        let store = Store::new();
        store.set("foo", StoreValue::Str("bar".to_string()));
        store.set_ttl("foo", Instant::now() + Duration::from_secs(60));
        store.get_del("foo");
        assert_eq!(store.get_ttl("foo"), None);
    }

    #[test]
    fn test_get_del_expired_key() {
        use std::time::Duration;
        let store = Store::new();
        store.set("foo", StoreValue::Str("bar".to_string()));
        store.set_ttl("foo", Instant::now() - Duration::from_secs(1));
        assert_eq!(store.get_del("foo"), None);
    }

    // exists
    #[test]
    fn test_exists_present_key() {
        let store = Store::new();
        store.set("foo", StoreValue::Str("bar".to_string()));
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
        store.set("foo", StoreValue::Str("bar".to_string()));
        store.delete("foo");
        assert!(!store.exists("foo"));
    }

    #[test]
    fn test_exists_expired_key() {
        use std::time::Duration;
        let store = Store::new();
        store.set("foo", StoreValue::Str("bar".to_string()));
        store.set_ttl("foo", Instant::now() - Duration::from_secs(1));
        assert!(!store.exists("foo"));
    }

    // get_ttl
    #[test]
    fn test_get_ttl_no_expiry() {
        let store = Store::new();
        store.set("foo", StoreValue::Str("bar".to_string()));
        assert_eq!(store.get_ttl("foo"), None);
    }

    #[test]
    fn test_get_ttl_with_expiry() {
        use std::time::Duration;
        let store = Store::new();
        store.set("foo", StoreValue::Str("bar".to_string()));
        let expiry = Instant::now() + Duration::from_secs(60);
        store.set_ttl("foo", expiry);
        assert_eq!(store.get_ttl("foo"), Some(expiry));
    }

    // set_ttl
    #[test]
    fn test_set_ttl_existing_key() {
        use std::time::Duration;
        let store = Store::new();
        store.set("foo", StoreValue::Str("bar".to_string()));
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
        store.set("foo", StoreValue::Str("bar".to_string()));
        let first_expiry = Instant::now() + Duration::from_secs(60);
        let second_expiry = Instant::now() + Duration::from_secs(120);
        store.set_ttl("foo", first_expiry);
        store.set_ttl("foo", second_expiry);
        assert_eq!(store.get_ttl("foo"), Some(second_expiry));
    }

    // persist
    #[test]
    fn test_persist_key_with_ttl() {
        use std::time::Duration;
        let store = Store::new();
        store.set("foo", StoreValue::Str("bar".to_string()));
        store.set_ttl("foo", Instant::now() + Duration::from_secs(60));
        assert!(store.persist("foo"));
    }

    #[test]
    fn test_persist_removes_ttl() {
        use std::time::Duration;
        let store = Store::new();
        store.set("foo", StoreValue::Str("bar".to_string()));
        store.set_ttl("foo", Instant::now() + Duration::from_secs(60));
        store.persist("foo");
        assert_eq!(store.get_ttl("foo"), None);
    }

    #[test]
    fn test_persist_key_without_ttl() {
        let store = Store::new();
        store.set("foo", StoreValue::Str("bar".to_string()));
        assert!(!store.persist("foo"));
    }

    #[test]
    fn test_persist_missing_key() {
        let store = Store::new();
        assert!(!store.persist("missing"));
    }

    #[test]
    fn test_delete_bulk() {
        let store = Store::new();
        for i in 0..10u8 {
            store.set(&format!("k{}", i), StoreValue::Str(format!("v{}", i)));
        }

        let keys: Vec<String> = (1..=5).map(|i| format!("k{}", i)).collect();
        store.delete_bulk(&keys);

        for i in 1..=5 {
            assert_eq!(store.get(&format!("k{}", i)), None);
        }
        for i in [0, 6, 7, 8, 9] {
            assert!(store.get(&format!("k{}", i)).is_some());
        }
    }

    #[test]
    fn test_delete_bulk_removes_expiry() {
        use std::time::Duration;
        let store = Store::new();
        store.set("foo", StoreValue::Str("bar".to_string()));
        store.set_ttl("foo", Instant::now() + Duration::from_secs(60));

        store.delete_bulk(&vec!["foo".to_string()]);

        assert_eq!(store.get("foo"), None);
        assert_eq!(store.get_ttl("foo"), None);
    }

    #[test]
    fn test_delete_bulk_skips_missing_keys() {
        let store = Store::new();
        store.set("foo", StoreValue::Str("bar".to_string()));

        store.delete_bulk(&vec!["foo".to_string(), "missing".to_string()]);

        assert_eq!(store.get("foo"), None);
    }

    // keys
    // Keys come out of a `HashMap` in no particular order, so sort before comparing.
    fn sorted_keys(store: &Store, pattern: &[u8]) -> Vec<String> {
        let mut keys = store.keys(pattern);
        keys.sort();
        keys
    }

    #[test]
    fn test_keys_empty_store() {
        let store = Store::new();
        assert_eq!(store.keys(b"*"), Vec::<String>::new());
    }

    #[test]
    fn test_keys_star_matches_every_key() {
        let store = Store::new();
        store.set("foo", StoreValue::Str("a".to_string()));
        store.set("bar", StoreValue::List(VecDeque::from(["b".to_string()])));
        store.set("baz", StoreValue::Set(HashSet::from(["c".to_string()])));

        assert_eq!(sorted_keys(&store, b"*"), vec!["bar", "baz", "foo"]);
    }

    #[test]
    fn test_keys_prefix_pattern() {
        let store = Store::new();
        store.set("user:1", StoreValue::Str("a".to_string()));
        store.set("user:2", StoreValue::Str("b".to_string()));
        store.set("session:1", StoreValue::Str("c".to_string()));

        assert_eq!(sorted_keys(&store, b"user:*"), vec!["user:1", "user:2"]);
    }

    #[test]
    fn test_keys_question_mark_matches_single_character() {
        let store = Store::new();
        store.set("key1", StoreValue::Str("a".to_string()));
        store.set("key12", StoreValue::Str("b".to_string()));

        assert_eq!(sorted_keys(&store, b"key?"), vec!["key1"]);
    }

    #[test]
    fn test_keys_character_class() {
        let store = Store::new();
        store.set("hallo", StoreValue::Str("a".to_string()));
        store.set("hello", StoreValue::Str("b".to_string()));
        store.set("hillo", StoreValue::Str("c".to_string()));

        assert_eq!(sorted_keys(&store, b"h[ae]llo"), vec!["hallo", "hello"]);
    }

    #[test]
    fn test_keys_literal_pattern_matches_exact_key_only() {
        let store = Store::new();
        store.set("foo", StoreValue::Str("a".to_string()));
        store.set("foobar", StoreValue::Str("b".to_string()));

        assert_eq!(store.keys(b"foo"), vec!["foo".to_string()]);
    }

    #[test]
    fn test_keys_no_match_returns_empty() {
        let store = Store::new();
        store.set("foo", StoreValue::Str("a".to_string()));

        assert_eq!(store.keys(b"bar*"), Vec::<String>::new());
    }

    #[test]
    fn test_keys_unterminated_character_class_is_not_an_error() {
        let store = Store::new();
        store.set("foo", StoreValue::Str("a".to_string()));
        store.set("f", StoreValue::Str("b".to_string()));

        // `[foo` is not rejected: it is the class {f, o} applied to a single character, so it
        // matches the one-character key but not "foo".
        assert_eq!(store.keys(b"[foo"), vec!["f".to_string()]);
    }

    #[test]
    fn test_keys_star_matches_the_empty_key() {
        let store = Store::new();
        store.set("", StoreValue::Str("a".to_string()));
        store.set("foo", StoreValue::Str("b".to_string()));

        // Redis reports the empty key for `*` through its all-keys shortcut, but not for `**`,
        // which goes through the matcher and needs at least one character to consume.
        assert_eq!(sorted_keys(&store, b"*"), vec!["", "foo"]);
        assert_eq!(store.keys(b"**"), vec!["foo".to_string()]);
    }

    #[test]
    fn test_keys_skips_expired_key() {
        use std::time::Duration;
        let store = Store::new();
        store.set("live", StoreValue::Str("a".to_string()));
        store.set("expired", StoreValue::Str("b".to_string()));
        store.set_ttl("expired", Instant::now() - Duration::from_secs(1));

        assert_eq!(store.keys(b"*"), vec!["live".to_string()]);
    }

    #[test]
    fn test_keys_includes_key_with_future_expiry() {
        use std::time::Duration;
        let store = Store::new();
        store.set("foo", StoreValue::Str("a".to_string()));
        store.set_ttl("foo", Instant::now() + Duration::from_secs(60));

        assert_eq!(store.keys(b"*"), vec!["foo".to_string()]);
    }
}
