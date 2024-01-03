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

type ColorMap = TileMap<Color>;

// Here's my idea: what if we do two passes using dts or something
// similar.
// one to identify the maxima, and then we do another flooding
// pass to identify the borders.

// TODO: document logic/process
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

/// Returns a color map for the disjoint tile set and
/// a color that is the highest color value used + 1.
fn color_disjoint_tile_set(
  height_map: &DistMatrix,
  dts: &mut DisjointTileSet,
) -> (Box<ColorMap>, Color) {
  let mut color_map = ColorMap::new_box(0);
  let mut color_count: Color = START_COLOR;
  // tracks colors assigned to already encountered sets
  let mut color_index: BTreeMap<u16, Color> = BTreeMap::new();

  // color the map and create a list of indexes.
  for idx in 0..(ROOM_AREA as u16) {
    if height_map.get_index(idx as usize) == 0 {
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

  (color_map, color_count)
}

/// Get a list of maximas.
///
/// Creates a list of local maxima and a [`DisjointTileSet`]
/// of how it was found. This is not good segmentation, but
/// is useful for debugging.
fn calculate_local_maxima(
  height_map: &DistMatrix
) -> (impl Iterator<Item = RoomXY>, Box<DisjointTileSet>) {
  let mut dts = DisjointTileSet::new_box(height_map);

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

  let mut maxima_map: BTreeMap<u16, RoomXY> = BTreeMap::new();

  // color the map and create a list of indexes.
  for idx in 0..(ROOM_AREA as u16) {
    if height_map.get_index(idx as usize) == 0 {
      continue
    }
    let maxima = dts.maxima_for(idx);
    maxima_map.entry(idx).or_insert(maxima);
  }

  (maxima_map.into_values(), dts)
}

/// Check if two colors are different values, and that neither
/// is NO_COLOR.
#[inline]
fn diff_color(a: Color, b: Color) -> bool {
  a != NO_COLOR && b != NO_COLOR && a != b
}

/// Flood a color map, avoiding coloring a cell when it would cause
/// diagonals or adjacents of one color to touch another color.
/// Returns a list of coordinates that are on the border between
/// two colors.
///
/// Uses BFS with a priority queue, which maintains fairness.
fn flood_color_map(
  height_map: &DistMatrix,
  maximas: impl Iterator<Item = RoomXY>
) -> (Box<ColorMap>, Color, Vec<RoomXY>) {
  use priority_queue::PriorityQueue;
  let mut queue: PriorityQueue<RoomXY, Height> = PriorityQueue::new();
  let mut borders: Vec<RoomXY> = Vec::new();
  let mut color_map = ColorMap::new_box(NO_COLOR);

  let mut color_count = START_COLOR;
  for maxima in maximas {
    color_map[maxima] = color_count;
    color_count += 1;
    let height = height_map.get(maxima);
    queue.push(maxima, height);
  }

  while let Some((xy, height)) = queue.pop() {
    let xy_color = color_map[xy];
    // if we've been changed to be a border, we skip
    if xy_color == BORDER_COLOR {
      continue
    }
    // we should have a color assigned.
    assert_ne!(xy_color, NO_COLOR, "color at {xy} did not exist; instead {xy_color}");
    // we use taxicab because that's the distance metric
    // used, and otherwise it could mess up the queue
    // by jumping down 2 levels instead of 1.
    // well, the priority queue would probably keep it safe, but
    // why bother. This should keep the size of the queue smaller
    // over time at least.
    for adj in surrounding_xy(xy) {
      if height_map.get(adj) == 0 {
        continue
      }
      match color_map[adj] {
        BORDER_COLOR => {
          continue
        }
        NO_COLOR => {
          let adj_height = height_map.get(adj);

          color_map[adj] = color_map[xy];
          queue.push(adj, adj_height);
        }
        other_color => {
          if other_color != xy_color {
            color_map[adj] = BORDER_COLOR;
            break
          }
        }
      }
    }
  }

  (color_map, color_count, borders)
}

pub enum Render {
  Height,
  Colors,
}

/// Generate a nice color for our visuals.
fn calculate_color(total_count: Color, color: Color) -> String {
  if matches!(color, NO_COLOR | BORDER_COLOR) {
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
  let y: f64 = 1.0; // brightness.

  // now we convert into RGB.
  let red: f64 = y + v/0.88;
  let green: f64 = y - 0.38 * u - 0.58 * v;
  let blue: f64 = y + u/0.49;
  // debug!("Radians: {radians} RGB: {red} {green} {blue}");

  fn convert(val: f64) -> f64 {
    let rgb_percent = val.clamp(0.0, 2.0) / 2.0;
    let rgb_float = rgb_percent * 255.0;
    rgb_float.floor()
  }

  // now we turn our floats into bytes
  // we have to clamp because the YUV color space is bizzare.
  let redb: u8 = convert(red) as u8;
  let greenb: u8 = convert(green) as u8;
  let blueb: u8 = convert(blue) as u8;

  // then we format
  let out = format!("#{:02x}{:02x}{:02x}", redb, greenb, blueb);

  /*
  if log {
    debug!("color {color} {radians} ({red} : {}, {green} : {}, {blue} : {}) {out}",
           convert(red), convert(green), convert(blue));
  }
  */

  out
}

/// Search through a color map, trav
pub fn room_cut(terrain: &LocalRoomTerrain, visual: RoomVisual, render: Render) -> Vec<RoomXY> {
  let height_map = DistMatrix::new_taxicab(terrain);
  // let maximas: Vec<Maxima> = find_maxima(&dist_matrix).collect();
  let (maxima_iter, mut maxima_dts) = calculate_local_maxima(&height_map);
  let maxima_set: HashSet<RoomXY> = maxima_iter.collect();
  let maxima_colors = color_disjoint_tile_set(&height_map, &mut maxima_dts);
  let (color_map, color_count, borders) = flood_color_map(
    &height_map, maxima_set.iter().copied());

  let (disp_color_map, disp_color_count) = (color_map, color_count); // maxima_colors;

  for xy in all_room_xy() {
    let (x,y): (u8,u8) = xy.into();
    let fx: f32 = x.into();
    let fy: f32 = y.into();
    let num = match &render {
      Render::Height => height_map.get(xy) as u16,
      Render::Colors => disp_color_map[xy],
    };
    let num_str = format!("{:?}", num);
    let style = if maxima_set.contains(&xy) {
      TextStyle::default()
        .color("#FFFFFF")
    } else {
      let color_str = calculate_color(disp_color_count, disp_color_map[xy]);
      TextStyle::default()
        .color(&color_str)
    };
    visual.text(fx, fy, num_str, Some(style));
  }
  vec![]
}
