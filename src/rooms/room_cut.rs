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
//!
//! NOTE: there's a minor situation where we can have suboptimal wall placement:
//! 0 1 2 1 0 0
//! 0 1 2 2 1 0
//! In this situation, it currently can't tell that the 3 wide chokepoint is
//! superior to the 4 wide chokepoint.

// NOTE: I think there's an efficient priority flood algorithm I could use but I
// don't care. https://arxiv.org/abs/1511.04463

use std::assert_matches;
use std::collections::HashMap;
use std::fmt;
use std::ops::{Index, IndexMut, Range};
use std::cmp::{max, min};
use std::collections::{VecDeque, BTreeSet, HashSet, BTreeMap};
use std::hash::Hash;

use itertools::iproduct;
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
          borders.push(xy);
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

/// Contains information about a border between two segments.
#[derive(Clone, Debug, Default)]
struct SegmentBorder {
  walls: Vec<RoomXY>
}

impl SegmentBorder {
  #[inline]
  fn add_wall(&mut self, xy: RoomXY) {
    self.walls.push(xy);
  }

  #[inline]
  fn len(&self) -> usize {
    self.walls.len()
  }

  #[inline]
  fn walls(&self) -> impl Iterator<Item = RoomXY> {
    self.walls.iter()
  }
}

/// Data structure representing information about the borders
/// between segments.
///
/// We unique the index by ordering them.
struct SegmentBorders(BTreeMap<(ColorIdx, ColorIdx), SegmentBorder>);

/// Quick utility function.
fn unique_pairs(range: Range<usize>) -> impl Iterator<Item = (usize, usize)> {
  use std::iter::repeat;
  range.clone().flat_map(move |i| repeat(i).zip((i+1)..range.end))
}

impl SegmentBorders {
  fn new(
    color_map: &ColorMap,
    border_xys: &[RoomXY]
  ) -> SegmentBorders {
    use Color::*;
    use itertools::Itertools;

    #[inline]
    fn adj_color_pairs(
      color_map: &ColorMap,
      neighbors: impl Iterator<Item = RoomXY>
    ) -> impl Iterator<Item = (ColorIdx, ColorIdx)> {
      let adj_colors: Vec<ColorIdx> = neighbors
        .filter_map(|adj| match color_map[adj] {
          Resolved(color) => Some(color),
          _ => None,
        })
      // it would probably be faster to insert and use .contains
      // since this uses a hash map, but not worth it.
        .unique()
        .collect();
      unique_pairs(0..adj_colors.len())
        .map(move |(i,j)| (adj_colors[i], adj_colors[j]))
    }

    let mut seg_borders = SegmentBorders(BTreeMap::new());
    for border_xy in border_xys {
      // we do two rounds: an initial round using taxicab and creates
      // borders if they don't exist already, and then a second that
      // only adds to existing borders.

      // we use taxicab adjacent so that in this situation:
      // 1 1 1 E 2
      // B B B E 2
      // 3 3 B 2 2
      // we don't end up with 1 connected to 2.
      // However, we still want  the corner piece to be included in
      // 1-3 and 3-2. This is done in the second phase.
      for (a,b) in adj_color_pairs(color_map, taxicab_adjacent(*border_xy)) {
        seg_borders.get_mut_or_default(a, b);
      }

      for (a,b) in adj_color_pairs(color_map, surrounding_xy(*border_xy)) {
        if let Some(border) = seg_borders.get_mut(a, b) {
          border.add_wall(*border_xy);
        }
      }
    }

    seg_borders
  }

  fn get_mut_or_default(&mut self, a: ColorIdx, b: ColorIdx) -> &mut SegmentBorder {
    let idx = if a < b { (a,b) } else { (b,a) };
    self.0.entry(idx).or_default()
  }

  fn get_mut(&mut self, a: ColorIdx, b: ColorIdx) -> Option<&mut SegmentBorder> {
    let idx = if a < b { (a,b) } else { (b,a) };
    self.0.get_mut(&idx)
  }

