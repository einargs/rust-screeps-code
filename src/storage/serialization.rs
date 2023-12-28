use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::default::Default;
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
use crate::memory::Memory;
use crate::storage::cbor;

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

fn to_buffer(memory: &Memory, buffer: &mut Vec<u8>) -> Result<(), MemError> {
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

/// Quick utility thing for when I want to log a buffer as a hex string.
struct HexSlice<'a>(&'a [u8]);

use std::fmt::{Formatter, UpperHex};
impl<'a> UpperHex for HexSlice<'a> {
  fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
    for byte in self.0 {
      UpperHex::fmt(byte, f)?;
    }
    Ok(())
  }
}

pub fn with_memory(fun: impl FnOnce(&mut Memory) -> ()) -> () {
  const index: u8 = 1;
  raw_memory::set_active_segments(&[index]);

  MEMORY_DECODE_BUFFER.with(|buf_refcell| {
    let active_segments = raw_memory::segments();
    //debug!("active segments {:?}", active_segments);
    if let Some(mem_str) = active_segments.get(index) {
      let mut buffer = buf_refcell.borrow_mut();
      buffer.clear();
      let mut memory = match load_mem(mem_str, buffer.deref_mut()) {
        Err(err) => {
          warn!("generating default memory because of error: {err:?}");
          Memory::default()
        },
        Ok(mem) => mem
      };
      fun(&mut memory);
      info!("memory state: {:?}", &memory);
      memory.last_time = screeps::game::time();
      buffer.clear();
      to_buffer(&memory, buffer.deref_mut()).expect("encoding problem");
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
  use screeps::objects::{Source};
  use persist_memory::Persist;
  // in hex: 6497E5D8E58BD3C61B071B9000000018
  const OBJECT_ID_RAW: u128 = 133711498260253793587658037051825061912;
  const SPAWN_ID_RAW: u128 = 251504297449469618279889252367202254872;

  #[derive(PartialEq, Eq, Debug, Decode, Encode)]
  struct Holder(#[n(0)] #[cbor(with="cbor::object_id")] ObjectId<Source>);

  /*
  #[derive(Persist)]
  struct Test {
    a: u32,
    #[persist(0)] b: u32,
    c: u32,
    #[persist(1, "cbor::object_id")] d: ObjectId<Creep>,
  }

  #[test]
  fn can_use_persist() {
    let t = Test {
      a: 0, b: 0, c: 0, d: ObjectId::from_packed(SPAWN_ID_RAW),
    };
    let o = t.to_persist();
  }
  */

  #[test]
  fn serialize_deserialize_object_id() {
    use cbor::object_id;
    let mut buffer = Vec::new();
    let h1: Holder = Holder(ObjectId::from_packed(OBJECT_ID_RAW));
    let mut encoder = minicbor::Encoder::new(&mut buffer);
    encoder.encode(&h1).expect("encode error");
    let buffer2 = encoder.into_writer();
    let mem_string = to_mem_string(buffer2);
    let mut decode_buffer: Vec<u8> = Vec::new();
    from_mem_string(mem_string, &mut decode_buffer).expect("decoded");
    let h2: Holder = minicbor::decode(decode_buffer.as_slice()).expect("test decode");
    assert_eq!(h1, h2);
  }
  #[test]
  fn serialize_deserialize_memory() {
    use crate::role::*;
    let mut buffer = Vec::new();
    let mut mem = Memory::default();
    mem.creeps.insert("Builder-0".to_string(), CreepMemory::Builder (Builder {
      target: BuilderTarget::Harvest(TargetResource::TravelingTo(ObjectId::from_packed(OBJECT_ID_RAW)))
    }));
    let spawn_id = ObjectId::<StructureSpawn>::from_packed(SPAWN_ID_RAW);
    mem.spawns.insert(spawn_id, SpawnMemory {
      next_role: RoleTag::Harvester,
    });
    /*mem.creeps.insert("Harvester-1".to_string(), CreepMemory::Harvester (Harvester {
      target: HarvesterTarget::Harvest(ObjectId::from_packed(OBJECT_ID_RAW))
    }));*/
    to_buffer(&mem, &mut buffer).expect("buffer conversion error");
    let mem_str = to_mem_string(&buffer);
    buffer.clear();
    let mut memory = load_mem(mem_str, &mut buffer).expect("memory");
    assert_eq!(memory, mem);
  }
}
