pub use persist_memory_derive::*;

/// A trait for types where only parts of it need to be fully serialized
/// and deserialized every tick.
pub trait Persist {
  /// The type that will be fully persisted, serialized to RawMemory.
  type Persisted;

/*
  /// Generate the type that will be persisted this tick.
  fn to_persist(&self) -> Self::Persisted;

  /// Revive the important information from the persisted memory.
  ///
  /// For certain 
  fn revive(stored: Self::Persisted) -> Self;*/
}
