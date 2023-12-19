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
use crate::storage::cbor;

pub trait Role {
  fn run(&mut self, creep: Creep) -> ();
}

// In the future I can cache a pathing matrix.

/// This is a convience wrapper for things a creep wants to find and go to
/// to accomplish some task.
#[derive(Debug, Encode, Decode)]
pub enum TargetResource<T> {
  #[n(0)] Searching,
  #[n(1)] TravelingTo(
    #[n(0)] #[cbor(with = "cbor::object_id")]
    ObjectId<T>
  ),
  #[n(2)] Arrived(
    #[n(0)] #[cbor(with = "cbor::object_id")]
    ObjectId<T>
  ),
}

impl<T> PartialEq for TargetResource<T> {
  fn eq(&self, other: &Self) -> bool {
    use TargetResource::*;
    match (self, other) {
      (Searching, Searching) => true,
      (TravelingTo(id1), TravelingTo(id2)) => id1 == id2,
      (Arrived(id1), Arrived(id2)) => id1 == id2,
      _ => false
    }
  }
}

impl<T> Default for TargetResource<T> {
  fn default() -> Self {
    Self::Searching
  }
}

fn mk_find<T: MaybeHasTypedId<T>>(
  find: impl FnOnce() -> Option<T>
) -> impl FnOnce() -> Option<(ObjectId<T>, T)> {
  || find().and_then(|object| object.try_id().map(|id| (id, object)))
}

impl<T: Resolvable> TargetResource<T> where T: AsRef<RoomObject> {
  pub fn with(
    &mut self,
    creep: &Creep,
    arrived: impl FnOnce(&T) -> bool,
    find: impl FnOnce() -> Option<(ObjectId<T>, T)>,
    handle: impl FnOnce(T) -> (),
  ) {
    use TargetResource::*;
    match self {
      Searching => {
        if let Some((new_id, object)) = find() {
          if arrived(&object) {
            *self = Arrived(new_id);
            handle(object);
          } else {
            *self = TravelingTo(new_id);
          }
        }
      }
      TravelingTo(id) => {
        let id = *id;
        // id.resolve() can fail if, for instance, something dies or is destroyed.
        // TODO: consider whether I should have some way for a depleted source to cause
        // people to retarget?
        let (new_id, object) = if let Some(object) = id.resolve() {
          (id, object)
        } else if let Some((new_id, object)) = find() {
          *self = TravelingTo(new_id);
          (new_id, object)
        } else {
          *self = Searching;
          return;
        };

        if arrived(&object) {
          *self = Arrived(new_id);
          handle(object);
        } else {
          creep.move_to(object);
        }
      }
      Arrived(id) => {
        let id = *id;
        let object = if let Some(object) = id.resolve() {
          object
        } else if let Some((new_id, object)) = find() {
          if arrived(&object) {
            object
          } else {
            *self = TravelingTo(new_id);
            creep.move_to(object);
            return;
          }
        } else {
          *self = Searching;
          return;
        };

        handle(object);
      }
    }
  }
}

impl TargetResource<Source> {
  fn harvest(&mut self, creep: &Creep) {
    self.with(
      creep,
      |source| creep.pos().is_near_to(source.pos()),
      mk_find(|| util::select_source(creep)),
      |source| {
        match creep.harvest(&source) {
          Err(e) => {
            warn!("Couldn't harvest: {:?}", e);
          }
          Ok(_) => (),
        }
      }
    )
  }
}

#[derive(PartialEq, Debug, Encode, Decode)]
pub enum HarvesterTarget {
  #[n(0)] Transfer(#[n(0)] TargetResource<StructureSpawn>),
  #[n(1)] Harvest(#[n(0)] TargetResource<Source>),
}

#[derive(PartialEq, Debug, Encode, Decode)]
pub struct Harvester {
  #[n(0)] pub target: HarvesterTarget
}

#[derive(PartialEq, Debug, Encode, Decode)]
pub enum Upgrader {
  #[n(0)] Upgrade(#[n(0)] TargetResource<StructureController>),
  #[n(1)] Harvest(#[n(0)] TargetResource<Source>),
}

#[derive(PartialEq, Debug, Encode, Decode)]
pub enum BuilderTarget {
  #[n(0)] Build(#[n(0)] TargetResource<ConstructionSite>),
  #[n(1)] Harvest(#[n(0)] TargetResource<Source>),
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
        self.target = HarvesterTarget::Harvest(TargetResource::Searching);
      }
      HarvesterTarget::Harvest(_) if store.get_free_capacity(Some(ResourceType::Energy)) == 0 => {
        self.target = HarvesterTarget::Transfer(TargetResource::Searching);
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
        self.target = Harvest(TargetResource::Searching);
      }
      Harvest(_) if store.get_free_capacity(Some(ResourceType::Energy)) == 0 => {
        self.target = Build(TargetResource::Searching);
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
        *self = Harvest(TargetResource::Searching);
      }
      Harvest(_) if store.get_free_capacity(Some(ResourceType::Energy)) == 0 => {
        *self = Upgrade(TargetResource::Searching);
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

macro_rules! gen_roles {
  ($($n:literal => $t:ident)*) => {
    #[derive(PartialEq, Eq, Debug, Copy, Clone, Encode, Decode)]
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

    #[derive(PartialEq, Debug, Encode, Decode)]
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
  0 => Harvester
  1 => Builder
  2 => Upgrader
}

impl RoleTag {
  pub fn next(self) -> Self {
    use RoleTag::*;
    match self {
      Harvester => Builder,
      Builder => Harvester,
      Upgrader => Harvester,
    }
  }
}
