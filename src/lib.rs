#[allow(dead_code)]
mod ir_map;
mod loader {
    pub mod json_loader;
}
mod map;
#[allow(dead_code)]
mod render;
#[allow(dead_code)]
mod spatial;

pub use ir_map::{IrObject, IrObjectShape, Properties, PropertyValue};
pub use map::{LayerId, Map, ObjectLayer};
