use wasm_bindgen::prelude::*;
use screeps::{
  StructureSpawn, StructureExtension, Transferable, RoomObject,
  Structure, HasStore, Store,
};

/// Energy sink can be:
/// - spawn
/// - spawn extension
///
/// Should only wrap owned structures that are Transferable.
#[wasm_bindgen]
extern "C" {
  /// Object representing someplace a worker should transfer energy to.
  ///
  /// Currently should only be a spawn.
  #[wasm_bindgen(extends = RoomObject, extends = Structure)]
  #[derive(Clone, Debug)]
  pub type EnergySink;

  /// Get the store.
  #[wasm_bindgen(method, getter)]
  pub fn store(this: &EnergySink) -> Store;
}

// TODO: might be nice to have an `EnergySink::from_object` that converts
// from a `StructureObject` for use with `filter_map`.

impl From<StructureSpawn> for EnergySink {
  fn from(value: StructureSpawn) -> Self {
    JsValue::from(value).into()
  }
}

impl From<StructureExtension> for EnergySink {
  fn from(value: StructureExtension) -> Self {
    JsValue::from(value).into()
  }
}

impl Transferable for EnergySink {}

impl HasStore for EnergySink {
  fn store(&self) -> screeps::Store {
    EnergySink::store(self)
  }
}
