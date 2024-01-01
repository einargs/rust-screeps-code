# Math
The road ratio means that to maintain a constant 1 tick speed, we want to have
every CARRY part have a MOVE part, and every 2 WORK parts to have a MOVE part.

## Hauler Math
We're going to just graph this and see what values maximize it. Or I could bust
out the multivariate calculus which would be fun!

Optimizing:
I'd just need to calculate the gradiant and see where it's zero and whether that's
true or not? For multi-variable I just need the gradiant, which is a vector of the
function with respect to each component, then set those to zero and solve.

I guess max_cost I can fix, and then `step_time` and `distance` I can vary.

```
# 50 energy to build both CARRY and MOVE
part_cost: cost/part = 50
# maximum cost we can spend on a hauler
max_cost: cost = input
# Number of ticks to move one square on a road
step_time: tick/tile = input
# Distance along a road to travel
distance: tile = input
# number of CARRY parts per MOVE part we can have and still maintain step_time
# when fully loaded.
carry_ratio: scalar = 1*step_time
# Number of parts in a body segment/unit -- N CARRY + 1 MOVE
segment_size: part = carry_ratio + 1
# Total number of segments
total_segments: part = max_cost / (segment_size * part_cost)
# Resources each CARRY part can carry
part_capacity: energy/part = 50
# Number of CARRY parts
carry_part_num: part = carry_ratio * total_segments
# Energy capacity
capacity: energy = part_capacity * carry_part_num
# time to travel the distance
duration: tick = distance*speed
# Amount of energy hauled along a road per tick
energy/tick = capacity/duration
```

## Harvester Math
```
# energy mined per WORK part
m: energy/part = 2
```

## TODO
- I need to do the math on towers vs roving wall/rampart repairers.

## Cut Problem Description
I have a graph algorithms question. I have a 2d plane of fixed size filled with regular tiles.
Each tile connects to the eight tiles surrounding it; four adjacent and four in the corners.
Each tile can be a wall or a floor. I have a small set of nodes (n<6) that cannot be on the
edge of the planar space; I'll call them sources. I then have a larger set of nodes that
can only exist on the edges of this 2d planar space that I'll call sinks. I want to find the
minimum set of tiles I need to change from a floor to a wall to cut the sources off from the
sinks. Sinks are defined by being a floor tile on the edge of the 2d plane.

The floor and wall tiles aren't evenly distributed; they tend to cluster together, forming
rooms of floors.

Obviously, this can be solved using max-flow min-cut. However, it feels like there should be
a way to optimize it further taking advantage of the fact that the graph can be embedded into
a plane, is unweighted and that the tiles have distance between them (I think the mathematical
way to say it would be that the vertices form a metric space?).

Reading up on the subject I found my way towards the planar separator theorem, tree decomposition,
and delauny triangulation among others. At first I was excited by planar separation, but it
doesn't actually seem to solve my problem. Googling and looking through papers generally led
me to papers about optimizing max-flow min-cut for computer vision.

### My (Bad) Approach
I could run a distance transform using taxicab distance (so that it tracks the number of walls
to cut it off) and then use the watershed algorithm to identify rooms of open floors and edges
betweeen them. This just creates a new graph then, though a far smaller one. Because the shapes
of rooms are irregular, you would have multiple maximums within a large room. So I would want
to apply a heuristic to merge them.

At that point I think it still becomes just a max-flow min-cut problem, though on a far smaller
graph. Which I think is probably still a good optimization.

## Useful Math
- minimum spanning tree
- shortest path tree
- nesting algorithm (fitting base components)

from: https://wiki.screepspl.us/index.php/Automatic_base_building#Useful_Algorithms
- minimum cut
- distance transform
- flood fill

Maybe
- minimum bottleneck spanning tree: spanning tree that minimizes the size of
  largest edge (minimizes bottleneck).
- what was that thing for finding sub graphs? Oh, it was strongly connected sub-graphs.
- Voronoi diagrams
- computational geometry
- sweep the line algorithms

# Management
- Always need to watch the room controller to see if it's going to decay soon
  and needs to be healed.
- Need to have creeps predict whether they need to be refreshed at a spawn
  before going off to do something.
- Trigger safe mode if enemies show up and I have safe mode resources available.
  - In future, instead trigger if they're doing a lot of damage.
  - Some kind of attack severity rating system that governs safe mode deployment,
    pulling creeps and resources from elsewhere, etc.
  - Ranged attackers for attacking over walls.
- Need to split creeps between fueling the spawner, upgrading the controller, etc.
  I could use a priority system where different tasks are assigned priorities? Then
  turn that into percentages?
- Reservations: uses claim body parts to increase a timer preventing others from
  taking over a neutral room controller. Enemies can use CLAIM to downgrade it.
  CLAIM body parts cap lifespan at 600 ticks and prevent renewal.
  See: https://docs.screeps.com/api/#Creep.reserveController
- When under attack I can have creeps construct ramparts behind walls to take cover
  beneath in order to launch ranged attacks.

