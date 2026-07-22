use std::collections::HashSet;

use rand::RngExt;
use rand::seq::IteratorRandom;

use crate::errors::StoreError;

use super::{Store, StoreValue};

impl Store {
    /// Adds `members` to the set at `key`, creating the key if it does not exist. Returns the
    /// number of members that were newly added (i.e. not already present). Returns
    /// `Err(StoreError::WrongType)` if the key holds a non-set value.
    pub fn sadd(
        &self,
        key: &str,
        members: impl IntoIterator<Item = String>,
    ) -> Result<usize, StoreError> {
        let mut guard = self.inner.write().unwrap();
        match guard.get_mut(key) {
            None => {
                guard.expiry_index.remove(key);
                let set: HashSet<String> = members.into_iter().collect();
                let added = set.len();
                guard.data.insert(key.to_string(), StoreValue::Set(set));
                Ok(added)
            }
            Some(StoreValue::Set(s)) => {
                Ok(members.into_iter().filter(|m| s.insert(m.clone())).count())
            }
            Some(_) => Err(StoreError::WrongType),
        }
    }

    /// Returns all members of the set at `key`. Returns an empty vector if the key does not
    /// exist. Returns `Err(StoreError::WrongType)` if the key holds a non-set value.
    pub fn smembers(&self, key: &str) -> Result<Vec<String>, StoreError> {
        let guard = self.inner.read().unwrap();
        match guard.get(key) {
            None => Ok(Vec::with_capacity(0)),
            Some(StoreValue::Set(s)) => Ok(s.iter().cloned().collect::<Vec<String>>()),
            Some(_) => Err(StoreError::WrongType),
        }
    }

    /// Returns if member is a member of the set stored at key.
    pub fn sismember(&self, key: &str, member: String) -> Result<bool, StoreError> {
        Ok(self.smembers(key)?.contains(&member))
    }

    /// For every member, 1 is returned if the value is a member of the set,
    /// or 0 if the element is not a member of the set or if key does not exist.
    pub fn smismember(&self, key: &str, members: Vec<String>) -> Result<Vec<bool>, StoreError> {
        let guard = self.inner.read().unwrap();
        match guard.get(key) {
            None => Ok(vec![false; members.len()]),
            Some(StoreValue::Set(s)) => Ok(members
                .iter()
                .map(|member| s.contains(member))
                .collect::<Vec<bool>>()),
            Some(_) => Err(StoreError::WrongType),
        }
    }

    /// Returns up to `count` random members from the set at `key` (defaulting to a single member
    /// if `count` is `None`). Without repetition, returns up to `count` distinct members — fewer
    /// if the set has fewer members than `count`. With repetition, returns exactly `count`
    /// members, allowing duplicates. Returns an empty vector if `key` does not exist. Returns
    /// `Err(StoreError::WrongType)` if the key holds a non-set value.
    pub fn srandmember(
        &self,
        key: &str,
        count: Option<usize>,
        allow_repetition: bool,
    ) -> Result<Vec<String>, StoreError> {
        let guard = self.inner.read().unwrap();
        match guard.get(key) {
            None => Ok(Vec::new()),
            Some(StoreValue::Set(s)) => {
                let amount = count.unwrap_or(1);
                let mut rng = rand::rng();

                if allow_repetition {
                    if s.is_empty() {
                        return Ok(Vec::new());
                    }
                    let members: Vec<&String> = s.iter().collect();
                    Ok((0..amount)
                        .map(|_| members[rng.random_range(0..members.len())].clone())
                        .collect())
                } else {
                    Ok(s.iter().cloned().sample(&mut rng, amount))
                }
            }
            Some(_) => Err(StoreError::WrongType),
        }
    }

