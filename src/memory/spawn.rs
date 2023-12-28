use minicbor::{Encode, Decode};
use std::default::Default;
use crate::creeps::RoleTag;

#[derive(PartialEq, Debug, Encode, Decode)]
pub struct SpawnMemory {
  /// Whether the room has been initialized.
  ///
  /// This includes setting up construction sites at each spawn
  /// for energy to be deposited at.
  #[n(0)] pub initialized: bool
}

impl Default for SpawnMemory {
  fn default() -> SpawnMemory {
    SpawnMemory {
      initialized: false
    }
  }
}
