use enum_iterator::all;
use screeps::{
  RoomXY, RoomCoordinate, Direction,
};

/// We re-export this because it's useful.
pub use screeps::constants::ROOM_SIZE;

/// The number of tiles in a room and the size of a tile array.
pub const ROOM_AREA: usize = ROOM_SIZE as usize * ROOM_SIZE as usize;

pub type TileSlice<T> = [T; ROOM_AREA];

/// Converts a [`RoomXY`] coordinate pair to a linear index appropriate for use
/// with a static [`TileSlice`].
pub use screeps::local::linear_index_to_xy;

/// Converts a linear index from the internal representation of a static array
/// [`ROOM_AREA`] to [`RoomXY`].
pub use screeps::local::xy_to_linear_index;

#[inline]
pub fn all_room_xy() -> impl Iterator<Item = RoomXY> {
  (0..ROOM_AREA)
    .map(|idx| linear_index_to_xy(idx))
}

pub fn room_edges_xy() -> impl Iterator<Item = RoomXY> {
  use log::*;
  use itertools::chain;
  use std::iter::{repeat, zip};
  let room_edges = chain!(
    // left wall
    zip(repeat(0), 0..50),
    // top wall
    zip(1..49, repeat(0)),
    // bottom wall
    zip(1..49, repeat(49)),
    // right wall
    zip(repeat(49), 0..50),
  );
  room_edges.map(|(x,y)| {
    RoomXY::try_from((x,y)).expect("safe")
  })
}

#[inline]
pub fn surrounding_xy(xy: RoomXY) -> impl Iterator<Item = RoomXY> {
  all::<Direction>()
    .filter_map(move |dir| xy.checked_add_direction(dir))
}

#[inline]
pub fn surrounding_xy_with_dir(xy: RoomXY) -> impl Iterator<Item = (Direction, RoomXY)> {
  all::<Direction>()
    .filter_map(move |dir| xy.checked_add_direction(dir).map(|xy| (dir, xy)))
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