    /// Return the set cardinality (number of elements) of the set at `key`. Returns 0 if the key
    /// does not exist. Returns `Err(StoreError::WrongType)` if the key holds a non-set value.
    pub fn scard(&self, key: &str) -> Result<usize, StoreError> {
        let guard = self.inner.read().unwrap();
        match guard.get(key) {
            None => Ok(0),
            Some(StoreValue::Set(s)) => Ok(s.len()),
            Some(_) => Err(StoreError::WrongType),
        }
    }

    /// Moves `member` from the set at `source` to the set at `destination`, creating
    /// `destination` if it does not exist. Returns `Ok(true)` if the member was moved. Returns
    /// `Ok(false)` without modifying either key if `source` does not exist or does not contain
    /// `member`. Returns `Err(StoreError::WrongType)` if `source` or `destination` holds a
    /// non-set value.
    pub fn smove(
        &self,
        source: &str,
        destination: &str,
        member: String,
    ) -> Result<bool, StoreError> {
        let mut guard = self.inner.write().unwrap();

        // Validate types and membership
        let has_member = match guard.get(source) {
            None => return Ok(false),
            Some(StoreValue::Set(s)) => s.contains(&member),
            Some(_) => return Err(StoreError::WrongType),
        };

        match guard.get(destination) {
            None | Some(StoreValue::Set(_)) => {}
            Some(_) => return Err(StoreError::WrongType),
        };

        if !has_member {
            return Ok(false);
        }

        // Remove member from source
        match guard.get_mut(source) {
            Some(StoreValue::Set(s)) => {
                s.remove(&member);
                if s.is_empty() {
                    guard.data.remove(source);
                    guard.expiry_index.remove(source);
                }
            }
            _ => unreachable!("source type was already checked above"),
        }

        // Insert member to destination
        match guard.get_mut(destination) {
            None => {
                guard.expiry_index.remove(destination);
                guard.data.insert(
                    destination.to_string(),
                    StoreValue::Set(HashSet::from([member])),
                );
            }
            Some(StoreValue::Set(s)) => {
                s.insert(member);
            }
            Some(_) => unreachable!("source type was already checked above"),
        }

        Ok(true)
    }

    /// Removes and returns up to `count` random, distinct members from the set at `key`. Removes
    /// the key when the set becomes empty. Returns `Ok(None)` if the key does not exist. Returns
    /// `Err(StoreError::WrongType)` if the key holds a non-set value.
    pub fn spop(&self, key: &str, count: Option<u64>) -> Result<Option<Vec<String>>, StoreError> {
        let mut guard = self.inner.write().unwrap();
        let set = match guard.get_mut(key) {
            Some(StoreValue::Set(s)) => s,
            Some(_) => return Err(StoreError::WrongType),
            None => return Ok(None),
        };

        let amount = count.unwrap_or(1) as usize;
        let popped: Vec<String> = set.iter().cloned().sample(&mut rand::rng(), amount);
        for member in &popped {
            set.remove(member);
        }

        if set.is_empty() {
            guard.data.remove(key);
            guard.expiry_index.remove(key);
        }

        Ok(Some(popped))
    }

