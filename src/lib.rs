#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
// TODO: remove
#![feature(more_qualified_paths)]
#![feature(assert_matches)]

mod body;
mod util;
mod storage;
mod logging;
mod memory;
mod creeps;
mod managers;

use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

use js_sys::{JsString, Object, Reflect};
use log::*;
use screeps::constants::{ErrorCode, Part, ResourceType};
use screeps::enums::StructureObject;
use screeps::local::ObjectId;
use screeps::objects::{Creep, Source, StructureController, Room, StructureSpawn};
use screeps::prelude::*;
use screeps::{find, game};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::memory::Memory;
use crate::storage::serialization::with_memory;
use crate::managers::city::spawn_loop;

static INIT_LOGGING: std::sync::Once = std::sync::Once::new();

#[wasm_bindgen(js_name = loop)]
pub fn game_loop() {
  INIT_LOGGING.call_once(|| {
    // show all output of Info level, adjust as needed
    logging::setup_logging(logging::Debug);
  });
  with_memory(|mem| {
    //info!("count: {}", mem.creep_counter);
    managers::city::spawn_loop(mem);
    creeps::creep_loop::creep_loop(mem);
    clean_up(mem);

  });
  info!("done! cpu: {}", game::cpu::get_used());
}

mod my_game {
  use wasm_bindgen::prelude::*;
  #[wasm_bindgen]
  extern "C" {
    pub type Game;

    #[wasm_bindgen(static_method_of = Game, getter = creeps)]
    pub fn creeps() -> JsValue;
  }
}

fn clean_up(memory: &mut Memory) {
  if game::time() % 1000 == 0 {
    debug!("running memory cleanup");
    let game_creeps = my_game::Game::creeps();
    if let Ok(memory_creeps) = Reflect::get(&screeps::memory::ROOT, &JsString::from("creeps")) {
      // convert from JsValue to Object
      let memory_creeps: Object = memory_creeps.unchecked_into();
      // iterate memory creeps
      for creep_name_js in Object::keys(&memory_creeps).iter() {
        // convert to String (after converting to JsString)
        let creep_name = String::from(creep_name_js.dyn_ref::<JsString>().unwrap());

        // check the HashSet for the creep name, deleting if not alive
        if Reflect::has(&game_creeps, &creep_name_js).unwrap() {
          info!("deleting memory for dead creep {}", creep_name);
          let _ = Reflect::delete_property(&memory_creeps, &creep_name_js);
          let _ = memory.creeps.remove(&creep_name);
        }
      }
    }
  }
}
