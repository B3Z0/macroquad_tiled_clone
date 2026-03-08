//! Tile state build from authored tile layers and tilesets.

use super::super::{
    LayerId, LayerKindInfo, TileLayerDrawInfo, TileRuntimeState, TileState, TilesetRuntimeInfo,
};
use super::{draw::tile_draw_origin, index_sync::tile_chunk_span};
use crate::ir_map::{IrLayer, IrLayerKind, IrMap, IrTileset};
use crate::spatial::{GlobalIndex, LayerIdx, TileHandle, TileId};
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

struct TileIndexBuildCtx<'a> {
    tilesets: &'a [TilesetRuntimeInfo],
    gid_lut: &'a [u16],
    index: &'a mut GlobalIndex,
    tile_location_by_handle: &'a mut Vec<Option<(usize, usize)>>,
}

pub(crate) fn build_tile_state_from_ir(
    ir: &IrMap,
    layer_kind_by_id: &HashMap<LayerId, LayerKindInfo>,
    index: &mut GlobalIndex,
) -> TileState {
    let (tilesets, gid_lut) = build_tileset_runtime_and_lut(ir);

    let mut tile_location_by_handle = Vec::new();
    let mut tile_handles_by_layer = Vec::new();
    let mut tile_runtime_by_layer = Vec::new();
    let mut tile_layer_draw_info: Vec<TileLayerDrawInfo> = Vec::new();
    let mut build_ctx = TileIndexBuildCtx {
        tilesets: &tilesets,
        gid_lut: &gid_lut,
        index,
        tile_location_by_handle: &mut tile_location_by_handle,
    };

    for (layer_z, layer) in ir.layers.iter().enumerate() {
        let tile_layer_idx = tile_layer_draw_info.len();
        let mut layer_handles = Vec::new();
        let mut layer_runtime = Vec::new();
        let Some(tile_layer_id) = index_tile_layer_records(
            ir,
            layer,
            layer_z,
            tile_layer_idx,
            &mut build_ctx,
            &mut layer_handles,
            &mut layer_runtime,
        ) else {
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
        tile_handles_by_layer.push(layer_handles);
        tile_runtime_by_layer.push(layer_runtime);
    }

    TileState {
        tile_location_by_handle,
        tile_handles_by_layer,
        tile_runtime_by_layer,
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
    tile_layer_idx: usize,
    build_ctx: &mut TileIndexBuildCtx<'_>,
    layer_handles: &mut Vec<Option<TileHandle>>,
    layer_runtime: &mut Vec<Option<TileRuntimeState>>,
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
        let Some((tileset, _)) =
            tileset_for_gid_from(tile_id, build_ctx.gid_lut, build_ctx.tilesets)
        else {
            continue;
        };
        let draw_origin = tile_draw_origin(world, ir.tile_h, tileset.tile_h);
        let handle = index_tile_record(
            ir,
            build_ctx.index,
            tile_layer_id,
            tile_id,
            draw_origin,
            tileset,
        );

        let slot_idx = layer_handles.len();
        layer_handles.push(Some(handle));
        layer_runtime.push(Some(TileRuntimeState {
            alive: true,
            visible: true,
            id: tile_id,
            x: draw_origin.x,
            y: draw_origin.y,
        }));

        let hidx = handle.0 as usize;
        if hidx >= build_ctx.tile_location_by_handle.len() {
            build_ctx.tile_location_by_handle.resize(hidx + 1, None);
        }
        build_ctx.tile_location_by_handle[hidx] = Some((tile_layer_idx, slot_idx));
    }
    Some(tile_layer_id)
}

fn index_tile_record(
    ir: &IrMap,
    index: &mut GlobalIndex,
    tile_layer_id: LayerIdx,
    tile_id: TileId,
    draw_origin: Vec2,
    tileset: &TilesetRuntimeInfo,
) -> TileHandle {
    // TODO(T3.1): route tile insertions through shared tile-runtime/index-sync helpers once
    // handle-centric tile mutation APIs are introduced.
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
        handle
    } else {
        index.add_tile(tile_id, tile_layer_id, draw_origin)
    }
}
