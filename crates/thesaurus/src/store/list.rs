use std::collections::VecDeque;

use crate::errors::StoreError;

use super::{Store, StoreValue};

impl Store {
    /// Helper method. Pushes `element` onto the list at `key` using `push_fn` for direction. Creates the key if
    /// `create_if_missing` is `true`. Returns the new list length, or `0` if the key was absent
    /// and `create_if_missing` is `false`. Returns `Err(StoreError::WrongType)` if the key holds a non-list value.
    fn push_inner(
        &self,
        key: &str,
        element: String,
        create_if_missing: bool,
        push_fn: impl Fn(&mut VecDeque<String>, String),
    ) -> Result<usize, StoreError> {
        let mut guard = self.inner.write().unwrap();
        match guard.get_mut(key) {
            None if create_if_missing => {
                // Remove any stale expiry entry so the newly-created list is not immediately
                // considered expired. This is a no-op when the key was genuinely absent.
                guard.expiry_index.remove(key);
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
            Some(_) => Err(StoreError::WrongType),
        }
    }

    /// Prepends `element` to the list at `key`, creating the key if it does not exist. Returns the new list length.
    pub fn lpush(&self, key: &str, element: String) -> Result<usize, StoreError> {
        self.push_inner(key, element, true, VecDeque::push_front)
    }

    /// Appends `element` to the list at `key`, creating the key if it does not exist. Returns the new list length.
    pub fn rpush(&self, key: &str, element: String) -> Result<usize, StoreError> {
        self.push_inner(key, element, true, VecDeque::push_back)
    }

    /// Prepends `element` to the list at `key`. Returns `0` without inserting if the key does not exist.
    pub fn lpushx(&self, key: &str, element: String) -> Result<usize, StoreError> {
        self.push_inner(key, element, false, VecDeque::push_front)
    }

    /// Appends `element` to the list at `key`. Returns `0` without inserting if the key does not exist.
    pub fn rpushx(&self, key: &str, element: String) -> Result<usize, StoreError> {
        self.push_inner(key, element, false, VecDeque::push_back)
    }

    /// Helper method. Pops up to `count` elements from the list at `key` using `pop_fn` for
    /// direction. Removes the key when the list becomes empty. Returns `Ok(None)` if the key does
    /// not exist, `Ok(Some(elements))` with the popped elements otherwise. Returns
    /// `Err(StoreError::WrongType)` if the key holds a non-list value.
    fn pop_inner(
        &self,
        key: &str,
        count: u64,
        pop_fn: impl Fn(&mut VecDeque<String>) -> Option<String>,
    ) -> Result<Option<Vec<String>>, StoreError> {
        let mut elements: Vec<String> = Vec::with_capacity(count as usize);
        let mut guard = self.inner.write().unwrap();

        // Ensure that the key exists and stores a `StoreValue::List` value
        let list = match guard.get_mut(key) {
            Some(StoreValue::List(l)) => l,
            Some(_) => return Err(StoreError::WrongType),
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
    /// `Err(StoreError::WrongType)` if the key holds a non-list value.
    pub fn lpop(&self, key: &str, count: u64) -> Result<Option<Vec<String>>, StoreError> {
        self.pop_inner(key, count, VecDeque::pop_front)
    }

    /// Removes and returns up to `count` elements from the back of the list at `key`. Returns
    /// `Ok(None)` if the key does not exist. Removes the key when the list becomes empty. Returns
    /// `Err(StoreError::WrongType)` if the key holds a non-list value.
    pub fn rpop(&self, key: &str, count: u64) -> Result<Option<Vec<String>>, StoreError> {
        self.pop_inner(key, count, VecDeque::pop_back)
    }

    /// Sets the list element at index to element. Supports negative indexing, just like `LINDEX`.
    pub fn lset(&self, key: &str, index: i64, element: String) -> Result<(), StoreError> {
        let mut guard = self.inner.write().unwrap();
        match guard.get_mut(key) {
            Some(StoreValue::List(l)) => {
                let i = calculate_index(index, l.len())?;
                if i >= l.len() {
                    return Err(StoreError::OutOfIndex);
                }

                l.push_back(element);
                l.swap_remove_back(i);

                Ok(())
            }
            Some(_) => Err(StoreError::WrongType),
            None => Err(StoreError::NoSuchKey),
        }
    }

    /// Returns the number of elements in the list at `key`. Returns `Ok(0)` if the key does not
    /// exist. Returns `Err(StoreError::WrongType)` if the key holds a non-list value.
    pub fn llen(&self, key: &str) -> Result<usize, StoreError> {
        let guard = self.inner.read().unwrap();
        match guard.get(key) {
            None => Ok(0),
            Some(StoreValue::List(l)) => Ok(l.len()),
            Some(_) => Err(StoreError::WrongType),
        }
    }

    /// Returns the element at index index in the list stored at key. The index is zero-based.
    /// When the value at key is not a list, an error is returned. When the index is out of range, `Ok(None)` is returned.
    pub fn lindex(&self, key: &str, index: i64) -> Result<Option<String>, StoreError> {
        let guard = self.inner.read().unwrap();
        match guard.get(key) {
            Some(StoreValue::List(list)) => {
                let i = match calculate_index(index, list.len()) {
                    Ok(i) => i,
                    Err(_) => return Ok(None),
                };
                Ok(list.get(i).cloned())
            }
            Some(_) => Err(StoreError::WrongType),
            None => Ok(None),
        }
    }

    /// Returns the elements of the list at `key` between `start` and `stop`, inclusive. Both bounds
    /// are zero-based and support negative indexing, with `-1` denoting the last element.
    /// Out-of-range bounds are clamped rather than erroring. Returns an empty vector if the key does
    /// not exist, the list is empty, or `start` is greater than `stop`. Returns
    /// `Err(StoreError::WrongType)` if the key holds a non-list value.
    pub fn lrange(&self, key: &str, start: i64, stop: i64) -> Result<Vec<String>, StoreError> {
        let guard = self.inner.read().unwrap();
        match guard.get(key) {
            Some(StoreValue::List(l)) => {
                let len = l.len() as i64;
                if len == 0 {
                    return Ok(Vec::new());
                }

                // Resolve negative indices relative to the list length. A `start` that is still
                // negative afterwards clamps to 0; a `stop` that is still negative is left as-is so
                // that the start > stop check below correctly yields an empty result.
                let start = if start < 0 {
                    (start + len).max(0)
                } else {
                    start
                };
                let stop = if stop < 0 { stop + len } else { stop };

                // Return an empty array if start > stop, or if start is beyond the list
                if start > stop || start >= len {
                    return Ok(Vec::new());
                }

                // Clamp stop to the last valid index rather than erroring
                let stop = stop.min(len - 1);

                Ok(l.range(start as usize..=stop as usize)
                    .cloned()
                    .collect::<Vec<String>>())
            }
            Some(_) => Err(StoreError::WrongType),
            None => Ok(Vec::new()),
        }
    }
}

/// Helper method which handles negative indexes such as -1, -2, etc.
fn calculate_index(index: i64, list_len: usize) -> Result<usize, StoreError> {
    if index < 0 {
        let abs = index.unsigned_abs() as usize;
        if abs > list_len {
            return Err(StoreError::OutOfIndex);
        }

        Ok(list_len - abs)
    } else {
        Ok(index as usize)
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
        assert_eq!(
            store.lpush("key", "a".to_string()),
            Err(StoreError::WrongType)
        );
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
        assert_eq!(
            store.rpush("key", "a".to_string()),
            Err(StoreError::WrongType)
        );
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
        assert_eq!(
            store.lpushx("key", "a".to_string()),
            Err(StoreError::WrongType)
        );
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
        assert_eq!(
            store.rpushx("key", "a".to_string()),
            Err(StoreError::WrongType)
        );
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
        assert_eq!(store.lpop("key", 1), Err(StoreError::WrongType));
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
        assert_eq!(store.rpop("key", 1), Err(StoreError::WrongType));
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
        assert_eq!(store.llen("key"), Err(StoreError::WrongType));
    }

    // lindex
    #[test]
    fn test_lindex_returns_nil_on_missing_key() {
        let store = Store::new();
        assert_eq!(store.lindex("missing", 0), Ok(None));
    }

    #[test]
    fn test_lindex_returns_wrongtype_on_non_list_key() {
        let store = Store::new();
        store.set("key", StoreValue::Str("val".to_string()));
        assert_eq!(store.lindex("key", 0), Err(StoreError::WrongType));
    }

    #[test]
    fn test_lindex_returns_element_at_positive_index() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.rpush("key", "b".to_string()).unwrap();
        store.rpush("key", "c".to_string()).unwrap();
        assert_eq!(store.lindex("key", 0), Ok(Some("a".to_string())));
        assert_eq!(store.lindex("key", 2), Ok(Some("c".to_string())));
    }

    #[test]
    fn test_lindex_positive_index_out_of_bounds_returns_nil() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        assert_eq!(store.lindex("key", 1), Ok(None));
    }

    #[test]
    fn test_lindex_returns_element_at_negative_index() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.rpush("key", "b".to_string()).unwrap();
        store.rpush("key", "c".to_string()).unwrap();
        assert_eq!(store.lindex("key", -1), Ok(Some("c".to_string())));
        assert_eq!(store.lindex("key", -3), Ok(Some("a".to_string())));
    }

