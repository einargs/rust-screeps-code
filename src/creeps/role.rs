use enum_dispatch::enum_dispatch;
use minicbor::{Encode, Decode};
use log::*;
use screeps::enums::StructureObject;
use screeps::local::ObjectId;
use screeps::objects::{RoomObject, Creep, Source, StructureController, ConstructionSite, StructureSpawn};
use screeps::prelude::*;
use screeps::find;
use screeps::traits::{Resolvable, HasTypedId};
use screeps::constants::{ResourceType, ErrorCode};

use crate::util;
use crate::memory::Memory;

pub trait Role: Clone {
  /// Run the creep actions for this tick.
  ///
  /// To avoid borrowing the creep's memory twice, we just clone the self at
  /// the start and then assign it back at the end.
  fn run(&mut self, creep: &Creep, memory: &mut Memory);
}

/*
use crate::storage::cbor;
use super::target_object::TargetObject;
use super::target_object::mk_find;

#[derive(PartialEq, Debug, Encode, Decode)]
pub enum HarvesterTarget {
  #[n(0)] Transfer(#[n(0)] TargetObject<StructureSpawn>),
  #[n(1)] Harvest(#[n(0)] TargetObject<Source>),
}

#[derive(PartialEq, Debug, Encode, Decode)]
pub struct Harvester {
  #[n(0)] pub target: HarvesterTarget
}

#[derive(PartialEq, Debug, Encode, Decode)]
pub enum Upgrader {
  #[n(0)] Upgrade(#[n(0)] TargetObject<StructureController>),
  #[n(1)] Harvest(#[n(0)] TargetObject<Source>),
}

#[derive(PartialEq, Debug, Encode, Decode)]
pub enum BuilderTarget {
  #[n(0)] Build(#[n(0)] TargetObject<ConstructionSite>),
  #[n(1)] Harvest(#[n(0)] TargetObject<Source>),
}

#[derive(PartialEq, Debug, Encode, Decode)]
pub struct Builder {
  #[n(0)] pub target: BuilderTarget
}

impl Role for Harvester {
  fn run(&mut self, creep: Creep) {
    let room = creep.room().expect("creep not in room");
    let store = creep.store();
    // first we check if we've reached the end conditions for any of our states
    match self.target {
      HarvesterTarget::Transfer(_) if store.get_used_capacity(Some(ResourceType::Energy)) == 0 => {
        self.target = HarvesterTarget::Harvest(TargetObject::Searching);
      }
      HarvesterTarget::Harvest(_) if store.get_free_capacity(Some(ResourceType::Energy)) == 0 => {
        self.target = HarvesterTarget::Transfer(TargetObject::Searching);
      }
      _ => (),
    }
    match self.target {
      HarvesterTarget::Transfer(ref mut target) => {
        target.with(
          &creep,
          |spawn| creep.pos().is_near_to(spawn.pos()),
          mk_find(|| creep.pos().find_closest_by_range(find::MY_SPAWNS)),
          |spawn| {
            match creep.transfer(&spawn, ResourceType::Energy, None) {
              Ok(()) => (),
              Err(e) => warn!("Couldn't upgrade controller {e:?}"),
            }
          }
        )
      },
      HarvesterTarget::Harvest(ref mut target) => {
        target.harvest(&creep);
      }
    }
  }
}

impl Role for Builder {
  fn run(&mut self, creep: Creep) {
    use BuilderTarget::*;
    let room = creep.room().expect("creep not in room");
    let store = creep.store();
    // first we check if we've reached the end conditions for any of our states
    match self.target {
      Build(_) if store.get_used_capacity(Some(ResourceType::Energy)) == 0 => {
        self.target = Harvest(TargetObject::Searching);
      }
      Harvest(_) if store.get_free_capacity(Some(ResourceType::Energy)) == 0 => {
        self.target = Build(TargetObject::Searching);
      }
      _ => (),
    }
    match self.target {
      Build(ref mut target) => {
        target.with(
          &creep,
          |con_site| creep.pos().in_range_to(con_site.pos(), 3),
          mk_find(|| creep.pos().find_closest_by_range(find::MY_CONSTRUCTION_SITES)),
          |con_site| {
            match creep.build(&con_site) {
              Ok(()) => (),
              Err(e) => warn!("Couldn't build construction site {e:?}"),
            }
          }
        )
      },
      Harvest(ref mut target) => {
        target.harvest(&creep);
      }
    }
  }
}

impl Role for Upgrader {
  fn run(&mut self, creep: Creep) {
    use Upgrader::*;
    let room = creep.room().expect("creep not in room");
    let store = creep.store();
    // first we check if we've reached the end conditions for any of our states
    match self {
      Upgrade(_) if store.get_used_capacity(Some(ResourceType::Energy)) == 0 => {
        *self = Harvest(TargetObject::Searching);
      }
      Harvest(_) if store.get_free_capacity(Some(ResourceType::Energy)) == 0 => {
        *self = Upgrade(TargetObject::Searching);
      }
      _ => (),
    }
    match self {
      Upgrade(ref mut target) => {
        target.with(
          &creep,
          |controller| creep.pos().in_range_to(controller.pos(), 3),
          mk_find(|| room.controller()),
          |controller| {
            match creep.upgrade_controller(&controller) {
              Ok(()) => (),
              Err(e) => warn!("Couldn't upgrade controller {e:?}"),
            }
          }
        )
      },
      Harvest(ref mut target) => {
        target.harvest(&creep);
      }
    }
  }
}
*/