  #[inline]
  fn iter(&self) -> impl Iterator<Item = (ColorIdx, ColorIdx, &SegmentBorder)> {
    self.0.iter().map(|((a,b), border)| (*a, *b, border))
  }
}

/// This is an adjacency list based graph of room segments.
///
/// TODO: try using this instead of the 2d array implementation and profile.
struct SegmentGraphAdj(Vec<Vec<(ColorIdx, Height)>>);

impl SegmentGraphAdj {
  fn new(color_count: ColorIdx) -> SegmentGraphAdj {
    use std::iter::repeat;
    let iter = repeat(Vec::new()).take(color_count as usize);
    SegmentGraphAdj(iter.collect())
  }

  fn get(&self, a: ColorIdx, b: ColorIdx) -> Option<Height> {
    for (adj_color, height) in self.0[a as usize].iter() {
      if *adj_color == b {
        return Some(*height);
      }
    }
    return None;
  }

  fn insert(&mut self, a: ColorIdx, b: ColorIdx, new_height: Height) {
    use std::cmp::max;
    for (adj_color, old_height) in self.0[a as usize].iter_mut() {
      if *adj_color == b {
        *old_height = max(*old_height, new_height);
        return;
      }
    }
    self.0[a as usize].push((b, new_height));
  }
}

/// Enum for whether a segment node is a source -- somewhere
/// we want to build -- a sink -- next to exits -- or just
/// a normal intermediary node.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SegmentKind {
  Source,
  Sink,
  Normal
}

/// Create a map indicating whether a colored
/// segment of the room is a source, sink, or neither.
///
/// If some of the sources are located in a room that
/// is next to exits, it's labeled as a sink, because
/// we'll instead just build walls around the buildings
/// themselves instead of trying to cut stuff off at a
/// border.
fn make_segment_kind_map(
  color_count: ColorIdx,
  color_map: &ColorMap,
  sources: &[RoomXY]
) -> Vec<SegmentKind> {
  use SegmentKind::*;
  let mut segment_kinds = vec![Normal; color_count as usize];

  // Mark the sources
  for xy in sources.iter().copied() {
    match color_map[xy] {
      Color::Resolved(idx) => {
        segment_kinds[idx as usize] = Source;
      },
      Color::Border => {
        // I guess we just add all adjacent rooms to this.
        for adj_xy in surrounding_xy(xy) {
          match color_map[xy] {
            Color::Resolved(idx) => {
              segment_kinds[idx as usize] = Source;
            },
            _ => (),
          }
        }
      }
      Color::Empty => {
        warn!("source {xy} is a wall");
      }
      Color::Pending(_) => {
        panic!("shouldn't be any pending colors at this stage");
      }
    }
  }

  // Mark the sinks (segments next to exits). Important that
  // we do this after sources; see fn docs.
  for xy in room_edges_xy() {
    match color_map[xy] {
      Color::Resolved(idx) => {
        segment_kinds[idx as usize] = Sink;
      },
      _ => (),
    }
  }

  segment_kinds
}

struct SegmentGraph {
  /// Number of colors in the graph.
  color_count: ColorIdx,
  /// Mapping of segments to their kind.
  segment_kinds: Vec<SegmentKind>,
  /// the actual 2d array backing this graph.
  graph: Vec<Height>
}

// TODO: once the bunker vs room border logic is built
// come back and check on this.
impl SegmentGraph {
  fn new(
    color_count: ColorIdx,
    color_map: &ColorMap,
    sources: &[RoomXY]
  ) -> SegmentGraph {
    let size = color_count as usize;

    let mut graph = SegmentGraph {
      color_count,
      segment_kinds: make_segment_kind_map(
        color_count, color_map, sources),
      graph: vec![0; size*size],
    };

    max_flow_transform(&mut graph);

    graph
  }

  #[inline]
  fn internal_index(&self, a: ColorIdx, b: ColorIdx) -> usize {
    a as usize * self.color_count as usize + b as usize
  }

