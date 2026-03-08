use super::*;

impl MapData {
    /// Loads runtime/query map data without binding render textures.
    pub fn load(path: &str) -> Result<Self, MapError> {
        let (ir, _) = decode_map_file_to_ir(path)?;
        Self::from_ir(ir)
    }

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

    pub(crate) fn from_ir(ir: IrMap) -> Result<Self, MapError> {
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

        let mut index = GlobalIndex::new();
        let mut object_layers = Vec::new();
        let mut object_location_by_handle = Vec::new();
        let mut object_handles_by_layer = Vec::new();
        let mut object_runtime_by_layer = Vec::new();
        let mut tile_layers: Vec<TileLayerDrawInfo> = Vec::new();
        let (draw_order, layer_kind_by_id) = build_draw_order_and_kind(&ir.layers);

        for (layer_z, layer) in ir.layers.iter().enumerate() {
            match &layer.kind {
                IrLayerKind::Objects { objects } => {
                    let bucket_layer = layer_z as LayerIdx;
                    let layer_idx = object_layers.len();
                    object_layers.push(ObjectLayer {
                        id: layer_z as LayerId,
                        name: layer.name.clone(),
                        visible: layer.visible,
                        opacity: layer.opacity,
                        offset: layer.offset,
                        properties: layer.properties.clone(),
                        objects: objects.clone(),
                        bucket_layer,
                    });
                    let mut handles_in_layer = Vec::with_capacity(objects.len());
                    let mut runtime_in_layer = Vec::with_capacity(objects.len());

                    for (object_idx, obj) in objects.iter().enumerate() {
                        let handle = index.alloc_object_handle();
                        let handle_idx = handle.0 as usize;
                        if handle_idx >= object_location_by_handle.len() {
                            object_location_by_handle.resize(handle_idx + 1, None);
                        }
                        object_location_by_handle[handle_idx] = Some((layer_idx, object_idx));
                        handles_in_layer.push(Some(handle));
                        runtime_in_layer.push(Some(ObjectRuntimeState {
                            alive: true,
                            visible: obj.visible,
                            x: obj.x,
                            y: obj.y,
                            width: obj.width,
                            height: obj.height,
                        }));

                        let runtime =
                            runtime_in_layer[object_idx].expect("runtime must exist during build");
                        let world = vec2(runtime.x, runtime.y) + layer.offset;
                        let (chunk_min, chunk_max) =
                            object_chunk_span_runtime(obj, runtime, layer.offset);

                        for cy in chunk_min.y..=chunk_max.y {
                            for cx in chunk_min.x..=chunk_max.x {
                                let cc = crate::spatial::ChunkCoord { x: cx, y: cy };
                                let chunk_origin =
                                    vec2((cc.x * CHUNK_SIZE) as f32, (cc.y * CHUNK_SIZE) as f32);
                                index.insert_object(
                                    bucket_layer,
                                    cc,
                                    crate::spatial::ObjectRec {
                                        handle,
                                        rel_pos: world - chunk_origin,
                                    },
                                );
                            }
                        }
                    }
                    object_handles_by_layer.push(handles_in_layer);
                    object_runtime_by_layer.push(runtime_in_layer);
                    debug_assert!(matches!(
                        layer_kind_by_id.get(&(layer_z as LayerId)),
                        Some(LayerKindInfo::Objects(idx)) if *idx == layer_idx
                    ));
                }
                IrLayerKind::Tiles {
                    width,
                    height: _,
                    data,
                } => {
                    let tile_layer_id = layer_z as LayerIdx;
                    let tile_layer_idx = tile_layers.len();

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
                        let Some((ts, _)) =
                            Self::tileset_for_gid_from(tile_id, &gid_lut, &tilesets)
                        else {
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

                    tile_layers.push(TileLayerDrawInfo {
                        layer_id: tile_layer_id,
                        visible: layer.visible,
                        opacity: layer.opacity.clamp(0.0, 1.0),
                    });
                    debug_assert!(matches!(
                        layer_kind_by_id.get(&(layer_z as LayerId)),
                        Some(LayerKindInfo::Tiles(idx)) if *idx == tile_layer_idx
                    ));
                }
                IrLayerKind::Unsupported => {}
            }
        }

        Ok(Self {
            source_ir: ir,
            index,
            tilesets,
            object_layers,
            object_location_by_handle,
            object_handles_by_layer,
            object_runtime_by_layer,
            gid_lut,
            tile_layers,
            draw_order,
            layer_kind_by_id,
        })
    }
}
