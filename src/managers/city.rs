use minicbor::{Encode, Decode};
use log::*;
use std::cell::Cell;
use screeps::local::ObjectId;
use screeps::look::{LookResult, PositionedLookResult};
use screeps::{
  RoomObject, Creep, Source, StructureController, ConstructionSite, StructureSpawn,
  RoomName, find, prelude::*, Room, ResourceType, StructureType, RoomXY, Position,
  look, Terrain, game, RoomCoordinate, FindPathOptions, Part, StructureObject, Resource,
};
use crate::creeps::early_worker::EarlyWorker;
use crate::util::{self, look_at_square, PrettyId};
use crate::body::BodyDesign;
use crate::creeps::CreepMemory;
use crate::memory::Memory;
use crate::{mk_cache, log_warn};
use crate::creeps::RoleTag;
use crate::creeps::harvester::{self, lair_for_source};
use crate::creeps::worker;

fn edges_around(xy: &RoomXY, radius: i8) -> impl Iterator<Item = RoomXY> {
  let xy = xy.clone();
  let top = radius - 1;
  let left = -radius;
  let bot = 1 - radius;
  let right = radius;
  let bot_edge = (left..right).map(move |x| (x, bot));
  let top_edge = (left..right).map(move |x| (x, top));
  let left_edge = (top..bot).map(move |y| (y, left));
  let right_edge = (top..bot).map(move |y| (y, right));
  let edge_iter = bot_edge.chain(left_edge).chain(top_edge).chain(right_edge)
    .filter_map(move |offset| xy.checked_add(offset));
  return edge_iter
}

fn open_edges_around(room: &Room, xy: &RoomXY, radius: i8) -> impl Iterator<Item = RoomXY> {
  let terrain = room.get_terrain();
  edges_around(xy, radius)
    .filter(move |xy| terrain.get(xy.x.into(), xy.y.into()) != Terrain::Wall)
}

/// Identify a location to put a simple storage container.
///
/// Just grabs the first open terrain three squares away.
fn place_storage_container_for(source: &Source) {
  use screeps::pathfinder::{SearchGoal, search_many};
  use screeps::{CostMatrix, RoomName};
  let room = source.room().expect("source has room");
  let source_pos = source.pos();
  let xy = source_pos.xy();
  let room_terrain = room.get_terrain();
  let goals = open_edges_around(&room, &xy, 3)
    .map(|xy| SearchGoal::new(Position::new(xy.x, xy.y, room.name()), 0));

  let search_result = search_many(source_pos, goals, Some(util::local_search_opts()));
  // NOTE: this could be a problem later down the line, but we're replacing this with
  // proper city planning.
  let pos = search_result.path().pop()
    .expect("couldn't find an open spot for the storage location");

  // spawning the construction site won't take effect this tick.
  if let Err(e) = pos.create_construction_site(StructureType::Container, None) {
    warn!("Error creating construction site {} {}: {e:?}", pos.x(), pos.y());
  }
  harvester::have_updated_source_storage(source);
}

fn building_locations(room: &Room) -> impl Iterator<Item = Position> + '_ {
  const RADIUS_AROUND_SPAWN: i8 = 5;
  const INNER_RADIUS: u32 = 3;
  room.find(find::MY_SPAWNS, None)
    .into_iter()
    .flat_map(move |spawn| {
      let spawn_pos = spawn.pos();
      let area = util::look_at_square(spawn.pos(), RADIUS_AROUND_SPAWN);
      area
        .into_iter()
        .filter(move |(xy, results)| {
          let look_pos = Position::new(xy.x, xy.y, room.name());
          let too_close = look_pos.in_range_to(spawn_pos, INNER_RADIUS);
          use LookResult::*;
          let can_build = results.iter().any(|result| match result {
            ConstructionSite(_site) => false,
            Mineral(_) => false,
            Resource(_) => false,
            Structure(_) => false,
            Terrain(screeps::Terrain::Wall) => false,
            _ => true,
          });
          !too_close && can_build
        })
    })
    .map(|(xy, results)| Position::new(xy.x, xy.y, room.name()))
}

thread_local! {
  /// See `place_spawn_extension`.
  static HAS_ADDED_SPAWN_EXT_THIS_TICK: Cell<bool> = Cell::new(false);
}

/// Add a new spawn extension.
///
/// Uses a thread local cell to ensure that we can only place one of these per
/// tick since construction sites don't show up until next tick, so it's easy
/// for e.g. all creeps of a kind (like EarlyWorker) to place a bunch at once.
///
/// Eventually when we do proper room planning that won't be necessary, so it's
/// a bit of a hack.
pub fn place_spawn_extension(room: &Room) {
  if !HAS_ADDED_SPAWN_EXT_THIS_TICK.get() {
    match building_locations(room).nth(0) {
      Some(pos) => {
        match pos.create_construction_site(StructureType::Extension, None) {
          Ok(()) => {
            HAS_ADDED_SPAWN_EXT_THIS_TICK.set(true);
          }
          Err(err) => {
            warn!("Failed to make spawn extension in {} because {err:?}", room.name());
          }
        }
      }
      None => {
        warn!("Ran out of building locations in room {}", room.name());
      }
    }
  }
}

mk_cache! {
  cache_max_harvesters lifetime 200 by RoomName => u32
}

fn max_room_harvesters(room: &Room) -> u32 {
  cache_max_harvesters::caches(&room.name(), |_| {
    room.find(find::SOURCES, None)
      .into_iter()
      .map(|source| harvester::calc_harvest_spots(&source) as u32)
      .sum()
  })
}

mk_cache! {
  cache_current_role_count lifetime 20 by (RoomName, RoleTag) => u32
}

