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

impl DistMatrix {
  /// Create a new distance transform matrix for the room's local terrain.
  pub fn new(terrain: &LocalRoomTerrain) -> DistMatrix {
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
        .map(|dir| {
          // We use 0 because this will only happen for places along the edges,
          // we want the edges to be zero.
          let val = xy.checked_add_direction(*dir)
            .map_or(0, |adjacent| xy_access(adjacent, &data) + 1);
          /*if xy.x.u8() == ROOM_SIZE - 4 && xy.y.u8() > 25 {
          debug!("{dir:?} {val} at {} {}", xy.x.u8(), xy.y.u8());
        }*/
          val
        })
        .chain(std::iter::once(prev))
        .min()
        .expect("Cannot be zero length because of the once");
      //debug!("Updating {} {} from {prev} to {new_value}", xy.x.u8(), xy.y.u8());
      data[xy_to_linear_index(xy)] = new_value;
    }

    // We do our first pass going from the top left to the bottom right
    let right_down_directions = [ BottomLeft, Left, TopLeft, Top, TopRight ];

    for x in 0..ROOM_SIZE {
      for y in 0..ROOM_SIZE {
        apply_directions(x, y, &right_down_directions, &mut data);
      }
    }

    // We do our second pass going from the bottom right to the top left
    let left_up_directions = [ BottomLeft, Bottom, BottomRight, Right, TopRight ];
    for x in (0..ROOM_SIZE).rev() {
      for y in (0..ROOM_SIZE).rev() {
        apply_directions(x, y, &left_up_directions, &mut data);
      }
    }

    DistMatrix { array: data }
  }

  /// Get the distance using a coordinate pair.
  pub fn get(&self, xy: RoomXY) -> u8 {
    *xy_access(xy, &self.array)
  }

  /// Get the distance using the linear index.
  pub fn get_index(&self, idx: usize) -> u8 {
    self.array[idx]
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
