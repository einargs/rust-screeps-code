use log::*;
use screeps::{
  RoomName, Room, ConstructionSite,
};

use crate::mk_cache;


/// You can turn
struct RoomPlan {


}

// we use a cache because that way we can make it update when we change room planners.
// wait no that would happen anyway because this is all stored in memory.
mk_cache! {
  room_plan_cache lifetime 1000 by RoomName => RoomPlan
}
