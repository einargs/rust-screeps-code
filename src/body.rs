use screeps::constants::{Part, MAX_CREEP_SIZE};
use std::iter::Iterator;

macro_rules! gen_body_design {
  ($($p:ident => $m:ident $u:ident)*) => {
    pub struct BodyDesign {
      $(
        pub $p : u8
      ),*
    }
    impl Default for BodyDesign {
      fn default() -> Self {
        BodyDesign {
          $($p: 0),*
        }
      }
    }
    impl BodyDesign {
      pub fn new() -> Self {
        Default::default()
      }

      $(
        pub fn $m(mut self, val: u8) -> Self {
          self.$p = val;
          self
        }
      )*

      pub fn size(&self) -> u8 {
        0 $(+ self.$p)*
      }

      pub fn scale_to_iter(&self, scale: u32) -> impl Iterator<Item = Part> {
        use casey::pascal;
        [$(
          std::iter::repeat(Part::$u).take((scale * (self.$p as u32)) as usize)
        ),*].into_iter().flatten()
      }

      pub fn base_cost(&self) -> u32 {
        use casey::pascal;
        0 $(+ (self.$p as u32) * Part::$u.cost())*
      }

      fn scale_factor(&self, max_energy: u32) -> u32 {
        use std::cmp::{min, max};
        let baseCost = self.base_cost();
        let baseScale = min(max_energy / baseCost, MAX_CREEP_SIZE / baseCost);
        max(1, baseScale)
      }

      pub fn max_cost(&self, max_energy: u32) -> u32 {
        self.base_cost() * self.scale_factor(max_energy)
      }

      pub fn scale(&self, max_energy: u32) -> Vec<Part> {
        let scale = self.scale_factor(max_energy);
        self.scale_to_iter(scale).collect()
      }
    }
  }
}

gen_body_design! {
  work_count => work Work
  move_count => r#move Move
  carry_count => carry Carry
  attack_count => attack Attack
  ranged_attack_count => ranged_attack RangedAttack
  heal_count => heal Heal
  claim_count => claim Claim
  tough_count => tough Tough
}
