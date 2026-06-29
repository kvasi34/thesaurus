use crate::resp2::RespValue;
use crate::store::{Store, WrongType};

use super::{Executor, WRONGTYPE_ERROR};

/// Function pointer type for store-level pop operations (`Store::lpop` / `Store::rpop`).
type PopFn = fn(&Store, &str, u64) -> Result<Option<Vec<String>>, WrongType>;

impl Executor {
    /// Shared logic for all push variants. Iterates `elements`, calling `push_fn` for each, and
    /// returns the final list length. Short-circuits with a WRONGTYPE error if the key holds a
    /// non-list value.
    fn push_inner(
        store: &Store,
        key: &str,
        elements: &[String],
        push_fn: fn(&Store, &str, String) -> Result<usize, WrongType>,
    ) -> RespValue {
        let mut size = 0;
        for element in elements {
            match push_fn(store, key, element.clone()) {
                Ok(n) => size = n,
                Err(_) => return RespValue::SimpleError(WRONGTYPE_ERROR.to_string()),
            }
        }

        RespValue::Integer(size as i64)
    }

    /// Handles LPUSH: prepends each element in order, returns the new list length.
    pub(super) fn lpush(&self, key: &str, elements: &[String]) -> RespValue {
        Self::push_inner(&self.store, key, elements, Store::lpush)
    }

    /// Handles RPUSH: appends each element in order, returns the new list length.
    pub(super) fn rpush(&self, key: &str, elements: &[String]) -> RespValue {
        Self::push_inner(&self.store, key, elements, Store::rpush)
    }

    /// Handles LPUSHX: prepends each element only if the key already exists, returns the new list length.
    pub(super) fn lpushx(&self, key: &str, elements: &[String]) -> RespValue {
        Self::push_inner(&self.store, key, elements, Store::lpushx)
    }

    /// Handles RPUSHX: appends each element only if the key already exists, returns the new list length.
    pub(super) fn rpushx(&self, key: &str, elements: &[String]) -> RespValue {
        Self::push_inner(&self.store, key, elements, Store::rpushx)
    }

    /// Shared logic for all pop variants. When `count` is `None`, returns a single bulk string
    /// (matching the no-count form of LPOP/RPOP). When `count` is `Some`, returns an array.
    /// Returns nil if the key does not exist. Short-circuits with a WRONGTYPE error if the key
    /// holds a non-list value.
    fn pop_inner(store: &Store, key: &str, count: Option<u64>, pop_fn: PopFn) -> RespValue {
        let has_count = count.is_some();
        match pop_fn(store, key, count.unwrap_or(1)) {
            Ok(Some(elements)) => {
                if !has_count {
                    return RespValue::BulkString(elements.into_iter().next());
                }

                RespValue::Array(Some(
                    elements
                        .into_iter()
                        .map(|element| RespValue::BulkString(Some(element)))
                        .collect(),
                ))
            }
            Ok(None) => RespValue::BulkString(None),
            Err(_) => RespValue::SimpleError(WRONGTYPE_ERROR.to_string()),
        }
    }

    /// Handles LPOP: removes and returns element(s) from the front of the list. Without a count,
    /// returns a single bulk string; with a count, returns an array.
    pub(super) fn lpop(&self, key: &str, count: Option<u64>) -> RespValue {
        Self::pop_inner(&self.store, key, count, Store::lpop)
    }

    /// Handles RPOP: removes and returns element(s) from the back of the list. Without a count,
    /// returns a single bulk string; with a count, returns an array.
    pub(super) fn rpop(&self, key: &str, count: Option<u64>) -> RespValue {
        Self::pop_inner(&self.store, key, count, Store::rpop)
    }

