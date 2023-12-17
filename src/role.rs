use enum_dispatch::enum_dispatch;
use minicbor::{Encode, Decode};
use log::*;
use screeps::enums::StructureObject;
use screeps::local::ObjectId;
use screeps::objects::{Creep, Source, StructureController, ConstructionSite};
use screeps::prelude::*;
use screeps::constants::{ResourceType, ErrorCode};

use crate::util;
use crate::storage::cbor;

pub trait Role {
  fn run(&mut self, creep: Creep) -> ();
}

#[derive(Debug, Encode, Decode)]
pub enum HarvesterTarget {
  #[n(0)] Upgrade(
    #[n(0)] #[cbor(with = "cbor::object_id")]
    ObjectId<StructureController>
  ),
  #[n(1)] Harvest(
    #[n(0)] #[cbor(with = "cbor::object_id")]
    ObjectId<Source>
  ),
}

#[derive(Debug, Encode, Decode)]
pub struct Harvester {
  #[n(0)] pub target: HarvesterTarget
}

#[derive(Debug, Encode, Decode)]
pub enum BuilderTarget {
  #[n(0)] Upgrade(#[n(0)] #[cbor(with = "cbor::object_id")]
                  ObjectId<ConstructionSite>),
  #[n(1)] Harvest(#[n(0)] #[cbor(with = "cbor::object_id")]
                  ObjectId<Source>),
}

#[derive(Debug, Encode, Decode)]
pub struct Builder {
  #[n(0)] pub target: BuilderTarget
}

impl Role for Harvester {
  fn run(&mut self, creep: Creep) {
    info!("running harvester {}!", creep.name());
    let room = creep.room().expect("creep not in room");
    let has_energy = creep.store().get_used_capacity(Some(ResourceType::Energy)) > 0;
    match self.target {
      HarvesterTarget::Upgrade(ref controller_id) => {
        if has_energy {
          let controller = controller_id.resolve().expect("controller didn't exist");
          match creep.upgrade_controller(&controller) {
            Err(ErrorCode::NotInRange) => {
              creep.move_to(&controller);
            }
            Err(e) => {
              warn!("Couldn't upgrade {:?}", e);
            }
            Ok(_) => (),
          }
        } else {
          let source = util::select_source(&room);
          self.target = HarvesterTarget::Harvest(source.id());
        }
      },
      HarvesterTarget::Harvest(ref source_id) => {
        if has_energy {
          let source = source_id.resolve().expect("source didn't exist");
          if creep.pos().is_near_to(source.pos()) {
            match creep.harvest(&source) {
              Err(e) => {
                warn!("Couldn't harvest: {:?}", e);
              }
              Ok(_) => (),
            }
          } else {
            creep.move_to(&source);
          }
        } else {
          let controller = room.controller().expect("room doesn't have controller");
          self.target = HarvesterTarget::Upgrade(controller.id());
        }
      }
    }
  }
}

impl Role for Builder {
  fn run(&mut self, creep: Creep) {
    info!("running builder {}!", creep.name());
  }
}

macro_rules! gen_roles {
  ($($n:literal => $t:ident),*) => {
    #[derive(Debug, Copy, Clone, Encode, Decode)]
    #[cbor(index_only)]
    #[repr(u8)]
    pub enum RoleTag {
      $(
        #[n($n)] $t = $n
      ),*
    }

    impl Role for CreepMemory {
      fn run(&mut self, creep: Creep) {
        match self {
          $(
            CreepMemory::$t(ref mut mem) => mem.run(creep)
          ),*
        }
      }
    }

    #[derive(Debug, Encode, Decode)]
    pub enum CreepMemory {
      $(
        #[n($n)] $t(#[n(0)] $t)
      ),*
    }

    impl CreepMemory {
      pub fn tag(&self) -> RoleTag {
        match self {
          $(
            CreepMemory::$t(_) => RoleTag::$t
          ),*
        }
      }
    }
  }
}

gen_roles! {
  0 => Harvester,
  1 => Builder
}

impl RoleTag {
  pub fn next(self) -> Self {
    use RoleTag::*;
    match self {
      Harvester => Builder,
      Builder => Harvester,
    }
  }
}