pub fn current_role_count(room: &Room, memory: &Memory, role: RoleTag) -> u32 {
  cache_current_role_count::caches(&(room.name(), role), |_| {
    room.find(find::MY_CREEPS, None)
      .into_iter()
      .filter(|creep| memory
              .creep(creep)
              .map_or(false, |mem| mem.tag() == role))
      .count() as u32
  })
}

pub fn initial_city_construction(room: &Room) {
  for source in room.find(find::SOURCES, None) {
    place_storage_container_for(&source);
  }
}

fn design_for_memory(memory: &CreepMemory, max_energy: u32) -> BodyDesign {
  match memory {
    CreepMemory::Harvester(_) => {
      // TODO: calculate the best number of worker parts to deplete the source
      // in time for it to refill.
      let design = BodyDesign::new().r#move(1).carry(1);
      let base_cost = design.base_cost();
      let available = max_energy - base_cost;
      let work_num = available / Part::Work.cost();
      design.work(work_num.try_into().unwrap())
    }
    CreepMemory::Worker(_) =>
      BodyDesign::new()
      .r#move(3)
      .carry(2)
      .work(1),
    CreepMemory::EarlyWorker(_) =>
      BodyDesign::new()
      .r#move(1)
      .carry(2)
      .work(1),
  }
}

fn spots_left_at_source(source: &Source, memory: &Memory) -> u32 {
  use harvester::*;
  let max = calc_harvest_spots(source) as u32;
  let cur = num_assigned_harvesters(source, memory) as u32;
  max - cur
}

fn find_least_harvested_source(room: &Room, memory: &Memory) -> Option<Source> {
  room.find(find::SOURCES, None)
    .into_iter()
    .map(|source| {
      let i = spots_left_at_source(&source, memory);
      (source, i)
    })
    .filter(|(source, val)| *val != 0 && lair_for_source(&source).is_none())
    .max_by_key(|(_, val)| *val)
    .map(|(source, _)| source)
}

mk_cache! {
  city_early_cache lifetime 100 by RoomName => bool
}

/// This is primarily used to determine whether we should be spawning early workers
/// or workers and harvesters.
///
/// Key development metrics:
/// - number of extensions
/// - controller level
/// - number of early workers
///
/// TODO: I think we should have it exit early city before RCL 3, since that's so
/// expensive.
fn is_city_early(room: &Room, memory: &Memory) -> bool {
  city_early_cache::caches(&room.name(), |_| {
    let Some(controller) = room.controller() else {
      warn!("Running city code on room without controller {}", room.name());
      return true;
    };
    let level = controller.level();
    let early_worker_count = current_role_count(room, memory, RoleTag::EarlyWorker);
    let spawn_ext_num = room.find(find::MY_STRUCTURES, None)
        .into_iter()
        .filter(|s| matches!(s, StructureObject::StructureExtension(_)))
        .count();
    level < 3 || early_worker_count < 10 || spawn_ext_num < 8
  })
}

fn pick_next_creep(room: &Room, memory: &Memory) -> Option<CreepMemory> {
  use std::cmp::max;

  if is_city_early(room, memory) {
    Some(CreepMemory::EarlyWorker(EarlyWorker::Idle))
  } else {
    let max_harvesters = max_room_harvesters(room);
    let num_harvesters = current_role_count(room, memory, RoleTag::Harvester);
    let num_workers = current_role_count(room, memory, RoleTag::Worker);

    let build_harvester = num_harvesters / max(1, num_workers) < 2
      && max_harvesters > num_harvesters;
    let harvester_source = if build_harvester {
      let src = find_least_harvested_source(room, memory);
      src
    } else { None };

    if let Some(source) = harvester_source {
      use harvester::*;
      harvester_assignments_changed_for(&source);
      Some((Harvester {
        state: HarvesterState::Harvesting,
        source: source.id(),
      }).into())
    } else if num_workers < 10 {
      use worker::*;
      Some(Worker::Idle.into())
    } else {
      None
    }
  }
}

pub fn spawn_creep(spawn: &StructureSpawn,
                   creep_memory: CreepMemory,
                   memory: &mut Memory) {
  let spawn_energy = spawn.store().get(ResourceType::Energy).unwrap_or(0);
  let spawn_capacity = spawn.store().get_capacity(Some(ResourceType::Energy));
  let design = design_for_memory(&creep_memory, spawn_capacity);
  let body_cost = design.max_cost(spawn_capacity);
  info!("spawn energy {} cost {}", spawn_energy, body_cost);

  if spawn_energy < body_cost {
    info!("need {} more energy for cost {}", body_cost - spawn_energy, body_cost);
  } else {
    let name = memory.creep_name(creep_memory.tag());
    let body = design.scale(spawn_energy);
    match spawn.spawn_creep(&body, &name) {
      Ok(()) => {
        memory.initialize_creep(name, creep_memory);
      }
      Err(err) => {
        warn!("Spawn failed: {err:?}");
      }
    }
  }
}

pub fn spawn_loop(memory: &mut Memory) {
  debug!("did add spawn ext {}", HAS_ADDED_SPAWN_EXT_THIS_TICK.get());
  HAS_ADDED_SPAWN_EXT_THIS_TICK.set(false);
  for spawn in game::spawns().values() {
    debug!("running spawn {}", String::from(spawn.name()));
    let mem = memory.spawn_mut(spawn.id()).or_default();
    let room = spawn.room().unwrap();
    if !mem.initialized {
      initial_city_construction(&room);
      mem.initialized = true;
    }
    if spawn.spawning().is_none() {
      if let Some(creep_mem) = pick_next_creep(&room, memory) {
        spawn_creep(&spawn, creep_mem, memory);
      }
    }
  }
}
