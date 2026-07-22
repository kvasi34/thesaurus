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
            Ok(members) => RespValue::Array(Some(
                members
                    .into_iter()
                    .map(|member| RespValue::BulkString(Some(member)))
                    .collect::<Vec<RespValue>>(),
            )),
            Err(e) => RespValue::SimpleError(e.to_string()),
        }
    }

    /// Handles SISMEMBER: returns whether `member` is a member of the set at `key`. Returns 0 if
    /// the key does not exist. Returns a WRONGTYPE error if the key holds a non-set value.
    pub(super) fn sismember(&self, key: &str, member: &str) -> RespValue {
        match self.store.sismember(key, member.to_string()) {
            Ok(b) => RespValue::Integer(b as i64),
            Err(e) => RespValue::SimpleError(e.to_string()),
        }
    }

    /// Handles SMISMEMBER: returns, for each member in `members`, whether it is a member of the
    /// set at `key`, as an array of 0/1 integers in the same order as `members`. Returns all
    /// zeros if the key does not exist. Returns a WRONGTYPE error if the key holds a non-set
    /// value.
    pub(super) fn smismember(&self, key: &str, members: &[String]) -> RespValue {
        match self.store.smismember(key, members.to_vec()) {
            Ok(flags) => RespValue::Array(Some(
                flags
                    .into_iter()
                    .map(|b| RespValue::Integer(b as i64))
                    .collect::<Vec<RespValue>>(),
            )),
            Err(e) => RespValue::SimpleError(e.to_string()),
        }
    }

    /// Handles SRANDMEMBER: returns one or more random members from the set at `key`, without
    /// removing them. Without a count, returns a single bulk string (or nil if the key does not
    /// exist). With a non-negative count, returns an array of up to `count` distinct members
    /// (fewer if the set has fewer members, empty if the key does not exist). With a negative
    /// count, returns an array of exactly `count.abs()` members, allowing duplicates (empty if
    /// the key does not exist). Returns a WRONGTYPE error if the key holds a non-set value.
    pub(super) fn srandmember(&self, key: &str, count: Option<i64>) -> RespValue {
        let amount = count.map(|c| c.unsigned_abs() as usize);
        let allow_repetition = count.is_some_and(|c| c < 0);

        match self.store.srandmember(key, amount, allow_repetition) {
            Ok(mut members) => {
                if count.is_some() {
                    return RespValue::Array(Some(
                        members
                            .into_iter()
                            .map(|member| RespValue::BulkString(Some(member)))
                            .collect::<Vec<RespValue>>(),
                    ));
                }

                RespValue::BulkString(members.pop())
            }
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

    /// Handles SMOVE: moves `member` from the set at `source` to the set at `destination`,
    /// creating `destination` if it does not exist. Returns 1 if the member was moved. Returns 0
    /// without modifying either key if `source` does not exist or does not contain `member`.
    /// Returns a WRONGTYPE error if `source` or `destination` holds a non-set value.
    pub(super) fn smove(&self, source: &str, destination: &str, member: &str) -> RespValue {
        match self.store.smove(source, destination, member.to_string()) {
            Ok(b) => RespValue::Integer(b as i64),
            Err(e) => RespValue::SimpleError(e.to_string()),
        }
    }

    /// Handles SPOP: removes and returns one or more random members from the set at `key`.
    /// Without a count, returns a single bulk string (or nil if the key does not exist); with a
    /// count, returns an array (an empty array if the key does not exist, fewer elements if the
    /// set has fewer members than `count`, and the key is removed once the set becomes empty).
    /// Returns a WRONGTYPE error if the key holds a non-set value.
    pub(super) fn spop(&self, key: &str, count: Option<u64>) -> RespValue {
        match self.store.spop(key, count) {
            Ok(None) => {
                if count.is_some() {
                    return RespValue::Array(Some(Vec::new()));
                }

                RespValue::BulkString(None)
            }
            Ok(Some(mut members)) => {
                if count.is_some() {
                    return RespValue::Array(Some(
                        members
                            .into_iter()
                            .map(|member| RespValue::BulkString(Some(member)))
                            .collect::<Vec<RespValue>>(),
                    ));
                }

                RespValue::BulkString(members.pop())
            }
            Err(e) => RespValue::SimpleError(e.to_string()),
        }
    }

    /// Handles SREM: removes each member in `members` from the set at `key`. Returns the number
    /// of members that were removed (i.e. that were present). Returns a WRONGTYPE error if the
    /// key holds a non-set value.
    pub(super) fn srem(&self, key: &str, members: &[String]) -> RespValue {
        match self.store.srem(key, members.iter().cloned()) {
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

    // sismember
    #[test]
    fn test_sismember_returns_zero_on_missing_key() {
        let ex = executor();
        assert_eq!(ex.sismember("missing", "a"), RespValue::Integer(0));
    }

    #[test]
    fn test_sismember_returns_one_when_member_present() {
        let ex = executor();
        ex.sadd("key", &els(&["a"]));
        assert_eq!(ex.sismember("key", "a"), RespValue::Integer(1));
    }

    #[test]
    fn test_sismember_returns_zero_when_member_absent() {
        let ex = executor();
        ex.sadd("key", &els(&["a"]));
        assert_eq!(ex.sismember("key", "b"), RespValue::Integer(0));
    }

    #[test]
    fn test_sismember_returns_wrongtype_on_non_set_key() {
        let ex = executor();
        ex.store.set_string("key", "val");
        assert!(matches!(
            ex.sismember("key", "a"),
            RespValue::SimpleError(_)
        ));
    }

    // smismember
    #[test]
    fn test_smismember_returns_all_zero_on_missing_key() {
        let ex = executor();
        assert_eq!(
            ex.smismember("missing", &els(&["a", "b"])),
            RespValue::Array(Some(vec![RespValue::Integer(0), RespValue::Integer(0)]))
        );
    }

    #[test]
    fn test_smismember_returns_flag_per_member_in_order() {
        let ex = executor();
        ex.sadd("key", &els(&["a", "b"]));
        assert_eq!(
            ex.smismember("key", &els(&["a", "c", "b"])),
            RespValue::Array(Some(vec![
                RespValue::Integer(1),
                RespValue::Integer(0),
                RespValue::Integer(1),
            ]))
        );
    }

    #[test]
    fn test_smismember_returns_wrongtype_on_non_set_key() {
        let ex = executor();
        ex.store.set_string("key", "val");
        assert!(matches!(
            ex.smismember("key", &els(&["a"])),
            RespValue::SimpleError(_)
        ));
    }

    // srandmember
    #[test]
    fn test_srandmember_returns_nil_on_missing_key_without_count() {
        let ex = executor();
        assert_eq!(ex.srandmember("missing", None), RespValue::BulkString(None));
    }

    #[test]
    fn test_srandmember_returns_empty_array_on_missing_key_with_count() {
        let ex = executor();
        assert_eq!(
            ex.srandmember("missing", Some(3)),
            RespValue::Array(Some(Vec::new()))
        );
    }

    #[test]
    fn test_srandmember_returns_wrongtype_on_non_set_key() {
        let ex = executor();
        ex.store.set_string("key", "val");
        assert!(matches!(
            ex.srandmember("key", None),
            RespValue::SimpleError(_)
        ));
    }

    #[test]
    fn test_srandmember_without_count_returns_single_bulk_string() {
        let ex = executor();
        ex.sadd("key", &els(&["a", "b", "c"]));

        let member = match ex.srandmember("key", None) {
            RespValue::BulkString(Some(s)) => s,
            other => panic!("expected bulk string, got {other:?}"),
        };
        assert!(["a", "b", "c"].contains(&member.as_str()));
        // srandmember does not remove the member
        assert_eq!(ex.scard("key"), RespValue::Integer(3));
    }

    #[test]
    fn test_srandmember_with_positive_count_returns_distinct_members() {
        let ex = executor();
        ex.sadd("key", &els(&["a", "b", "c"]));

        let members = members_of(ex.srandmember("key", Some(2)));
        assert_eq!(members.len(), 2);
        assert_eq!(ex.scard("key"), RespValue::Integer(3));
    }

    #[test]
    fn test_srandmember_with_positive_count_exceeding_size_returns_all_distinct_members() {
        let ex = executor();
        ex.sadd("key", &els(&["a", "b"]));

        assert_eq!(
            members_of(ex.srandmember("key", Some(10))),
            HashSet::from(["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn test_srandmember_with_negative_count_allows_duplicates() {
        let ex = executor();
        ex.sadd("key", &els(&["a"]));

        let resp = ex.srandmember("key", Some(-5));
        let members = match resp {
            RespValue::Array(Some(items)) => items
                .into_iter()
                .map(|item| match item {
                    RespValue::BulkString(Some(s)) => s,
                    other => panic!("expected bulk string, got {other:?}"),
                })
                .collect::<Vec<String>>(),
            other => panic!("expected array, got {other:?}"),
        };
        assert_eq!(members, vec!["a".to_string(); 5]);
        assert_eq!(ex.scard("key"), RespValue::Integer(1));
    }

    #[test]
    fn test_srandmember_with_zero_count_returns_empty_array() {
        let ex = executor();
        ex.sadd("key", &els(&["a"]));
        assert_eq!(
            ex.srandmember("key", Some(0)),
            RespValue::Array(Some(Vec::new()))
        );
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

    // smove
    #[test]
    fn test_smove_returns_zero_on_missing_source() {
        let ex = executor();
        ex.sadd("dst", &els(&["a"]));
        assert_eq!(ex.smove("missing", "dst", "a"), RespValue::Integer(0));
    }

    #[test]
    fn test_smove_returns_zero_when_member_not_in_source() {
        let ex = executor();
        ex.sadd("src", &els(&["a"]));
        assert_eq!(ex.smove("src", "dst", "b"), RespValue::Integer(0));
    }

    #[test]
    fn test_smove_returns_wrongtype_on_non_set_source() {
        let ex = executor();
        ex.store.set_string("src", "val");
        assert!(matches!(
            ex.smove("src", "dst", "a"),
            RespValue::SimpleError(_)
        ));
    }

    #[test]
    fn test_smove_returns_wrongtype_on_non_set_destination() {
        let ex = executor();
        ex.sadd("src", &els(&["a"]));
        ex.store.set_string("dst", "val");
        assert!(matches!(
            ex.smove("src", "dst", "a"),
            RespValue::SimpleError(_)
        ));
    }

    #[test]
    fn test_smove_moves_member_and_returns_one() {
        let ex = executor();
        ex.sadd("src", &els(&["a", "b"]));
        assert_eq!(ex.smove("src", "dst", "a"), RespValue::Integer(1));
        assert_eq!(
            members_of(ex.smembers("src")),
            HashSet::from(["b".to_string()])
        );
        assert_eq!(
            members_of(ex.smembers("dst")),
            HashSet::from(["a".to_string()])
        );
    }

    // spop
    #[test]
    fn test_spop_returns_nil_on_missing_key() {
        let ex = executor();
        assert_eq!(ex.spop("missing", None), RespValue::BulkString(None));
    }

    #[test]
    fn test_spop_returns_wrongtype_on_non_set_key() {
        let ex = executor();
        ex.store.set_string("key", "val");
        assert!(matches!(ex.spop("key", None), RespValue::SimpleError(_)));
    }

    #[test]
    fn test_spop_without_count_returns_single_bulk_string() {
        let ex = executor();
        ex.sadd("key", &els(&["a", "b", "c"]));

        let popped = match ex.spop("key", None) {
            RespValue::BulkString(Some(s)) => s,
            other => panic!("expected bulk string, got {other:?}"),
        };
        assert!(["a", "b", "c"].contains(&popped.as_str()));
        assert_eq!(ex.scard("key"), RespValue::Integer(2));
    }

    #[test]
    fn test_spop_with_count_returns_array() {
        let ex = executor();
        ex.sadd("key", &els(&["a", "b", "c"]));

        let popped = members_of(ex.spop("key", Some(2)));
        assert_eq!(popped.len(), 2);
        assert_eq!(ex.scard("key"), RespValue::Integer(1));
    }

    #[test]
    fn test_spop_with_count_exceeding_size_returns_all_members() {
        let ex = executor();
        ex.sadd("key", &els(&["a", "b"]));

        assert_eq!(
            members_of(ex.spop("key", Some(10))),
            HashSet::from(["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn test_spop_with_zero_count_returns_empty_array() {
        let ex = executor();
        ex.sadd("key", &els(&["a"]));
        assert_eq!(ex.spop("key", Some(0)), RespValue::Array(Some(Vec::new())));
        assert_eq!(ex.scard("key"), RespValue::Integer(1));
    }

    #[test]
    fn test_spop_with_count_on_missing_key_returns_empty_array() {
        let ex = executor();
        assert_eq!(
            ex.spop("missing", Some(1)),
            RespValue::Array(Some(Vec::new()))
        );
    }

    #[test]
    fn test_spop_deletes_key_when_set_becomes_empty() {
        let ex = executor();
        ex.sadd("key", &els(&["a"]));
        ex.spop("key", None);
        assert_eq!(ex.scard("key"), RespValue::Integer(0));
    }

    // srem
    #[test]
    fn test_srem_returns_zero_on_missing_key() {
        let ex = executor();
        assert_eq!(ex.srem("missing", &els(&["a"])), RespValue::Integer(0));
    }

    #[test]
    fn test_srem_returns_wrongtype_on_non_set_key() {
        let ex = executor();
        ex.store.set_string("key", "val");
        assert!(matches!(
            ex.srem("key", &els(&["a"])),
            RespValue::SimpleError(_)
        ));
    }

    #[test]
    fn test_srem_removes_present_members_and_returns_count() {
        let ex = executor();
        ex.sadd("key", &els(&["a", "b", "c"]));
        assert_eq!(ex.srem("key", &els(&["a", "b"])), RespValue::Integer(2));
        assert_eq!(
            members_of(ex.smembers("key")),
            HashSet::from(["c".to_string()])
        );
    }

    #[test]
    fn test_srem_only_counts_members_that_were_present() {
        let ex = executor();
        ex.sadd("key", &els(&["a"]));
        assert_eq!(ex.srem("key", &els(&["a", "b"])), RespValue::Integer(1));
    }
}
