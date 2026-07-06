use crate::errors::StoreError;

use super::{Store, StoreValue};

impl Store {
    /// Returns the string value for `key`.
    /// Returns `Ok(None)` if the key does not exist, `Err(StoreError::WrongType)` if it holds a non-string value.
    pub fn get_string(&self, key: &str) -> Result<Option<String>, StoreError> {
        match self.get(key) {
            Some(StoreValue::Str(s)) => Ok(Some(s)),
            Some(_) => Err(StoreError::WrongType),
            None => Ok(None),
        }
    }

    /// Inserts or overwrites `key` with the string `value`.
    /// Always succeeds regardless of the previous value type.
    pub fn set_string(&self, key: &str, value: &str) {
        self.set(key, StoreValue::Str(value.to_string()));
    }

    /// Returns the string value for `key` and immediately deletes the key.
    /// Returns `Ok(None)` if the key does not exist, `Err(StoreError::WrongType)` if it holds a non-string value.
    pub fn get_del_string(&self, key: &str) -> Result<Option<String>, StoreError> {
        match self.get_del(key) {
            Some(StoreValue::Str(s)) => Ok(Some(s)),
            Some(_) => Err(StoreError::WrongType),
            None => Ok(None),
        }
    }
}
