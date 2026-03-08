//! Core runtime model boundary.
//!
//! `MapData` is the canonical mutable runtime truth model.
//! Query/index structures used for fast lookup are derived from canonical content.

mod map_data;

pub(crate) use map_data::LayerKindInfo;
#[cfg(test)]
pub(crate) use map_data::{
    build_draw_order_and_kind, object_chunk_span, object_chunk_span_runtime, tile_draw_origin,
    TileLayerDrawInfo,
};
pub use map_data::{
    LayerId, MapData, ObjectLayer, ObjectQueryFilter, ObjectRuntimeState, TileQueryFilter,
};
pub(crate) use map_data::{
    LayerPlan, ObjectState, TileAuthoredState, TileDerivedState, TileRuntimeStore, TileState,
};
