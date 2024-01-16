// To find the minimum cut, we can just look for the set of all verticies connected by
// edges with residual capacity.
//
// we could look for those whose residual capacity is zero -- are fully loaded -- but that means
// that e.g. we'd identify an entire tunnel?

// See the documentation for min cut in international:
// https://github.com/The-International-Screeps-Bot/The-International-Open-Source/blob/Main/src/room/construction/minCut.ts

// TODO: rename TileId to TileIdx. Maybe move to TileSlice.
// TODO: rename NodeId to NodeIdx.

// NOTE: I think the problem right now is with how I define the depth of things. If I go
// and define the capacities by their distance from sources it should maybe work?
//
// No, maybe I need to define my capacities by their distance from the exits!
//
// So I guess I just need to adapt my distance transform code huh.

use core::ops::{Index, IndexMut};
use core::slice::SliceIndex;
use std::collections::{VecDeque, BTreeSet};
use std::cmp::{max, min};

use log::*;
use screeps::{Direction, RoomXY, RoomCoordinate,
              LocalRoomTerrain, LocalCostMatrix, Terrain, RoomVisual};
use enum_iterator::{all, Sequence};

use super::tile_slice::*;

/// The type we're using to represent the capacity of an edge.
type Flow = u16;

/// This means that this edge doesn't exist.
const NO_EDGE: Flow = 0;

const MAX_FLOW: Flow = 1000;

type Edges = [Flow; 8];

/// There are ROOM_AREA tiles, which is 2500, which fits in 12 bits.
type TileIdx = usize;

/*

/// We split every tile into an s and d node, in order to help our tile cut algorithm
/// properly work.
///
/// All primary edges between tiles go from a d node to an s node. Internal edges go from
/// an s node to a d node. However, as part of max-flow min-cut, you need reverse
/// edges.
struct Tile {
/// These are outgoing edges.
d_out_edges: Edges,
/// The primary s to d internal edge.
s_to_d_edge: Flow,
/// The reversed internal edge.
d_to_s_edge: Flow,
/// These are incoming edges; the reverse of the incoming edge.
s_in_edges: Edges,
}

type Tiles = TileSlice<Tile>;

/// A way of referring to a specific edge inside a tile.
enum TileEdge {
  S(Direction),
  SD,
  DS,
  D(Direction),
}
*/

/// We split every tile into an s and d node, in order to help our tile cut algorithm
/// properly work.
#[derive(Copy, Clone, Debug)]
struct Node {
  internal_edge: Flow,
  external_edges: Edges,
}

impl Index<Direction> for Node {
  type Output = Flow;
  fn index(&self, index: Direction) -> &Flow {
    &self.external_edges[index as usize - 1]
  }
}

impl IndexMut<Direction> for Node {
  fn index_mut(&mut self, index: Direction) -> &mut Flow {
    &mut self.external_edges[index as usize - 1]
  }
}

#[derive(Copy, Clone, Debug)]
enum NodeEdge {
  Internal,
  External(Direction)
}

impl NodeEdge {
  fn reverse(self) -> NodeEdge {
    use NodeEdge::*;
    match self {
      Internal => Internal,
      External(dir) => External(dir.multi_rot(4))
    }
  }
}

impl Index<NodeEdge> for Node {
  type Output = Flow;
  fn index(&self, index: NodeEdge) -> &Flow {
    use NodeEdge::*;
    match index {
      Internal => &self.internal_edge,
      External(dir) => &self[dir]
    }
  }
}

impl IndexMut<NodeEdge> for Node {
  fn index_mut(&mut self, index: NodeEdge) -> &mut Flow {
    use NodeEdge::*;
    match index {
      Internal => &mut self.internal_edge,
      External(dir) => &mut self[dir]
    }
  }
}

/// Refer to a s or d node at a tile. 12 bits to determine
/// the tile, and then the 13th is a bit flag for being an
/// s node or d node. 0 is s, 1 is d.
#[derive(Copy, Clone)]
#[repr(transparent)]
struct NodeId {
  raw: usize
}

impl NodeId {
  const FLAG_MASK: usize = 1 << 12;
  const TILE_IDX_MASK: usize = Self::FLAG_MASK - 1;

  #[inline]
  fn d(tile_idx: TileIdx) -> NodeId {
    NodeId { raw: tile_idx | Self::FLAG_MASK }
  }

  #[inline]
  fn s(tile_idx: TileIdx) -> NodeId {
    NodeId { raw: tile_idx }
  }

  #[inline]
  fn d_from_xy(xy: RoomXY) -> NodeId {
    Self::d(xy_to_linear_index(xy))
  }

  #[inline]
  fn s_from_xy(xy: RoomXY) -> NodeId {
    Self::s(xy_to_linear_index(xy))
  }

