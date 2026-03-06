mod map_data;

pub(crate) use map_data::LayerKindInfo;
#[cfg(test)]
pub(crate) use map_data::{
    build_draw_order_and_kind, object_chunk_span, tile_draw_origin, TileLayerDrawInfo,
};
pub use map_data::{LayerId, MapData, ObjectLayer};
