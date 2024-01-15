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

use std::assert_matches;
use std::collections::HashMap;
use std::fmt;
use std::collections::{VecDeque, BTreeSet, HashSet, BTreeMap};
use std::hash::Hash;

use log::*;

use screeps::{
  Direction, RoomXY, LocalRoomTerrain, xy_to_linear_index, RoomVisual, TextStyle
};

use super::disjoint_tile_set::DisjointTileSet;
use super::tile_slice::*;
use super::{dist_transform::*, tile_slice::all_room_xy};

type Height = u8;

/// Color is used to identify which room a source belongs to when performing
/// bfs.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Color {
  Empty,
  Border,
  Pending(ColorIdx),
  Resolved(ColorIdx)
}

impl fmt::Display for Color {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Color::Empty => write!(f, "E"),
      Color::Border => write!(f, "B"),
      Color::Pending(idx) => write!(f, "P{idx}"),
      Color::Resolved(idx) => write!(f, "{idx}"),
    }
  }
}

/// The data type used to represent the color index.
type ColorIdx = u8;

// TODO: build a custom data type keeps track of the number of colors
// inside this.
type ColorMap = TileMap<Color>;

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
) -> (Box<ColorMap>, ColorIdx) {
  let mut color_map = ColorMap::new_box(Color::Empty);
  let mut color_count: ColorIdx = 0;
  // tracks colors assigned to already encountered sets
  let mut color_index: BTreeMap<u16, ColorIdx> = BTreeMap::new();

  // color the map and create a list of indexes.
  for idx in 0..(ROOM_AREA as u16) {
    if height_map.get_index(idx as usize) == 0 {
      continue
    }
    let set = dts.find(idx);
    let color_idx = *color_index.entry(set).or_insert_with(|| {
      let val = color_count;
      color_count += 1;
      val
    });
    color_map[idx as usize] = Color::Resolved(color_idx);
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

/// Flood a color map, avoiding coloring a cell when it would cause
/// diagonals or adjacents of one color to touch another color.
/// Returns a list of coordinates that are on the border between
/// two colors.
///
/// Uses BFS with a priority queue, which maintains fairness.
fn flood_color_map(
  height_map: &DistMatrix,
  maximas: impl Iterator<Item = RoomXY>
) -> (Box<ColorMap>, ColorIdx, Vec<RoomXY>) {
  use priority_queue::PriorityQueue;
  let mut queue: PriorityQueue<RoomXY, Height> = PriorityQueue::new();
  let mut borders: Vec<RoomXY> = Vec::new();
  let mut color_map = ColorMap::new_box(Color::Empty);

  let mut color_count: ColorIdx = 0;
  for maxima in maximas {
    color_map[maxima] = Color::Resolved(color_count);
    color_count += 1;
    let height = height_map.get(maxima);
    queue.push(maxima, height);
  }

  // NOTE: not sure if we want taxicab or chessboard.
  let get_neighbors = surrounding_xy;

  while let Some((xy, height)) = queue.pop() {
    let xy_color = color_map[xy];
    let xy_height = height_map.get(xy);
    // The maxima are the only points that should have a color when
    // this stage happens.
    let xy_color_idx = match xy_color {
      Color::Pending(color) => {
        // We check to see if there's a resolved neighbor that has a different color.
        // If there is, we become a border and continue to the next tile.
        let neighboring_other_color = get_neighbors(xy).any(|adj| match color_map[adj] {
          Color::Resolved(other_color) => other_color != color,
          // if it's a pending, we'll end up on that pending looking at this
          // tile later.
          _ => false,
        });
        if neighboring_other_color {
          color_map[xy] = Color::Border;
          continue
        } else {
          color_map[xy] = Color::Resolved(color);
        }
        color
      }
      // only the maxima should have a resolved color right now
      Color::Resolved(color) => color,
      Color::Border | Color::Empty => {
        panic!("color at {xy} was {xy_color} when it should be pending or resolved (if it's a maxima)");
      }
    };

    for adj in get_neighbors(xy) {
      let adj_height = height_map.get(adj);
      // TODO: maybe skip if higher than xy_height. If it's higher, someone else
      // will reach it? that's just a minor optimization I think? If it's
      // taxicab, then definitely there's no way for a tile to see a higher
      // piece that isn't in the queue, but with chessboard I think there is.
      // Imagine: 3 2 3, where xy is the left 3. We get to the 2 as adj, which
      // can then see the right 3.

      // we skip walls
      if adj_height == 0 {
        continue
      }

      match color_map[adj] {
        Color::Empty => {
          let adj_height = height_map.get(adj);

          color_map[adj] = Color::Pending(xy_color_idx);
          queue.push(adj, adj_height);
        }
        _ => {
          continue
        }
      }
    }
  }

  (color_map, color_count, borders)
}

/// This is an adjacency list based graph of room segments.
struct SegmentGraphAdj(Vec<Vec<(ColorIdx, Height)>>);

impl SegmentGraphAdj {
  fn new(color_count: ColorIdx) -> SegmentGraphAdj {
    use std::iter::repeat;
    let iter = repeat(Vec::new()).take(color_count as usize);
    SegmentGraphAdj(iter.collect())
  }

  fn get(&self, a: ColorIdx, b: ColorIdx) -> Option<Height> {
    for (adj_color, height) in self.0[a as usize].iter() {
      if adj_color == b {
        return Some(height);
      }
    }
    return None;
  }

  fn insert(&mut self, a: ColorIdx, b: ColorIdx, new_height: Height) {
    use std::cmp::max;
    for (adj_color, old_height) in self.0[a as usize].iter_mut() {
      if adj_color == b {
        *old_height = max(*old_height, new_height);
        return;
      }
    }
    self.0[a as usize].push((b, new_height));
  }
}

fn create_segment_graph(color_map: &ColorMap, color_count: ColorIdx, borders: &[RoomXY]) -> SegmentGraphAdj {
  for border_xy in borders {

  }
}

pub enum Render {
  Height,
  Colors,
}

/// Generate a nice color for our visuals.
fn calculate_color(size: ColorIdx, color: Color) -> String {
  let index = match color {
    Color::Border | Color::Empty => {
      return "#000000".to_string()
    }
    Color::Pending(_) => {
      error!("color was pending after map finalized");
      return "#FF0000".to_string()
    }
    Color::Resolved(idx) => idx,
  };
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

  /// Convert one of the generated RGB values into an actual
  /// byte value.
  fn convert(val: f64) -> u8 {
    let rgb_percent = val.clamp(0.0, 2.0) / 2.0;
    let rgb_float = rgb_percent * 255.0;
    rgb_float.floor() as u8
  }

  // now we turn our floats into bytes
  // we have to clamp because the YUV color space is bizzare.
  let redb: u8 = convert(red);
  let greenb: u8 = convert(green);
  let blueb: u8 = convert(blue);

  // then we format
  let out = format!("#{:02x}{:02x}{:02x}", redb, greenb, blueb);

  out
}

// TODO: add the ability to pass in a list of sources to cause to unify.
/// Segment a room.
pub fn room_cut(
  terrain: &LocalRoomTerrain,
  visual: RoomVisual,
  render: Render
) -> Vec<RoomXY> {
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
    let num_str = match &render {
      Render::Height => format!("{:?}", height_map.get(xy)),
      Render::Colors => format!("{}", disp_color_map[xy]),
    };
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
