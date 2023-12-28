use std::collections::HashMap;

use screeps::pathfinder::{MultiRoomCostResult, SearchOptions, search_many, SearchGoal};
use screeps::{
  Source, Creep, Room, Terrain, Mineral, Deposit,
  Structure, Flag, Resource, ConstructionSite, Nuke, Tombstone,
  Ruin, find, game, look, RoomXY, ResourceType,
  prelude::*, RoomObject, RoomCoordinate, Path,
  ErrorCode, FindConstant, RoomName,
};
use screeps::look::{LookResult, PositionedLookResult};
use screeps::local::Position;

use log::*;

fn is_hostile(creep: &Creep) -> bool {
  // NOTE: when this eventually changes, I'm going to need to change
  // `find_closest_hostile`.
  !creep.my()
}

/// This is specfically for generic movement times for planning out
/// base layout within a single room.
///
/// We ignore creep positions and other things that change over time.
///
/// NOTE: I need to replace a lot of uses of this with search_many.
pub fn path_cost_between(pos: Position, xy: RoomXY) -> u32 {
  use screeps::{CostMatrix, RoomName};
  use screeps::pathfinder::{MultiRoomCostResult, SearchOptions, search};
  let options = SearchOptions
    ::new(|_| MultiRoomCostResult::CostMatrix(CostMatrix::new()))
    .plain_cost(1)
    .swamp_cost(5);
  let mut pos2 = pos.with_x(xy.x);
  pos2.set_y(xy.y);
  // range is the distance to the destination before it's considered reached.
  // very useful for e.g. moving to a source.
  let range = 0;
  let result = search(pos, pos2, range, Some(options));
  result.cost()
}

/*
// Accumulated information about a tile returned from a look result.
pub struct TileInfo {
  results: Vec<LookResult>,
  terrain: Terrain
}
*/

type AreaInfo = HashMap<RoomXY, Vec<LookResult>>;

fn build_area_info(pos_results: Vec<PositionedLookResult>) -> AreaInfo {
  let mut map: HashMap<RoomXY, Vec<LookResult>> = HashMap::new();
  for result in pos_results.into_iter() {
    let xy = RoomXY::try_from((result.x, result.y))
      .expect("coords from look results should be within room bounds");
    map.entry(xy).or_default().push(result.look_result);
  }
  map
}

/// top, left, bot, right
pub fn square_around(pos: Position, radius: i8) -> (RoomCoordinate, RoomCoordinate, RoomCoordinate, RoomCoordinate) {
  let xy = pos.xy();
  let top = xy.y.saturating_add(-radius);
  let left = xy.x.saturating_add(-radius);
  let bot = xy.y.saturating_add(radius);
  let right = xy.x.saturating_add(radius);
  (top, left, bot, right)
}

pub fn look_at_square(pos: Position, radius: i8) -> AreaInfo {
  let (top, left, bot, right) = square_around(pos, radius);
  let room = game::rooms().get(pos.room_name()).unwrap();
  let results = room.look_at_area(top.into(), left.into(), bot.into(), right.into());
  build_area_info(results)
}

pub fn find_closest_hostile(creep: &Creep) -> Option<Creep> {
  creep.pos().find_closest_by_path(find::HOSTILE_CREEPS, None)
}

/// This builds the standard search options built for findClosestByPath.
/// It assumes we are not ignoring roads. As such, we double the cost of plains
/// and swamps to 2 and 10 (since roads will be 1).
///
/// See: https://github.com/screeps/engine/blob/master/src/game/rooms.js#L304
pub fn local_search_opts() -> SearchOptions<fn(RoomName) -> MultiRoomCostResult> {
  // we do
  SearchOptions::default()
    .plain_cost(2)
    .swamp_cost(10)
    .max_rooms(1)
}

/// Version of `Position::find_closest_by_path` that takes a filter.
///
/// Uses `pathfinder::search_many` with `local_search_opts`.
pub fn filter_map_closest_by_path<F: FindConstant, T>(
  find: F,
  pos: Position,
  mut pred: impl FnMut(F::Item) -> Option<T>
) -> Option<T> where F::Item: HasPosition {
  // We default to a range of 1 because we assume for room objects we just want to be
  // next to it.
  const RANGE: u32 = 1;
  let room = game::rooms().get(pos.room_name())
    .expect("Could not get room for position");
  let mut nodes: HashMap<Position, T> = room.find(find, None)
    .into_iter()
    .filter_map(move |item| {
      let item_pos = item.pos();
      pred(item).map(|val| (item_pos, val))
    })
    .collect();
  let goals = nodes.keys()
    .map(|item_pos| SearchGoal::new(item_pos.clone(), RANGE));
  let results = search_many(pos, goals, Some(local_search_opts()));
  results.path().pop().and_then(|closest_pos| {
    // this can fail if the path didn't fully complete
    nodes.remove(&closest_pos)
  })
}

