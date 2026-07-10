use std::collections::HashSet;

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
}
