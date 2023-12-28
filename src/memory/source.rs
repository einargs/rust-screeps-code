// TODO: add a way to track the saturation of a source.

use minicbor::{Encode, Decode};
use std::default::Default;

#[derive(Default, Debug, PartialEq, Eq, Encode, Decode)]
pub struct SourceMemory {
}
