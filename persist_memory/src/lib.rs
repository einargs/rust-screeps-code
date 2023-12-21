pub use persist_memory_derive::*;
use minicbor::{Encode, Decode};
// TODO: add a way for persisted to be known to have encode and decode from minicbor

/// A trait for types where only parts of it need to be fully serialized
/// and deserialized every tick.
pub trait Persist {
  /// The type that will be fully persisted, serialized to RawMemory.
  type Persisted;

  /// Generate the type that will be persisted this tick.
  fn to_persist(&self) -> Self::Persisted;

  /// Revive the important information from the persisted memory.
  ///
  /// For certain
  fn revive(stored: Self::Persisted) -> Self;
}

pub trait Stored<T: Persist> {
  fn revive(self) -> T;
}

impl<T> Stored<T> for T::Persisted where T: Persist {
  fn revive(self) -> T {
    T::revive(self)
  }
}

macro_rules! base_impl {
  ($t:ident) => {
    impl Persist for $t {
      type Persisted = $t;
      fn to_persist(&self) -> $t {
        self.clone()
      }
      fn revive(stored: $t) -> $t {
        stored
      }
    }
  }
}

base_impl!(u32);
