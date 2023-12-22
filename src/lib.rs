#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
// TODO: remove
#![feature(more_qualified_paths)]

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

static INIT_LOGGING: std::sync::Once = std::sync::Once::new();

#[wasm_bindgen(js_name = loop)]
pub fn game_loop() {
  INIT_LOGGING.call_once(|| {
    // show all output of Info level, adjust as needed
    logging::setup_logging(logging::Info);
  });
  memory::with_memory(|mem| {
    //info!("count: {}", mem.creep_counter);
    spawn_loop(mem);
    creeps::creep_loop::creep_loop(mem);
    clean_up(mem);

  });
  info!("done! cpu: {}", game::cpu::get_used());
}

fn spawn_loop(memory: &mut Memory) {
  use body::BodyDesign;
  for spawn in game::spawns().values() {
    debug!("running spawn {}", String::from(spawn.name()));
    let mem = memory.spawn_mut(spawn.id()).or_default();

    let room = spawn.room().unwrap();
    let spawn_energy = spawn.store().get(ResourceType::Energy).unwrap_or(0);
    let spawn_capacity = spawn.store().get_capacity(Some(ResourceType::Energy));
    let design = BodyDesign::new()
      .r#move(2)
      .carry(1)
      .work(1);
    let body_cost = design.max_cost(spawn_capacity);

    if spawn_energy < body_cost {
      info!("need {} more energy for cost {}", body_cost - spawn_energy, body_cost);
    } else {
      let role = mem.get_role();
      let name = memory.creep_name(role);
      let body = design.scale(spawn_capacity);
      // create a unique name, spawn.
      match spawn.spawn_creep(&body, &name) {
        Err(e) => warn!("couldn't spawn: {:?}", e),
        Ok(()) => {
          // Initialize the creep
          debug!("initializing creep {}", &name);
          // let creep = game::creeps().get(name).expect("couldn't get freshly created creep");
          let creep_mem = initial_creep_memory(&room, role);
          let tmp = name.clone();
          memory.initialize_creep(name, creep_mem);
          debug!("has {}", memory.creeps.contains_key(&tmp));
        }
      }
    }
  }
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
