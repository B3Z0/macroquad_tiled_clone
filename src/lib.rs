pub mod map;
pub mod spatial;
pub mod render;
pub mod ir_map;
pub mod loader {
    pub mod json_loader;
}

pub use map::Map;
pub use spatial::{GlobalIndex, TileId, TileHandle, LayerIdx};
pub use render::query_visible;
pub use loader::json_loader::*;
