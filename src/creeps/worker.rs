use minicbor::{Encode, Decode};
use log::*;
use screeps::enums::StructureObject;
use screeps::local::ObjectId;
use screeps::objects::{RoomObject, Creep, Source, StructureController, ConstructionSite, StructureSpawn};
use screeps::prelude::*;
use screeps::find;
use screeps::traits::{Resolvable, HasTypedId};
use screeps::constants::{ResourceType, ErrorCode};

use super::role::Role;
use super::target_object::TargetObject;

// develop a system to

#[derive(Encode, Decode)]
pub enum Worker {
  #[n(0)] Idle,
  #[n(1)] Transfer(#[n(0)] TargetObject<StructureSpawn>),
  #[n(2)] Upgrade(#[n(0)] TargetObject<StructureController>),
  #[n(3)] Build(#[n(0)] TargetObject<ConstructionSite>),
  #[n(4)] Harvest(#[n(0)] TargetObject<Source>),
}

impl Role for Worker {
  fn run(&mut self, creep: Creep) {
    match self {

    }
  }
}
