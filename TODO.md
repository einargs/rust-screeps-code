- nuke warnings
- power creeps
- mineral mining vs deposit mining
- highways have resources?
- remote miners?
- what's the difference between a mineral and a deposit?
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
- setup respawn system
- have miners retreat from hostiles
- Build an alerts system for seeing hostiles
- are the flags custom or not?
- I need to build profiling tools to tell what approach is best.
- collect data on how frequently I need to restore from memory

# Tools
- some tool to let me run on multiples of ticks. And to avoid overlap between
  things on a timer, so I don't get a ton of things happening the same time.

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

# Memory
`bitcode` is a very powerful and concise encoding system. However, I don't want
to deal with different versions and migrations just yet. So I'm going to use
CBOR (minicbor).

minicbor isn't generic like serde, but it doesn't matter in this situation and is
still very easy to drag and drop back in place.

# Rust
- I bet I can create a custom serialization/deserialization for room names.
