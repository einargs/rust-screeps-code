use minicbor::{Encode, Decode};
use log::*;
use screeps::pathfinder::SingleRoomCostResult;
use std::assert_matches::assert_matches;
use screeps::enums::StructureObject;
use screeps::local::ObjectId;
use screeps::{prelude::*, RawObjectId};
use screeps::{
  find, TransferableObject, Creep, Source, StructureController, ConstructionSite,
  StructureSpawn, StoreObject, Position, StructureContainer, StructureLink,
  Structure, StructureType, RoomObject, Store, pathfinder, Room, Path,
};
use screeps::traits::{Resolvable, HasTypedId};
use screeps::constants::{ResourceType, ErrorCode};
use wasm_bindgen::prelude::*;

use crate::storage::cbor;
use crate::memory::Memory;
use crate::util::{path_len, energy_full, energy_empty, move_to_do};
use super::role::Role;
use super::energy_sink::*;
use crate::log_warn;

#[wasm_bindgen]
extern "C" {
  /// Object representing something that a worker can get energy from.
  ///
  /// Currently should only be a container or link.
  #[wasm_bindgen(extends = RoomObject, extends = Structure)]
  #[derive(Clone, Debug)]
  pub type EnergySupplier;

  /// The [`Store`] of the extension, which contains information about the
  /// amount of energy in it.
  ///
  /// [Screeps documentation](https://docs.screeps.com/api/#StructureLink.store)
  #[wasm_bindgen(method, getter)]
  pub fn store(this: &EnergySupplier) -> Store;
}

impl Transferable for EnergySupplier {}
impl Withdrawable for EnergySupplier {}

impl HasStore for EnergySupplier {
  fn store(&self) -> Store {
    Self::store(self)
  }
}

impl From<StructureContainer> for EnergySupplier {
  fn from(value: StructureContainer) -> Self {
    JsValue::from(value).into()
  }
}

impl From<StructureLink> for EnergySupplier {
  fn from(value: StructureLink) -> Self {
    JsValue::from(value).into()
  }
}

#[derive(Clone, PartialEq, Debug, Encode, Decode)]
pub enum Worker {
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
  #[n(4)] TakeFrom(
    #[n(0)] #[cbor(with = "cbor::object_id")]
    ObjectId<EnergySupplier>),
}

/// Get which store the place should travel to to resupply on energy.
fn get_nearest_energy_supplier(room: &Room, pos: Position) -> Option<EnergySupplier> {
  use StructureObject::*;
  use pathfinder::SingleRoomCostResult;
  use screeps::{CostMatrix, RoomName};
  room.find(find::MY_STRUCTURES, None)
    .into_iter()
    .filter_map(|structure| match structure {
      StructureContainer(cont) => Some(EnergySupplier::from(cont)),
      StructureLink(link) => Some(EnergySupplier::from(link)),
      _ => None,
    })
    .min_by_key(|supplier| path_len(&pos.find_path_to
                                    ::<EnergySupplier,
                                       fn(RoomName, CostMatrix) -> SingleRoomCostResult,
                                       SingleRoomCostResult
                                       >(supplier, None)))
}

impl Worker {
  #[inline]
  fn refuel(&mut self, creep: &Creep) {
    let room = creep.room().unwrap();
    if let Some(supplier) = get_nearest_energy_supplier(&room, creep.pos()) {
      *self = Worker::TakeFrom(supplier.id());
    } else {
      *self = Worker::Idle;
    }
  }

  /// Since we have full energy find something to do.
  fn get_task(&mut self, creep: &Creep, memory: &mut Memory) {
    use Worker::*;
    use js_sys::Math::random;
    // TODO: do we even need to transfer energy to spawns? Can they just draw energy
    // from anything in the room?
    let room = creep.room().unwrap();
    let non_full_spawn = room.find(find::MY_SPAWNS, None)
      .pop()
      .filter(|spawn| spawn.store().get_free_capacity(Some(ResourceType::Energy)) != 0);

    let do_spawn: bool = random() > 0.5;

    match non_full_spawn {
      Some(spawn) if do_spawn => {
        *self = Transfer(EnergySink::from(spawn.clone()).id());
      }
      _ => {
        let site_id = creep.pos()
          .find_closest_by_range(find::MY_CONSTRUCTION_SITES)
          .and_then(|site| site.try_id());

        *self = match site_id {
          Some(site) => Build(site),
          None => {
            let Some(controller) = room.controller() else {
              warn!("Worker {:?} in room without controller", creep.try_id());
              return
            };
            Upgrade(controller.id())
          }
        };
      }
    }
  }
}

impl Role for Worker {
  fn run(&mut self, creep: &Creep, memory: &mut Memory) {
    use Worker::*;
    match self {
      Idle => if energy_empty(creep) {
        self.refuel(creep);
      } else {
        self.get_task(creep, memory);
      },
      Transfer(_) if energy_empty(creep) => self.refuel(creep),
      Build(_) if energy_empty(creep) => self.refuel(creep),
      Upgrade(_) if energy_empty(creep) => self.refuel(creep),
      TakeFrom(_) if energy_full(creep) => self.get_task(creep, memory),
      _ => (),
    }

    match self {
      // do nothing
      Idle => (),
      TakeFrom(id) => {
        let Some(supplier) = id.resolve() else {
          *self = Idle;
          self.run(creep, memory);
          return
        };
        move_to_do(creep, &supplier, 1, || {
          match creep.withdraw(&supplier, ResourceType::Energy, None) {
            Ok(()) => (),
            Err(ErrorCode::NotEnough) => {
              *self = Idle;
              self.run(creep, memory);
            }
            Err(err) => warn!("Could not withdraw energy from {err:?}"),
          }
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
        let Some(sink) = id.resolve() else {
          *self = Idle;
          self.run(creep, memory);
          return
        };
        move_to_do(creep, &sink, 1, || {
          creep.transfer(&sink, ResourceType::Energy, None);
        });
      }
    }
  }
}
