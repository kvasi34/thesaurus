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

    /// Helper method. Pops up to `count` elements from the list at `key` using `pop_fn` for
    /// direction. Removes the key when the list becomes empty. Returns `Ok(None)` if the key does
    /// not exist, `Ok(Some(elements))` with the popped elements otherwise. Returns
    /// `Err(WrongType)` if the key holds a non-list value.
    fn pop_inner(
        &self,
        key: &str,
        count: u64,
        pop_fn: impl Fn(&mut VecDeque<String>) -> Option<String>,
    ) -> Result<Option<Vec<String>>, WrongType> {
        let mut elements: Vec<String> = Vec::with_capacity(count as usize);
        let mut guard = self.inner.write().unwrap();

        // Ensure that the key exists and stores a `StoreValue::List` value
        let list = match guard.data.get_mut(key) {
            Some(StoreValue::List(l)) => l,
            Some(_) => return Err(WrongType),
            None => return Ok(None),
        };

        for _ in 0..count {
            match pop_fn(list) {
                Some(s) => elements.push(s),
                None => break,
            }
        }

        // Delete the key if the list is now empty
        let is_empty = list.is_empty();
        if is_empty {
            guard.data.remove(key);
        }

        Ok(Some(elements))
    }

    /// Removes and returns up to `count` elements from the front of the list at `key`. Returns
    /// `Ok(None)` if the key does not exist. Removes the key when the list becomes empty. Returns
    /// `Err(WrongType)` if the key holds a non-list value.
    pub fn lpop(&self, key: &str, count: u64) -> Result<Option<Vec<String>>, WrongType> {
        self.pop_inner(key, count, VecDeque::pop_front)
    }

    /// Removes and returns up to `count` elements from the back of the list at `key`. Returns
    /// `Ok(None)` if the key does not exist. Removes the key when the list becomes empty. Returns
    /// `Err(WrongType)` if the key holds a non-list value.
    pub fn rpop(&self, key: &str, count: u64) -> Result<Option<Vec<String>>, WrongType> {
        self.pop_inner(key, count, VecDeque::pop_back)
    }

    /// Returns the number of elements in the list at `key`. Returns `Ok(0)` if the key does not
    /// exist. Returns `Err(WrongType)` if the key holds a non-list value.
    pub fn llen(&self, key: &str) -> Result<usize, WrongType> {
        let guard = self.inner.read().unwrap();
        match guard.data.get(key) {
            None => Ok(0),
            Some(StoreValue::List(l)) => Ok(l.len()),
            Some(_) => Err(WrongType),
        }
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

    // lpop
    #[test]
    fn test_lpop_returns_none_on_missing_key() {
        let store = Store::new();
        assert_eq!(store.lpop("key", 1), Ok(None));
    }

    #[test]
    fn test_lpop_pops_from_front() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.rpush("key", "b".to_string()).unwrap();
        assert_eq!(store.lpop("key", 1), Ok(Some(vec!["a".to_string()])));
    }

    #[test]
    fn test_lpop_pops_multiple_elements() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.rpush("key", "b".to_string()).unwrap();
        store.rpush("key", "c".to_string()).unwrap();
        assert_eq!(
            store.lpop("key", 2),
            Ok(Some(vec!["a".to_string(), "b".to_string()]))
        );
    }

    #[test]
    fn test_lpop_removes_key_when_list_is_exhausted() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.lpop("key", 1).unwrap();
        assert!(!store.exists("key"));
    }

    #[test]
    fn test_lpop_returns_wrongtype_on_non_list_key() {
        let store = Store::new();
        store.set("key", StoreValue::Str("val".to_string()));
        assert_eq!(store.lpop("key", 1), Err(WrongType));
    }

    // rpop
    #[test]
    fn test_rpop_returns_none_on_missing_key() {
        let store = Store::new();
        assert_eq!(store.rpop("key", 1), Ok(None));
    }

    #[test]
    fn test_rpop_pops_from_back() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.rpush("key", "b".to_string()).unwrap();
        assert_eq!(store.rpop("key", 1), Ok(Some(vec!["b".to_string()])));
    }

    #[test]
    fn test_rpop_pops_multiple_elements() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.rpush("key", "b".to_string()).unwrap();
        store.rpush("key", "c".to_string()).unwrap();
        assert_eq!(
            store.rpop("key", 2),
            Ok(Some(vec!["c".to_string(), "b".to_string()]))
        );
    }

    #[test]
    fn test_rpop_removes_key_when_list_is_exhausted() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.rpop("key", 1).unwrap();
        assert!(!store.exists("key"));
    }

    #[test]
    fn test_rpop_returns_wrongtype_on_non_list_key() {
        let store = Store::new();
        store.set("key", StoreValue::Str("val".to_string()));
        assert_eq!(store.rpop("key", 1), Err(WrongType));
    }

    // llen
    #[test]
    fn test_llen_returns_zero_on_missing_key() {
        let store = Store::new();
        assert_eq!(store.llen("missing"), Ok(0));
    }

    #[test]
    fn test_llen_returns_length_of_list() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.rpush("key", "b".to_string()).unwrap();
        store.rpush("key", "c".to_string()).unwrap();
        assert_eq!(store.llen("key"), Ok(3));
    }

    #[test]
    fn test_llen_returns_wrongtype_on_non_list_key() {
        let store = Store::new();
        store.set("key", StoreValue::Str("val".to_string()));
        assert_eq!(store.llen("key"), Err(WrongType));
    }
}
