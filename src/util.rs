use screeps::objects::{Source, Room};
use screeps::prelude::*;
use screeps::{find, game};

pub fn select_source(room: &Room) -> Source {
  // todo: cycle between sources?
  // well, if I move to stationary miners, that's not a problem.
  room.find(find::SOURCES_ACTIVE, None).pop()
    .or_else(|| room.find(find::SOURCES, None).pop())
    .expect("room had no sources in it")
}
