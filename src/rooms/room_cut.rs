//! This algorithm works by:
//! - building a distance transform matrix using taxicab
//!   distance, and using that to identify centers of rooms.
//! - Then we use a watershed algorithm to segment the rooms.
//! - Then we use the highest value on a boundary as a the weight
//!   for that edge between the rooms.
//! - Then we apply a maximum flow minimum cut algorithm.
//!
//! NOTE: It may be beneficial to merge nearby maxima when identifiying rooms.
//! Possibly via some kind of scaling factor where the further from zero, the
//! further they can be to each other and still be merged.

// NOTE: I think there's an efficient priority flood algorithm I could use but I
// don't care. https://arxiv.org/abs/1511.04463

use std::collections::{VecDeque, BTreeSet, HashSet, BTreeMap};

use log::*;

use screeps::{
  Direction, RoomXY, LocalRoomTerrain, xy_to_linear_index, RoomVisual, TextStyle
};

use super::disjoint_tile_set::DisjointTileSet;
use super::tile_slice::*;
use super::{dist_transform::*, tile_slice::all_room_xy};

const TAXICAB_DIRECTIONS: [Direction; 4] = [
  Direction::Top,
  Direction::Bottom,
  Direction::Left,
  Direction::Right,
];

type Height = u8;

/// Color is used to identify which room a source belongs to when performing
/// bfs.
type Color = u16;

/// 0 means no color has been assigned yet.
const NO_COLOR: Color = 0;

/// 1 means it's on the border.
const BORDER_COLOR: Color = 1;

/// The first valid color.
const START_COLOR: Color = 2;

/// Check if two colors are different values, and that neither
/// is NO_COLOR.
#[inline]
fn diff_color(a: Color, b: Color) -> bool {
  a != 0 && b != 0 && a != b
}

struct Maxima {
  xy: RoomXY,
  // TODO: may remove this field
  height: Height,
}

/// These are like maxima, but they have been assigned a unique
/// color identifier. adjacent maxima are given the same
struct FloodSource {
  xy: RoomXY,
  height: Height,
  color: Color
}

fn taxicab_adjacent(xy: RoomXY) -> impl Iterator<Item = RoomXY> {
  use Direction::*;
  TAXICAB_DIRECTIONS.into_iter()
    .filter_map(move |dir| xy.checked_add_direction(dir))
}

// TODO: we're going to use a custom disjoint set and then build a custom
// color map for better display with smaller unique indices. I can do that
// with a BTreeMap.
/// Data structure used for assigning small unique colors to stuff
/// after building a disjoint set data structure from tiles.
struct ColorMapI {
  color_count: u16,
  map: BTreeMap<usize, u16>
}

/// Find the local maxima.
///
/// If we have two points of the same height surrounded by lesser points,
/// we count them both as maxima. We will perform a merge operation.
fn find_maxima<'a>(dist: &'a DistMatrix) -> impl Iterator<Item = Maxima> + 'a {
  all_room_xy().filter_map(move |xy| {
    let height = dist.get(xy);
    for adj in surrounding_xy(xy) {
      if dist.get(adj) >= height {
        return None
      }
    }
    Some(Maxima { xy, height })
  })
}

type ColorMap = TileMap<Color>;


// Here's my idea: what if we do two passes of this.
// one to identify the maxima, and then we do another flooding
// pass to identify the borders.

// TODO: get a name that implies it isn't involved in actually
// coloring.
/// Visit a tile to add it to the DisjointTileSet regions.
///
/// we check the height of nearby tiles, looking for
/// ones higher than us.
///
/// if it's only one, we union join with it.
///
/// If there are two or more higher than us, we recurse
/// and then check if they're all in the same set.
/// - If they are all in the same set, we join them.
/// - Otherwise, we add ourselves to the border list.
///
/// It is possible a border tile to end up in a set. Imagine
/// a tile whose only neighbor is a tile on the border of
/// two rooms.
fn color_tile<'a>(
  xy: RoomXY,
  height_map: &'a DistMatrix,
  dts: &mut DisjointTileSet<'a>
) {
  if visited[xy] {
    return
  }

  let height = height_map.get(xy);

  let higher_neighbors: Vec<RoomXY> = surrounding_xy(xy)
    .filter(|adj| height_map.get(*adj) > height)
    .collect();

  match higher_neighbors.as_slice() {
    [] => (),
    /* should be equivalent to clause below
    [adj] => {
      let adj = *adj;
      color_tile(adj, visited, border_tiles, height_map, dts);
      dts.union_xy(xy, adj);
      return
    } */
    all@[first, rest@..] => {
      let first = *first;
      for &adj in all {
        color_tile(adj, visited, border_tiles, height_map, dts);
      }
      let id = dts.find_xy(first);
      if rest.iter().all(|adj| dts.find_xy(*adj) == id) {
        dts.union_xy(xy, first)
      } else {
        border_tiles.push(xy);
      }
      return
    }
  }

  let other_neighbors = || surrounding_xy(xy)
    .filter(|adj| height_map.get(*adj) <= height);
  // temporary
  for &adj in other_neighbors() {
    color_tile(adj, visited, border_tiles, height_map, dts);
  }
  let mut neighbors = other_neighbors();
  if let Some(first) = neighbors.next() {
    let set = dts.find_xy(first);
    if neighbors.all(|adj| dts.find_xy(*adj) == set) {
      dts.
    }
  }
}

