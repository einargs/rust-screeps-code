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


// Here's my idea: what if we do two passes using dts or something
// similar.
// one to identify the maxima, and then we do another flooding
// pass to identify the borders.

// TODO: get a name that implies it isn't involved in actually
// coloring.

// NOTE: below is old
// we check the height of nearby tiles, looking for
// ones higher than us.
//
// if it's only one, we union join with it.
//
// If there are two or more higher than us, we recurse
// and then check if they're all in the same set.
// - If they are all in the same set, we join them.
// - Otherwise, we add ourselves to the border list.
//
// It is possible a border tile to end up in a set. Imagine
// a tile whose only neighbor is a tile on the border of
// two rooms.

/// Visit a tile to add it to the DisjointTileSet regions.
///
/// Returns true if we found a higher tile.
fn link_to_maxima<'a>(
  xy: RoomXY,
  height_map: &'a DistMatrix,
  dts: &mut DisjointTileSet<'a>,
  do_log: bool
) -> bool {
  let height = height_map.get(xy);

  // TODO: a specific coordinate I'm debugging
  let is_start_log = <RoomXY as Into<(u8, u8)>>::into(xy) == (32, 7);
  let do_log = is_start_log || do_log;

  if is_start_log {
    debug!("Logging for {xy}");
  }

  // NOTE: not sure if we want chessboard or taxicab.
  // chessboard helps reduce a bunch of situations where rooms
  // should clearly be grouped together.
  let get_neighbors = surrounding_xy;
  // let get_neighbors = taxicab_adjacent;


  // TODO: I have a correct maxima but the


  // the problem is that there's no back tracking to ensure we don't end
  // up in a dead end.
  //
  // Solutions:
  // - make other things connect to us (bad track record)
  // - breadth first search?
  if let Some(higher) = get_neighbors(xy)
    .find(|adj| height_map.get(*adj) > height) {
      let was_single = dts.is_singleton_xy(higher);
      dts.union_xy(higher, xy);
      if was_single {
        link_to_maxima(higher, height_map, dts, do_log);
      }
      return true;
    }

  let equals = get_neighbors(xy)
    .filter(|adj| height_map.get(*adj) == height);

  for equal in equals {
    if !dts.is_singleton_xy(equal) {
      let higher = dts.maxima_height_for_xy(equal) > height;
      dts.union_xy(equal, xy);
      if higher {
        return true
      }
    } else {
      dts.union_xy(equal, xy);
      if link_to_maxima(equal, height_map, dts, do_log) {
        return true
      }
    }
  }
  return false
}

/// Visit a tile to add it to the DisjointTileSet regions.
fn color_tile<'a>(
  xy: RoomXY,
  height_map: &'a DistMatrix,
  dts: &mut DisjointTileSet<'a>
) {
  let height = height_map.get(xy);

  let cur_maxima_height = dts.maxima_height_for_xy(xy);

  // not sure if we want this or taxicab.
  let get_neighbors = surrounding_xy;

  for adj in get_neighbors(xy) {
    if dts.maxima_height_for_xy(adj) > cur_maxima_height {
      continue
    }
    dts.union_xy(adj, xy);
  }

  /*
  for adj in get_neighbors(xy).filter(|adj| height_map.get(*adj) == height) {
    dts.union_xy(xy, adj);
  }
  */

  /*
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
  */
}

/// Creates the color map and the maximas.
fn create_color_map(
  height_map: &DistMatrix
) -> (HashSet<RoomXY>, Box<ColorMap>, Color) {
  let mut dts = DisjointTileSet::new_box(height_map);
  let mut visited = TileMap::<bool>::new_box(false);

  for (xy, idx) in all_room_xy_and_idx() {
    // skip if it's a wall
    if height_map.get_index(idx as usize) == 0 {
      continue
    }
    // skip if we've *definitely* visited already
    if !dts.is_singleton(idx as u16) {
      continue
    }
    link_to_maxima(xy, height_map, &mut dts, false);
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

  for idx in 0..(ROOM_AREA as u16) {
    if height_map.get_index(idx as usize) == 0 {
      continue
    }
    if color_map[idx as usize] != 0 {
      continue
    }
    let set = dts.find(idx);
    let color = *color_index.entry(set).or_insert_with(|| {
      let val = color_count;
      color_count += 1;
      val
    });
    color_map[idx as usize] = color;
  }
  debug!("Created {} colors!", color_count - 1);

  let maximas: HashSet<RoomXY> = all_room_xy()
    .filter(|xy| height_map.get(*xy) != 0)
    .map(|xy| dts.maxima_for_xy(xy))
    .collect();
  debug!("maxima #{}", maximas.len());

  (maximas, color_map, color_count)
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

/// Generate a nice color for our visuals.
fn calculate_color(total_count: Color, color: Color) -> String {
  if color == NO_COLOR {
    return "#000000".to_string()
  }
  let size = total_count - START_COLOR;
  let index = color - START_COLOR;
  let percentage = f64::from(index) / f64::from(size);
  let radians: f64 = percentage * core::f64::consts::TAU;
  // see: https://stackoverflow.com/questions/10731147/evenly-distributed-color-range-depending-on-a-count
  // we basically pick a spot on the color wheel and then turn
  // that into the U/V plane with fixed brightness.
  let u: f64 = radians.cos();
  let v: f64 = radians.sin();
  let y: f64 = 0.5; // brightness.

  // now we convert into RGB.
  let red: f64 = y + v/0.88;
  let green: f64 = y - 0.38 * u - 0.58 * v;
  let blue: f64 = y + u/0.49;
  // debug!("Radians: {radians} RGB: {red} {green} {blue}");

  // now we turn our floats into bytes
  // we have to clamp because the YUV color space is bizzare.
  let redb: u8 = (red.clamp(0.0,1.0) * 255.0).floor() as u8;
  let greenb: u8 = (green.clamp(0.0, 1.0) * 255.0).floor() as u8;
  let blueb: u8 = (blue.clamp(0.0, 1.0) * 255.0).floor() as u8;

  // then we format
  format!("#{:02x}{:02x}{:02x}", redb, greenb, blueb)
}

/// Search through a color map, trav
pub fn room_cut(terrain: &LocalRoomTerrain, visual: RoomVisual, render: Render) -> Vec<RoomXY> {
  let dist_matrix = DistMatrix::new_taxicab(terrain);
  // let maximas: Vec<Maxima> = find_maxima(&dist_matrix).collect();
  let (maxima_set, mut color_map, color_count) = create_color_map(&dist_matrix);
  // let borders = flood_color_map(&dist_matrix, &mut color_map, &maximas);
  // let maxima_set: HashSet<RoomXY> = maximas.iter().map(|m| m.xy).collect();

  /*
  for maxima in maximas.iter() {
    debug!("{}: color = {}", maxima.xy, color_map[maxima.xy]);
  }
  */

  for xy in all_room_xy() {
    let (x,y): (u8,u8) = xy.into();
    let fx: f32 = x.into();
    let fy: f32 = y.into();
    let num = match &render {
      Render::Height => dist_matrix.get(xy) as u16,
      Render::Colors => color_map[xy],
    };
    let num_str = format!("{:?}", num);
    let style = if maxima_set.contains(&xy) {
      TextStyle::default()
        .color("#FFFFFF")
    } else {
      let color_str = calculate_color(color_count, color_map[xy]);
      TextStyle::default()
        .color(&color_str)
    };
    visual.text(fx, fy, num_str, Some(style));
  }
  vec![]
}