  #[inline]
  fn counterpart_for(self, xy: RoomXY) -> NodeId {
    let idx = xy_to_linear_index(xy);
    // We XOR with the FLAG_MASK to flip the flag bit, then
    // do an AND to get only that bit.
    let opposite_flag_bit = (self.raw ^ Self::FLAG_MASK) & Self::FLAG_MASK;
    // then we just merge that bit with the tile idx.
    let raw = idx | opposite_flag_bit;
    NodeId { raw }
  }

  /// Flip this node from an s node to a d node or vice versa.
  ///
  /// Uses the XOR operation to flip the flag bit.
  #[inline]
  fn flip(self) -> NodeId {
    NodeId { raw: self.raw ^ Self::FLAG_MASK }
  }

  #[inline]
  fn tile_idx(self) -> TileIdx {
    self.raw & Self::TILE_IDX_MASK
  }

  #[inline]
  fn xy(self) -> RoomXY {
    linear_index_to_xy(self.tile_idx())
  }

  #[inline]
  fn is_d(self) -> bool {
    self.raw & Self::FLAG_MASK != 0
  }

  #[inline]
  fn is_s(self) -> bool {
    self.raw & Self::FLAG_MASK == 0
  }
}

/// Provides the s node and then the d node for the xy coordinates.
fn node_ids_for(xy: RoomXY) -> (NodeId, NodeId) {
  let idx = xy_to_linear_index(xy);
  let s = NodeId::s(idx);
  let d = NodeId::d(idx);
  (s, d)
}

/// Get the flows, edge enum, and id for the other node for all
/// outward going edges from this node.
fn edges_for_node<'a>(
  idx: NodeId, node: &'a Node
) -> impl Iterator<Item = (Flow, NodeEdge, NodeId)> + 'a {
  use std::iter::once;
  let internal = (node.internal_edge, NodeEdge::Internal, idx.flip());
  surrounding_xy_with_dir(idx.xy())
    .map(move |(dir, xy)| {
      let other = idx.counterpart_for(xy);
      (node[dir], NodeEdge::External(dir), other)
    })
    .chain(once(internal))
}

const D_NODES_START: usize = 1 << 12;
const NODES_LEN: usize = D_NODES_START + ROOM_AREA;

/// We store the nodes in the memory with the s and d nodes
/// directly indexed by the raw [`NodeId`].
///
/// This means it only takes up 2^12 + ROOM_AREA slots, with
/// 2^12 - ROOM_AREA un-used between the end of the s
/// nodes and start of the d nodes.
///
/// We could instead pack them right next to each other, and
/// put the flag bit at the bottom, which would prevent
/// the wasted space.
///
/// TODO: once this code is written consider doing that.
struct Nodes {
  slice: [Node; NODES_LEN]
}

impl Nodes {
  fn new() -> Box<Nodes> {
    Box::new(Nodes {
      slice: [Node {
        external_edges: [0; 8],
        internal_edge: 0,
      }; NODES_LEN]
    })
  }
}

impl Index<NodeId> for Nodes {
  type Output = Node;
  fn index(&self, index: NodeId) -> &Node {
    &self.slice[index.raw]
  }
}

impl IndexMut<NodeId> for Nodes {
  fn index_mut(&mut self, index: NodeId) -> &mut Node {
    &mut self.slice[index.raw]
  }
}

#[derive(Copy, Clone)]
enum ParentVal {
  /// An edge going in one direction to another tile.
  /// We can tell whether it points to an s or d node
  /// based on whether the node we are at is an s or d
  /// node.
  ///
  /// The direction is from the child to the parent. The u16 is the
  /// parent.
  ExternalEdge(Direction, NodeId),
  /// An edge between an s and d node.
  InternalEdge,
  /// The node this is a key for hasn't been visited yet
  NotVisited,
  /// This is a source node
  IsSource
}

#[repr(transparent)]
struct Parents{
  slice: [ParentVal; NODES_LEN]
}

impl Parents {
  fn new() -> Box<Parents> {
    Box::new(Parents {
      slice: [ParentVal::NotVisited; NODES_LEN]
    })
  }
}

impl Index<NodeId> for Parents {
  type Output = ParentVal;
  fn index(&self, index: NodeId) -> &ParentVal {
    &self.slice[index.raw]
  }
}

impl IndexMut<NodeId> for Parents {
  fn index_mut(&mut self, index: NodeId) -> &mut ParentVal {
    &mut self.slice[index.raw]
  }
}

