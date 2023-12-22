use std::collections::HashMap;
use std::hash::Hash;
use core::default::Default;
use std::cell::RefCell;

use screeps::{ObjectId, Source, game};

#[derive(Debug)]
struct CacheEntry<T> {
  created: u32,
  value: T,
}

/// How often do we go through and delete unused entries
/// in the cache.
const CLEAN_UP_PERIOD: u32 = 998;

struct InternalCache<I, T, const D: u32> {
  has_been_cleaned: bool,
  map: HashMap<I, T>
}

impl<I, T, const D: u32> Default for InternalCache<I,T,D> {
  fn default() -> Self {
    InternalCache {
      has_been_cleaned: true,
      map: HashMap::default()
    }
  }
}

impl<I, T, const D: u32> InternalCache<I,T,D> {
  #[inline]
  fn clean_up(&mut self) {
    let time = game::time();
    if time % CLEAN_UP_PERIOD == 0 {
      if !self.has_been_cleaned {
        self.has_been_cleaned = true;
        self.0.retain(|k, v| {
          // we add 20 just to make sure we aren't accidentally removing
          // anything with tight timing, or a cache that has a lifetime of
          // zero.
          v.created + D + 20 >= time
        });
      }
    } else {
      self.has_been_cleaned = false;
    }
  }
}

macro_rules! mk_cache {
  ($($n:ident lifetime $dur:literal by $ity:ty => $t:ty),*) => {
    struct CacheIndex {
      $($n : SingleCache<$ity, $t, $dur>),*
    }

    impl CacheIndex {
      fn clean_up_if_needed(&mut self) {
        if game::time() % CLEAN_UP_PERIOD == 0 {
          $(
            $n.exec_clean_up();
          )*
        }
      }
    }

    $(
      /// This accesses the cached value if it is present and up to date.
      /// Otherwise, it is recalculated using calc. `calc`
      pub fn $n<K: Hash, T: Clone>(
        key: K,
        calc: impl FnOnce(T) -> T
      ) -> T {
        let time = game::time();
        let index = CACHE_INDEX.borrow();
        match index.$n.0.get(&key) {
          Some(entry) if entry.created + $dur >= time => {
            entry.value.clone()
          }
          _ => {
            // we have to drop index so that if calc uses
            // a different cache it all works out.
            drop(index);
            let val = calc();
            let entry = CacheEntry {
              created: time,
              value: val.clone()
            };
            let index = CACHE_INDEX.borrow_mut();
            index.$n.0.insert(key, entry);
            val
          }
        }
      }
    )*
  };
}

thread_local! {
  static CACHE_INDEX: RefCell<CacheIndex> = RefCell::new(CacheIndex::default());
}

mk_cache! {
  calc_harvest_spots lifetime 150 by ObjectId<Source> => u8
}
