use std::mem::MaybeUninit;

use screeps::RoomXY;

use super::tile_slice::*;
use super::dist_transform::DistMatrix;

#[derive(Copy, Clone, Debug)]
struct Meta {
  /// Index of the maxima.
  maxima: u16,
  /// The rank of this tree.
  rank: u16,
}

impl Default for Meta {
  fn default() -> Meta {
    Meta { maxima: 0, rank: 0 }
  }
}

/// A custom disjoint set data structure.
///
/// We keep track of the maxima for a set. When we
/// combine two sets, we pick the higher maxima
/// of the two to be the new maxima.
pub struct DisjointTileSet<'a> {
  height_map: &'a DistMatrix,
  meta: TileMap<Meta>,
  parent: TileMap<u16>
}

impl<'a> DisjointTileSet<'a> {
  pub fn new_box(height_map: &'a DistMatrix) -> Box<DisjointTileSet<'a>> {
    let mut dts = Box::new(DisjointTileSet {
      height_map,
      meta: TileMap::default(),
      parent: TileMap::default(),
    });
    for i in 0..ROOM_AREA {
      dts.parent[i] = i as u16;
      dts.meta[i].maxima = i as u16;
    }
    dts
  }

  pub fn is_singleton(&self, index: u16) -> bool {
    self.meta[index as usize].rank == 0 && self.parent[index as usize] == index
  }

  pub fn find(&mut self, index: u16) -> u16 {
    let i = index as usize;
    if self.parent[i] != index {
      self.parent[i] = self.find(self.parent[i]);
    }
    self.parent[i]
  }

  /// Gets the maxima for the set containing the index.
  pub fn maxima_for(&mut self, index: u16) -> RoomXY {
    let root = self.find(index);
    linear_index_to_xy(self.meta[root as usize].maxima as usize)
  }

  pub fn maxima_and_height_for(&mut self, index: u16) -> (RoomXY, u8) {
    let root = self.find(index);
    let maxima = self.meta[root as usize].maxima as usize;
    let xy = linear_index_to_xy(maxima);
    let height = self.height_map.get_index(maxima);
    (xy, height)
  }

  #[inline]
  pub fn find_xy(&mut self, xy: RoomXY) -> RoomXY {
    linear_index_to_xy(self.find(
      xy_to_linear_index(xy) as u16
    ) as usize)
  }

  pub fn same_set(&mut self, a: u16, b: u16) -> bool {
    self.find(a) == self.find(b)
  }

  #[inline]
  pub fn same_set_xy(&mut self, a: RoomXY, b: RoomXY) -> bool {
    self.find(xy_to_linear_index(a) as u16) == self.find(xy_to_linear_index(b) as u16)
  }

  pub fn union(&mut self, a: u16, b: u16) {
    use std::cmp::{max_by_key, Ordering::*};
    let aset = self.find(a);
    let bset = self.find(b);

    if aset == bset {
      return
    }

    let new_maxima = max_by_key(
      self.meta[aset as usize].maxima,
      self.meta[bset as usize].maxima,
      |idx| self.height_map.get_index(*idx as usize)
    );

    match self.meta[aset as usize].rank.cmp(&self.meta[bset as usize].rank) {
      Less => {
        self.parent[aset as usize] = bset;
        self.meta[bset as usize].maxima = new_maxima;
      }
      Greater => {
        self.parent[bset as usize] = aset;
        self.meta[aset as usize].maxima = new_maxima;
      }
      Equal => {
        self.parent[bset as usize] = aset;
        self.meta[aset as usize].rank += 1;
        self.meta[aset as usize].maxima = new_maxima;
      }
    }
  }

  #[inline]
  pub fn union_xy(&mut self, a: RoomXY, b: RoomXY) {
    self.union(xy_to_linear_index(a) as u16, xy_to_linear_index(b) as u16);
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn maxima_between_two() {
    let mut height_map = DistMatrix::empty();
    height_map.set(0, 8);
    height_map.set(1, 10);
    let mut dts = DisjointTileSet::new_box(&height_map);
    assert_eq!(dts.maxima_and_height_for(0).1, 8);
    assert_eq!(dts.maxima_and_height_for(1).1, 10);
    assert!(!dts.same_set(0, 1));
    dts.union(0, 1);
    assert!(dts.same_set(0, 1));
    assert_eq!(dts.maxima_and_height_for(0).1, 10);
  }

  #[test]
  fn maxima_between_three() {
    let mut height_map = DistMatrix::empty();
    height_map.set(0, 8);
    height_map.set(1, 10);
    height_map.set(2, 12);
    let mut dts = DisjointTileSet::new_box(&height_map);
    assert_eq!(dts.maxima_and_height_for(0).1, 8);
    assert_eq!(dts.maxima_and_height_for(1).1, 10);
    assert_eq!(dts.maxima_and_height_for(2).1, 12);
    assert!(!dts.same_set(0, 1));
    assert!(!dts.same_set(0, 2));
    assert!(!dts.same_set(1, 2));
    dts.union(0, 1);
    assert!(dts.same_set(0, 1));
    assert!(!dts.same_set(0, 2));
    assert!(!dts.same_set(1, 2));
    assert_eq!(dts.maxima_and_height_for(0).1, 10);
    dts.union(0, 2);
    assert!(dts.same_set(0, 1));
    assert!(dts.same_set(0, 2));
    assert!(dts.same_set(1, 2));
    assert_eq!(dts.maxima_and_height_for(0).1, 12);
  }

  #[test]
  fn general() {
    let mut height_map = DistMatrix::empty();
    height_map.set(16, 8);
    height_map.set(32, 6);
    height_map.set(100, 2);
    height_map.set(101, 3);
    height_map.set(104, 10);
    let mut dts = DisjointTileSet::new_box(&height_map);
    assert_eq!(dts.maxima_and_height_for(16).1, 8);
    assert_eq!(4, dts.find(4));
    dts.union(4,5);
    assert!(dts.same_set(4,5));
    dts.union(4,16);
    dts.union(4,1);
    assert_eq!(dts.maxima_and_height_for(16).1, 8);
    assert_eq!(dts.maxima_and_height_for(1).1, 8);
    assert_eq!(dts.maxima_and_height_for(32).1, 6);
    dts.union(5, 32);
    assert_eq!(dts.maxima_and_height_for(32).1, 8);
    dts.union(32, 16);
    assert!(dts.same_set(1, 32));
    assert!(dts.same_set(4, 32));
    dts.union(100, 101);
    assert_eq!(dts.maxima_and_height_for(100).1, 3);
    dts.union(101, 102);
    dts.union(102, 103);
    dts.union(103, 104);
    assert_eq!(dts.maxima_and_height_for(100).1, 10);
    assert!(dts.same_set(100, 104));
    assert!(!dts.same_set(100, 4));
    assert!(!dts.same_set(0, 4));
    assert!(!dts.same_set(0, 100));
  }
}
