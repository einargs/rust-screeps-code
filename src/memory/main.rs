// Design:
// We want a local cache, a way to periodically persist important information to
// the main memory, and a way to survey the game state to recreate important
// cache information.
// But that's for the future.
//
// For now I just want a simple serialize and deserialize tool.

use std::ops::{Deref, DerefMut};
use std::default::{Default};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet, BTreeMap};
use std::fmt::Debug;
use std::io::Write;

use minicbor::{Encode, Decode, Encoder};
use js_sys::{JsString, Object, Reflect};
use base64::{Engine as _, engine::general_purpose};

use screeps::raw_memory;
use screeps::local::ObjectId;
use screeps::objects::{Creep, StructureSpawn, Source};
use screeps::prelude::*;

use log::*;
use super::spawn::*;
use super::source::*;
use crate::creeps::{Role, RoleTag, CreepMemory};
use crate::storage::cbor;

#[derive(PartialEq, Debug, Encode, Decode)]
pub struct Memory {
  #[n(0)] pub creep_counter: u32,
  #[n(1)] pub creeps: BTreeMap<String, CreepMemory>,
  #[n(2)] #[cbor(with = "cbor::object_id_map")]
  pub spawns: HashMap<ObjectId<StructureSpawn>, SpawnMemory>,
  #[n(3)] #[cbor(with = "cbor::object_id_map")]
  pub sources: HashMap<ObjectId<Source>, SourceMemory>,
  /// Tracks the last known tick so we can tell if we need to deserialize or not.
  #[n(4)] pub last_time: u32
}

impl Memory {
  pub fn creep_name(&mut self, role: RoleTag) -> String {
    let c = self.creep_counter;
    self.creep_counter += 1;
    format!("{:?}-{}", role, c)
  }

  pub fn initialize_creep(&mut self, name: String, mem: CreepMemory) {
    if self.creeps.contains_key(&name) {
      warn!("Tried to initialze the memory of an already existing creep {}", name);
      return;
    }
    self.creeps.insert(name, mem);
  }

  pub fn creep(&self, creep: &Creep) -> Option<&CreepMemory> {
    self.creeps.get(&creep.name())
  }

  pub fn creep_mut(&mut self, name: &String) -> Option<&mut CreepMemory> {
    self.creeps.get_mut(name)
  }

  pub fn spawn_mut(
    &mut self, id: ObjectId<StructureSpawn>
  ) -> Entry<'_, ObjectId<StructureSpawn>, SpawnMemory> {
    self.spawns.entry(id)
  }
}

impl Default for Memory {
  fn default() -> Memory {
    Memory {
      creep_counter: 0,
      creeps: BTreeMap::default(),
      spawns: HashMap::default(),
      sources: HashMap::default(),
      last_time: 0, // may need to avoid zero if sim starts at 0? but 1 tick delay.
    }
  }
}
