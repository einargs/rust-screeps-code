use minicbor::{Encode, Decode};
use screeps::enums::StructureObject;
use screeps::local::ObjectId;
use screeps::objects::{RoomObject, Creep, Source, StructureController, ConstructionSite, StructureSpawn};
use screeps::prelude::*;
use log::*;
use screeps::find;
use screeps::traits::{Resolvable, HasTypedId};
use screeps::constants::{ResourceType, ErrorCode};

use crate::util;
use crate::storage::cbor;

// TODO: In the future I can cache a pathing matrix.

/// This is a convience wrapper for things a creep wants to find and go to
/// to accomplish some task.
#[derive(Debug, Encode, Decode)]
pub enum TargetObject<T> {
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

impl<T> PartialEq for TargetObject<T> {
  fn eq(&self, other: &Self) -> bool {
    use TargetObject::*;
    match (self, other) {
      (Searching, Searching) => true,
      (TravelingTo(id1), TravelingTo(id2)) => id1 == id2,
      (Arrived(id1), Arrived(id2)) => id1 == id2,
      _ => false
    }
  }
}

impl<T> Default for TargetObject<T> {
  fn default() -> Self {
    Self::Searching
  }
}

pub fn mk_find<T: MaybeHasTypedId<T>>(
  find: impl FnOnce() -> Option<T>
) -> impl FnOnce() -> Option<(ObjectId<T>, T)> {
  || find().and_then(|object| object.try_id().map(|id| (id, object)))
}

impl<T: Resolvable> TargetObject<T> where T: AsRef<RoomObject> {
  pub fn with(
    &mut self,
    creep: &Creep,
    arrived: impl FnOnce(&T) -> bool,
    find: impl FnOnce() -> Option<(ObjectId<T>, T)>,
    handle: impl FnOnce(T) -> (),
  ) {
    use TargetObject::*;
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

impl TargetObject<Source> {
  pub fn harvest(&mut self, creep: &Creep) {
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
