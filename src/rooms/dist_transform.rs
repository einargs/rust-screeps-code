//! Code for calculating the distance transform matrix for a room.
//! This associates every tile with it's distance to the nearest wall.
//!
//! Adapted from the code for the city block transformation here:
//! https://arxiv.org/pdf/2106.03503.pdf

use std::fmt;
use log::*;

use screeps::{
  LocalRoomTerrain, RoomXY, Terrain, Direction,
};

use super::tile_slice::*;

/// We use this as the initial value for all of the non-wall tiles,
/// because no distance can be larger than it.
const MAX_DIST: u8 = ROOM_SIZE;

/// A distance transform matrix for a room. It associates every tile
/// with it's Chebyshev (chessboard) distance to the nearest wall.
pub struct DistMatrix {
  array: Box<TileSlice<u8>>
}

/// Performs a pass update on a cell, updating it's new value with information
/// from the cells in the nearby given directions.
#[inline]
fn apply_directions<const N: usize>(
  x: u8, y: u8,
  directions: &[Direction; N],
  data: &mut TileSlice<u8>
) {
  // can replace with unchecked later if I want.
  let xy = RoomXY::try_from((x, y))
    .expect("Should be valid coords");
  let prev = data[xy_to_linear_index(xy)];

  // Calculate the new value for this cell by looking at the minimum value of adjacent
  // cells + 1 and it's current value.
  let new_value = directions
    .iter()
    .filter_map(|dir| {
      // we want to make sure that exits have the correct height, so we
      // don't default to 0 when we end up out of bounds.
      xy.checked_add_direction(*dir)
        .map(|adjacent| xy_access(adjacent, &data) + 1)
    })
    .chain(std::iter::once(prev))
    .min()
    .expect("Cannot be zero length because of the once");
  //debug!("Updating {} {} from {prev} to {new_value}", xy.x.u8(), xy.y.u8());
  data[xy_to_linear_index(xy)] = new_value;
}

/// Create a new distance transform matrix for the room's local terrain.
fn mk_with_dirs<const N: usize>(
  terrain: &LocalRoomTerrain,
  forward_directions: &[Direction; N],
  backward_directions: &[Direction; N],
) -> DistMatrix {
  use Direction::*;
  let mut data = Box::new([0; ROOM_AREA]);
  // We initialize the walls to be 0 and everything else to be the
  // max possible distance.
  for idx in 0..ROOM_AREA {
    data[idx] = match terrain.get(linear_index_to_xy(idx)) {
      Terrain::Wall => 0,
      _ => MAX_DIST,
    };
  }
  let test_xy = RoomXY::try_from((0,10)).unwrap();
  debug!("{}: {:?}", test_xy, terrain.get(test_xy));

  // We do our first pass going from the top left to the bottom right

  for x in 0..ROOM_SIZE {
    for y in 0..ROOM_SIZE {
      apply_directions(x, y, forward_directions, &mut data);
    }
  }

  // We do our second pass going from the bottom right to the top left
  for x in (0..ROOM_SIZE).rev() {
    for y in (0..ROOM_SIZE).rev() {
      apply_directions(x, y, backward_directions, &mut data);
    }
  }

  DistMatrix { array: data }
}

impl DistMatrix {
  /// This uses chebyshev distance, or chessboard distance. Tiles are connected
  /// to their diagonals.
  pub fn new_chessboard(terrain: &LocalRoomTerrain) -> DistMatrix {
    use Direction::*;
    let forward_directions = [ BottomLeft, Left, TopLeft, Top, TopRight ];
    let backward_directions = [ BottomLeft, Bottom, BottomRight, Right, TopRight ];
    mk_with_dirs(terrain, &forward_directions, &backward_directions)
  }

  /// Uses taxicab distance. Tiles are only connected vertically and horizontally.
  pub fn new_taxicab(terrain: &LocalRoomTerrain) -> DistMatrix {
    use Direction::*;
    let forward_directions = [ Left, Top ];
    let backward_directions = [ Bottom, Right ];
    mk_with_dirs(terrain, &forward_directions, &backward_directions)
  }

  /// Get the distance using a coordinate pair.
  pub fn get(&self, xy: RoomXY) -> u8 {
    *xy_access(xy, &self.array)
  }

  /// Get the distance using the linear index.
  pub fn get_index(&self, idx: usize) -> u8 {
    self.array[idx]
  }

  #[cfg(test)]
  pub fn empty() -> DistMatrix {
    DistMatrix {
      array: Box::new([0; ROOM_AREA])
    }
  }

  #[cfg(test)]
  pub fn set(&mut self, idx: usize, val: u8) {
    self.array[idx] = val;
  }
}

impl fmt::Display for DistMatrix {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for y in 0..ROOM_SIZE {
      let first_xy = RoomXY::try_from((0, y))
        .expect("Should be valid coords");
      write!(f, "{:2}", self.get(first_xy))?;
      for x in 1..ROOM_SIZE {
        let xy = RoomXY::try_from((x, y))
          .expect("Should be valid coords");
        write!(f, " {:2}", self.get(xy))?;
      }
      write!(f, "\n")?;
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  // TODO: test that it doesn't make a difference which phase bottom left is
  // included in.
}
