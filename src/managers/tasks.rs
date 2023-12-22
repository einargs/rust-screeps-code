use minicbor::{Encode, Decode};
use log::*;
use screeps::local::ObjectId;
use screeps::objects::{RoomObject, Creep, Source, StructureController, ConstructionSite, StructureSpawn};

use crate::storage::cache;

// TODO: Metric for assessing the saturation of a source.

// TODO: system that dictates the development of the city.
// assess the current system, and declare priorities.

// TODO: allow me to build roads around harvest spots.
fn calc_harvest_spots(source: Source) -> u8 {
  use screeps::look::{self, LookResult};
  use screeps::Terrain;
  cache::calc_harvest_spots(source.id(), || {
    let room = match source.room() {
      None => {
        warn!("Could not get room for source {}", source.id());
        return 0
      },
      Some(room) => room
    };
    let pos = source.pos;
    room.look_at_area(pos.y + 1, pos.x -1, pos.y - 1, pos.x + 1)
      .filter(|res| (res.x != pos.x || res.y != pos.y) && match res {
        LookResult::Terrain(Terrain::Plain) => true,
        LookResult::Terrain(Terrain::Swamp) => true,
        _ => false,
      })
      .count()
  })
}

enum WorkTask {
  Build
}