#[inline]
fn setup_nodes(
  cost_matrix: &LocalCostMatrix,
  sources: &[RoomXY]
) -> Box<Nodes> {
  let mut nodes = Nodes::new();

  for xy in all_room_xy() {
    let cost = cost_matrix.get(xy);
    if cost == 255 {
      continue
    }
    let idx = xy_to_linear_index(xy);
    let s = NodeId::s(idx);
    let d = NodeId::d(idx);

    // we only set the internal edges for s nodes; d->s is the reserve
    // capacity edge and is thus 0.
    nodes[s].internal_edge = 20;

    // we only set the external edges for d nodes.
    for (dir, neighbor_xy) in surrounding_xy_with_dir(xy) {
      if cost_matrix.get(neighbor_xy) == 255 {
        continue
      }
      // We make the capacities between flows really large so that
      // the min cut will happen inside a tile.
      nodes[d][dir] = MAX_FLOW;
    }
  }

  for &xy in sources {
    let (s,d) = node_ids_for(xy);
    nodes[s].internal_edge = MAX_FLOW;
    for (dir, neighbor_xy) in surrounding_xy_with_dir(xy) {
      if cost_matrix.get(neighbor_xy) == 255 {
        continue
      }
      nodes[d][dir] = MAX_FLOW;
    }
  }

  nodes
}

// NOTE: I don't know if I should implement is_exit with the cost
// matrix and coordinates, or a set, or a tile slice of booleans.
// maybe a tile slice of booleans.

/// Returns the minimum flow along this path to an exit.
fn bfs(
  exits: &TileSlice<bool>,
  sources: &[RoomXY],
  nodes: &mut Nodes,
  parents: &mut Parents
) -> Option<(Flow, NodeId)> {
  use ParentVal::*;
  // clear the parents
  parents.slice.fill(NotVisited);

  let mut queue: VecDeque<(NodeId, Flow)> = VecDeque::with_capacity(50);

  // setup the sources
  for xy in sources {
    let (s, d) = node_ids_for(*xy);
    queue.push_back((s, 20));
    parents[s] = IsSource;
  }

  while let Some((cur_idx, flow)) = queue.pop_front() {
    let opposite = cur_idx.flip();
    let cur_xy = cur_idx.xy();
    let cur_node: &mut Node = &mut nodes[cur_idx];
    /*
    debug!("#{}: {cur_xy} Current node {cur_node:?} is s {:?}, not visited opposite {}",
           queue.len(), cur_idx.is_s(), matches!(parents[opposite], NotVisited));
    */

    // check the internal edge.
    if cur_node.internal_edge != 0 {
      // the invert of the s/d node
      if matches!(parents[opposite], NotVisited) {
        let new_flow = min(flow, cur_node.internal_edge);
        parents[opposite] = InternalEdge;
        queue.push_back((opposite, new_flow));
      }
    }

    // check the external edges
    for (dir, adj_xy) in surrounding_xy_with_dir(cur_xy) {
      if cur_node[dir] == 0 {
        continue
      }

      let adj_idx = cur_idx.counterpart_for(adj_xy);

      if matches!(parents[adj_idx], NotVisited) {
        parents[adj_idx] = ExternalEdge(dir, cur_idx);
        let new_flow = min(flow, cur_node[dir]);

        if *xy_access(adj_xy, exits) {
          // debug!("reached exit {adj_xy:?}");
          return Some((new_flow, adj_idx))
        } else {
          queue.push_back((adj_idx, new_flow))
        }
      }
    }
  }

  // debug!("did not reach exit");
  return None
}

fn dfs_get_cut(
  nodes: &Nodes,
  cost_matrix: &LocalCostMatrix,
  sources: &[RoomXY]
) -> Vec<RoomXY> {
  // TODO: make this a normal vector.
  let mut ret: BTreeSet<(RoomCoordinate, RoomCoordinate)> = BTreeSet::new();
  let mut visited = Box::new([false; ROOM_AREA]);
  // TODO: I think that part of my problem is that I'm fully searching the
  // graph instead of only checking reverse capacities.
  let mut stack: Vec<RoomXY> = sources.iter()
    .copied()
    .collect();

  for &xy in stack.iter() {
    visited[xy_to_linear_index(xy)] = true;
  }

  // NOTE: I think I could maybe do this by just doing a dfs
  // on d nodes and checking the d node internal? Not sure. No,
  // I think that might fail too.
  while let Some(cur_xy) = stack.pop() {
    let (s,d) = node_ids_for(cur_xy);
    let d_node = &nodes[d];

    if nodes[s].internal_edge == 0 {
      // we record this as a place to build a wall
      let pair = (cur_xy.x, cur_xy.y);
      if ret.contains(&pair) {
        warn!("Reached {cur_xy} despite having already added it to ret");
        return vec![]
      }
      ret.insert(pair);
    }

    for adj_xy in surrounding_xy(cur_xy) {
      let tile_idx = xy_to_linear_index(adj_xy);
      // Skip if it's a wall
      if cost_matrix.get(adj_xy) == 255 {
        continue
      }
      if visited[tile_idx] {
        continue
      }
      visited[tile_idx] = true;
      // we travel here
      stack.push(adj_xy);
    }
  }
  debug!("walls found {}", ret.len());

  ret.into_iter().map(|(x,y)| RoomXY { x, y}).collect()
}