    #[test]
    fn test_lindex_negative_index_out_of_bounds_returns_nil() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.rpush("key", "b".to_string()).unwrap();
        assert_eq!(store.lindex("key", -3), Ok(None));
    }

    // lset
    #[test]
    fn test_lset_returns_nosuchkey_on_missing_key() {
        let store = Store::new();
        assert_eq!(
            store.lset("key", 0, "x".to_string()),
            Err(StoreError::NoSuchKey)
        );
    }

    #[test]
    fn test_lset_returns_wrongtype_on_non_list_key() {
        let store = Store::new();
        store.set("key", StoreValue::Str("val".to_string()));
        assert_eq!(
            store.lset("key", 0, "x".to_string()),
            Err(StoreError::WrongType)
        );
    }

    #[test]
    fn test_lset_sets_element_at_positive_index() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.rpush("key", "b".to_string()).unwrap();
        store.rpush("key", "c".to_string()).unwrap();
        assert_eq!(store.lset("key", 1, "x".to_string()), Ok(()));
        assert_eq!(store.lindex("key", 0), Ok(Some("a".to_string())));
        assert_eq!(store.lindex("key", 1), Ok(Some("x".to_string())));
        assert_eq!(store.lindex("key", 2), Ok(Some("c".to_string())));
    }

    #[test]
    fn test_lset_sets_element_at_negative_index() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.rpush("key", "b".to_string()).unwrap();
        store.rpush("key", "c".to_string()).unwrap();
        assert_eq!(store.lset("key", -1, "x".to_string()), Ok(()));
        assert_eq!(store.lindex("key", 0), Ok(Some("a".to_string())));
        assert_eq!(store.lindex("key", 1), Ok(Some("b".to_string())));
        assert_eq!(store.lindex("key", 2), Ok(Some("x".to_string())));
    }

    #[test]
    fn test_lset_returns_outofindex_on_positive_index_out_of_bounds() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        assert_eq!(
            store.lset("key", 1, "x".to_string()),
            Err(StoreError::OutOfIndex)
        );
    }

    #[test]
    fn test_lset_returns_outofindex_on_negative_index_out_of_bounds() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.rpush("key", "b".to_string()).unwrap();
        assert_eq!(
            store.lset("key", -3, "x".to_string()),
            Err(StoreError::OutOfIndex)
        );
    }

    #[test]
    fn test_lset_returns_nosuchkey_on_expired_key() {
        use std::time::{Duration, Instant};
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.set_ttl("key", Instant::now() - Duration::from_secs(1));
        assert_eq!(
            store.lset("key", 0, "x".to_string()),
            Err(StoreError::NoSuchKey)
        );
    }

    // lrange
    #[test]
    fn test_lrange_returns_empty_on_missing_key() {
        let store = Store::new();
        assert_eq!(store.lrange("missing", 0, -1), Ok(Vec::new()));
    }

    #[test]
    fn test_lrange_returns_wrongtype_on_non_list_key() {
        let store = Store::new();
        store.set("key", StoreValue::Str("val".to_string()));
        assert_eq!(store.lrange("key", 0, -1), Err(StoreError::WrongType));
    }

    #[test]
    fn test_lrange_returns_full_list_with_zero_and_negative_one() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.rpush("key", "b".to_string()).unwrap();
        store.rpush("key", "c".to_string()).unwrap();
        assert_eq!(
            store.lrange("key", 0, -1),
            Ok(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        );
    }

    #[test]
    fn test_lrange_returns_subset_with_positive_indices() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.rpush("key", "b".to_string()).unwrap();
        store.rpush("key", "c".to_string()).unwrap();
        assert_eq!(
            store.lrange("key", 0, 1),
            Ok(vec!["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn test_lrange_returns_subset_with_negative_indices() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.rpush("key", "b".to_string()).unwrap();
        store.rpush("key", "c".to_string()).unwrap();
        assert_eq!(
            store.lrange("key", -2, -1),
            Ok(vec!["b".to_string(), "c".to_string()])
        );
    }

    #[test]
    fn test_lrange_clamps_start_more_negative_than_list_length() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.rpush("key", "b".to_string()).unwrap();
        assert_eq!(
            store.lrange("key", -100, -1),
            Ok(vec!["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn test_lrange_clamps_stop_beyond_list_length() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.rpush("key", "b".to_string()).unwrap();
        assert_eq!(
            store.lrange("key", 0, 100),
            Ok(vec!["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn test_lrange_returns_empty_when_start_greater_than_stop() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.rpush("key", "b".to_string()).unwrap();
        store.rpush("key", "c".to_string()).unwrap();
        assert_eq!(store.lrange("key", 2, 0), Ok(Vec::new()));
    }

    #[test]
    fn test_lrange_returns_empty_when_start_beyond_list_length() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        assert_eq!(store.lrange("key", 5, 10), Ok(Vec::new()));
    }

    #[test]
    fn test_lrange_returns_empty_when_stop_still_negative_after_resolution() {
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.rpush("key", "b".to_string()).unwrap();
        store.rpush("key", "c".to_string()).unwrap();
        assert_eq!(store.lrange("key", 0, -100), Ok(Vec::new()));
    }

    // expiry
    #[test]
    fn test_llen_returns_zero_on_expired_key() {
        use std::time::{Duration, Instant};
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.set_ttl("key", Instant::now() - Duration::from_secs(1));
        assert_eq!(store.llen("key"), Ok(0));
    }

    #[test]
    fn test_lpop_returns_none_on_expired_key() {
        use std::time::{Duration, Instant};
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.set_ttl("key", Instant::now() - Duration::from_secs(1));
        assert_eq!(store.lpop("key", 1), Ok(None));
    }

    #[test]
    fn test_rpop_returns_none_on_expired_key() {
        use std::time::{Duration, Instant};
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.set_ttl("key", Instant::now() - Duration::from_secs(1));
        assert_eq!(store.rpop("key", 1), Ok(None));
    }

    #[test]
    fn test_lpush_creates_fresh_list_on_expired_key() {
        use std::time::{Duration, Instant};
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.set_ttl("key", Instant::now() - Duration::from_secs(1));
        assert_eq!(store.lpush("key", "b".to_string()), Ok(1));
        assert_eq!(store.get_ttl("key"), None);
    }

    #[test]
    fn test_rpush_creates_fresh_list_on_expired_key() {
        use std::time::{Duration, Instant};
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.set_ttl("key", Instant::now() - Duration::from_secs(1));
        assert_eq!(store.rpush("key", "b".to_string()), Ok(1));
        assert_eq!(store.get_ttl("key"), None);
    }

    #[test]
    fn test_lpushx_returns_zero_on_expired_key() {
        use std::time::{Duration, Instant};
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.set_ttl("key", Instant::now() - Duration::from_secs(1));
        assert_eq!(store.lpushx("key", "b".to_string()), Ok(0));
    }

    #[test]
    fn test_rpushx_returns_zero_on_expired_key() {
        use std::time::{Duration, Instant};
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.set_ttl("key", Instant::now() - Duration::from_secs(1));
        assert_eq!(store.rpushx("key", "b".to_string()), Ok(0));
    }

    #[test]
    fn test_lindex_returns_nil_on_expired_key() {
        use std::time::{Duration, Instant};
        let store = Store::new();
        store.rpush("key", "a".to_string()).unwrap();
        store.set_ttl("key", Instant::now() - Duration::from_secs(1));
        assert_eq!(store.lindex("key", 0), Ok(None));
    }
}
