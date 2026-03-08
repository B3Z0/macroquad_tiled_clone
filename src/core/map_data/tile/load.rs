use super::super::*;
use super::{draw::tile_draw_origin, index_sync::tile_chunk_span};

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
    let mut tilesets = Vec::new();

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

    let mut tile_layer_draw_info: Vec<TileLayerDrawInfo> = Vec::new();

    for (layer_z, layer) in ir.layers.iter().enumerate() {
        let IrLayerKind::Tiles {
            width,
            height: _,
            data,
        } = &layer.kind
        else {
            continue;
        };

        let tile_layer_id = layer_z as LayerIdx;
        let tile_layer_idx = tile_layer_draw_info.len();

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
            let Some((ts, _)) = tileset_for_gid_from(tile_id, &gid_lut, &tilesets) else {
                continue;
            };
            let draw_origin = tile_draw_origin(world, ir.tile_h, ts.tile_h);
            let oversized = ts.tile_w > ir.tile_w || ts.tile_h > ir.tile_h;

            if oversized {
                let handle = index.alloc_handle();
                let (chunk_min, chunk_max) =
                    tile_chunk_span(draw_origin, ts.tile_w as f32, ts.tile_h as f32);
                for cy in chunk_min.y..=chunk_max.y {
                    for cx in chunk_min.x..=chunk_max.x {
                        let cc = crate::spatial::ChunkCoord { x: cx, y: cy };
                        index.insert_tile_with_handle(
                            handle,
                            tile_id,
                            tile_layer_id,
                            cc,
                            draw_origin,
                        );
                    }
                }
            } else {
                index.add_tile(tile_id, tile_layer_id, draw_origin);
            }
        }

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
