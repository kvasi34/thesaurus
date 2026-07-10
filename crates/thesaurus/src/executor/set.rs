use crate::resp2::RespValue;

use super::Executor;

impl Executor {
    /// Handles SADD: adds each member in `members` to the set at `key`, creating the key if it
    /// does not exist. Returns the number of members that were newly added (i.e. not already
    /// present). Returns a WRONGTYPE error if the key holds a non-set value.
    pub(super) fn sadd(&self, key: &str, members: &[String]) -> RespValue {
        match self.store.sadd(key, members.iter().cloned()) {
            Ok(u) => RespValue::Integer(u as i64),
            Err(e) => RespValue::SimpleError(e.to_string()),
        }
    }

    /// Handles SMEMBERS: returns all members of the set at `key` as an array of bulk strings, in
    /// no particular order. Returns an empty array if the key does not exist. Returns a WRONGTYPE
    /// error if the key holds a non-set value.
    pub(super) fn smembers(&self, key: &str) -> RespValue {
        match self.store.smembers(key) {
            Ok(elements) => RespValue::Array(Some(
                elements
                    .into_iter()
                    .map(|element| RespValue::BulkString(Some(element)))
                    .collect::<Vec<RespValue>>(),
            )),
            Err(e) => RespValue::SimpleError(e.to_string()),
        }
    }

    /// Handles SCARD: returns the number of members in the set at `key`. Returns 0 if the key
    /// does not exist. Returns a WRONGTYPE error if the key holds a non-set value.
    pub(super) fn scard(&self, key: &str) -> RespValue {
        match self.store.scard(key) {
            Ok(u) => RespValue::Integer(u as i64),
            Err(e) => RespValue::SimpleError(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::resp2::RespValue;
    use crate::store::Store;

    use super::super::Executor;

    fn executor() -> Executor {
        Executor::new(Store::new(), false)
    }

    fn els(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // SMEMBERS returns members from a `HashSet`, so order is not guaranteed. This unwraps the
    // RespValue::Array into a set of strings for order-independent comparison.
    fn members_of(resp: RespValue) -> HashSet<String> {
        match resp {
            RespValue::Array(Some(items)) => items
                .into_iter()
                .map(|item| match item {
                    RespValue::BulkString(Some(s)) => s,
                    other => panic!("expected bulk string, got {other:?}"),
                })
                .collect(),
            other => panic!("expected array, got {other:?}"),
        }
    }

    // sadd
    #[test]
    fn test_sadd_creates_set_and_returns_count() {
        let ex = executor();
        assert_eq!(ex.sadd("key", &els(&["a"])), RespValue::Integer(1));
    }

    #[test]
    fn test_sadd_dedupes_members_and_returns_count() {
        let ex = executor();
        assert_eq!(
            ex.sadd("key", &els(&["a", "a", "b"])),
            RespValue::Integer(2)
        );
    }

    #[test]
    fn test_sadd_returns_count_of_newly_added_members() {
        let ex = executor();
        ex.sadd("key", &els(&["a"]));
        assert_eq!(ex.sadd("key", &els(&["a", "b"])), RespValue::Integer(1));
    }

    #[test]
    fn test_sadd_returns_zero_when_all_members_already_present() {
        let ex = executor();
        ex.sadd("key", &els(&["a"]));
        assert_eq!(ex.sadd("key", &els(&["a"])), RespValue::Integer(0));
    }

    #[test]
    fn test_sadd_returns_wrongtype_on_non_set_key() {
        let ex = executor();
        ex.store.set_string("key", "val");
        assert!(matches!(
            ex.sadd("key", &els(&["a"])),
            RespValue::SimpleError(_)
        ));
    }

    // smembers
    #[test]
    fn test_smembers_returns_empty_array_on_missing_key() {
        let ex = executor();
        assert_eq!(ex.smembers("missing"), RespValue::Array(Some(Vec::new())));
    }

    #[test]
    fn test_smembers_returns_all_members() {
        let ex = executor();
        ex.sadd("key", &els(&["a", "b", "c"]));
        assert_eq!(
            members_of(ex.smembers("key")),
            HashSet::from(["a".to_string(), "b".to_string(), "c".to_string()])
        );
    }

    #[test]
    fn test_smembers_returns_wrongtype_on_non_set_key() {
        let ex = executor();
        ex.store.set_string("key", "val");
        assert!(matches!(ex.smembers("key"), RespValue::SimpleError(_)));
    }

    // scard
    #[test]
    fn test_scard_returns_zero_on_missing_key() {
        let ex = executor();
        assert_eq!(ex.scard("missing"), RespValue::Integer(0));
    }

    #[test]
    fn test_scard_returns_member_count() {
        let ex = executor();
        ex.sadd("key", &els(&["a", "b", "c"]));
        assert_eq!(ex.scard("key"), RespValue::Integer(3));
    }

    #[test]
    fn test_scard_does_not_count_duplicates() {
        let ex = executor();
        ex.sadd("key", &els(&["a", "a"]));
        assert_eq!(ex.scard("key"), RespValue::Integer(1));
    }

    #[test]
    fn test_scard_returns_wrongtype_on_non_set_key() {
        let ex = executor();
        ex.store.set_string("key", "val");
        assert!(matches!(ex.scard("key"), RespValue::SimpleError(_)));
    }
}
