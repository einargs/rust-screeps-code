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

use minicbor::{Encode, Decode};
use js_sys::{JsString, Object, Reflect};
use base64::{Engine as _, engine::general_purpose};
use screeps::raw_memory;

#[derive(Encode, Decode)]
pub struct Memory {
  #[n(0)] pub creep_counter: u32,
}

impl Default for Memory {
  fn default() -> Memory {
    Memory {
      creep_counter: 0
    }
  }
}

#[derive(Debug)]
enum MemError {
  Base64(base64::DecodeError),
  StringConversion,
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

impl<T> From<minicbor::encode::Error<T>> for MemError {
  fn from(err: minicbor::encode::Error<T>) -> MemError {
    MemError::EncodeCbor
  }
}

fn from_buffer(buffer: &[u8]) -> Result<Memory, MemError> {
  Ok(minicbor::decode(buffer)?)
}

fn to_buffer(memory: Memory, buffer: &mut Vec<u8>) -> Result<(), MemError> {
  Ok(minicbor::encode(memory, buffer.as_mut_slice())?)
}

// For the memory we're going to keep around a vector in the local heap
// that we'll clear and deserialize the memory into every time we need it.
fn to_js_string(data: &[u8]) -> JsString {
  general_purpose::STANDARD_NO_PAD.encode(data).into()
}

fn from_js_string(string: JsString, target: &mut Vec<u8>) -> Result<(), MemError> {
  // I may need to write a custom decoder to avoid a redudant copy?
  // but that's premature right now.

  // given that we'll only be building them from rust strings, this wouldn't make much sense.
  let rust_str = string.as_string().ok_or(MemError::StringConversion)?;
  Ok(general_purpose::STANDARD_NO_PAD.decode_vec(rust_str, target)?)
}

fn load_mem(buffer: &mut Vec<u8>) -> Result<Memory, MemError> {
  let js_memory = raw_memory::get();
  from_js_string(js_memory, buffer)?;
  from_buffer(buffer.deref())
}

thread_local! {
  static MEMORY_DECODE_BUFFER: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}

pub fn with_memory(fun: impl FnOnce(&mut Memory) -> ()) -> () {
  MEMORY_DECODE_BUFFER.with(|buf_refcell| {
    let mut buffer = buf_refcell.borrow_mut();
    buffer.clear();
    let mut memory = match load_mem(buffer.deref_mut()) {
      Err(_) => Memory::default(),
      Ok(mem) => mem
    };
    fun(&mut memory);
    to_buffer(memory, buffer.deref_mut()).expect("encoding problem");
    let new_js_mem = to_js_string(buffer.deref());
    raw_memory::set(&new_js_mem);
  });
}
