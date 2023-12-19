use screeps::objects::{Source, Creep, Room};
use screeps::prelude::*;
use screeps::look::LookResult;
use screeps::{find, game, look};
use screeps::local::Position;

use log::*;

struct SourceArea {
  open_slots: u8
}

fn is_hostile(creep: &Creep) -> bool {
  // NOTE: when this eventually changes, I'm going to need to change
  // `find_closest_hostile`.
  !creep.my()
}

pub fn find_closest_hostile(creep: &Creep) -> Option<Creep> {
  creep.pos().find_closest_by_path(find::HOSTILE_CREEPS, None)
}

pub fn are_hostiles_near(room: &Room, pos: Position, radius: u8) -> bool {
  room.look_for_at_area(look::CREEPS, radius, radius, radius, radius)
    .into_iter()
    .any(|result| match result.look_result {
      LookResult::Creep(ref creep) => is_hostile(creep),
      _ => false,
    })
}

/// Are no hostiles near and the source is accessible.
fn is_source_okay(source: &Source, room: &Room) -> bool {
  use std::collections::HashMap;
  let mut map: HashMap<Position, String> = HashMap::new();
  let walkable = true;
  walkable && !are_hostiles_near(room, source.pos(), 5)
}

pub fn select_source(creep: &Creep) -> Option<Source> {
  // todo: cycle between sources?
  // well, if I move to stationary miners, that's not a problem.
  let room = creep.room().expect("Creep was not in room");
  let nearest = room
    .find(find::SOURCES_ACTIVE, None)
    .into_iter()
    .filter(|source| !are_hostiles_near(&room, source.pos(), 5))
    .min_by_key(|source| source.pos().get_range_to(creep.pos()));
  debug!("nearest {:?}", nearest);
  nearest
}
