use std::collections::HashMap;
use std::hash::Hash;
use core::default::Default;
use std::cell::RefCell;

use screeps::game;

#[derive(Debug)]
struct CacheEntry<T> {
  created: u32,
  invalidate_at: Option<u32>,
  value: T,
}

/// How often do we go through and delete unused entries
/// in the cache.
const CLEAN_UP_PERIOD: u32 = 998;

#[derive(Debug)]
struct InternalCache<I, T, const D: u32> {
  /// Has this been cleaned up this tick already.
  has_been_cleaned: bool,
  map: HashMap<I, CacheEntry<T>>
}

impl<I, T, const D: u32> Default for InternalCache<I,T,D> {
  fn default() -> Self {
    InternalCache {
      has_been_cleaned: true,
      map: HashMap::default()
    }
  }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct ThreadLocalCache<K, T, const D: u32> {
  cell: RefCell<InternalCache<K, T, D>>
}

impl<K, T, const D: u32> Default for ThreadLocalCache<K,T,D> {
  fn default() -> Self {
    ThreadLocalCache {
      cell: RefCell::new(InternalCache::default())
    }
  }
}

impl<K: Eq + Hash + Clone, T: Clone, const D: u32> ThreadLocalCache<K,T,D> {
  /// Run the clean up if needed. Makes sure it only runs the clean up
  /// once.
  #[inline]
  fn clean_up(&self, time: u32) {
    let mut cache = self.cell.borrow_mut();
    if time % CLEAN_UP_PERIOD == 0 {
      if !cache.has_been_cleaned {
        cache.has_been_cleaned = true;
        cache.map.retain(|k, v| {
          // we add 20 just to make sure we aren't accidentally removing
          // anything with tight timing, or a cache that has a lifetime of
          // zero.
          v.created + D + 20 >= time
        });
      }
    } else {
      cache.has_been_cleaned = false;
    }
  }

  /// This accesses the cached value if it is present and up to date.
  /// Otherwise, it is recalculated using calc. `calc`
  pub fn caches(
    &self,
    key: &K,
    calc: impl FnOnce(Option<T>) -> T
  ) -> T {
    let time = game::time();
    self.clean_up(time);
    let cache = self.cell.borrow();
    match cache.map.get(key) {
      Some(entry) if entry.created + D >= time
          && entry.invalidate_at.map_or(true, |t| t > time) => {
        entry.value.clone()
      }
      _ => {
        // we have to drop index so that if calc uses
        // a different cache it all works out.
        drop(cache);
        let mut cache = self.cell.borrow_mut();
        let old_value = cache.map.remove(key);
        let val = calc(old_value.map(|v| v.value));
        let entry = CacheEntry {
          created: time,
          invalidate_at: None,
          value: val.clone()
        };
        cache.map.insert(key.clone(), entry);
        val
      }
    }
  }

  pub fn invalidate_now(&self, key: &K) {
    let mut cache = self.cell.borrow_mut();
    cache.map.remove(key);
  }

  /// Tell the cache to update next tick.
  pub fn invalidate_next_tick(&self, key: &K) {
    let mut cache = self.cell.borrow_mut();
    if let Some(entry) = cache.map.get_mut(key) {
      entry.invalidate_at = Some(game::time() + 1);
    }
  }
}

#[macro_export]
macro_rules! mk_cache {
  ($n:ident lifetime $dur:literal by $ity:ty => $t:ty) => {
    mod $n {
      use std::hash::Hash;
      use crate::storage::cache::ThreadLocalCache;
      use super::*;

      thread_local! {
        static CACHE: ThreadLocalCache<$ity, $t, $dur> = ThreadLocalCache::default();
      }
      pub fn caches(
        key: & $ity,
        calc: impl FnOnce(Option<$t>) -> $t
      ) -> $t {
        CACHE.with(|local_cache| {
          local_cache.caches(key, calc)
        })
      }
      pub fn invalidate_now(key: & $ity) {
        CACHE.with(|local_cache| {
          local_cache.invalidate_now(key)
        })
      }
      pub fn invalidate_next_tick(key: & $ity) {
        CACHE.with(|local_cache| {
          local_cache.invalidate_next_tick(key)
        })
      }
    }
  };
}
