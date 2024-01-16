#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]
// TODO: remove
#![feature(more_qualified_paths)]
#![feature(assert_matches)]

mod rooms;
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
use rooms::tile_min_cut::{build_cost_matrix, min_cut_to_exit};
use screeps::constants::{ErrorCode, Part, ResourceType};
use screeps::enums::StructureObject;
use screeps::local::ObjectId;
use screeps::objects::{Creep, Source, StructureController, Room, StructureSpawn};
use screeps::{prelude::*, RoomXY};
use screeps::{find, game, RoomName};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::memory::Memory;
use crate::storage::serialization::with_memory;
use crate::managers::city::spawn_loop;

static INIT_LOGGING: std::sync::Once = std::sync::Once::new();

#[wasm_bindgen(js_name = testDistTransform)]
pub fn test_dist_transform(name: JsString) {
  use rooms::dist_transform::DistMatrix;
  let name: RoomName = match name.clone().try_into() {
    Err(e) => {
      error!("{} was not a room name", name);
      return
    }
    Ok(name) => name,
  };
  let room = game::rooms().get(name).unwrap();
  let local_terrain = room.get_terrain().into();
  let dist_matrix = DistMatrix::new_chessboard(&local_terrain);
  let visual = room.visual();
  for x in 0..50 {
    for y in 0..50 {
      let xy = RoomXY::try_from((x,y)).unwrap();
      let num = format!("{}", dist_matrix.get(xy));
      visual.text(f32::from(x), f32::from(y), num, None);
    }
  }
  // info!("dist_matrix {dist_matrix}");
}

#[wasm_bindgen(js_name = testRoomCut)]
pub fn test_room_cut(name: JsString, render: JsString) {
  use rooms::room_cut::*;
  use itertools::iproduct;
  use std::iter::{repeat, zip};
  info!("Called from js");
  let name: RoomName = match name.clone().try_into() {
    Err(e) => {
      error!("{} was not a room name", name);
      return
    }
    Ok(name) => name,
  };
  let render_str = match render.as_string() {
    None => return,
    Some(s) => s,
  };
  let render = match render_str.as_str() {
    "height" => Render::Height,
    "colors" => Render::Colors,
    _ => return,
  };
  let room = game::rooms().get(name).unwrap();
  let local_terrain = room.get_terrain().into();
  let sources: Vec<RoomXY> = [
    (34, 41),
    // both in the same segment rn
    (25, 17),
    (28, 10)
  ].into_iter().map(|p| RoomXY::try_from(p).unwrap()).collect();
  room_cut(&local_terrain, room.visual(), render, &sources);
  /*
  let sources: Vec<RoomXY> = iproduct!(15..26, 15..26)
    //zip(4..25, repeat(15))
    .map(|p| RoomXY::try_from(p).expect("safe"))
    .collect();
  //[(22, 15), (21, 25), (35, 20)]
  */
}

thread_local! {
  static WALL_DRAW: RefCell<Vec<RoomXY>> = RefCell::new(Vec::new());
}

#[wasm_bindgen(js_name = testMinCut)]
pub fn test_min_cut(name: JsString) {
  use rooms::tile_min_cut::*;
  use itertools::iproduct;
  use std::iter::{repeat, zip};
  info!("Called from js");
  let name: RoomName = match name.clone().try_into() {
    Err(e) => {
      error!("{} was not a room name", name);
      return
    }
    Ok(name) => name,
  };
  let room = game::rooms().get(name).unwrap();
  let local_terrain = room.get_terrain().into();
  let cost_matrix = build_cost_matrix(&local_terrain);
  let sources: Vec<RoomXY> = iproduct!(15..26, 15..26)
    //zip(4..25, repeat(15))
    .map(|p| RoomXY::try_from(p).expect("safe"))
    .collect();
  //[(22, 15), (21, 25), (35, 20)]
  let wall_xys = min_cut_to_exit(&sources, &cost_matrix, &room.visual());
  WALL_DRAW.with(move |cell| {
    let mut var = cell.borrow_mut();
    *var = wall_xys;
    debug!("Changed");
  });
}

#[wasm_bindgen(js_name = loop)]
pub fn dummy_loop() {
  INIT_LOGGING.call_once(|| {
    // show all output of Info level, adjust as needed
    logging::setup_logging(logging::Debug);
  });
  WALL_DRAW.with(|cell| {
    let wall_xys = cell.borrow();
    let room = game::rooms().get(RoomName::new("sim").unwrap()).unwrap();
    let visual = room.visual();
    for xy in wall_xys.iter() {
      let x: f32 = xy.x.u8().into();
      let y: f32 = xy.y.u8().into();
      visual.rect(x - 0.5, y - 0.5, 1.0, 1.0, None);
    }
  });
}

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
