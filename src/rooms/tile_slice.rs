use std::ops::{Index, IndexMut};
use enum_iterator::all;
use screeps::{
  RoomXY, RoomCoordinate, Direction,
};

/// We re-export this because it's useful.
pub use screeps::constants::ROOM_SIZE;

// TODO: remove TileSlice and replace with TileMap.
pub type TileSlice<T> = [T; ROOM_AREA];

/// Re-export all of the position related utilities.
pub use crate::util::xy::*;

#[repr(transparent)]
pub struct TileMap<T>(TileSlice<T>);

impl<T> TileMap<T> where T: Copy {
  #[inline]
  pub fn new_box(default: T) -> Box<TileMap<T>> {
    Box::new(TileMap([default; ROOM_AREA]))
  }
}

impl<T: Default + Copy> Default for TileMap<T> {
  fn default() -> TileMap<T> {
    TileMap([T::default(); ROOM_AREA])
  }
}

impl<T> Index<usize> for TileMap<T> {
  type Output = T;
  fn index(&self, index: usize) -> &T {
    &self.0[index]
  }
}

impl<T> IndexMut<usize> for TileMap<T> {
  fn index_mut(&mut self, index: usize) -> &mut T {
    &mut self.0[index]
  }
}

impl<T> Index<RoomXY> for TileMap<T> {
  type Output = T;
  fn index(&self, index: RoomXY) -> &T {
    &self.0[xy_to_linear_index(index)]
  }
}

impl<T> IndexMut<RoomXY> for TileMap<T> {
  fn index_mut(&mut self, index: RoomXY) -> &mut T {
    &mut self.0[xy_to_linear_index(index)]
  }
}

/// Borrow a value inside a [`TileSlice`] at the given coordinates.
#[inline]
pub fn xy_access<T>(xy: RoomXY, slice: &TileSlice<T>) -> &T {
  let index = xy_to_linear_index(xy);
  // room coordinate indices cannot be outside of bounds for this.
  unsafe {
    slice.get_unchecked(index)
  }
}

/// Mutably borrow a value inside a [`TileSlice`] at the given coordinates.
#[inline]
pub fn xy_access_mut<T>(xy: RoomXY, slice: &mut TileSlice<T>) -> &mut T {
  let index = xy_to_linear_index(xy);
  // room coordinate indices cannot be outside of bounds for this.
  unsafe {
    slice.get_unchecked_mut(index)
  }
}