/// Build the lookup table for whether a tile is an exit or not.
fn build_exits(cost: &LocalCostMatrix) -> Box<TileSlice<bool>> {
  let mut exits: Box<TileSlice<bool>> = Box::new([false; ROOM_AREA]);
  debug!("build exits");

  for xy in room_edges_xy() {
    if cost.get(xy) != 255 {
      let idx = xy_to_linear_index(xy);
      exits[idx] = true;
      for adj in surrounding_xy(xy) {
        if cost.get(adj) != 255 {
          exits[xy_to_linear_index(adj)] = true;
        }
      }
    }
  }

  exits
}

/// Used to identify the coordinates we should build walls to cut the sources
/// off of the exits.
///
/// 255 in the local cost matrix means a wall.
///
/// We'll just implement the algorithm as if there's a super source.
pub fn min_cut_to_exit(
  sources: &[RoomXY],
  cost: &LocalCostMatrix,
  visual: &RoomVisual,
) -> Vec<RoomXY> {
  use ParentVal::*;
  debug!("test");
  let mut parents: Box<Parents> = Parents::new();
  let mut nodes: Box<Nodes> = setup_nodes(cost, sources);
  let mut exits = build_exits(cost);

  for xy in sources {
    let s = NodeId::s_from_xy(*xy);
    debug!("cost for s node in {xy:?} is {}", nodes[s].internal_edge);
  }

  // Perform the max flow diagram
  while let Some((new_flow, exit_idx)) = bfs(&*exits, sources, &mut *nodes, &mut *parents) {
    if new_flow == 0 {
      break
    }
    let mut cur_idx = exit_idx;
    loop {
      match parents[cur_idx] {
        IsSource => break,
        NotVisited => panic!("Shouldn't be possible"),
        ExternalEdge(parent_to_child_dir, parent_idx) => {
          let child_to_parent_dir = parent_to_child_dir .multi_rot(4);
          nodes[parent_idx][parent_to_child_dir] -= new_flow;
          nodes[cur_idx][child_to_parent_dir] += new_flow;
          cur_idx = parent_idx;
        }
        InternalEdge => {
          let parent_idx = cur_idx.flip();
          nodes[parent_idx].internal_edge -= new_flow;
          nodes[cur_idx].internal_edge += new_flow;
          cur_idx = parent_idx;
        }
      }
    }
  }

  for xy in all_room_xy() {
    let (s_idx, d_idx) = node_ids_for(xy);
    for dir in all::<Direction>() {
      let val = nodes[d_idx][dir];
      if val > 50 && !matches!(parents[d_idx], IsSource) {
        debug!("at {xy} in {dir:?} the value is {val}");
      }
    }
    let num = format!("{:?}", nodes[s_idx].internal_edge);
    let (x,y): (u8,u8) = xy.into();
    let fx: f32 = x.into();
    let fy: f32 = y.into();

    if !exits[s_idx.tile_idx()] {
      visual.text(fx, fy, num, None);
      /*
      for dir in all::<Direction>() {
        use Direction::*;
        use screeps::RectStyle;
        const STEP: f32 = 0.33;
        let val: f32 = nodes[d_idx][dir].into();
        let ratio = val / 20.0;
        let number = (ratio * 100.0) as i32;
        let color = format!("#7777{}", number);
        let style = RectStyle::default()
            .fill(&color);
        let (offset_x, offset_y): (f32, f32) = match dir {
          Left => (0.0, -STEP),
          Right => (0.0, STEP),
          Bottom => (0.0, -STEP),
          Top => (0.0, STEP),
          TopRight => (STEP, STEP),
          TopLeft => (-STEP, STEP),
          BottomRight => (STEP, -STEP),
          Bottomleft => (-STEP, -STEP),
        };
        let dx = fx + offset_x;
        let dy = fy + offset_y;
        let left_x = dx - (STEP/2.0);
        let right_x = dy - (STEP/2.0);
        visual.rect(left_x, right_x, STEP, STEP, Some(style));
      }
      */
    }
  }

  dfs_get_cut(&*nodes, cost, sources)
}

/// This is temporary for now. Later it will be produced by our room planning
/// and include things like building locations(?).
pub fn build_cost_matrix(terrain: &LocalRoomTerrain) -> LocalCostMatrix {
  let mut cost = LocalCostMatrix::new();
  for (xy, val) in cost.iter_mut() {
    let offset_x = xy.x.u8().abs_diff(25);
    let offset_y = xy.y.u8().abs_diff(25);
    let dist = max(offset_x, offset_y);
    *val = match terrain.get(xy) {
      Terrain::Wall => 255,
      Terrain::Plain => dist,
      Terrain::Swamp => dist,
    };
  }
  cost
}
