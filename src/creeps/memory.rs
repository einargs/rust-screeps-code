use minicbor::{Encode, Decode};
use std::hash::Hash;
use screeps::Creep;
use super::role::Role;
use super::harvester::*;
use super::worker::*;
use super::early_worker::*;
use crate::memory::Memory;

macro_rules! gen_roles {
  ($($n:literal => $t:ident)*) => {
    #[derive(PartialEq, Eq, Hash, Debug, Copy, Clone, Encode, Decode)]
    #[cbor(index_only)]
    #[repr(u8)]
    pub enum RoleTag {
      $(
        #[n($n)] $t = $n
      ),*
    }

    impl Role for CreepMemory {
      fn run(&mut self, creep: &Creep, memory: &mut Memory) {
        match self {
          $(
            CreepMemory::$t(ref mut mem) => mem.run(creep, memory)
          ),*
        }
      }
    }

    #[derive(Clone, PartialEq, Debug, Encode, Decode)]
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

    $(
      impl From<$t> for CreepMemory {
        fn from(mem: $t) -> CreepMemory {
          CreepMemory::$t(mem)
        }
      }
    )*
  }
}

gen_roles! {
  0 => Harvester
  1 => Worker
  2 => EarlyWorker
}
