use minicbor::{Encode, Decode};
use log::*;
use screeps::local::ObjectId;
use screeps::look::{PositionedLookResult, self, LookResult};
use screeps::{
  prelude::*, RoomXY, find, Position, StructureContainer, StructureLink,
  Room, RoomObject, Creep, Source, StructureController, ConstructionSite,
  StructureSpawn, Terrain, StructureObject, StructureType, StoreObject,
  HasTypedId, HasNativeId, HasId, Resolvable, RoomCoordinate, Direction, StructureKeeperLair,
};
use screeps::constants::{ResourceType, ErrorCode};
use screeps::Path;

use super::role::Role;
use crate::log_warn;
use crate::creeps::CreepMemory;
use crate::memory::Memory;
use crate::storage::cache;
use crate::util::{self, energy_empty, energy_full, move_to_do, PrettyId};
use crate::storage::cbor;

use crate::mk_cache;

mk_cache! {
  source_harvest_spots lifetime 150 by ObjectId<Source> => u8
}

// TODO: Metric for assessing the saturation of a source.

// TODO: system that dictates the development of the city.
// assess the current system, and declare priorities.

fn is_at_pos(result: &PositionedLookResult, pos: Position) -> bool {
  pos.xy() == RoomXY::try_from((result.x, result.y)).unwrap()
}

fn open_terrain_around(room: &Room, xy: RoomXY) -> u8 {
  use enum_iterator::all;
  let terrain = room.get_terrain();
  all::<Direction>()
    .filter_map(|dir| xy.checked_add_direction(dir))
    .filter(|xy| terrain.get(xy.x.into(), xy.y.into()) != Terrain::Wall)
    .count() as u8
}

// TODO: allow me to build roads around harvest spots.
pub fn calc_harvest_spots(source: &Source) -> u8 {
  use screeps::look::{self, LookResult};
  use screeps::Terrain;
  source_harvest_spots::caches(&source.id(), |_| {
    let room = match source.room() {
      None => {
        warn!("Could not get room for source {}", source.id());
        return 0
      },
      Some(room) => room
    };
    open_terrain_around(&room, source.pos().xy())
  })
}

mk_cache! {
  source_num_harvesters lifetime 30 by ObjectId<Source> => u8
}

pub fn num_assigned_harvesters(source: &Source, memory: &Memory) -> u8 {
  source_num_harvesters::caches(&source.id(), |_| {
    let room = match source.room() {
      None => {
        warn!("Could not get room for source {}", source.id());
        return 0
      },
      Some(room) => room
    };
    let id = source.id();
    room.find(find::MY_CREEPS, None)
      .into_iter()
      .filter(|creep| matches!(
        memory.creep(&creep),
        Some(CreepMemory::Harvester(mem)) if mem.source == id
      ))
      .count() as u8
  })
}

/// Used to tell us that we have assigned a new harvester to a source or transfered
/// one to or from it, and the number assigned will need to be recalculated.
pub fn harvester_assignments_changed_for(source: &Source) {
  source_num_harvesters::invalidate_now(&source.id())
}

// TODO: in the future, upgrade to use StoreObject instead of just
// structure link and container. Or maybe don't if I'm dead set on
// only using those two.

#[derive(Clone, Debug)]
pub enum HarvestStorageId {
  Link(ObjectId<StructureLink>),
  Container(ObjectId<StructureContainer>),
  Build(ObjectId<ConstructionSite>),
}

#[derive(Clone, Debug)]
pub enum HarvestStorage {
  Link(StructureLink),
  Container(StructureContainer),
  /// If we can't find a container, then we'll look for a construction site.
  Build(ConstructionSite),
}

impl HarvestStorage {
  fn id(&self) -> HarvestStorageId {
    match self {
      HarvestStorage::Link(bldg) =>
        HarvestStorageId::Link(bldg.id()),
      HarvestStorage::Container(bldg) =>
        HarvestStorageId::Container(bldg.id()),
      HarvestStorage::Build(bldg) =>
        // TODO: handle gracefully
        HarvestStorageId::Build(bldg.try_id().expect("construction site has no id")),
    }
  }
}

impl HarvestStorageId {
  fn resolve(&self) -> Option<HarvestStorage> {
    match self {
      HarvestStorageId::Link(id) =>
        id.resolve().map(HarvestStorage::Link),
      HarvestStorageId::Container(id) =>
        id.resolve().map(HarvestStorage::Container),
      HarvestStorageId::Build(id) =>
        id.resolve().map(HarvestStorage::Build),
    }
  }
}

impl HasPosition for HarvestStorage {
  fn pos(&self) -> Position {
    match self {
      HarvestStorage::Link(bldg) => bldg.pos(),
      HarvestStorage::Container(bldg) => bldg.pos(),
      HarvestStorage::Build(bldg) => bldg.pos(),
    }
  }
}

mk_cache! {
  source_nearby_storage lifetime 31 by ObjectId<Source> => Option<HarvestStorageId>
}

