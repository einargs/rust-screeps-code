use screeps::{
  RoomXY, RoomCoordinate, Direction
};

/// We re-export the all iterator for direction.
pub use enum_iterator::all;

/// We re-export this because it's useful.
pub use screeps::constants::ROOM_SIZE;

/// The number of tiles in a room and the size of a tile array.
pub const ROOM_AREA: usize = ROOM_SIZE as usize * ROOM_SIZE as usize;

pub use screeps::local::{linear_index_to_xy, xy_to_linear_index};

/// Taxicab or manhattan distances.
pub const TAXICAB_DIRECTIONS: [Direction; 4] = [
  Direction::Top,
  Direction::Bottom,
  Direction::Left,
  Direction::Right,
];

/// Is the [`RoomXY`] on the edge of the room.
#[inline]
fn xy_on_edge(room_xy: RoomXY) -> bool {
  let xy: (u8, u8) = room_xy.into();
  matches!(xy, (0, _) | (ROOM_SIZE, _) | (_, 0) | (_, ROOM_SIZE))
}

#[inline]
pub fn all_room_xy() -> impl Iterator<Item = RoomXY> {
  (0..ROOM_AREA)
    .map(|idx| linear_index_to_xy(idx))
}

#[inline]
pub fn all_room_xy_and_idx() -> impl Iterator<Item = (RoomXY, usize)> {
  (0..ROOM_AREA)
    .map(|idx| (linear_index_to_xy(idx), idx))
}

fn on_edge(room_xy: RoomXY) -> bool {
  let xy: (u8, u8) = room_xy.into();
  matches!(xy, (0, _) | (ROOM_SIZE, _) | (_, 0) | (_, ROOM_SIZE))
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

#[inline]
pub fn taxicab_adjacent(xy: RoomXY) -> impl Iterator<Item = RoomXY> {
  use Direction::*;
  TAXICAB_DIRECTIONS.into_iter()
    .filter_map(move |dir| xy.checked_add_direction(dir))
}
