// Design:
// We want a local cache, a way to periodically persist important information to
// the main memory, and a way to survey the game state to recreate important
// cache information.
// But that's for the future.
//
// For now I just want a simple serialize and deserialize tool.

use std::cell::{RefCell};
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
use screeps::objects::{Creep, StructureSpawn};
use screeps::prelude::*;

use log::*;
use crate::role::{Role, RoleTag, Harvester, Builder, CreepMemory};
use crate::storage::cbor;

#[derive(Debug, Encode, Decode)]
pub struct SpawnMemory {
  #[n(0)] next_role: RoleTag,
}

impl Default for SpawnMemory {
  fn default() -> SpawnMemory {
    SpawnMemory {
      next_role: RoleTag::Builder
    }
  }
}

impl SpawnMemory {
  pub fn get_role(&mut self) -> RoleTag {
    let tag = self.next_role;
    self.next_role = tag.next();
    tag
  }
}

#[derive(Debug, Encode, Decode)]
pub struct Memory {
  #[n(0)] pub creep_counter: u32,
  #[n(1)] pub creeps: BTreeMap<String, CreepMemory>,
  #[n(2)] #[cbor(with = "cbor::object_id_map")]
  pub spawns: HashMap<ObjectId<StructureSpawn>, SpawnMemory>,
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
    }
  }
}

#[derive(Debug)]
enum MemError {
  Base64(base64::DecodeError),
  DecodeCbor(minicbor::decode::Error),
  EncodeCbor,
}

impl From<base64::DecodeError> for MemError {
  fn from(err: base64::DecodeError) -> MemError {
    MemError::Base64(err)
  }
}

impl From<minicbor::decode::Error> for MemError {
  fn from(err: minicbor::decode::Error) -> MemError {
    MemError::DecodeCbor(err)
  }
}

impl<T: Debug> From<minicbor::encode::Error<T>> for MemError {
  fn from(err: minicbor::encode::Error<T>) -> MemError {
    warn!("Encoding error with cbor: {:?}", err);
    MemError::EncodeCbor
  }
}

fn from_buffer(buffer: &[u8]) -> Result<Memory, MemError> {
  Ok(minicbor::decode(buffer)?)
}

fn to_buffer(memory: Memory, buffer: &mut Vec<u8>) -> Result<(), MemError> {
  let mut encoder = Encoder::new(buffer);
  encoder.encode(memory)?;
  Ok(())
}

// For the memory we're going to keep around a vector in the local heap
// that we'll clear and deserialize the memory into every time we need it.
fn to_mem_string(data: &[u8]) -> String {
  general_purpose::STANDARD_NO_PAD.encode(data)
}

fn from_mem_string(string: String, target: &mut Vec<u8>) -> Result<(), MemError> {
  // I may need to write a custom decoder to avoid a redudant copy?
  // but that's premature right now.

  Ok(general_purpose::STANDARD_NO_PAD.decode_vec(string, target)?)
}

fn load_mem(mem_str: String, buffer: &mut Vec<u8>) -> Result<Memory, MemError> {
  from_mem_string(mem_str, buffer)?;
  from_buffer(buffer.deref())
}

thread_local! {
  static MEMORY_DECODE_BUFFER: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}

pub fn with_memory(fun: impl FnOnce(&mut Memory) -> ()) -> () {
  let index: u8 = 1;
  //raw_memory::set_active_segments(&[0]);

  MEMORY_DECODE_BUFFER.with(|buf_refcell| {
    let active_segments = raw_memory::segments();
    //debug!("active segments {:?}", active_segments);
    if let Some(mem_str) = active_segments.get(index) {
      let mut buffer = buf_refcell.borrow_mut();
      buffer.clear();
      let mut memory = match load_mem(mem_str, buffer.deref_mut()) {
        Err(_) => Memory::default(),
        Ok(mem) => mem
      };
      fun(&mut memory);
      to_buffer(memory, buffer.deref_mut()).expect("encoding problem");
      let new_mem_str = to_mem_string(buffer.deref());
      // TODO: at end clear the normal memory. Also maybe switch to using raw memory segments?
      active_segments.set(index, new_mem_str);
    } else {
      active_segments.set(index, "".to_string());
      warn!("active segments not loaded yet");
    }
  });
  //raw_memory::set_active_segments(&[0]);
}

#[cfg(test)]
mod tests {
  use super::*;
  /*
  #[derive(Debug, Encode, Decode)]
  struct Test {
    #[n(0)] a: u8,
    #[n(1)] b: BTreeMap<u32, u32>
  }

  #[test]
  fn can_encode() {
    let mut map = BTreeMap::<u32, u32>::new();
    map.insert(1, 1);
    let test = Test { a: 1, b: map };
    let mut other = Vec::<u8>::new();
    //let other_ref: &mut [u8] = other.as_mut_slice();
    let mut encoder = minicbor::Encoder::new(&mut other);
    let res = encoder.encode(test).expect("encode error");
    println!("result {:?}", res);
    let other2 = encoder.into_writer();
    //minicbor::encode(test, other_ref);
    println!("other at end: {:?}", &other2);
    let t2: Test = minicbor::decode(other.as_slice()).expect("test decode");
    println!("final test {:?}", t2);
  }*/
  #[test]
  fn serialize_deserialize() {
    use crate::role::*;
    let mut buffer = Vec::new();
    let mut mem = Memory::default();
    mem.creeps.insert("test".to_string(), CreepMemory::Harvester (Harvester {
      target: HarvesterTarget::Upgrade(ObjectId::from_packed(0))
    }));
    to_buffer(mem, &mut buffer);
    let mem_str = to_mem_string(&buffer);
    buffer.clear();
    let mut memory = load_mem(mem_str, &mut buffer).expect("memory");
    assert_eq!(memory.creeps.len(), 1);
  }
}