fn raw_nearby_storage(source: &Source) -> Option<HarvestStorageId> {
  let pos = source.pos();
  let struct_iter = pos.find_in_range(find::STRUCTURES, 3)
    .into_iter()
    .filter_map(|building| match building {
      StructureObject::StructureLink(link) =>
        Some(HarvestStorage::Link(link)),
      StructureObject::StructureContainer(cont) =>
        Some(HarvestStorage::Container(cont)),
      _ => None
    });
  let site_iter = pos.find_in_range(find::CONSTRUCTION_SITES, 3)
    .into_iter()
    .map(|site| HarvestStorage::Build(site));
  struct_iter.chain(site_iter)
    .min_by_key(|bldg| bldg.pos().get_range_to(pos))
    .map(|bldg| bldg.id())
}

fn nearby_storage(source: &Source) -> Option<HarvestStorage> {
  source_nearby_storage::caches(&source.id(), |_| {
    raw_nearby_storage(source)
  }).and_then(|id| id.resolve())
}

/// Inform that the storage where energy for a given source should be deposited
/// has been changed and that it will take effect next tick.
pub fn have_updated_source_storage(source: &Source) {
  source_nearby_storage::invalidate_next_tick(&source.id());
}

mk_cache! {
  source_keeper_cache lifetime 1000 by ObjectId<Source> => Option<ObjectId<StructureKeeperLair>>
}

pub fn lair_for_source(source: &Source) -> Option<ObjectId<StructureKeeperLair>> {
  source_keeper_cache::caches(&source.id(), |_| {
    let pos = source.pos();
    let lair = pos.find_in_range(find::HOSTILE_STRUCTURES, 5)
      .into_iter()
      .filter_map(|structure| match structure {
        StructureObject::StructureKeeperLair(lair) => Some(lair.id()),
        _ => None,
      })
      .nth(0);
    lair
  })
}

#[derive(Clone, PartialEq, Debug, Encode, Decode)]
pub enum HarvesterState {
  #[n(0)] Harvesting,
  #[n(1)] Depositing
}

#[derive(Clone, PartialEq, Debug, Encode, Decode)]
pub struct Harvester {
  #[n(0)] pub state: HarvesterState,
  #[n(1)] #[cbor(with = "cbor::object_id")]
  pub source: ObjectId<Source>
}

/// This only works within the room.
fn find_source_needing_harvester(room: &Room) -> Option<Source> {
  // TODO: plan out infrastructure to allow this to work across all rooms.
  room.find(find::SOURCES_ACTIVE, None)
    .into_iter()
    .filter(|source| !util::are_hostiles_near(room, source.pos(), 5))
    .next()
}

// TODO: in the manager code, implement whatever will assign a harvester to a source
// and keep that synched. Wait shit that doesn't work, I do need to cache it, because
// creeps die. Okay, so I'll just invalidate that cache every time a harvester is transfered.

/*
mk_cache! {
  harvester_path lifetime 15 by ObjectId<Creep> => Path
}
*/

impl Harvester {
  /// Create a new harvester for this source.
  pub fn new(source: &Source) -> Self {
    Harvester {
      state: HarvesterState::Harvesting,
      source: source.id(),
    }
  }
}

impl Role for Harvester {
  fn run(&mut self, creep: &Creep, memory: &mut Memory) {
    use HarvesterState::*;
    match &self.state {
      Deposit if energy_empty(creep) => {
        self.state = Harvesting;
      }
      Harvesting if energy_full(creep) => {
        self.state = Depositing;
      }
      _ => ()
    }

    // this shouldn't be possible but we'll be careful I guess.
    let Some(source) = self.source.resolve() else {
      error!("source id for harvester {:?} did not resolve", creep.try_id());
      return
    };

    let creep_pos = creep.pos();

    match &self.state {
      Harvesting => {
        move_to_do(creep, &source, 1, || {
          log_warn!(creep.harvest(&source), err =>
                    "Harvester {} couldn't harvest because: {err:?}", creep.id_str()
          )
        })
      }
      Deposit => {
        // TODO: handle full storage.
        match nearby_storage(&source) {
          // TODO: do we need to periodically repair the container? I think so.
          Some(HarvestStorage::Container(cont)) => {
            move_to_do(creep, &cont, 1, || {
              if cont.hits_max() / cont.hits() >= 2 {
                debug!("Repairing container bc max hits {} hits {}", cont.hits_max(), cont.hits());
                log_warn!(creep.repair(&cont), err =>
                          "Harvester {} failed repair because: {err:?}", creep.id_str());

              } else {
                log_warn!(
                  creep.transfer(&cont, ResourceType::Energy, None), err =>
                    "Harvester {:?} couldn't transfer because: {err:?}", creep.id_str()
                )
              }if let Err(e) = creep.transfer(&cont, ResourceType::Energy, None) {
              }
            })
          }
          Some(HarvestStorage::Link(link)) => {
            move_to_do(creep, &link, 1, || {
              log_warn!(
                creep.transfer(&link, ResourceType::Energy, None), err =>
                  "Harvester {} couldn't transfer because: {err:?}", creep.id_str()
              );
            })
          }
          Some(HarvestStorage::Build(site)) => {
            move_to_do(creep, &site, 3, || {
              log_warn!(
                creep.build(&site), err =>
                  "Harvester {} couldn't transfer because: {err:?}", creep.id_str()
              );
            })
          }
          None => {
            warn!("No construction site for storage placed for source {}", source.id_str());
          }
        }
      }
    }
  }
}