# Planning
I think hivemind is using some kind of A* for room planning, since it uses open
and closed lists.

Use the room visualization stuff and the private server to test my room planning.

- has useful algorithms and info about tilesets.
  [wiki](https://wiki.screepspl.us/index.php/Automatic_base_building#Stamp/Tile-sets)
- One approach people use is to have bunkers that they can modularly build.
  - they have tilesets of things, and build around an anchor.
- Roads are 45,000 energy to build on a wall, so not something worth doing until later
  if at all.
- creeps can stand on containers
- Roads make a body part only generate 1 fatigue, instead of 2 fatigue on plains or 10
  fatigue on swamp.
- you can build roads through ramparts. You can even have ramparts on top of some things,
  like IIRC towers? Definitely labs.
- ramparts can be temporary things you only build in times of need to block off choke
  points created by other walls? I think I'll have a handful of ramparts that I maintain
  at all times, at least at first.

## Room Control Level Table
This table lists the room control level and all of the different benefits gained at each
new RCL.
| RCL | Benefit                                                                      |
|-----|------------------------------------------------------------------------------|
| 0   | 5 Containers                                                                 |
| 1   | 1 Spawn                                                                      |
| 2   | 5 Extensions (50 capacity), Ramparts (300K max hits), Walls                  |
| 3   | 10 Extensions, Ramparts (1M max hits), 1 Tower                               |
| 4   | 20 Extensions, Ramparts (3M max hits), Storage                               |
| 5   | 30 Extensions, Ramparts (10M max hits), 2 Towers, 2 Links                    |
| 6   | 40 Extensions, Ramparts (30M max hits), 3 Links, Extractor, 3 Labs, Terminal |
| 7   | 2 Spawns, 50 Extensions (100 capacity), Ramparts (100M max hits),            |
|     | 3 Towers, 4 Links, 6 Labs                                                    |
| 8   | 3 Spawns, 60 Extensions (200 capacity), Ramparts (300M max hits), 6 Towers,  |
|     | 6 Links, 10 Labs, Observer, Power Spawn, Nuker                               |

## International Notes
- in `communePlanner.ts`, `findFastFillerOrigin` generates a list of origins based on the
  room controller and sources. Each plan attempt uses a different one of these.
- Q: what is the grid?
- It seems like International can have things outside the mincut walls.

## Brainstorming
- I can put spawn extensions closer to sources. But that'll just be a bonus thing, not
  a main focus.
- My road network should be a shortest-path tree that minimizes total edge weight.
  Basically I want to make sure that it re-uses large chunks of road.
- Do I want multiple separate spawns, or all of my spawns together?
- Tower placement I want to define an algorithm that finds
- Later I could add a feature that says "oh, too many walls I'll use a bunker approach
  instead of walling everything in," but I actually don't think that's necessary. Walls
  don't decay.
  - Actually no, more walls means towers are more spread out, and also more enemy creeps
    can attack at once.

Select base location:
- Get location of the room controller, sources, and mineral. Basically stuff we want inside the
  base
- Generate a room plan containing the room controller and varying numbers of the others inside
  the walled in portion of the base. So room controller and everything else, room controller
  and just one source. Minimum is room controller and a source.
- Score these based on rampart upkeep, wall surface area, etc.
  - Eventually could work in a "danger level" metric that influences it more towards not having
    anything unprotected, but not worth it.

Generate room plan
- Do distance transform algorithm to find good open areas near the selected sources
  - Or just place it near the room controller?
  - Both. Distance transform and then look for a local maximum.
- Generate the sketch room plan
  - What's imporant
- perform min cut algorithm to find good chokepoints around there. Keep safe distance from
  the buildings? I guess I can always cover some with ramparts if they're in range.

- min cut to find most defensible area
- distance fill to find promising, wide open areas.
- then use modular chunks to expand outwards in greedy fashion? Or maybe reserve some
  for important stuff like labs and factories. I'm only going to worry about spawn extensions
  for now though.
  - Can use nesting algorithm to fit them all in!

## Plan
- Energy intake is a good metric I can use for transitioning between stages.

### Manager
- Have a way to indicate whether a building should have a rampart built on top of it or not.
- limit on max number of construction sites at a time.
- When creating the entire plan and registering different structures, I should register the
  RCL (or other metrics) to use to determine when to start working on it. Other metrics could
  be stuff like energy intake? I think I want to just use RCL at first.
- for my first planner I want to generate a single static plan and not worry about e.g.
  dynamically updating it.
- When there aren't any construction sites, we just ask the manager to generate more.

### First Base

Early Phase
- Uses general purpose EarlyWorkers to build extensions, roads, etc.
- Ends before Room Control 2.
- Builds roads from all safe sources.

### Expansion
When expanding from a cold start, it's good to quickly grab several rooms for
fast expansion. So I guess expansion should start once the early phase is done?

I can categorize different rooms by their development level and say that if X
room is developed it can go send aid to a less developed room.

### Remote Mining
- I need to plan roads across room boundaries.
