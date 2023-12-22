use minicbor::{Encode, Decode};
use screeps::Creep;
use super::role::*;

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