/// Version of `filter_map_closest_by_path` that uses filter instead of `filter_map`.
#[inline]
pub fn filter_closest_by_path<F: FindConstant>(
  find: F,
  pos: Position,
  mut pred: impl FnMut(&F::Item) -> bool
) -> Option<F::Item> where F::Item: HasPosition {
  filter_map_closest_by_path(find, pos, move |item|
                             if pred(&item) { Some(item) } else { None })
}

/// Find the closest object in the room that passes the filter.
pub fn filter_map_closest_by_range<F: FindConstant, T>(
  find: F,
  pos: Position,
  mut pred: impl FnMut(F::Item) -> Option<T>
) -> Option<T> where F::Item: HasPosition {
  let room = game::rooms().get(pos.room_name()).unwrap();
  room.find(find, None)
    .into_iter()
    .filter_map(move |item| {
      let item_pos = item.pos();
      pred(item).map(|val| (item_pos, val))
    })
    .min_by_key(|(item_pos,_)| item_pos.get_range_to(pos))
    .map(|(_,val)| val)
}

/// Version of `filter_map_closest_by_range` that uses filter instead of `filter_map`.
#[inline]
pub fn filter_closest_by_range<F: FindConstant>(
  find: F,
  pos: Position,
  mut pred: impl FnMut(&F::Item) -> bool
) -> Option<F::Item> where F::Item: HasPosition {
  filter_map_closest_by_range(find, pos, move |item|
                              if pred(&item) { Some(item) } else { None })
}

pub fn are_hostiles_near(room: &Room, pos: Position, range: u8) -> bool {
  pos.find_in_range(find::HOSTILE_CREEPS, range)
    .into_iter()
    .filter(is_hostile)
    .next()
    .is_some()
}

/*
/// Are no hostiles near and the source is accessible.
fn is_source_okay(source: &Source) -> bool {
  use std::collections::HashMap;
  let room = source.room().expect("Should never be null for a source according to docs");
  let map: HashMap<Position, String> = HashMap::new();
  let walkable = true;
  walkable && !are_hostiles_near(&room, source.pos(), 5)
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
*/

pub fn energy_full<T: HasStore>(obj: &T) -> bool {
  obj.store().get_free_capacity(Some(ResourceType::Energy)) == 0
}

pub fn energy_empty<T: HasStore>(obj: &T) -> bool {
  obj.store().get_used_capacity(Some(ResourceType::Energy)) == 0
}

pub fn move_to_do<T: HasPosition + AsRef<RoomObject>>(
  creep: &Creep, obj: &T, range: u32, op: impl FnOnce() -> ()
) {
  if creep.pos().in_range_to(obj.pos(), range) {
    op();
  } else {
    // TODO: eventually get custom path code.
    match creep.move_to(obj) {
      Ok(()) => (),
      Err(ErrorCode::Tired) => (),
      Err(e) => warn!("Creep {} couldn't move because: {:?}", creep.id_str(), e),
    }
  }
}

pub fn path_len(path: &Path) -> usize {
  match path {
    Path::Serialized(string) => string.len(),
    Path::Vectorized(vector) => vector.len(),
  }
}

pub trait PrettyId {
  fn id_str(&self) -> String;
}

impl<T: MaybeHasId> PrettyId for T {
  fn id_str(&self) -> String {
    match self.try_raw_id() {
      Some(id) => id.to_string(),
      None => "(no id)".to_string()
    }
  }
}

#[macro_export]
macro_rules! log_warn {
  ($e:expr, $err_name:ident => $l:literal) => {
    if let Err($err_name) = $e {
      warn!($l);
    }
  };
  ($e:expr, $err_name:ident => $l:literal, $($arg:expr),*) => {
    if let Err($err_name) = $e {
      log::warn!($l, $($arg),*);
    }
  };
}
