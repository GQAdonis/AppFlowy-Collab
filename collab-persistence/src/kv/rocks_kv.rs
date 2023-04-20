use crate::kv::{KVEntry, KVStore};
use crate::PersistenceError;
use rocksdb::{
  DBAccess, DBCommon, DBIteratorWithThreadMode, Direction, IteratorMode, ReadOptions,
  SingleThreaded, Transaction, TransactionDB, DB,
};
use std::ops;
use std::ops::RangeBounds;
use std::path::Path;
use std::sync::Arc;

pub struct RocksKVStore {
  db: Arc<DB>,
}

impl RocksKVStore {
  pub fn open(path: impl AsRef<Path>) -> Result<Self, PersistenceError> {
    let db = Arc::new(DB::open_default(path)?);
    Ok(Self { db })
  }

  pub fn store(&self) -> KVStoreImpl {
    todo!()
  }
}

pub struct KVStoreImpl {}

pub type RocksDBVec = Vec<u8>;

impl KVStore for RocksKVStore {
  type Range = RocksDBRange;
  type Entry = RocksDBEntry;
  type Value = RocksDBVec;
  type Error = PersistenceError;

  fn get<K: AsRef<[u8]>>(&self, key: K) -> Result<Option<Self::Value>, Self::Error> {
    if let Some(value) = self.db.get(key)? {
      Ok(Some(value))
    } else {
      Ok(None)
    }
  }

  fn insert<K: AsRef<[u8]>, V: AsRef<[u8]>>(&self, key: K, value: V) -> Result<(), Self::Error> {
    self.db.put(key, value)?;
    Ok(())
  }

  fn remove(&self, key: &[u8]) -> Result<(), Self::Error> {
    self.db.delete(key)?;
    Ok(())
  }

  fn remove_range(&self, from: &[u8], to: &[u8]) -> Result<(), Self::Error> {
    let mut opt = ReadOptions::default();
    opt.set_iterate_lower_bound(from);
    opt.set_iterate_upper_bound(to);
    let mut i = self
      .db
      .iterator_opt(IteratorMode::From(from, Direction::Forward), opt);
    while let Some(res) = i.next() {
      let (key, _) = res?;
      self.db.delete(key)?;
    }
    Ok(())
  }

  fn range<K: AsRef<[u8]>, R: RangeBounds<K>>(&self, range: R) -> Result<Self::Range, Self::Error> {
    let mut opt = ReadOptions::default();
    let mut from: &[u8] = &[];
    match range.start_bound() {
      ops::Bound::Included(start) => {
        from = start.as_ref();
        opt.set_iterate_lower_bound(start.as_ref());
      },
      ops::Bound::Excluded(start) => {
        from = start.as_ref();
        opt.set_iterate_lower_bound(start.as_ref());
      },
      ops::Bound::Unbounded => {},
    };

    match range.end_bound() {
      ops::Bound::Included(end) => {
        opt.set_iterate_upper_bound(end.as_ref());
      },
      ops::Bound::Excluded(end) => {
        opt.set_iterate_upper_bound(end.as_ref());
      },
      ops::Bound::Unbounded => {},
    };
    let iterator_mode = IteratorMode::From(from, rocksdb::Direction::Forward);
    // let iter = self.db.iterator_opt(iterator_mode, opt);
    // Ok(RocksDBRange {
    //   iter: unsafe { std::mem::transmute(raw) },
    // })
    todo!()
  }

  fn next_back_entry(&self, key: &[u8]) -> Result<Option<Self::Entry>, Self::Error> {
    let opt = ReadOptions::default();
    let mut raw = self.db.raw_iterator_opt(opt);
    raw.seek_for_prev(key);
    if let Some((key, value)) = raw.item() {
      Ok(Some(RocksDBEntry::new(key.to_vec(), value.to_vec())))
    } else {
      Ok(None)
    }
  }
}

//
pub struct RocksDBRange {
  db: Arc<DB>,
  from: Vec<u8>,
  options: ReadOptions,
}

impl Iterator for RocksDBRange {
  type Item = RocksDBEntry;

  fn next(&mut self) -> Option<Self::Item> {
    todo!()
  }
}

pub struct RocksDBEntry {
  key: Vec<u8>,
  value: Vec<u8>,
}

impl RocksDBEntry {
  pub fn new(key: Vec<u8>, value: Vec<u8>) -> Self {
    Self { key, value }
  }
}

impl KVEntry for RocksDBEntry {
  fn key(&self) -> &[u8] {
    self.key.as_ref()
  }

  fn value(&self) -> &[u8] {
    self.value.as_ref()
  }
}

// #[cfg(test)]
// mod tests {
// }
