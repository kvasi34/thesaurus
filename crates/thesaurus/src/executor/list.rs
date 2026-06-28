use crate::resp2::RespValue;
use crate::store::{Store, WrongType};

use super::{Executor, WRONGTYPE_ERROR};

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
}