    /// Removes `members` from the set at `key`. Returns the number of members that were removed
    /// (i.e. that were present). Returns `Ok(0)` if the key does not exist. Returns
    /// `Err(StoreError::WrongType)` if the key holds a non-set value.
    pub fn srem(
        &self,
        key: &str,
        members: impl IntoIterator<Item = String>,
    ) -> Result<usize, StoreError> {
        let mut guard = self.inner.write().unwrap();
        let set = match guard.get_mut(key) {
            Some(StoreValue::Set(s)) => s,
            Some(_) => return Err(StoreError::WrongType),
            None => return Ok(0),
        };

        let mut count = 0usize;
        for member in members {
            if set.contains(&member) {
                count += 1;
                set.remove(&member);
            }
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // sadd
    #[test]
    fn test_sadd_creates_set_on_missing_key() {
        let store = Store::new();
        assert_eq!(store.sadd("key", vec!["a".to_string()]), Ok(1));
        assert_eq!(store.smembers("key"), Ok(vec!["a".to_string()]));
    }

    #[test]
    fn test_sadd_dedupes_members_on_missing_key() {
        let store = Store::new();
        assert_eq!(
            store.sadd(
                "key",
                vec!["a".to_string(), "a".to_string(), "b".to_string()]
            ),
            Ok(2)
        );
        assert_eq!(
            store
                .smembers("key")
                .unwrap()
                .into_iter()
                .collect::<HashSet<String>>(),
            HashSet::from(["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn test_sadd_returns_count_of_newly_added_members() {
        let store = Store::new();
        store.sadd("key", vec!["a".to_string()]).unwrap();
        assert_eq!(
            store.sadd("key", vec!["a".to_string(), "b".to_string()]),
            Ok(1)
        );
        assert_eq!(
            store
                .smembers("key")
                .unwrap()
                .into_iter()
                .collect::<HashSet<String>>(),
            HashSet::from(["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn test_sadd_returns_zero_when_all_members_already_present() {
        let store = Store::new();
        store.sadd("key", vec!["a".to_string()]).unwrap();
        assert_eq!(store.sadd("key", vec!["a".to_string()]), Ok(0));
    }

    #[test]
    fn test_sadd_returns_wrongtype_on_non_set_key() {
        let store = Store::new();
        store.set("key", StoreValue::Str("val".to_string()));
        assert_eq!(
            store.sadd("key", vec!["a".to_string()]),
            Err(StoreError::WrongType)
        );
    }

    #[test]
    fn test_sadd_creates_fresh_set_on_expired_key() {
        use std::time::{Duration, Instant};
        let store = Store::new();
        store.sadd("key", vec!["a".to_string()]).unwrap();
        store.set_ttl("key", Instant::now() - Duration::from_secs(1));
        assert_eq!(store.sadd("key", vec!["b".to_string()]), Ok(1));
        assert_eq!(store.get_ttl("key"), None);
        assert_eq!(store.smembers("key"), Ok(vec!["b".to_string()]));
    }

    // smembers
    #[test]
    fn test_smembers_returns_empty_on_missing_key() {
        let store = Store::new();
        assert_eq!(store.smembers("missing"), Ok(Vec::new()));
    }

    #[test]
    fn test_smembers_returns_wrongtype_on_non_set_key() {
        let store = Store::new();
        store.set("key", StoreValue::Str("val".to_string()));
        assert_eq!(store.smembers("key"), Err(StoreError::WrongType));
    }

    #[test]
    fn test_smembers_returns_all_members() {
        let store = Store::new();
        store
            .sadd(
                "key",
                vec!["a".to_string(), "b".to_string(), "c".to_string()],
            )
            .unwrap();
        assert_eq!(
            store
                .smembers("key")
                .unwrap()
                .into_iter()
                .collect::<HashSet<String>>(),
            HashSet::from(["a".to_string(), "b".to_string(), "c".to_string()])
        );
    }

    #[test]
    fn test_smembers_returns_empty_on_expired_key() {
        use std::time::{Duration, Instant};
        let store = Store::new();
        store.sadd("key", vec!["a".to_string()]).unwrap();
        store.set_ttl("key", Instant::now() - Duration::from_secs(1));
        assert_eq!(store.smembers("key"), Ok(Vec::new()));
    }

    // sismember
    #[test]
    fn test_sismember_returns_false_on_missing_key() {
        let store = Store::new();
        assert_eq!(store.sismember("missing", "a".to_string()), Ok(false));
    }

    #[test]
    fn test_sismember_returns_wrongtype_on_non_set_key() {
        let store = Store::new();
        store.set("key", StoreValue::Str("val".to_string()));
        assert_eq!(
            store.sismember("key", "a".to_string()),
            Err(StoreError::WrongType)
        );
    }

    #[test]
    fn test_sismember_returns_true_when_member_present() {
        let store = Store::new();
        store.sadd("key", vec!["a".to_string()]).unwrap();
        assert_eq!(store.sismember("key", "a".to_string()), Ok(true));
    }

    #[test]
    fn test_sismember_returns_false_when_member_absent() {
        let store = Store::new();
        store.sadd("key", vec!["a".to_string()]).unwrap();
        assert_eq!(store.sismember("key", "b".to_string()), Ok(false));
    }

    #[test]
    fn test_sismember_returns_false_on_expired_key() {
        use std::time::{Duration, Instant};
        let store = Store::new();
        store.sadd("key", vec!["a".to_string()]).unwrap();
        store.set_ttl("key", Instant::now() - Duration::from_secs(1));
        assert_eq!(store.sismember("key", "a".to_string()), Ok(false));
    }

    // smismember
    #[test]
    fn test_smismember_returns_all_false_on_missing_key() {
        let store = Store::new();
        assert_eq!(
            store.smismember("missing", vec!["a".to_string(), "b".to_string()]),
            Ok(vec![false, false])
        );
    }

    #[test]
    fn test_smismember_returns_wrongtype_on_non_set_key() {
        let store = Store::new();
        store.set("key", StoreValue::Str("val".to_string()));
        assert_eq!(
            store.smismember("key", vec!["a".to_string()]),
            Err(StoreError::WrongType)
        );
    }

    #[test]
    fn test_smismember_returns_flag_per_member() {
        let store = Store::new();
        store
            .sadd("key", vec!["a".to_string(), "b".to_string()])
            .unwrap();
        assert_eq!(
            store.smismember(
                "key",
                vec!["a".to_string(), "c".to_string(), "b".to_string()]
            ),
            Ok(vec![true, false, true])
        );
    }

    #[test]
    fn test_smismember_returns_all_false_on_expired_key() {
        use std::time::{Duration, Instant};
        let store = Store::new();
        store.sadd("key", vec!["a".to_string()]).unwrap();
        store.set_ttl("key", Instant::now() - Duration::from_secs(1));
        assert_eq!(
            store.smismember("key", vec!["a".to_string()]),
            Ok(vec![false])
        );
    }

    // srandmember
    #[test]
    fn test_srandmember_returns_empty_on_missing_key() {
        let store = Store::new();
        assert_eq!(store.srandmember("missing", None, false), Ok(Vec::new()));
        assert_eq!(store.srandmember("missing", Some(3), false), Ok(Vec::new()));
        assert_eq!(store.srandmember("missing", Some(3), true), Ok(Vec::new()));
    }

    #[test]
    fn test_srandmember_returns_wrongtype_on_non_set_key() {
        let store = Store::new();
        store.set("key", StoreValue::Str("val".to_string()));
        assert_eq!(
            store.srandmember("key", None, false),
            Err(StoreError::WrongType)
        );
    }

    #[test]
    fn test_srandmember_without_count_returns_single_member() {
        let store = Store::new();
        store
            .sadd(
                "key",
                vec!["a".to_string(), "b".to_string(), "c".to_string()],
            )
            .unwrap();

        let result = store.srandmember("key", None, false).unwrap();
        assert_eq!(result.len(), 1);
        assert!(["a", "b", "c"].contains(&result[0].as_str()));
        // srandmember does not remove the member
        assert_eq!(store.scard("key"), Ok(3));
    }

    #[test]
    fn test_srandmember_with_count_returns_distinct_members() {
        let store = Store::new();
        store
            .sadd(
                "key",
                vec!["a".to_string(), "b".to_string(), "c".to_string()],
            )
            .unwrap();

        let result = store.srandmember("key", Some(2), false).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result.iter().cloned().collect::<HashSet<String>>().len(), 2);
        assert_eq!(store.scard("key"), Ok(3));
    }

    #[test]
    fn test_srandmember_with_count_exceeding_size_returns_all_distinct_members() {
        let store = Store::new();
        store
            .sadd("key", vec!["a".to_string(), "b".to_string()])
            .unwrap();

        let result = store.srandmember("key", Some(10), false).unwrap();
        assert_eq!(
            result.into_iter().collect::<HashSet<String>>(),
            HashSet::from(["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn test_srandmember_with_zero_count_returns_empty() {
        let store = Store::new();
        store.sadd("key", vec!["a".to_string()]).unwrap();
        assert_eq!(store.srandmember("key", Some(0), false), Ok(Vec::new()));
    }

    #[test]
    fn test_srandmember_with_repetition_and_count_exceeding_size_allows_duplicates() {
        let store = Store::new();
        store.sadd("key", vec!["a".to_string()]).unwrap();

        let result = store.srandmember("key", Some(5), true).unwrap();
        assert_eq!(result, vec!["a".to_string(); 5]);
    }

    #[test]
    fn test_srandmember_with_repetition_returns_only_existing_members() {
        let store = Store::new();
        store
            .sadd("key", vec!["a".to_string(), "b".to_string()])
            .unwrap();

        let result = store.srandmember("key", Some(10), true).unwrap();
        assert_eq!(result.len(), 10);
        assert!(result.iter().all(|m| m == "a" || m == "b"));
    }

    // scard
    #[test]
    fn test_scard_returns_zero_on_missing_key() {
        let store = Store::new();
        assert_eq!(store.scard("missing"), Ok(0));
    }

    #[test]
    fn test_scard_returns_wrongtype_on_non_set_key() {
        let store = Store::new();
        store.set("key", StoreValue::Str("val".to_string()));
        assert_eq!(store.scard("key"), Err(StoreError::WrongType));
    }

    #[test]
    fn test_scard_returns_member_count() {
        let store = Store::new();
        store
            .sadd(
                "key",
                vec!["a".to_string(), "b".to_string(), "c".to_string()],
            )
            .unwrap();
        assert_eq!(store.scard("key"), Ok(3));
    }

    #[test]
    fn test_scard_does_not_count_duplicates() {
        let store = Store::new();
        store
            .sadd("key", vec!["a".to_string(), "a".to_string()])
            .unwrap();
        assert_eq!(store.scard("key"), Ok(1));
    }

    #[test]
    fn test_scard_returns_zero_on_expired_key() {
        use std::time::{Duration, Instant};
        let store = Store::new();
        store.sadd("key", vec!["a".to_string()]).unwrap();
        store.set_ttl("key", Instant::now() - Duration::from_secs(1));
        assert_eq!(store.scard("key"), Ok(0));
    }

    // smove
    #[test]
    fn test_smove_returns_false_on_missing_source() {
        let store = Store::new();
        store.sadd("dst", vec!["a".to_string()]).unwrap();
        assert_eq!(store.smove("missing", "dst", "a".to_string()), Ok(false));
        assert_eq!(store.smembers("dst"), Ok(vec!["a".to_string()]));
    }

    #[test]
    fn test_smove_returns_false_when_member_not_in_source() {
        let store = Store::new();
        store.sadd("src", vec!["a".to_string()]).unwrap();
        assert_eq!(store.smove("src", "dst", "b".to_string()), Ok(false));
        assert_eq!(store.smembers("src"), Ok(vec!["a".to_string()]));
        assert!(!store.exists("dst"));
    }

    #[test]
    fn test_smove_returns_wrongtype_on_non_set_source() {
        let store = Store::new();
        store.set("src", StoreValue::Str("val".to_string()));
        assert_eq!(
            store.smove("src", "dst", "a".to_string()),
            Err(StoreError::WrongType)
        );
    }

    #[test]
    fn test_smove_returns_wrongtype_on_non_set_destination() {
        let store = Store::new();
        store.sadd("src", vec!["a".to_string()]).unwrap();
        store.set("dst", StoreValue::Str("val".to_string()));
        assert_eq!(
            store.smove("src", "dst", "a".to_string()),
            Err(StoreError::WrongType)
        );
        assert_eq!(store.smembers("src"), Ok(vec!["a".to_string()]));
    }

    #[test]
    fn test_smove_creates_destination_and_moves_member() {
        let store = Store::new();
        store
            .sadd("src", vec!["a".to_string(), "b".to_string()])
            .unwrap();
        assert_eq!(store.smove("src", "dst", "a".to_string()), Ok(true));
        assert_eq!(store.smembers("src"), Ok(vec!["b".to_string()]));
        assert_eq!(store.smembers("dst"), Ok(vec!["a".to_string()]));
    }

    #[test]
    fn test_smove_moves_member_to_existing_destination_set() {
        let store = Store::new();
        store.sadd("src", vec!["a".to_string()]).unwrap();
        store.sadd("dst", vec!["b".to_string()]).unwrap();
        assert_eq!(store.smove("src", "dst", "a".to_string()), Ok(true));
        assert!(!store.exists("src"));
        assert_eq!(
            store
                .smembers("dst")
                .unwrap()
                .into_iter()
                .collect::<HashSet<String>>(),
            HashSet::from(["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn test_smove_does_not_duplicate_member_already_in_destination() {
        let store = Store::new();
        store.sadd("src", vec!["a".to_string()]).unwrap();
        store.sadd("dst", vec!["a".to_string()]).unwrap();
        assert_eq!(store.smove("src", "dst", "a".to_string()), Ok(true));
        assert!(!store.exists("src"));
        assert_eq!(store.smembers("dst"), Ok(vec!["a".to_string()]));
    }

    #[test]
    fn test_smove_deletes_source_key_when_set_becomes_empty() {
        let store = Store::new();
        store.sadd("src", vec!["a".to_string()]).unwrap();
        assert_eq!(store.smove("src", "dst", "a".to_string()), Ok(true));
        assert!(!store.exists("src"));
    }

    #[test]
    fn test_smove_does_not_delete_source_key_when_set_is_non_empty() {
        let store = Store::new();
        store
            .sadd("src", vec!["a".to_string(), "b".to_string()])
            .unwrap();
        store.smove("src", "dst", "a".to_string()).unwrap();
        assert!(store.exists("src"));
        assert_eq!(store.smembers("src"), Ok(vec!["b".to_string()]));
    }

    #[test]
    fn test_smove_within_same_key_keeps_member_and_returns_true() {
        let store = Store::new();
        store.sadd("key", vec!["a".to_string()]).unwrap();
        assert_eq!(store.smove("key", "key", "a".to_string()), Ok(true));
        assert_eq!(store.smembers("key"), Ok(vec!["a".to_string()]));
    }

    #[test]
    fn test_smove_returns_false_on_expired_source() {
        use std::time::{Duration, Instant};
        let store = Store::new();
        store.sadd("src", vec!["a".to_string()]).unwrap();
        store.set_ttl("src", Instant::now() - Duration::from_secs(1));
        assert_eq!(store.smove("src", "dst", "a".to_string()), Ok(false));
        assert!(!store.exists("dst"));
    }

    // spop
    #[test]
    fn test_spop_returns_none_on_missing_key() {
        let store = Store::new();
        assert_eq!(store.spop("missing", None), Ok(None));
    }

    #[test]
    fn test_spop_returns_wrongtype_on_non_set_key() {
        let store = Store::new();
        store.set("key", StoreValue::Str("val".to_string()));
        assert_eq!(store.spop("key", None), Err(StoreError::WrongType));
    }

    #[test]
    fn test_spop_default_count_removes_one_member() {
        let store = Store::new();
        store
            .sadd(
                "key",
                vec!["a".to_string(), "b".to_string(), "c".to_string()],
            )
            .unwrap();

        let popped = store.spop("key", None).unwrap().unwrap();
        assert_eq!(popped.len(), 1);
        assert!(["a", "b", "c"].contains(&popped[0].as_str()));
        assert_eq!(store.scard("key"), Ok(2));
        assert!(!store.smembers("key").unwrap().contains(&popped[0]));
    }

    #[test]
    fn test_spop_with_count_removes_multiple_distinct_members() {
        let store = Store::new();
        store
            .sadd(
                "key",
                vec!["a".to_string(), "b".to_string(), "c".to_string()],
            )
            .unwrap();

        let popped = store.spop("key", Some(2)).unwrap().unwrap();
        assert_eq!(popped.len(), 2);
        assert_eq!(popped.iter().cloned().collect::<HashSet<String>>().len(), 2);
        assert_eq!(store.scard("key"), Ok(1));
    }

    #[test]
    fn test_spop_with_count_exceeding_size_removes_all_members() {
        let store = Store::new();
        store
            .sadd("key", vec!["a".to_string(), "b".to_string()])
            .unwrap();

        let popped = store.spop("key", Some(10)).unwrap().unwrap();
        assert_eq!(
            popped.into_iter().collect::<HashSet<String>>(),
            HashSet::from(["a".to_string(), "b".to_string()])
        );
    }

    #[test]
    fn test_spop_with_zero_count_returns_empty_and_leaves_set_untouched() {
        let store = Store::new();
        store.sadd("key", vec!["a".to_string()]).unwrap();
        assert_eq!(store.spop("key", Some(0)), Ok(Some(Vec::new())));
        assert_eq!(store.scard("key"), Ok(1));
    }

    #[test]
    fn test_spop_deletes_key_when_set_becomes_empty() {
        let store = Store::new();
        store.sadd("key", vec!["a".to_string()]).unwrap();
        store.spop("key", None).unwrap();
        assert!(!store.exists("key"));
    }

    #[test]
    fn test_spop_returns_none_on_expired_key() {
        use std::time::{Duration, Instant};
        let store = Store::new();
        store.sadd("key", vec!["a".to_string()]).unwrap();
        store.set_ttl("key", Instant::now() - Duration::from_secs(1));
        assert_eq!(store.spop("key", None), Ok(None));
    }

    // srem
    #[test]
    fn test_srem_returns_zero_on_missing_key() {
        let store = Store::new();
        assert_eq!(store.srem("missing", vec!["a".to_string()]), Ok(0));
    }

    #[test]
    fn test_srem_returns_wrongtype_on_non_set_key() {
        let store = Store::new();
        store.set("key", StoreValue::Str("val".to_string()));
        assert_eq!(
            store.srem("key", vec!["a".to_string()]),
            Err(StoreError::WrongType)
        );
    }

    #[test]
    fn test_srem_removes_present_members() {
        let store = Store::new();
        store
            .sadd(
                "key",
                vec!["a".to_string(), "b".to_string(), "c".to_string()],
            )
            .unwrap();

        assert_eq!(
            store.srem("key", vec!["a".to_string(), "b".to_string()]),
            Ok(2)
        );
        assert_eq!(store.smembers("key"), Ok(vec!["c".to_string()]));
    }

    #[test]
    fn test_srem_only_counts_members_that_were_present() {
        let store = Store::new();
        store.sadd("key", vec!["a".to_string()]).unwrap();
        assert_eq!(
            store.srem("key", vec!["a".to_string(), "b".to_string()]),
            Ok(1)
        );
    }

    #[test]
    fn test_srem_does_not_delete_key_when_set_is_non_empty() {
        let store = Store::new();
        store
            .sadd("key", vec!["a".to_string(), "b".to_string()])
            .unwrap();
        store.srem("key", vec!["a".to_string()]).unwrap();
        assert!(store.exists("key"));
    }

    #[test]
    fn test_srem_returns_zero_on_expired_key() {
        use std::time::{Duration, Instant};
        let store = Store::new();
        store.sadd("key", vec!["a".to_string()]).unwrap();
        store.set_ttl("key", Instant::now() - Duration::from_secs(1));
        assert_eq!(store.srem("key", vec!["a".to_string()]), Ok(0));
    }
}
