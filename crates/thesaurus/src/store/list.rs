use std::collections::VecDeque;

use super::{Store, StoreValue, WrongType};

impl Store {
    /// Helper method. Pushes `element` onto the list at `key` using `push_fn` for direction. Creates the key if
    /// `create_if_missing` is `true`. Returns the new list length, or `0` if the key was absent
    /// and `create_if_missing` is `false`. Returns `Err(WrongType)` if the key holds a non-list value.
    fn push_inner(
        &self,
        key: &str,
        element: String,
        create_if_missing: bool,
        push_fn: impl Fn(&mut VecDeque<String>, String),
    ) -> Result<usize, WrongType> {
        let mut guard = self.inner.write().unwrap();
        match guard.data.get_mut(key) {
            None if create_if_missing => {
                guard
                    .data
                    .insert(key.to_string(), StoreValue::List(VecDeque::from([element])));
                Ok(1)
            }
            None => Ok(0),
            Some(StoreValue::List(l)) => {
                push_fn(l, element);
                Ok(l.len())
            }
            Some(_) => Err(WrongType),
        }
    }

    /// Prepends `element` to the list at `key`, creating the key if it does not exist. Returns the new list length.
    pub fn lpush(&self, key: &str, element: String) -> Result<usize, WrongType> {
        self.push_inner(key, element, true, VecDeque::push_front)
    }

    /// Appends `element` to the list at `key`, creating the key if it does not exist. Returns the new list length.
    pub fn rpush(&self, key: &str, element: String) -> Result<usize, WrongType> {
        self.push_inner(key, element, true, VecDeque::push_back)
    }

    /// Prepends `element` to the list at `key`. Returns `0` without inserting if the key does not exist.
    pub fn lpushx(&self, key: &str, element: String) -> Result<usize, WrongType> {
        self.push_inner(key, element, false, VecDeque::push_front)
    }

    /// Appends `element` to the list at `key`. Returns `0` without inserting if the key does not exist.
    pub fn rpushx(&self, key: &str, element: String) -> Result<usize, WrongType> {
        self.push_inner(key, element, false, VecDeque::push_back)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // lpush
    #[test]
    fn test_lpush_creates_list_on_missing_key() {
        let store = Store::new();
        assert_eq!(store.lpush("key", "a".to_string()), Ok(1));
        assert_eq!(
            store.get("key"),
            Some(StoreValue::List(VecDeque::from(["a".to_string()])))
        );
    }

    #[test]
    fn test_lpush_prepends_to_existing_list() {
        let store = Store::new();
        store.lpush("key", "a".to_string()).unwrap();
        assert_eq!(store.lpush("key", "b".to_string()), Ok(2));
        assert_eq!(
            store.get("key"),
            Some(StoreValue::List(VecDeque::from([
                "b".to_string(),
                "a".to_string()
            ])))
        );
    }

    #[test]
    fn test_lpush_returns_wrongtype_on_non_list_key() {
        let store = Store::new();
        store.set("key", StoreValue::Str("val".to_string()));
        assert_eq!(store.lpush("key", "a".to_string()), Err(WrongType));
    }

    // rpush
    #[test]
    fn test_rpush_creates_list_on_missing_key() {
        let store = Store::new();
        assert_eq!(store.rpush("key", "a".to_string()), Ok(1));
        assert_eq!(
            store.get("key"),
            Some(StoreValue::List(VecDeque::from(["a".to_string()])))
        );
    }

    #[test]
    fn test_rpush_appends_to_existing_list() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        assert_eq!(store.rpush("key", "b".to_string()), Ok(2));
        assert_eq!(
            store.get("key"),
            Some(StoreValue::List(VecDeque::from([
                "a".to_string(),
                "b".to_string()
            ])))
        );
    }

    #[test]
    fn test_rpush_returns_wrongtype_on_non_list_key() {
        let store = Store::new();
        store.set("key", StoreValue::Str("val".to_string()));
        assert_eq!(store.rpush("key", "a".to_string()), Err(WrongType));
    }

    // lpushx
    #[test]
    fn test_lpushx_returns_zero_on_missing_key() {
        let store = Store::new();
        assert_eq!(store.lpushx("key", "a".to_string()), Ok(0));
        assert!(!store.exists("key"));
    }

    #[test]
    fn test_lpushx_prepends_to_existing_list() {
        let store = Store::new();
        store.lpush("key", "a".to_string()).unwrap();
        assert_eq!(store.lpushx("key", "b".to_string()), Ok(2));
        assert_eq!(
            store.get("key"),
            Some(StoreValue::List(VecDeque::from([
                "b".to_string(),
                "a".to_string()
            ])))
        );
    }

    #[test]
    fn test_lpushx_returns_wrongtype_on_non_list_key() {
        let store = Store::new();
        store.set("key", StoreValue::Str("val".to_string()));
        assert_eq!(store.lpushx("key", "a".to_string()), Err(WrongType));
    }

    // rpushx
    #[test]
    fn test_rpushx_returns_zero_on_missing_key() {
        let store = Store::new();
        assert_eq!(store.rpushx("key", "a".to_string()), Ok(0));
        assert!(!store.exists("key"));
    }

    #[test]
    fn test_rpushx_appends_to_existing_list() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        assert_eq!(store.rpushx("key", "b".to_string()), Ok(2));
        assert_eq!(
            store.get("key"),
            Some(StoreValue::List(VecDeque::from([
                "a".to_string(),
                "b".to_string()
            ])))
        );
    }

    #[test]
    fn test_rpushx_returns_wrongtype_on_non_list_key() {
        let store = Store::new();
        store.set("key", StoreValue::Str("val".to_string()));
        assert_eq!(store.rpushx("key", "a".to_string()), Err(WrongType));
    }
}
