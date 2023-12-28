# Management
- Always need to watch the room controller to see if it's going to decay soon
  and needs to be healed.
- Need to have creeps predict whether they need to be refreshed at a spawn
  before going off to do something.
- Trigger safe mode if enemies show up and I have safe mode resources available.
  - In future, instead trigger if they're doing a lot of damage.
  - Some kind of attack severity rating system that governs safe mode deployment,
    pulling creeps and resources from elsewhere, etc.
- Need to split creeps between fueling the spawner, upgrading the controller, etc.
  I could use a priority system where different tasks are assigned priorities? Then
  turn that into percentages?

# Planning
I think hivemind is using some kind of A* for room planning, since it uses open
and closed lists.

- has useful algorithms and info about tilesets.
  [wiki](https://wiki.screepspl.us/index.php/Automatic_base_building#Stamp/Tile-sets)
- One approach people use is to have bunkers that they can modularly build.
  - they have tilesets of things, and build around an anchor.
- Roads are 45,000 energy to build on a wall, so not something worth doing lmao.


## Plan
### First Base
Roads change all move speed to 1. So I could have extremely slow haulers.

Early Phase
- Uses general purpose EarlyWorkers to build extensions, roads, etc.
- Ends before Room Control 2.
- Builds roads from all

### Expansion
When expanding from a cold start, it's good to quickly grab several rooms for
fast expansion. So I guess expansion should start once the early phase is done?

I can categorize different rooms by their development level and say that if X
room is developed it can go send aid to a less developed room.

### Remote Mining
- I need to plan roads across room boundaries.
