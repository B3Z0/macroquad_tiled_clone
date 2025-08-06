pub mod map;
pub mod spatial;
pub mod render;

pub use map::Map;
pub use map::load_basic_json;      // if you still need to expose it
pub use spatial::{GlobalIndex, TileId, TileHandle, LayerIdx};
pub use render::query_visible;
