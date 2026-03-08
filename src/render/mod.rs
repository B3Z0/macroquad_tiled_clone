//! Render subsystem boundary.
//!
//! Rendering consumes canonical runtime state from `core` and owns only render-only data
//! (textures, frame stamps, culling/debug toggles, dedupe buffers).

pub(crate) mod assets;
pub mod cull;
pub(crate) mod macroquad_renderer;
pub(crate) mod state;

pub(crate) use assets::{MacroquadRenderAssets, MacroquadTilesetAsset};
pub(crate) use state::RenderState;
