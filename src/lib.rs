pub mod ir_map;
pub mod map;
pub mod render;
pub mod spatial;
pub mod loader {
    pub mod json_loader;
}

pub use map::Map;
pub use spatial::{GlobalIndex, LayerIdx, TileHandle, TileId};