fn create_color_map(
  dist: &DistMatrix
) -> Box<ColorMap> {
  let mut dts = DisjointTileSet::new_box(dist);
  let mut border_tiles: Vec<RoomXY> = Vec::new();
  let mut visited = TileMap::<bool>::new_box(false);

  for (xy, idx) in all_room_xy_and_idx() {
    // skip if it's a wall
    if dist.get_index(idx as usize) == 0 {
      continue
    }
    // skip if we've *definitely* visited already
    if !dts.is_singleton(idx as u16) {
      continue
    }
  }
  /*
  for maxima in maximas {
    let adj_color_xy = surrounding_xy(maxima.xy)
      .find(|xy| color_map[*xy] != 0);
    match adj_color_xy {
      Some(adj_xy) => {
        color_map[maxima.xy] = color_map[adj_xy];
      }
      None => {
        color_map[maxima.xy] = color_count;
        color_count += 1;
      }
    }
  }
  */
  let mut color_map = ColorMap::new_box(0);
  let mut color_count: Color = START_COLOR;
  // tracks colors assigned to already encountered sets
  let mut color_index: BTreeMap<u16, Color> = BTreeMap::new();

  for border in border_tiles {
    color_map[border] = BORDER_COLOR;
  }

  for idx in 0..(ROOM_AREA as u16) {
    if color_map[idx as usize] != 0 {
      continue
    }
    let set = dts.find(idx);
    let color = color_index.get(&set).copied().unwrap_or_else(|| {
      let val = color_count;
      color_count += 1;
      val
    });
    color_map[idx as usize] = color;
  }

  color_map
}

/// Flood a color map, avoiding coloring a cell when it would cause
/// diagonals or adjacents of one color to touch another color.
/// Returns a list of coordinates that are on the border between
/// two colors.
///
/// Uses BFS (which should be fair?).
///
/// NOTE: this part of the algorithm could go wrong pretty easily.
fn flood_color_map(
  height_map: &DistMatrix,
  color_map: &mut ColorMap,
  maximas: &[Maxima]
) -> Vec<RoomXY> {
  let mut queue: VecDeque<RoomXY> = maximas
    .iter().map(|m| m.xy).collect();
  let mut visited = TileMap::<bool>::new_box(false);
  let mut borders: Vec<RoomXY> = Vec::new();

  while let Some(xy) = queue.pop_front() {
    for adj in surrounding_xy(xy) {
      if visited[adj] {
        continue
      }
      visited[adj] = true;
      if height_map.get(adj) == 0 {
        continue
      }

      if color_map[adj] != 0 {
        continue
      }

      color_map[adj] = color_map[xy];
      queue.push_back(adj);
      /*
      if diff_color(color_map[xy], color_map[adj]) {
        // we now need to change this color back.
        color_map[xy] = 1;
        borders.push(xy);
      } else {
        color_map[adj] = color_map[xy];
        queue.push_back(adj);
      }*/
    }
  }

  borders
}

pub enum Render {
  Height,
  Colors,
}

/// Search through a color map, trav
pub fn room_cut(terrain: &LocalRoomTerrain, visual: RoomVisual, render: Render) -> Vec<RoomXY> {
  let dist_matrix = DistMatrix::new_taxicab(terrain);
  let maximas: Vec<Maxima> = find_maxima(&dist_matrix).collect();
  let mut color_map = create_color_map(&dist_matrix);
  let borders = flood_color_map(&dist_matrix, &mut color_map, &maximas);
  let maxima_set: HashSet<RoomXY> = maximas.iter().map(|m| m.xy).collect();

  for maxima in maximas.iter() {
    debug!("{}: color = {}", maxima.xy, color_map[maxima.xy]);
  }

  for xy in all_room_xy() {
    let num = match &render {
      Render::Height => format!("{:?}", dist_matrix.get(xy)),
      Render::Colors =>format!("{:?}", color_map[xy]),
    };
    let (x,y): (u8,u8) = xy.into();
    let fx: f32 = x.into();
    let fy: f32 = y.into();
    let style = if maxima_set.contains(&xy) {
      TextStyle::default()
        .color("#0000BB")
    } else {
      TextStyle::default()
    };
    visual.text(fx, fy, num, Some(style));
  }
  vec![]
}
