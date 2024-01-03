- you need a certain amount of global control to claim a room?
- need a CPU unlock item to get 10 CPU for every global control level, capped at
  300.
- At RCL 7 and 8 spawn extensions increase in capacity.
- 60 spawn extensions at RCL 8.
- towers are most effective within 5. See `TOWER_OPTIMAL_RANGE`. They have a
  falloff range.
- should set up nuke warnings
- Can I use roads to open up more room around sources? Yes.
- power creeps
  - Power creeps are immortal hero units that can be ressurected after 8 hours,
    and need a special power resource to level up.
- mineral mining vs deposit mining
- highways have resources?
- remote miners
- Minerals: any room, build Extractor structure, send creep to harvest
- Deposit: spawn in highway rooms, send creep to harvest
- truck creeps (ferry) to move resources around
- misc worker for labs and similar
- melee units don't pursue things faster than them
- calculations for optimizing miner configuration
- ally identification and invalidation
- military
  - robbing resources
  - attacking structures
  - medics
  - ranged vs melee
- calculate the optimal mix of units to be using out of a given room
- medic system to heal units
- have miners retreat from hostiles
- Build an alerts system for seeing hostiles
- are the flags custom or not? They are not.
- I need to build profiling tools to tell what approach is best.
- collect data on how frequently I need to restore from memory
- creeps can pull other creeps.
- [steamless-client](https://github.com/laverdet/screeps-steamless-client)
  lets you run a client for a private server from a browser. More up to date I guess?
- is room size different from sim room? no

# Questions
- what are signs on spawns?
- what is the reservations thing?
- Why the fuck is it compiling `serde_json`?

# Metrics
I want metrics for making decisions. Obviously some should come from profiler stuff, but
I haven't written any of that yet.

- CPU spent running creeps
- CPU leftover
- unharvested energy from sources when they regenerate
- harvester container buffer deltas.

# Tools
- some tool to let me run on multiples of ticks. And to avoid overlap between
  things on a timer, so I don't get a ton of things happening the same time.
  - Maybe automatic offset?
  - How does this integrate with my cache tool?
  - I like the way International has a `Sleepable` base class that you can define
    a random sleep range for.

# Documentation
- Write docs for cache
- Write docs for `creeps/memory.rs` covering the macros and such.

# buildings
- you can have multiple spawns per room
- Nukes
- factories
- roads: reduce movement cost to always be 1 no matter what.
- fortifications
  - ramparts
  - towers (heal and attack)
- storage: one per room. need controller level 4
- observers: let you see into a distant room
- containers: let you store resources
- extractor: harvest a mineral

# start playing
- walls
- ramparts
- healing towers
- defense towers
- function to mark current area as a city
- have safe mode tools
- logging

# Current TODO
- Working on max flow min cut for wall planning
  - Need to make the DistMatrix able to efficiently count distance from just
    walls or walls and exits (so walls vs walls and edges).

# Pathing
IMPORTANT: when needing to implement custom search functions/finding the minimum
distance for something, just use `search_many`.

This is the implementation for `RoomPosition.findClosestByPath`:
https://github.com/screeps/engine/blob/master/src/game/rooms.js#L1429

This is the implementation using `Pathfinder` that it will call if `PathFinder`
isn't disabled.
https://github.com/screeps/engine/blob/master/src/game/rooms.js#L304

Future:
- I can pre-cache paths from sources to storage etc. Commonly traveled paths
  should be pre-cached. I can probably even write some custom code that e.g.
  checks if someone is in the way and does a really short navigation around
  them. Or better yet, I could build a traffic orchestrator that grabs everyone
  and tells some people to move etc.
- The constant `OBSTACLE_OBJECT_TYPES` is useful.

# Profiling
I desperately need a good profiler. Look at overmind and steal ideas.

Overmind has a cool decorator trick that I should steal.

# CPU Cost
Functions changing the game state cost .2 CPU.

- I could use a random offset to help avoid caches all being refreshed at the same
  time.

# Memory
`bitcode` is a very powerful and concise encoding system. However, I don't want
to deal with different versions and migrations just yet. So I'm going to use
CBOR (minicbor).

minicbor isn't generic like serde, but it doesn't matter in this situation and is
still very easy to drag and drop back in place.

- I can store creep memory indexed by the hash of their names, instead of by
  their actual names.
- Should I store my stuff as a property on Memory temporarily? That way no fetch delay
  like with the segments.
- I need to setup a thing to prevent raw memory from being parsed every time by the
  built-in memory.
  https://github.com/Mirroar/hivemind/blob/48a9dbdde0f6931546fb1955c1f4c9e0a1bd9359/src/main.ts#L232
- I've added the above code to the javascript part.
- need to cache rust Memory in the heap to avoid re-parsing it every time.

# Rust
- I bet I can create a custom serialization/deserialization for room names.
- add an entries iterator to JsHashMap
- switch over to BTrees; more efficient esp for smaller. And much more efficient
  when inserting one by one.

# Programming Future
- I need to move a bunch of utilities from `tile_slice` to `util`. I can just
  re-export them from `tile_slice`. Maybe make a `xy` submodule of `util` that
  I can `*` import, and then re-export all of them from `util` for individual
  imports.
- I need to do something to fix the godawful mess with positions and roomxy and shit.
- I need to finish the persist macro. But I'm going to work on something else
  right now.
- Tool to auto-deserialize ObjectIds inside my persistent memory.
- I can write a procedural derive macro that uses helper attributes to
  automatically split memory structures into stuff that's serialized and
  deserialized, and stuff that's cached and uses a default(?) value or some
  re-compute function. I'm thinking two attributes for that; `cache(default)`
  and `cache(compute=fn_name)`.
- Look into overriding `Memory._parsed` or whatever to avoid reparsing JSON
  every time.
- Tool for making an std::io::Writer or CBOR writer or whatever (minicbor has an interface
  for std::io::writer IIRC) that writes directly to a buffer in the needed encoding format.
- use that better encoding package someone mentioned in the rust channel.
- Build a generic, re-usable tool for looking for work, and for idling until work is found.
  I could wrap targets in it! Have it basically be a generic cache I can wrap around stuff,
  so when it doesn't have data it looks for how to find the data, but once it does, it's good.
  Can even bake in stuff like other people telling it to do something or only checking every X
  amount of time.
- make a with path in a variant #[persist] annotation an error.
- Checkout library darling which helps parse meta arguments for deriving stuff.
- there's a library error with Reflect.get when using `find_path_to_xy`
- Add an extension trait for Iterator that adds a `closest_by_path` and
  `closest_by_range` function where you pass a function to build a position.
```rust
impl<I> ClosestByIterator for I where I::Item: HasPosition {}
```
- Then do another extension function or impl or whatever that makes this easy
  to use for things implementing `HasPosition` (which includes `Position`).
- Calculating a way to put links between sources efficiently and deciding weather it's
  worth it. I think the answer is that I say that I have X many links, and then do
  a space cutting problem that fills the space around each point of interest until it
  hits midpoints. Then I pick intersections or edges as link locations? No. The problem
  is that I want to turn a single graph into N sub-graphs that are tightly connected.
  Actually maybe it would be a minimum spanning tree problem. No, it's definitely a
  graph cutting problem where I want to cut the graph into N shortest path trees.
