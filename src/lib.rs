pub mod map;
pub mod spatial;
pub mod ir_map;
pub mod render;
pub mod loader {
    pub mod json_loader;
}

pub use map::Map;
pub use spatial::{GlobalIndex, TileId, TileHandle, LayerIdx};