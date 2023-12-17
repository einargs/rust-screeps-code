#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

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

mod util;
mod storage;
mod role;
mod logging;
mod memory;

use crate::role::{Role, RoleTag, CreepMemory};
use crate::memory::Memory;

static INIT_LOGGING: std::sync::Once = std::sync::Once::new();

#[wasm_bindgen(js_name = loop)]
pub fn game_loop() {
  INIT_LOGGING.call_once(|| {
    // show all output of Info level, adjust as needed
    logging::setup_logging(logging::Debug);
  });
  memory::with_memory(|mem| {
    info!("count: {}", mem.creep_counter);
    spawn_loop(mem);
    creep_loop(mem);

  });
  info!("done! cpu: {}", game::cpu::get_used());
}

fn initial_creep_memory(room: &Room, tag: RoleTag) -> CreepMemory {
  use role::*;
  let source = util::select_source(room);
  match tag {
    RoleTag::Harvester => CreepMemory::Harvester(Harvester {
      target: HarvesterTarget::Harvest(source.id())
    }),
    RoleTag::Builder => CreepMemory::Builder(Builder {
      target: BuilderTarget::Harvest(source.id())
    }),
  }
}

fn creep_loop(memory: &mut Memory) {
  info!("length {}", memory.creeps.len());
  for (name, creep_mem) in &memory.creeps {
    debug!("{}: {:?}", name, creep_mem);
  }
  for creep in game::creeps().values() {
    let name = creep.name();
    if let Some(mem) = memory.creep_mut(&name) {
      mem.run(creep);
    } else {
      warn!("no memory for creep: {}", &name);
    }
  }
}

fn spawn_loop(memory: &mut Memory) {
  for spawn in game::spawns().values() {
    debug!("running spawn {}", String::from(spawn.name()));
    let mem = memory.spawn_mut(spawn.id()).or_default();
    let role = mem.get_role();

    let body = [Part::Move, Part::Move, Part::Carry, Part::Work];
    let body_cost = body.iter().map(|p| p.cost()).sum();
    let room = spawn.room().unwrap();

    if room.energy_available() >= body_cost {
      let name = memory.creep_name(role);
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
