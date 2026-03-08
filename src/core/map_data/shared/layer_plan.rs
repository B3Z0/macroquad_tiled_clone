//! Shared layer planning helpers for deterministic traversal.

use crate::core::map_data::LayerId;
use crate::ir_map::{IrLayer, IrLayerKind};
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub(crate) enum LayerKindInfo {
    Tiles(usize),
    Objects(usize),
    Unsupported,
}

pub(crate) fn build_draw_order_and_kind(
    layers: &[IrLayer],
) -> (Vec<LayerId>, HashMap<LayerId, LayerKindInfo>) {
    let mut draw_order = Vec::with_capacity(layers.len());
    let mut layer_kind_by_id = HashMap::with_capacity(layers.len());
    let mut tile_layer_idx = 0usize;
    let mut object_layer_idx = 0usize;

    for (layer_z, layer) in layers.iter().enumerate() {
        let stable_id = layer_z as LayerId;
        draw_order.push(stable_id);
        match layer.kind {
            IrLayerKind::Tiles { .. } => {
                layer_kind_by_id.insert(stable_id, LayerKindInfo::Tiles(tile_layer_idx));
                tile_layer_idx += 1;
            }
            IrLayerKind::Objects { .. } => {
                layer_kind_by_id.insert(stable_id, LayerKindInfo::Objects(object_layer_idx));
                object_layer_idx += 1;
            }
            IrLayerKind::Unsupported => {
                layer_kind_by_id.insert(stable_id, LayerKindInfo::Unsupported);
            }
        }
    }

    (draw_order, layer_kind_by_id)
}
