use crate::role::{Role, RoleTag, CreepMemory};
use crate::memory::Memory;

fn initial_creep_memory(room: &Room, tag: RoleTag) -> CreepMemory {
  use role::*;
  match tag {
    RoleTag::Harvester => CreepMemory::Harvester(Harvester {
      target: HarvesterTarget::Harvest(Default::default())
    }),
    RoleTag::Upgrader => CreepMemory::Upgrader(
      Upgrader::Harvest(Default::default()),
    ),
    RoleTag::Builder => CreepMemory::Builder(Builder {
      target: BuilderTarget::Harvest(TargetResource::default())
    }),
  }
}

pub fn creep_loop(memory: &mut Memory) {
  for creep in game::creeps().values() {
    if creep.spawning() {
      continue;
    }
    let name = creep.name();
    debug!("running creep {}", name);
    if let Some(mem) = memory.creep_mut(&name) {
      mem.run(creep);
    } else {
      warn!("no memory for creep: {}", &name);
    }
  }
}