  fn get(&self, a: ColorIdx, b: ColorIdx) -> Option<Height> {
    let idx = self.internal_index(a, b);
    Some(self.graph[idx]).filter(|height| *height != 0)
  }

  fn insert(&mut self, a: ColorIdx, b: ColorIdx, new_height: Height) {
    let idx = self.internal_index(a, b);
    self.graph[idx] = max(self.graph[idx], new_height);
  }

  fn adjacent<'a>(&'a self, node: ColorIdx) -> impl Iterator<Item = (ColorIdx, Height)> + 'a {
    (0..self.color_count).filter_map(move |idx| {
      self.get(node, idx).map(|height| (idx, height))
    })
  }

  #[inline]
  fn segment_kind(&self, node: ColorIdx) -> SegmentKind {
    self.segment_kinds[node as usize]
  }

  fn segments_of_kind<'a>(&'a self, kind: SegmentKind) -> impl Iterator<Item = ColorIdx> + 'a {
    (0..self.color_count).filter(move |idx| self.segment_kind(*idx) == kind)
  }

  fn is_cut_edge(&self, a: ColorIdx, b: ColorIdx) -> bool {
    let ab = self[(a,b)];
    let ba = self[(b,a)];
    a != b && min(ab, ba) == 0 && max(ab, ba) > 0
  }
}

impl Index<(ColorIdx, ColorIdx)> for SegmentGraph {
  type Output = Height;
  fn index(&self, (a,b): (ColorIdx, ColorIdx)) -> &Height {
    &self.graph[self.internal_index(a, b)]
  }
}

impl IndexMut<(ColorIdx, ColorIdx)> for SegmentGraph {
  fn index_mut(&mut self, (a,b): (ColorIdx, ColorIdx)) -> &mut Self::Output {
    let idx = self.internal_index(a, b);
    &mut self.graph[idx]
  }
}

impl fmt::Display for SegmentGraph {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for a in 0..self.color_count {
      write!(f, "{}->", a)?;
      for b in 0..self.color_count {
        let ab = self[(a,b)];
        let ba = self[(b,a)];
        if max(ab, ba) > 0 {
          write!(f, " {b}: {ab}, {ba};")?;
        }
      }
      write!(f, "\n")?;
    }
    Ok(())
  }
}

/// Build the initial form of the room segment graph we'll perform min cut on.
fn create_segment_graph(
  color_map: &ColorMap,
  color_count: ColorIdx,
  borders: &SegmentBorders,
  height_map: &DistMatrix,
  sources: &[RoomXY]
) -> SegmentGraph {
  use Color::*;
  let mut graph = SegmentGraph::new(
    color_count, color_map, sources);
  for (a, b, border) in borders.iter() {
    let height = border.len() as u8;
    graph.insert(a, b, height);
    graph.insert(b, a, height);
  }
  graph
}

/// Parents for the min cut max flow algorithm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SegmentParent {
  NotVisited,
  Segment(ColorIdx),
  IsSource
}

/// Returns the smallest edge found between two nodes in
/// this path to an exit.
fn max_flow_bfs(
  graph: &mut SegmentGraph,
  parents: &mut [SegmentParent]
) -> Option<(ColorIdx, Height)> {
  debug!("Started max flow bfs");
  use std::cmp::min;
  use SegmentParent::*;
  parents.fill(NotVisited);
  let mut queue: VecDeque<(ColorIdx, Height)> = VecDeque::new();

  for source_idx in graph.segments_of_kind(SegmentKind::Source) {
    parents[source_idx as usize] = IsSource;
    queue.push_back((source_idx, 200)); // might need to tweak this.
  }

  while let Some((idx, cur_flow)) = queue.pop_front() {
    for (adj_idx, height_to_adj) in graph.adjacent(idx) {
      if parents[adj_idx as usize] == NotVisited {
        parents[adj_idx as usize] = Segment(idx);
        let new_flow = min(cur_flow, height_to_adj);
        if graph.segment_kind(adj_idx) == SegmentKind::Sink {
          return Some((adj_idx, new_flow));
        }
        queue.push_back((adj_idx, new_flow));
      }
    }
  }

  // Default
  None
}

