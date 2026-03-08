//! Tile state build from authored tile layers and tilesets.

use super::super::{LayerId, LayerKindInfo, TileLayerDrawInfo, TileState, TilesetRuntimeInfo};
use super::{draw::tile_draw_origin, index_sync::tile_chunk_span};
use crate::ir_map::{IrLayer, IrLayerKind, IrMap, IrTileset};
use crate::spatial::{GlobalIndex, LayerIdx, TileId};
use macroquad::prelude::{vec2, Vec2};
use std::collections::HashMap;

fn tileset_for_gid_from<'a>(
    gid: TileId,
    gid_lut: &'a [u16],
    tilesets: &'a [TilesetRuntimeInfo],
) -> Option<(&'a TilesetRuntimeInfo, u32)> {
    let clean = gid.clean() as usize;
    if clean >= gid_lut.len() {
        return None;
    }

    let idx = gid_lut[clean];
    if idx == u16::MAX {
        return None;
    }

    let ts = &tilesets[idx as usize];
    Some((ts, gid.clean() - ts.first_gid))
}

pub(crate) fn build_tile_state_from_ir(
    ir: &IrMap,
    layer_kind_by_id: &HashMap<LayerId, LayerKindInfo>,
    index: &mut GlobalIndex,
) -> TileState {
    // TODO(T1.1): populate tile handle->layer/slot/runtime containers in parallel with
    // `tile_layer_draw_info` while preserving current draw/query behavior.
    let (tilesets, gid_lut) = build_tileset_runtime_and_lut(ir);

    let mut tile_layer_draw_info: Vec<TileLayerDrawInfo> = Vec::new();
    for (layer_z, layer) in ir.layers.iter().enumerate() {
        let tile_layer_idx = tile_layer_draw_info.len();
        let Some(tile_layer_id) =
            index_tile_layer_records(ir, layer, layer_z, &tilesets, &gid_lut, index)
        else {
            continue;
        };

        tile_layer_draw_info.push(TileLayerDrawInfo {
            layer_id: tile_layer_id,
            visible: layer.visible,
            opacity: layer.opacity.clamp(0.0, 1.0),
        });
        debug_assert!(matches!(
            layer_kind_by_id.get(&(layer_z as LayerId)),
            Some(LayerKindInfo::Tiles(idx)) if *idx == tile_layer_idx
        ));
    }

    TileState {
        tileset_runtime_info: tilesets,
        gid_lut,
        tile_layer_draw_info,
    }
}

fn build_tileset_runtime_and_lut(ir: &IrMap) -> (Vec<TilesetRuntimeInfo>, Vec<u16>) {
    let mut max_gid = 0u32;
    for t in &ir.tilesets {
        match t {
            IrTileset::Atlas {
                first_gid,
                tilecount,
                ..
            } => {
                max_gid = max_gid.max(*first_gid + tilecount - 1);
            }
        }
    }

    let mut tilesets = Vec::new();
    let mut gid_lut = vec![u16::MAX; (max_gid + 1) as usize];
    for (i, t) in ir.tilesets.iter().enumerate() {
        match t {
            IrTileset::Atlas {
                first_gid,
                image,
                tile_w,
                tile_h,
                tilecount,
                columns,
                spacing,
                margin,
                ..
            } => {
                tilesets.push(TilesetRuntimeInfo {
                    first_gid: *first_gid,
                    tilecount: *tilecount,
                    cols: *columns,
                    image: image.clone(),
                    tile_w: *tile_w,
                    tile_h: *tile_h,
                    spacing: *spacing,
                    margin: *margin,
                });

                for gid in *first_gid..(*first_gid + *tilecount) {
                    gid_lut[gid as usize] = i as u16;
                }
            }
        }
    }
    (tilesets, gid_lut)
}

fn index_tile_layer_records(
    ir: &IrMap,
    layer: &IrLayer,
    layer_z: usize,
    tilesets: &[TilesetRuntimeInfo],
    gid_lut: &[u16],
    index: &mut GlobalIndex,
) -> Option<LayerIdx> {
    let IrLayerKind::Tiles {
        width,
        height: _,
        data,
    } = &layer.kind
    else {
        return None;
    };

    let tile_layer_id = layer_z as LayerIdx;
    let tw = ir.tile_w as f32;
    let th = ir.tile_h as f32;

    for (idx, gid) in data.iter().enumerate() {
        if *gid == 0 {
            continue;
        }
        let col = idx % *width;
        let row = idx / *width;
        let mut world = vec2(col as f32 * tw, row as f32 * th);
        world += layer.offset;

        let tile_id = TileId(*gid);
        let Some((tileset, _)) = tileset_for_gid_from(tile_id, gid_lut, tilesets) else {
            continue;
        };
        index_tile_record(ir, index, tile_layer_id, tile_id, world, tileset);
    }
    Some(tile_layer_id)
}

fn index_tile_record(
    ir: &IrMap,
    index: &mut GlobalIndex,
    tile_layer_id: LayerIdx,
    tile_id: TileId,
    world: Vec2,
    tileset: &TilesetRuntimeInfo,
) {
    // TODO(T3.1): route tile insertions through shared tile-runtime/index-sync helpers once
    // handle-centric tile mutation APIs are introduced.
    let draw_origin = tile_draw_origin(world, ir.tile_h, tileset.tile_h);
    let oversized = tileset.tile_w > ir.tile_w || tileset.tile_h > ir.tile_h;
    if oversized {
        let handle = index.alloc_handle();
        let (chunk_min, chunk_max) =
            tile_chunk_span(draw_origin, tileset.tile_w as f32, tileset.tile_h as f32);
        for cy in chunk_min.y..=chunk_max.y {
            for cx in chunk_min.x..=chunk_max.x {
                let cc = crate::spatial::ChunkCoord { x: cx, y: cy };
                index.insert_tile_with_handle(handle, tile_id, tile_layer_id, cc, draw_origin);
            }
        }
    } else {
        index.add_tile(tile_id, tile_layer_id, draw_origin);
    }
}
