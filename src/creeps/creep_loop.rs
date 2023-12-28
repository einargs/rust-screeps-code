use super::role::Role;
use super::memory::{CreepMemory, RoleTag};
use crate::memory::Memory;

use log::*;
use screeps::prelude::*;
use screeps::{Room, game};

fn initial_creep_memory(room: &Room, tag: RoleTag) -> CreepMemory {
  todo!()
}

pub fn creep_loop(memory: &mut Memory) {
  for creep in game::creeps().values() {
    if creep.spawning() {
      continue;
    }
    let name = creep.name();
    debug!("running creep {}", name);
    if let Some(mem) = memory.creeps.get(&name) {
      let mut local = mem.clone();
      local.run(&creep, memory);
      memory.creeps.insert(name, local);
    } else {
      warn!("no memory for creep: {}", &name);
    }
  }
}