    /// Handles LLEN: returns the number of elements in the list at `key`. Returns 0 if the key
    /// does not exist. Returns a WRONGTYPE error if the key holds a non-list value.
    pub(super) fn llen(&self, key: &str) -> RespValue {
        match self.store.llen(key) {
            Ok(n) => RespValue::Integer(n as i64),
            Err(_) => RespValue::SimpleError(WRONGTYPE_ERROR.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::resp2::RespValue;
    use crate::store::Store;

    use super::super::Executor;

    fn executor() -> Executor {
        Executor::new(Store::new(), false)
    }

    fn els(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // lpush
    #[test]
    fn test_lpush_creates_list_and_returns_length() {
        let ex = executor();
        assert_eq!(ex.lpush("key", &els(&["a"])), RespValue::Integer(1));
    }

    #[test]
    fn test_lpush_multiple_elements_returns_final_length() {
        let ex = executor();
        assert_eq!(
            ex.lpush("key", &els(&["a", "b", "c"])),
            RespValue::Integer(3)
        );
    }

    #[test]
    fn test_lpush_returns_wrongtype_on_non_list_key() {
        let ex = executor();
        ex.store.set_string("key", "val");
        assert!(matches!(
            ex.lpush("key", &els(&["a"])),
            RespValue::SimpleError(_)
        ));
    }

    // rpush
    #[test]
    fn test_rpush_creates_list_and_returns_length() {
        let ex = executor();
        assert_eq!(ex.rpush("key", &els(&["a"])), RespValue::Integer(1));
    }

    #[test]
    fn test_rpush_multiple_elements_returns_final_length() {
        let ex = executor();
        assert_eq!(
            ex.rpush("key", &els(&["a", "b", "c"])),
            RespValue::Integer(3)
        );
    }

    #[test]
    fn test_rpush_returns_wrongtype_on_non_list_key() {
        let ex = executor();
        ex.store.set_string("key", "val");
        assert!(matches!(
            ex.rpush("key", &els(&["a"])),
            RespValue::SimpleError(_)
        ));
    }

    // lpushx
    #[test]
    fn test_lpushx_returns_zero_on_missing_key() {
        let ex = executor();
        assert_eq!(ex.lpushx("key", &els(&["a"])), RespValue::Integer(0));
    }

    #[test]
    fn test_lpushx_pushes_to_existing_list() {
        let ex = executor();
        ex.lpush("key", &els(&["a"]));
        assert_eq!(ex.lpushx("key", &els(&["b"])), RespValue::Integer(2));
    }

    #[test]
    fn test_lpushx_returns_wrongtype_on_non_list_key() {
        let ex = executor();
        ex.store.set_string("key", "val");
        assert!(matches!(
            ex.lpushx("key", &els(&["a"])),
            RespValue::SimpleError(_)
        ));
    }

    // rpushx
    #[test]
    fn test_rpushx_returns_zero_on_missing_key() {
        let ex = executor();
        assert_eq!(ex.rpushx("key", &els(&["a"])), RespValue::Integer(0));
    }

    #[test]
    fn test_rpushx_pushes_to_existing_list() {
        let ex = executor();
        ex.rpush("key", &els(&["a"]));
        assert_eq!(ex.rpushx("key", &els(&["b"])), RespValue::Integer(2));
    }

    #[test]
    fn test_rpushx_returns_wrongtype_on_non_list_key() {
        let ex = executor();
        ex.store.set_string("key", "val");
        assert!(matches!(
            ex.rpushx("key", &els(&["a"])),
            RespValue::SimpleError(_)
        ));
    }

    // lpop
    #[test]
    fn test_lpop_returns_nil_on_missing_key() {
        let ex = executor();
        assert_eq!(ex.lpop("key", None), RespValue::BulkString(None));
    }

    #[test]
    fn test_lpop_without_count_returns_bulk_string() {
        let ex = executor();
        ex.rpush("key", &els(&["a", "b"]));
        assert_eq!(
            ex.lpop("key", None),
            RespValue::BulkString(Some("a".to_string()))
        );
    }

    #[test]
    fn test_lpop_with_count_returns_array() {
        let ex = executor();
        ex.rpush("key", &els(&["a", "b", "c"]));
        assert_eq!(
            ex.lpop("key", Some(2)),
            RespValue::Array(Some(vec![
                RespValue::BulkString(Some("a".to_string())),
                RespValue::BulkString(Some("b".to_string())),
            ]))
        );
    }

    #[test]
    fn test_lpop_returns_wrongtype_on_non_list_key() {
        let ex = executor();
        ex.store.set_string("key", "val");
        assert!(matches!(ex.lpop("key", None), RespValue::SimpleError(_)));
    }

    // rpop
    #[test]
    fn test_rpop_returns_nil_on_missing_key() {
        let ex = executor();
        assert_eq!(ex.rpop("key", None), RespValue::BulkString(None));
    }

    #[test]
    fn test_rpop_without_count_returns_bulk_string() {
        let ex = executor();
        ex.rpush("key", &els(&["a", "b"]));
        assert_eq!(
            ex.rpop("key", None),
            RespValue::BulkString(Some("b".to_string()))
        );
    }

    #[test]
    fn test_rpop_with_count_returns_array() {
        let ex = executor();
        ex.rpush("key", &els(&["a", "b", "c"]));
        assert_eq!(
            ex.rpop("key", Some(2)),
            RespValue::Array(Some(vec![
                RespValue::BulkString(Some("c".to_string())),
                RespValue::BulkString(Some("b".to_string())),
            ]))
        );
    }

    #[test]
    fn test_rpop_returns_wrongtype_on_non_list_key() {
        let ex = executor();
        ex.store.set_string("key", "val");
        assert!(matches!(ex.rpop("key", None), RespValue::SimpleError(_)));
    }

    // llen
    #[test]
    fn test_llen_returns_zero_on_missing_key() {
        let ex = executor();
        assert_eq!(ex.llen("missing"), RespValue::Integer(0));
    }

    #[test]
    fn test_llen_returns_length_of_list() {
        let ex = executor();
        ex.rpush("key", &els(&["a", "b", "c"]));
        assert_eq!(ex.llen("key"), RespValue::Integer(3));
    }

    #[test]
    fn test_llen_returns_wrongtype_on_non_list_key() {
        let ex = executor();
        ex.store.set_string("key", "val");
        assert!(matches!(ex.llen("key"), RespValue::SimpleError(_)));
    }
}
