use minicbor::{Encode, Decode};
use log::*;
use screeps::constants::{ResourceType, ErrorCode};
use screeps::{
  find, Creep, Source, StructureController, ConstructionSite,
  StructureSpawn, Position, StructureObject,
  Structure, StructureType, RoomObject, Store, Room, prelude::*, ObjectId,
};

use crate::{log_warn, util};
use crate::memory::Memory;
use crate::storage::cbor;
use crate::util::{energy_full, energy_empty, move_to_do, filter_closest_by_range, filter_map_closest_by_range};
use super::energy_sink::EnergySink;
use super::role::Role;
use super::memory::RoleTag;
use crate::managers::city::{current_role_count, place_spawn_extension};

#[derive(Clone, PartialEq, Debug, Encode, Decode)]
pub enum EarlyWorker {
  #[n(0)] Idle,
  #[n(1)] Transfer(
    #[n(0)] #[cbor(with = "cbor::object_id")]
    ObjectId<EnergySink>),
  #[n(2)] Upgrade(
    #[n(0)] #[cbor(with = "cbor::object_id")]
    ObjectId<StructureController>),
  #[n(3)] Build(
    #[n(0)] #[cbor(with = "cbor::object_id")]
    ObjectId<ConstructionSite>),
  #[n(4)] Harvest(
    #[n(0)] #[cbor(with = "cbor::object_id")]
    ObjectId<Source>),
}

#[inline]
fn mk_transfer(creep: &Creep) -> EarlyWorker {
  let sink = filter_map_closest_by_range(
    find::MY_STRUCTURES, creep.pos(), |structure| {
      let sink = match structure {
        StructureObject::StructureSpawn(spawn) => Some(EnergySink::from(spawn)),
        StructureObject::StructureExtension(ext) => Some(EnergySink::from(ext)),
        _ => None,
      };
      sink.filter(|sink| !energy_full(sink))
    })
    .expect("Expect early worker to be in room with spawn");
  EarlyWorker::Transfer(sink.id())
}

#[inline]
fn build_ext(creep: &Creep) -> EarlyWorker {
  let site_opt = util::filter_closest_by_range(
    find::MY_CONSTRUCTION_SITES, creep.pos(), |site| {
      site.structure_type() == StructureType::Extension
    });
  debug!("found site {}", site_opt.is_some());

  match site_opt {
    None => {
      place_spawn_extension(&creep.room().unwrap());
      EarlyWorker::Idle
    }
    Some(site) => {
      site.try_id().map_or(EarlyWorker::Idle, EarlyWorker::Build)
    }
  }
}

impl EarlyWorker {
  fn refuel(&mut self, creep: &Creep) {
    if let Some(source) = creep.pos().find_closest_by_path(find::SOURCES_ACTIVE, None) {
      *self = EarlyWorker::Harvest(source.id());
    }
  }

  fn get_task(&mut self, creep: &Creep, memory: &mut Memory) {
    let room = creep.room().unwrap();
    let controller = room.controller().unwrap();
    let level = controller.level();
    let early_worker_count = current_role_count(&room, memory, RoleTag::EarlyWorker);
    let spawn_ext_num = room.find(find::MY_STRUCTURES, None)
        .into_iter()
        .filter(|s| matches!(s, StructureObject::StructureExtension(_)))
        .count();

    *self = if early_worker_count < 3 {
      mk_transfer(creep)
    } else if level < 2 {
      EarlyWorker::Upgrade(controller.id())
    } else if spawn_ext_num < 2 {
      build_ext(creep)
    } else if early_worker_count < 5 {
      mk_transfer(creep)
    } else if level < 3 {
      EarlyWorker::Upgrade(controller.id())
    } else if early_worker_count < 10 {
      mk_transfer(creep)
    } else if spawn_ext_num < 8 {
      build_ext(creep)
    } else {
      mk_transfer(creep)
    };
  }
}

impl Role for EarlyWorker {
  fn run(&mut self, creep: &Creep, memory: &mut Memory) {
    use EarlyWorker::*;
    match self {
      Idle => if energy_empty(creep) {
        self.refuel(creep);
      } else {
        self.get_task(creep, memory);
      },
      Transfer(_) if energy_empty(creep) => self.refuel(creep),
      Build(_) if energy_empty(creep) => self.refuel(creep),
      Upgrade(_) if energy_empty(creep) => self.refuel(creep),
      Harvest(_) if energy_full(creep) => self.get_task(creep, memory),
      _ => (),
    }

    // TODO: extract common code for e.g. harvesting, etc into one file
    // to allow for better error messages, etc, to be shared.
    match self {
      // do nothing
      Idle => (),
      Harvest(id) => {
        let Some(source) = id.resolve() else {
          *self = Idle;
          self.run(creep, memory);
          return
        };
        move_to_do(creep, &source, 1, || {
          // TODO: will probably want to handle the error where the source doesn't have
          // enough.
          log_warn!(creep.harvest(&source),
                    err => "Could not harvest energy from source {err:?}");
        });
      }
      Upgrade(id) => {
        let Some(controller) = id.resolve() else {
          *self = Idle;
          self.run(creep, memory);
          return
        };
        move_to_do(creep, &controller, 1, || {
          log_warn!(creep.upgrade_controller(&controller),
                    err => "Creep could not upgrade controller: {err:?}");
        });
      }
      Build(id) => {
        let Some(site) = id.resolve() else {
          // construction site must have finished
          *self = Idle;
          self.run(creep, memory);
          return
        };
        move_to_do(creep, &site, 1, || {
          log_warn!(creep.build(&site),
                    err => "Creep could not upgrade controller: {err:?}");
        });
      }
      Transfer(id) => {
        let Some(spawn) = id.resolve() else {
          *self = Idle;
          self.run(creep, memory);
          return
        };
        move_to_do(creep, &spawn, 1, || {
          match creep.transfer(&spawn, ResourceType::Energy, None) {
            Ok(()) => (),
            // if it's become full while we are depositing, go somewhere else.
            Err(ErrorCode::Full) => {
              *self = Idle;
              self.run(creep, memory);
            }
            Err(err) => warn!("Could not deposit energy at spawn because: {err:?}"),
          }
        });
      }
    }
  }
}