/// Run the edwards-karp max flow finding algorithm
/// on the segment graph.
fn max_flow_transform(
  graph: &mut SegmentGraph
) {
  let mut parents = vec![SegmentParent::NotVisited; graph.color_count as usize];
  let mut max_flow: Height = 0;

  while let Some((sink_idx, flow)) = max_flow_bfs(graph, &mut parents) {
    if flow == 0 {
      // I don't think this should happen
      warn!("Flow found by max_flow_bfs was 0");
      break;
    }
    max_flow += flow;
    let mut cur_idx = sink_idx;
    while graph.segment_kind(cur_idx) != SegmentKind::Source {
      match parents[cur_idx as usize] {
        SegmentParent::Segment(prev_idx) => {
          graph[(prev_idx, cur_idx)] -= flow;
          graph[(cur_idx, prev_idx)] += flow;
          cur_idx = prev_idx;
        }
        _ => panic!("Shouldn't be possible"),
      }
    }
  }
}

/// Create a list of coordinates where border walls should be built.
fn border_walls_to_build(
  graph: &SegmentGraph,
  borders: &SegmentBorders
) -> impl Iterator<Item = RoomXY> {
  let cut_edges = iproduct!(0..graph.color_count, 0..graph.color_count)
    .filter(|(a,b)| {
      let a = *a;
      let b = *b;
      let ab = graph[(a,b)];
      let ba = graph[(b,a)];
      a != b && min(ab, ba) == 0 && max(ab, ba) > 0
    });

  let mut walls: HashSet<RoomXY> = HashSet::new();

  for (a, b) in cut_edges {
    for wall in borders.get_mut_or_default(a, b).iter() {
      walls.insert(*wall)
    }
  }

  walls.into_iter()
}

/// Mark which segments are inside the walls and which aren't.
fn mark_inside_segments(
  &graph: SegmentGraph
) -> Vec<bool> {
  let mut inside = vec![true; graph.color_count as usize];

  let mut stack: Vec<ColorIdx> = graph
    .segments_of_kind(SegmentKind::Sink)
    .collect();

  while let Some(cur_idx) = stack.pop() {
    inside[cur_idx as usize] = false;
    for adj_idx in 0..graph.color_count {
      // if we haven't visited this yet, inside will still be true.
      if inside[adj_idx as usize] || !graph.is_cut_edge(cur_idx, adj_idx) {
        inside[adj_idx as usize] = false;
      }
    }
  }

  inside
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
///
/// Sources can be in the same room segment.
pub fn room_cut(
  terrain: &LocalRoomTerrain,
  visual: RoomVisual,
  render: Render,
  sources: &[RoomXY]
) -> impl Iterator<Item = RoomXY> {
  let height_map = DistMatrix::new_taxicab(terrain);
  // let maximas: Vec<Maxima> = find_maxima(&dist_matrix).collect();
  let (maxima_iter, mut maxima_dts) = calculate_local_maxima(&height_map);
  let maxima_set: HashSet<RoomXY> = maxima_iter.collect();
  let maxima_colors = color_disjoint_tile_set(&height_map, &mut maxima_dts);
  let (color_map, color_count, borders) = flood_color_map(
    &height_map, maxima_set.iter().copied());

  let segment_borders = SegmentBorders::new(&color_map, &borders);

  let mut segment_graph = create_segment_graph(
    &color_map, color_count, &segment_borders, &height_map, sources);

  let (disp_color_map, disp_color_count) = (color_map, color_count); // maxima_colors;

  debug!("Segment Graph:\n{segment_graph}");
  let walls = border_walls_to_build(&segment_graph, &segment_borders);

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

  walls
}
