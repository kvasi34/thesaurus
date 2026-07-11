use std::collections::HashSet;

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
