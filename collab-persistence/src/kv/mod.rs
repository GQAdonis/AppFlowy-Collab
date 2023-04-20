pub mod rocks_kv;
pub mod sled_lv;

use crate::PersistenceError;

use std::ops::RangeBounds;
use std::sync::Arc;

pub trait KVStore: Send + Sync {
  type Range: Iterator<Item = Self::Entry>;
  type Entry: KVEntry;
  type Value: AsRef<[u8]>;
  type Error: Into<PersistenceError>;

  /// Get a value by key
  fn get<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Self::Value>, Self::Error>;

  fn insert<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, value: V) -> Result<(), Self::Error>;

  /// Remove a key, returning the last value if it exists
  fn remove(&self, key: &[u8]) -> Result<(), Self::Error>;

  /// Remove all keys in the range [from, to]
  fn remove_range(&self, from: &[u8], to: &[u8]) -> Result<(), Self::Error>;

  /// Return an iterator over the range of keys
  fn range<K: AsRef<[u8]>, R: RangeBounds<K>>(&self, range: R) -> Result<Self::Range, Self::Error>;

  /// Return the entry prior to the given key
  fn next_back_entry(&self, key: &[u8]) -> Result<Option<Self::Entry>, Self::Error>;
}

pub trait KVRange<'a> {
  type Range: Iterator<Item = Self::Entry>;
  type Entry: KVEntry;
  type Error: Into<PersistenceError>;

  fn kv_range(self) -> Result<Self::Range, Self::Error>;
}

/// A key-value entry
pub trait KVEntry {
  fn key(&self) -> &[u8];
  fn value(&self) -> &[u8];
}

impl<T> KVStore for Arc<T>
where
  T: KVStore,
{
  type Range = <T as KVStore>::Range;
  type Entry = <T as KVStore>::Entry;
  type Value = <T as KVStore>::Value;
  type Error = <T as KVStore>::Error;

  fn get<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Self::Value>, Self::Error> {
    (**self).get(key)
  }

  fn insert<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, value: V) -> Result<(), Self::Error> {
    (**self).insert(key, value)
  }

  fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
    (**self).remove(key)
  }

  fn remove_range(&self, from: &[u8], to: &[u8]) -> Result<(), Self::Error> {
    (**self).remove_range(from, to)
  }

  fn range<K: AsRef<[u8]>, R: RangeBounds<K>>(&self, range: R) -> Result<Self::Range, Self::Error> {
    self.as_ref().range(range)
  }

  fn next_back_entry(&self, key: &[u8]) -> Result<Option<Self::Entry>, Self::Error> {
    (**self).next_back_entry(key)
  }
}
