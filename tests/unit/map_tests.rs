#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    fn load_fixture_ir(name: &str) -> IrMap {
        let path = fixture_path(name);
        let path_str = path.to_str().expect("fixture path must be utf-8");
        let (ir, _) = decode_map_file_to_ir(path_str).expect("fixture should decode");
        ir
    }

    fn temp_export_path(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock went backwards")
            .as_nanos();
        fixture_path(&format!("{prefix}_{nanos}.json"))
    }

    #[test]
    fn object_chunk_span_covers_multi_chunk_rectangles() {
        let obj = IrObject {
            id: 1,
            name: String::new(),
            class_name: String::new(),
            x: 250.0,
            y: 10.0,
            width: 20.0,
            height: 20.0,
            rotation: 0.0,
            visible: true,
            shape: IrObjectShape::Rectangle,
            properties: Properties::default(),
        };

        let (chunk_min, chunk_max) = Map::object_chunk_span(&obj, Vec2::ZERO);
        assert_eq!(chunk_min.x, 0);
        assert_eq!(chunk_max.x, 1);
        assert_eq!(chunk_min.y, 0);
        assert_eq!(chunk_max.y, 0);
    }

    #[test]
    fn tile_draw_origin_bottom_aligns_oversized_tiles() {
        let world = vec2(320.0, 256.0);
        let draw = Map::tile_draw_origin(world, 32, 128);
        assert_eq!(draw.x, 320.0);
        assert_eq!(draw.y, 160.0); // 256 + (32 - 128)
    }

    #[test]
    fn draw_order_matches_tiled_layer_order() {
        let layers = vec![
            IrLayer {
                name: "tiles_a".to_string(),
                visible: true,
                opacity: 1.0,
                offset: Vec2::ZERO,
                properties: Properties::default(),
                kind: IrLayerKind::Tiles {
                    width: 1,
                    height: 1,
                    data: vec![0],
                },
            },
            IrLayer {
                name: "objects_a".to_string(),
                visible: true,
                opacity: 1.0,
                offset: Vec2::ZERO,
                properties: Properties::default(),
                kind: IrLayerKind::Objects { objects: vec![] },
            },
            IrLayer {
                name: "tiles_b".to_string(),
                visible: true,
                opacity: 1.0,
                offset: Vec2::ZERO,
                properties: Properties::default(),
                kind: IrLayerKind::Tiles {
                    width: 1,
                    height: 1,
                    data: vec![0],
                },
            },
        ];

        let (draw_order, kind_by_id) = build_draw_order_and_kind(&layers);
        assert_eq!(draw_order, vec![0, 1, 2]);
        assert!(matches!(kind_by_id.get(&0), Some(LayerKindInfo::Tiles(0))));
        assert!(matches!(
            kind_by_id.get(&1),
            Some(LayerKindInfo::Objects(0))
        ));
        assert!(matches!(kind_by_id.get(&2), Some(LayerKindInfo::Tiles(1))));
    }

    #[test]
    fn stamp_buffers_are_realigned_and_all_objects_are_seen_once() {
        fn make_object(id: u32) -> IrObject {
            IrObject {
                id,
                name: String::new(),
                class_name: String::new(),
                x: 0.0,
                y: 0.0,
                width: 16.0,
                height: 16.0,
                rotation: 0.0,
                visible: true,
                shape: IrObjectShape::Rectangle,
                properties: Properties::default(),
            }
        }

        let data = MapData {
            source_ir: IrMap {
                tile_w: 16,
                tile_h: 16,
                properties: Properties::default(),
                tilesets: vec![],
                layers: vec![],
            },
            derived_index: GlobalIndex::new(),
            object_state: ObjectState {
                object_layers: vec![ObjectLayer {
                    id: 0,
                    name: "objects".to_string(),
                    visible: true,
                    opacity: 1.0,
                    offset: Vec2::ZERO,
                    properties: Properties::default(),
                    objects: vec![make_object(1), make_object(2), make_object(3)],
                    bucket_layer: 0,
                }],
                object_location_by_handle: vec![Some((0, 0)), Some((0, 1)), Some((0, 2))],
                object_handles_by_layer: vec![vec![
                    Some(crate::spatial::ObjectHandle(0)),
                    Some(crate::spatial::ObjectHandle(1)),
                    Some(crate::spatial::ObjectHandle(2)),
                ]],
                object_runtime_by_layer: vec![vec![
                    Some(ObjectRuntimeState {
                        alive: true,
                        visible: true,
                        x: 0.0,
                        y: 0.0,
                        width: 16.0,
                        height: 16.0,
                    }),
                    Some(ObjectRuntimeState {
                        alive: true,
                        visible: true,
                        x: 0.0,
                        y: 0.0,
                        width: 16.0,
                        height: 16.0,
                    }),
                    Some(ObjectRuntimeState {
                        alive: true,
                        visible: true,
                        x: 0.0,
                        y: 0.0,
                        width: 16.0,
                        height: 16.0,
                    }),
                ]],
            },
            tile_state: TileState {
                authored: TileAuthoredState {
                    tile_layers: vec![],
                    tileset_runtime_info: vec![],
                },
                runtime: TileRuntimeStore {
                    tile_location_by_handle: vec![],
                    tile_handles_by_layer: vec![],
                    tile_runtime_by_layer: vec![],
                },
                derived: TileDerivedState {
                    gid_lut: vec![],
                    tile_layer_draw_info: vec![],
                },
            },
            layer_plan: LayerPlan {
                draw_order: vec![],
                layer_kind_by_id: HashMap::new(),
            },
        };
        let mut state = RenderState::default();
        state.sync_with_data(&data);

        assert_eq!(state.seen_objects_tiles[0].len(), 3);
        assert_eq!(state.seen_objects_debug[0].len(), 3);

        let stamp = 42;
        let mut first_pass_drawn = 0usize;
        for object_idx in 0..data.object_state.object_layers[0].objects.len() {
            if state.seen_objects_tiles[0][object_idx] == stamp {
                continue;
            }
            state.seen_objects_tiles[0][object_idx] = stamp;
            first_pass_drawn += 1;
        }

        let mut second_pass_drawn = 0usize;
        for object_idx in 0..data.object_state.object_layers[0].objects.len() {
            if state.seen_objects_tiles[0][object_idx] == stamp {
                continue;
            }
            state.seen_objects_tiles[0][object_idx] = stamp;
            second_pass_drawn += 1;
        }

        assert_eq!(first_pass_drawn, 3);
        assert_eq!(second_pass_drawn, 0);
    }

    fn collect_tile_handles_with_stamp_for_test(
        map: &mut Map,
        coords: &[crate::spatial::ChunkCoord],
        tile_layer_idx: usize,
        stamp: u32,
    ) -> Vec<u32> {
        map.render_state.sync_with_data(&map.data);
        let Some(layer) = map
            .data
            .tile_state
            .derived
            .tile_layer_draw_info
            .get(tile_layer_idx)
            .copied()
        else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let seen = &mut map.render_state.seen_tiles;

        Map::for_each_visible_layer_bucket(
            &map.data.derived_index,
            coords,
            layer.layer_id,
            |_cc, bucket| {
                for rec in &bucket.tiles {
                    let idx = rec.handle.0 as usize;
                    if idx >= seen.len() {
                        continue;
                    }
                    if seen[idx] == stamp {
                        continue;
                    }
                    seen[idx] = stamp;
                    out.push(rec.handle.0);
                }
            },
        );

        out
    }

    fn collect_renderable_tile_handles_with_stamp_for_test(
        map: &mut Map,
        coords: &[crate::spatial::ChunkCoord],
        tile_layer_idx: usize,
        stamp: u32,
    ) -> Vec<u32> {
        map.render_state.sync_with_data(&map.data);
        let Some(layer) = map
            .data
            .tile_state
            .derived
            .tile_layer_draw_info
            .get(tile_layer_idx)
            .copied()
        else {
            return Vec::new();
        };
        if !layer.visible {
            return Vec::new();
        }

        let seen = &mut map.render_state.seen_tiles;
        let mut out = Vec::new();
        Map::for_each_visible_layer_bucket(
            &map.data.derived_index,
            coords,
            layer.layer_id,
            |_cc, bucket| {
                for rec in &bucket.tiles {
                    let idx = rec.handle.0 as usize;
                    if idx >= seen.len() {
                        continue;
                    }
                    if seen[idx] == stamp {
                        continue;
                    }
                    seen[idx] = stamp;

                    let Some(runtime) = map.data.tile_runtime_by_handle(rec.handle) else {
                        continue;
                    };
                    if !runtime.alive || !runtime.visible {
                        continue;
                    }
                    out.push(rec.handle.0);
                }
            },
        );
        out
    }

    fn collect_draw_sequence_for_test(
        map: &mut Map,
        view_min: Vec2,
        view_max: Vec2,
    ) -> Vec<(char, LayerId, i32, i32, u32)> {
        let coords = map.visible_coords_for_draw(view_min, view_max);
        let stamp = map.next_frame_stamp();
        let mut out = Vec::new();

        for i in 0..map.data.layer_plan.draw_order.len() {
            let layer_id = map.data.layer_plan.draw_order[i];
            let Some(kind) = map.data.layer_plan.layer_kind_by_id.get(&layer_id).copied() else {
                continue;
            };

            match kind {
                LayerKindInfo::Tiles(tile_layer_idx) => {
                    let Some(layer) = map
                        .data
                        .tile_state
                        .derived
                        .tile_layer_draw_info
                        .get(tile_layer_idx)
                    else {
                        continue;
                    };
                    if !layer.visible {
                        continue;
                    }

                    Map::for_each_visible_layer_bucket(
                        &map.data.derived_index,
                        &coords,
                        layer.layer_id,
                        |cc, bucket| {
                            for rec in &bucket.tiles {
                                out.push(('T', layer_id, cc.x, cc.y, rec.id.clean()));
                            }
                        },
                    );
                }
                LayerKindInfo::Objects(object_layer_idx) => {
                    let Some(layer) = map.data.object_state.object_layers.get(object_layer_idx) else {
                        continue;
                    };
                    let Some(seen_layer) = map
                        .render_state
                        .seen_objects_tiles
                        .get_mut(object_layer_idx)
                    else {
                        continue;
                    };
                    if !layer.visible {
                        continue;
                    }
                    let bucket_layer = layer.bucket_layer;

                    Map::for_each_visible_layer_bucket(
                        &map.data.derived_index,
                        &coords,
                        bucket_layer,
                        |cc, bucket| {
                            for rec in &bucket.objects {
                                let Some((handle_layer_idx, object_idx)) =
                                    map.data.object_location(rec.handle)
                                else {
                                    continue;
                                };
                                if handle_layer_idx != object_layer_idx
                                    || object_idx >= layer.objects.len()
                                {
                                    continue;
                                }
                                if seen_layer[object_idx] == stamp {
                                    continue;
                                }
                                seen_layer[object_idx] = stamp;

                                let Some(obj) = layer.objects.get(object_idx) else {
                                    continue;
                                };
                                let Some(runtime) = map
                                    .data
                                    .object_state.object_runtime_by_layer
                                    .get(object_layer_idx)
                                    .and_then(|v| v.get(object_idx))
                                    .and_then(|s| s.as_ref())
                                else {
                                    continue;
                                };
                                if !runtime.alive || !runtime.visible {
                                    continue;
                                }
                                if !matches!(obj.shape, IrObjectShape::Tile { .. }) {
                                    continue;
                                }
                                out.push(('O', layer_id, cc.x, cc.y, obj.id));
                            }
                        },
                    );
                }
                LayerKindInfo::Unsupported => {}
            }
        }

        out
    }

    fn make_tile_only_map_with_neighbor_chunk_tile() -> Map {
        let mut index = GlobalIndex::new();
        let handle = index.alloc_handle();
        let world = vec2((CHUNK_SIZE + 8) as f32, 8.0);
        index.insert_tile_with_handle(
            handle,
            TileId(1),
            0,
            crate::spatial::ChunkCoord { x: 1, y: 0 },
            world,
        );

        Map {
            data: MapData {
                source_ir: IrMap {
                    tile_w: 16,
                    tile_h: 16,
                    properties: Properties::default(),
                    tilesets: vec![],
                    layers: vec![],
                },
                derived_index: index,
                object_state: ObjectState {
                    object_layers: vec![],
                    object_location_by_handle: vec![],
                    object_handles_by_layer: vec![],
                    object_runtime_by_layer: vec![],
                },
                tile_state: TileState {
                    authored: TileAuthoredState {
                        tile_layers: vec![],
                        tileset_runtime_info: vec![],
                    },
                    runtime: TileRuntimeStore {
                        tile_location_by_handle: vec![],
                        tile_handles_by_layer: vec![],
                        tile_runtime_by_layer: vec![],
                    },
                    derived: TileDerivedState {
                        gid_lut: vec![],
                        tile_layer_draw_info: vec![TileLayerDrawInfo {
                            layer_id: 0,
                            visible: true,
                            opacity: 1.0,
                        }],
                    },
                },
                layer_plan: LayerPlan {
                    draw_order: vec![0],
                    layer_kind_by_id: {
                        let mut m = HashMap::new();
                        m.insert(0, LayerKindInfo::Tiles(0));
                        m
                    },
                },
            },
            assets: MacroquadRenderAssets { tilesets: vec![] },
            render_state: RenderState::default(),
        }
    }

    fn make_oversized_tile_runtime_test_map() -> Map {
        let ir = IrMap {
            tile_w: 16,
            tile_h: 16,
            properties: Properties::default(),
            tilesets: vec![IrTileset::Atlas {
                first_gid: 1,
                source: "mock.tsx".to_string(),
                image: "mock.png".to_string(),
                tile_w: 64,
                tile_h: 64,
                tilecount: 1,
                columns: 1,
                spacing: 0,
                margin: 0,
                properties: Properties::default(),
                tiles: vec![],
            }],
            layers: vec![IrLayer {
                name: "tiles".to_string(),
                visible: true,
                opacity: 1.0,
                offset: Vec2::ZERO,
                properties: Properties::default(),
                kind: IrLayerKind::Tiles {
                    width: 1,
                    height: 1,
                    data: vec![1],
                },
            }],
        };
        let mut data = MapData::from_ir(ir).expect("map data from IR should build");
        let handle = data.tile_state.runtime.tile_handles_by_layer[0][0].expect("tile handle");
        assert!(data.move_tile_by_handle(handle, (CHUNK_SIZE - 8) as f32, 40.0));

        Map {
            data,
            assets: MacroquadRenderAssets { tilesets: vec![] },
            render_state: RenderState::default(),
        }
    }

    #[test]
    fn draw_sequence_is_deterministic_across_runs() {
        let mut index = GlobalIndex::new();
        index.add_tile(TileId(1), 0, vec2(10.0, 10.0));
        index.add_tile(TileId(1), 0, vec2((CHUNK_SIZE + 10) as f32, 10.0));
        index.add_tile(TileId(1), 2, vec2(20.0, (CHUNK_SIZE + 10) as f32));
        let object_handle = index.alloc_object_handle();

        let objects = vec![IrObject {
            id: 77,
            name: "coin".to_string(),
            class_name: String::new(),
            x: (CHUNK_SIZE - 8) as f32,
            y: 32.0,
            width: 16.0,
            height: 16.0,
            rotation: 0.0,
            visible: true,
            shape: IrObjectShape::Tile { gid: 1 },
            properties: Properties::default(),
        }];

        // Same object appears in multiple chunks (as with AABB multi-chunk insertion).
        index.insert_object(
            1,
            crate::spatial::ChunkCoord { x: 0, y: 0 },
            crate::spatial::ObjectRec {
                handle: object_handle,
                rel_pos: vec2((CHUNK_SIZE - 8) as f32, 32.0),
            },
        );
        index.insert_object(
            1,
            crate::spatial::ChunkCoord { x: 1, y: 0 },
            crate::spatial::ObjectRec {
                handle: object_handle,
                rel_pos: vec2(0.0, 32.0),
            },
        );

        let mut map = Map {
            data: MapData {
                source_ir: IrMap {
                    tile_w: 16,
                    tile_h: 16,
                    properties: Properties::default(),
                    tilesets: vec![],
                    layers: vec![],
                },
                derived_index: index,
                object_state: ObjectState {
                    object_layers: vec![ObjectLayer {
                        id: 1,
                        name: "objects".to_string(),
                        visible: true,
                        opacity: 1.0,
                        offset: Vec2::ZERO,
                        properties: Properties::default(),
                        objects,
                        bucket_layer: 1,
                    }],
                    object_location_by_handle: {
                        let mut v = vec![None; (object_handle.0 as usize) + 1];
                        v[object_handle.0 as usize] = Some((0, 0));
                        v
                    },
                    object_handles_by_layer: vec![vec![Some(object_handle)]],
                    object_runtime_by_layer: vec![vec![Some(ObjectRuntimeState {
                        alive: true,
                        visible: true,
                        x: (CHUNK_SIZE - 8) as f32,
                        y: 32.0,
                        width: 16.0,
                        height: 16.0,
                    })]],
                },
                tile_state: TileState {
                    authored: TileAuthoredState {
                        tile_layers: vec![],
                        tileset_runtime_info: vec![],
                    },
                    runtime: TileRuntimeStore {
                        tile_location_by_handle: vec![],
                        tile_handles_by_layer: vec![],
                        tile_runtime_by_layer: vec![],
                    },
                    derived: TileDerivedState {
                        gid_lut: vec![],
                        tile_layer_draw_info: vec![
                            TileLayerDrawInfo {
                                layer_id: 0,
                                visible: true,
                                opacity: 1.0,
                            },
                            TileLayerDrawInfo {
                                layer_id: 2,
                                visible: true,
                                opacity: 1.0,
                            },
                        ],
                    },
                },
                layer_plan: LayerPlan {
                    draw_order: vec![0, 1, 2],
                    layer_kind_by_id: {
                        let mut m = HashMap::new();
                        m.insert(0, LayerKindInfo::Tiles(0));
                        m.insert(1, LayerKindInfo::Objects(0));
                        m.insert(2, LayerKindInfo::Tiles(1));
                        m
                    },
                },
            },
            assets: MacroquadRenderAssets { tilesets: vec![] },
            render_state: RenderState {
                debug_draw: false,
                cull_padding: CHUNK_SIZE as f32,
                frame_stamp: 0,
                seen_tiles: vec![],
                seen_objects_tiles: vec![vec![0]],
                seen_objects_debug: vec![vec![0]],
            },
        };

        let seq1 = collect_draw_sequence_for_test(&mut map, Vec2::ZERO, vec2(520.0, 520.0));
        let seq2 = collect_draw_sequence_for_test(&mut map, Vec2::ZERO, vec2(520.0, 520.0));
        assert_eq!(seq1, seq2);
    }

    #[test]
    fn oversized_tile_inserted_into_neighbor_chunk_is_queryable_there() {
        let mut index = GlobalIndex::new();
        let handle = index.alloc_handle();
        let world = vec2((CHUNK_SIZE - 8) as f32, 32.0);
        index.insert_tile_with_handle(
            handle,
            TileId(1),
            0,
            crate::spatial::ChunkCoord { x: 0, y: 0 },
            world,
        );
        index.insert_tile_with_handle(
            handle,
            TileId(1),
            0,
            crate::spatial::ChunkCoord { x: 1, y: 0 },
            world,
        );

        let view_coords = visible_chunk_coords_rect(
            vec2((CHUNK_SIZE + 1) as f32, 0.0),
            vec2((CHUNK_SIZE + 20) as f32, 20.0),
        );
        let mut found_in_chunk1 = false;
        for cc in view_coords {
            if cc.x != 1 || cc.y != 0 {
                continue;
            }
            if let Some(chunk) = index.buckets.get(&cc) {
                if let Some(bucket) = chunk.layers.get(&0) {
                    if !bucket.tiles.is_empty() {
                        found_in_chunk1 = true;
                    }
                }
            }
        }

        assert!(found_in_chunk1);
    }

    #[test]
    fn oversized_tile_draw_dedupes_across_multiple_chunks() {
        let mut index = GlobalIndex::new();
        let handle = index.alloc_handle();
        let world = vec2((CHUNK_SIZE - 8) as f32, 32.0);
        index.insert_tile_with_handle(
            handle,
            TileId(1),
            0,
            crate::spatial::ChunkCoord { x: 0, y: 0 },
            world,
        );
        index.insert_tile_with_handle(
            handle,
            TileId(1),
            0,
            crate::spatial::ChunkCoord { x: 1, y: 0 },
            world,
        );

        let mut map = Map {
            data: MapData {
                source_ir: IrMap {
                    tile_w: 16,
                    tile_h: 16,
                    properties: Properties::default(),
                    tilesets: vec![],
                    layers: vec![],
                },
                derived_index: index,
                object_state: ObjectState {
                    object_layers: vec![],
                    object_location_by_handle: vec![],
                    object_handles_by_layer: vec![],
                    object_runtime_by_layer: vec![],
                },
                tile_state: TileState {
                    authored: TileAuthoredState {
                        tile_layers: vec![],
                        tileset_runtime_info: vec![],
                    },
                    runtime: TileRuntimeStore {
                        tile_location_by_handle: vec![],
                        tile_handles_by_layer: vec![],
                        tile_runtime_by_layer: vec![],
                    },
                    derived: TileDerivedState {
                        gid_lut: vec![],
                        tile_layer_draw_info: vec![TileLayerDrawInfo {
                            layer_id: 0,
                            visible: true,
                            opacity: 1.0,
                        }],
                    },
                },
                layer_plan: LayerPlan {
                    draw_order: vec![0],
                    layer_kind_by_id: {
                        let mut m = HashMap::new();
                        m.insert(0, LayerKindInfo::Tiles(0));
                        m
                    },
                },
            },
            assets: MacroquadRenderAssets { tilesets: vec![] },
            render_state: RenderState::default(),
        };

        let coords =
            visible_chunk_coords_rect(vec2(0.0, 0.0), vec2((CHUNK_SIZE + 40) as f32, 80.0));
        let stamp = map.next_frame_stamp();
        let handles = collect_tile_handles_with_stamp_for_test(&mut map, &coords, 0, stamp);

        assert_eq!(handles.len(), 1);
        assert_eq!(handles[0], handle.0);
    }

    #[test]
    fn tile_runtime_mutation_preserves_render_dedupe_and_visibility() {
        let mut map = make_oversized_tile_runtime_test_map();
        let handle = map.data.tile_state.runtime.tile_handles_by_layer[0][0].expect("tile handle");
        let coords =
            visible_chunk_coords_rect(vec2(0.0, 0.0), vec2((CHUNK_SIZE + 60) as f32, 100.0));

        let s1 = map.next_frame_stamp();
        let h1 = collect_renderable_tile_handles_with_stamp_for_test(&mut map, &coords, 0, s1);
        assert_eq!(h1, vec![handle.0], "oversized tile should dedupe to one logical draw");

        assert!(map.data.set_tile_visible_by_handle(handle, false));
        let s2 = map.next_frame_stamp();
        let h2 = collect_renderable_tile_handles_with_stamp_for_test(&mut map, &coords, 0, s2);
        assert!(h2.is_empty(), "invisible tile must not be renderable");

        assert!(map.data.set_tile_visible_by_handle(handle, true));
        assert!(map.data.set_tile_alive_by_handle(handle, false));
        let s3 = map.next_frame_stamp();
        let h3 = collect_renderable_tile_handles_with_stamp_for_test(&mut map, &coords, 0, s3);
        assert!(h3.is_empty(), "dead tile must not be renderable");

        assert!(map.data.set_tile_alive_by_handle(handle, true));
        let s4 = map.next_frame_stamp();
        let h4 = collect_renderable_tile_handles_with_stamp_for_test(&mut map, &coords, 0, s4);
        assert_eq!(h4, vec![handle.0], "revived tile should become renderable again");
    }

    #[test]
    fn tile_move_preserves_cull_region_expectations() {
        let mut map = make_oversized_tile_runtime_test_map();
        let handle = map.data.tile_state.runtime.tile_handles_by_layer[0][0].expect("tile handle");

        let near_old =
            visible_chunk_coords_rect(vec2(0.0, 0.0), vec2((CHUNK_SIZE + 20) as f32, 120.0));
        let far_new = visible_chunk_coords_rect(vec2(1200.0, 1200.0), vec2(1280.0, 1280.0));

        let s1 = map.next_frame_stamp();
        let old_before =
            collect_renderable_tile_handles_with_stamp_for_test(&mut map, &near_old, 0, s1);
        assert_eq!(old_before, vec![handle.0]);

        assert!(map.data.move_tile_by_handle(handle, 1248.0, 1248.0));

        let s2 = map.next_frame_stamp();
        let old_after =
            collect_renderable_tile_handles_with_stamp_for_test(&mut map, &near_old, 0, s2);
        assert!(old_after.is_empty(), "tile moved away should leave old cull window");

        let s3 = map.next_frame_stamp();
        let new_after =
            collect_renderable_tile_handles_with_stamp_for_test(&mut map, &far_new, 0, s3);
        assert_eq!(new_after, vec![handle.0], "tile moved should appear in new cull window");
    }

    #[test]
    fn visible_tile_region_query_is_deterministic_and_deduped() {
        let map = make_oversized_tile_runtime_test_map();
        let handle = map.data.tile_state.runtime.tile_handles_by_layer[0][0].expect("tile handle");
        let a = map.query_visible_tile_handles(
            0,
            vec2(0.0, 0.0),
            vec2((CHUNK_SIZE + 80) as f32, 140.0),
            TileQueryFilter::default(),
        );
        let b = map.query_visible_tile_handles(
            0,
            vec2(0.0, 0.0),
            vec2((CHUNK_SIZE + 80) as f32, 140.0),
            TileQueryFilter::default(),
        );
        assert_eq!(a, b);
        assert_eq!(a, vec![handle]);
    }

    #[test]
    fn visible_tile_region_query_supports_gid_filter_and_relocation() {
        let ir = IrMap {
            tile_w: 16,
            tile_h: 16,
            properties: Properties::default(),
            tilesets: vec![IrTileset::Atlas {
                first_gid: 1,
                source: "mock.tsx".to_string(),
                image: "mock.png".to_string(),
                tile_w: 16,
                tile_h: 16,
                tilecount: 2,
                columns: 2,
                spacing: 0,
                margin: 0,
                properties: Properties::default(),
                tiles: vec![],
            }],
            layers: vec![IrLayer {
                name: "tiles".to_string(),
                visible: true,
                opacity: 1.0,
                offset: Vec2::ZERO,
                properties: Properties::default(),
                kind: IrLayerKind::Tiles {
                    width: 2,
                    height: 1,
                    data: vec![1, 2],
                },
            }],
        };
        let mut map = Map {
            data: MapData::from_ir(ir).expect("map data from IR should build"),
            assets: MacroquadRenderAssets { tilesets: vec![] },
            render_state: RenderState::default(),
        };

        let all = map.query_visible_tile_handles(
            0,
            vec2(0.0, 0.0),
            vec2(64.0, 32.0),
            TileQueryFilter::default(),
        );
        assert_eq!(all.len(), 2);
        let gid2 = map.query_visible_tile_handles(
            0,
            vec2(0.0, 0.0),
            vec2(64.0, 32.0),
            TileQueryFilter { gid: Some(2) },
        );
        assert_eq!(gid2.len(), 1);
        assert!(all.contains(&gid2[0]));

        assert!(map.data.move_tile_by_handle(gid2[0], 1248.0, 1248.0));
        let old = map.query_visible_tile_handles(
            0,
            vec2(0.0, 0.0),
            vec2(64.0, 32.0),
            TileQueryFilter { gid: Some(2) },
        );
        assert!(old.is_empty());
        let new = map.query_visible_tile_handles(
            0,
            vec2(1200.0, 1200.0),
            vec2(1280.0, 1280.0),
            TileQueryFilter { gid: Some(2) },
        );
        assert_eq!(new, vec![gid2[0]]);
    }

    #[test]
    fn tile_region_mutation_helpers_update_query_and_render_immediately() {
        let ir = IrMap {
            tile_w: 16,
            tile_h: 16,
            properties: Properties::default(),
            tilesets: vec![IrTileset::Atlas {
                first_gid: 1,
                source: "mock.tsx".to_string(),
                image: "mock.png".to_string(),
                tile_w: 16,
                tile_h: 16,
                tilecount: 2,
                columns: 2,
                spacing: 0,
                margin: 0,
                properties: Properties::default(),
                tiles: vec![],
            }],
            layers: vec![IrLayer {
                name: "tiles".to_string(),
                visible: true,
                opacity: 1.0,
                offset: Vec2::ZERO,
                properties: Properties::default(),
                kind: IrLayerKind::Tiles {
                    width: 2,
                    height: 1,
                    data: vec![1, 2],
                },
            }],
        };
        let mut map = Map {
            data: MapData::from_ir(ir).expect("map data from IR should build"),
            assets: MacroquadRenderAssets { tilesets: vec![] },
            render_state: RenderState::default(),
        };

        let changed = map.replace_visible_tiles_gid_in_rect(
            0,
            vec2(0.0, 0.0),
            vec2(64.0, 32.0),
            TileQueryFilter { gid: Some(2) },
            1,
        );
        assert_eq!(changed.len(), 1);
        assert!(
            map.query_visible_tile_handles(
                0,
                vec2(0.0, 0.0),
                vec2(64.0, 32.0),
                TileQueryFilter { gid: Some(2) }
            )
            .is_empty()
        );

        let disabled = map.disable_visible_tiles_in_rect(
            0,
            vec2(0.0, 0.0),
            vec2(64.0, 32.0),
            TileQueryFilter::default(),
        );
        assert_eq!(disabled.len(), 2);
        assert!(
            map.query_visible_tile_handles(
                0,
                vec2(0.0, 0.0),
                vec2(64.0, 32.0),
                TileQueryFilter::default()
            )
            .is_empty()
        );

        let stamp = map.next_frame_stamp();
        let coords = visible_chunk_coords_rect(vec2(0.0, 0.0), vec2(64.0, 32.0));
        let renderable =
            collect_renderable_tile_handles_with_stamp_for_test(&mut map, &coords, 0, stamp);
        assert!(renderable.is_empty());
    }

    #[test]
    fn tile_batch_helpers_are_deterministic_and_stale_safe() {
        let mut map = make_oversized_tile_runtime_test_map();
        let mut handles = map.query_visible_tile_handles(
            0,
            vec2(0.0, 0.0),
            vec2((CHUNK_SIZE + 80) as f32, 160.0),
            TileQueryFilter::default(),
        );
        assert!(!handles.is_empty());
        let real = handles[0];

        handles.push(real); // duplicate
        handles.push(TileHandle(999_999)); // invalid/stale

        let c1 = map.set_tiles_visible_by_handle(&handles, false);
        let c2 = map.set_tiles_visible_by_handle(&handles, false);
        assert_eq!(c1, c2);
        assert!(c1 >= 1);

        let removed = map.data.remove_tile_by_handle(real);
        assert!(removed);
        let alive_changed = map.set_tiles_alive_by_handle(&handles, true);
        assert!(alive_changed <= c1);
    }

    #[test]
    fn fixture_layer_ordering_matches_tiled_order() {
        let ir = load_fixture_ir("external_props_map.json");
        let (draw_order, kinds) = build_draw_order_and_kind(&ir.layers);
        assert_eq!(draw_order, vec![0, 1, 2]);
        assert!(matches!(kinds.get(&0), Some(LayerKindInfo::Tiles(0))));
        assert!(matches!(kinds.get(&1), Some(LayerKindInfo::Objects(0))));
        assert!(matches!(kinds.get(&2), Some(LayerKindInfo::Tiles(1))));
    }

    #[test]
    fn map_data_builds_without_texture_binding() {
        let path = fixture_path("external_props_map.json");
        let path_str = path.to_str().expect("fixture path must be utf-8");
        let data = MapData::load(path_str).expect("map data should load headlessly");
        assert_eq!(data.object_state.object_layers.len(), 1);
        assert_eq!(data.tile_state.derived.tile_layer_draw_info.len(), 2);
        assert_eq!(data.tile_state.authored.tile_layers.len(), 2);
        assert_eq!(data.layer_plan.draw_order, vec![0, 1, 2]);
    }

    #[test]
    fn tile_handle_maps_are_consistent_after_load() {
        let path = fixture_path("external_props_map.json");
        let path_str = path.to_str().expect("fixture path must be utf-8");
        let data = MapData::load(path_str).expect("map data should load headlessly");

        assert_eq!(
            data.tile_state.runtime.tile_handles_by_layer.len(),
            data.tile_state.derived.tile_layer_draw_info.len()
        );
        assert_eq!(
            data.tile_state.runtime.tile_runtime_by_layer.len(),
            data.tile_state.derived.tile_layer_draw_info.len()
        );

        for (layer_idx, layer_handles) in data
            .tile_state
            .runtime
            .tile_handles_by_layer
            .iter()
            .enumerate()
        {
            let runtime_layer = &data.tile_state.runtime.tile_runtime_by_layer[layer_idx];
            assert_eq!(layer_handles.len(), runtime_layer.len());

            for (slot_idx, handle_slot) in layer_handles.iter().enumerate() {
                let Some(handle) = handle_slot else {
                    continue;
                };

                let location = data
                    .tile_state
                    .runtime
                    .tile_location_by_handle
                    .get(handle.0 as usize)
                    .and_then(|loc| *loc)
                    .expect("tile location must exist for populated tile handle");
                assert_eq!(location, (layer_idx, slot_idx));

                let runtime = runtime_layer[slot_idx]
                    .as_ref()
                    .expect("runtime slot must exist for populated tile handle");
                assert!(runtime.alive);
                assert!(runtime.visible);
                assert_ne!(runtime.id.clean(), 0);
            }
        }
    }

    #[test]
    fn fixture_object_spans_multiple_chunks() {
        let ir = load_fixture_ir("multichunk_objects_map.json");
        let object_layer = ir
            .layers
            .iter()
            .find(|l| matches!(l.kind, IrLayerKind::Objects { .. }))
            .expect("object layer exists");
        let IrLayerKind::Objects { ref objects } = object_layer.kind else {
            panic!("expected object layer");
        };
        let obj = &objects[0];
        let (chunk_min, chunk_max) = Map::object_chunk_span(obj, object_layer.offset);
        assert_eq!(chunk_min.x, 0);
        assert_eq!(chunk_max.x, 1);
        assert_eq!(chunk_min.y, 0);
        assert_eq!(chunk_max.y, 0);
    }

    #[test]
    fn multi_chunk_object_reconstructs_same_world_pos_from_any_bucket() {
        let world = vec2(591.5974, 604.84875);
        let rel_home = crate::spatial::rel(world);
        let chunk_home = world_to_chunk(world);
        assert_eq!(chunk_home.x, 2);
        assert_eq!(chunk_home.y, 2);

        // Simulate a tall object spanning into the chunk above.
        let cc_other = crate::spatial::ChunkCoord { x: 2, y: 1 };
        let wrong_origin = vec2(
            (cc_other.x * CHUNK_SIZE) as f32,
            (cc_other.y * CHUNK_SIZE) as f32,
        ) + rel_home;
        assert_ne!(wrong_origin.y, world.y);

        let correct_rel = world
            - vec2(
                (cc_other.x * CHUNK_SIZE) as f32,
                (cc_other.y * CHUNK_SIZE) as f32,
            );
        let rebuilt = vec2(
            (cc_other.x * CHUNK_SIZE) as f32,
            (cc_other.y * CHUNK_SIZE) as f32,
        ) + correct_rel;
        assert_eq!(rebuilt, world);
    }

    #[test]
    fn fixture_culling_returns_expected_chunks() {
        let ir = load_fixture_ir("minimal_finite_map.json");
        let mut index = GlobalIndex::new();
        let tw = ir.tile_w as f32;
        let th = ir.tile_h as f32;

        for (lz, layer) in ir.layers.iter().enumerate() {
            let IrLayerKind::Tiles { width, data, .. } = &layer.kind else {
                continue;
            };
            for (idx, gid) in data.iter().enumerate() {
                if *gid == 0 {
                    continue;
                }
                let col = idx % *width;
                let row = idx / *width;
                index.add_tile(
                    TileId(*gid),
                    lz as LayerIdx,
                    vec2(col as f32 * tw, row as f32 * th) + layer.offset,
                );
            }
        }

        let near = query_visible_rect(&index, vec2(0.0, 0.0), vec2(40.0, 20.0));
        assert!(near.chunks.iter().any(|c| c.coord.x == 0 && c.coord.y == 0));

        let far = query_visible_rect(&index, vec2(2000.0, 2000.0), vec2(2100.0, 2100.0));
        assert!(far.chunks.is_empty());
    }

    #[test]
    fn default_cull_padding_is_one_chunk() {
        let map = Map::__new_for_stamp_overflow_test(0);
        let coords = map.visible_coords_for_draw(vec2(0.0, 0.0), vec2(10.0, 10.0));

        assert!(coords.iter().any(|c| c.x == -1 && c.y == -1));
        assert!(coords.iter().any(|c| c.x == 1 && c.y == 1));
    }

    #[test]
    fn cull_padding_zero_uses_exact_view_chunks() {
        let mut map = Map::__new_for_stamp_overflow_test(0);
        map.set_cull_padding(0.0);
        let coords = map.visible_coords_for_draw(vec2(0.0, 0.0), vec2(10.0, 10.0));

        assert_eq!(coords.len(), 1);
        assert_eq!(coords[0].x, 0);
        assert_eq!(coords[0].y, 0);
    }

    #[test]
    fn cull_padding_affects_draw_and_draw_visible_rect_consistently() {
        let view_min = vec2(0.0, 0.0);
        let view_max = vec2(32.0, 32.0);
        let stamp = 7;

        let mut map_draw_rect_no_pad = make_tile_only_map_with_neighbor_chunk_tile();
        map_draw_rect_no_pad.set_cull_padding(0.0);
        map_draw_rect_no_pad.draw_visible_rect_with_stamp(view_min, view_max, stamp);
        let rect_no_pad = map_draw_rect_no_pad
            .render_state
            .seen_tiles
            .iter()
            .filter(|&&s| s == stamp)
            .count();

        let mut map_draw_no_pad = make_tile_only_map_with_neighbor_chunk_tile();
        map_draw_no_pad.set_cull_padding(0.0);
        map_draw_no_pad.__set_frame_stamp_for_testing(stamp - 1);
        map_draw_no_pad.draw(view_min, view_max);
        let draw_no_pad = map_draw_no_pad
            .render_state
            .seen_tiles
            .iter()
            .filter(|&&s| s == stamp)
            .count();

        let mut map_draw_rect_pad = make_tile_only_map_with_neighbor_chunk_tile();
        map_draw_rect_pad.set_cull_padding(CHUNK_SIZE as f32);
        map_draw_rect_pad.draw_visible_rect_with_stamp(view_min, view_max, stamp);
        let rect_pad = map_draw_rect_pad
            .render_state
            .seen_tiles
            .iter()
            .filter(|&&s| s == stamp)
            .count();

        let mut map_draw_pad = make_tile_only_map_with_neighbor_chunk_tile();
        map_draw_pad.set_cull_padding(CHUNK_SIZE as f32);
        map_draw_pad.__set_frame_stamp_for_testing(stamp - 1);
        map_draw_pad.draw(view_min, view_max);
        let draw_pad = map_draw_pad
            .render_state
            .seen_tiles
            .iter()
            .filter(|&&s| s == stamp)
            .count();

        assert_eq!(rect_no_pad, 0);
        assert_eq!(draw_no_pad, 0);
        assert_eq!(rect_pad, 1);
        assert_eq!(draw_pad, 1);
    }

    #[test]
    fn stamp_overflow_does_not_break_dedupe() {
        use std::collections::HashSet;

        let mut map = Map::__new_for_stamp_overflow_test(3);
        map.__set_frame_stamp_for_testing(u32::MAX - 1);

        let seq1 = collect_draw_sequence_for_test(&mut map, Vec2::ZERO, vec2(64.0, 64.0));
        let seq2 = collect_draw_sequence_for_test(&mut map, Vec2::ZERO, vec2(64.0, 64.0));
        let seq3 = collect_draw_sequence_for_test(&mut map, Vec2::ZERO, vec2(64.0, 64.0));

        assert_eq!(seq1, seq2);
        assert_eq!(seq2, seq3);
        assert_eq!(seq1.len(), 3);

        let uniq: HashSet<_> = seq1.iter().copied().collect();
        assert_eq!(uniq.len(), 3);

        let layer = &map.data.object_state.object_layers[0];
        assert_eq!(
            map.render_state.seen_objects_tiles[0].len(),
            layer.objects.len()
        );
        assert_eq!(
            map.render_state.seen_objects_debug[0].len(),
            layer.objects.len()
        );
    }

    #[test]
    fn stamp_overflow_resets_seen_buffers_before_reuse() {
        let mut map = Map::__new_for_stamp_overflow_test(2);
        map.render_state.sync_with_data(&map.data);
        map.render_state.seen_objects_tiles[0].fill(999);
        map.render_state.seen_objects_debug[0].fill(999);
        map.__set_frame_stamp_for_testing(u32::MAX);

        let stamp = map.next_frame_stamp();
        assert_eq!(stamp, 1);
        assert!(map.render_state.seen_objects_tiles[0]
            .iter()
            .all(|&s| s == 0));
        assert!(map.render_state.seen_objects_debug[0]
            .iter()
            .all(|&s| s == 0));
    }

    #[test]
    fn map_data_runtime_query_path_works_without_renderer() {
        let path = fixture_path("external_props_map.json");
        let path_str = path.to_str().expect("fixture path must be utf-8");
        let data = MapData::load(path_str).expect("map data should load headlessly");

        let object_count = data.objects().count();
        let coords = data.visible_coords_for_draw(vec2(0.0, 0.0), vec2(10.0, 10.0), 0.0);

        assert_eq!(data.object_layers().len(), 1);
        assert!(object_count > 0);
        assert_eq!(coords.len(), 1);
        assert_eq!(coords[0].x, 0);
        assert_eq!(coords[0].y, 0);
    }

    #[test]
    fn handle_object_api_rejects_invalid_handles_safely() {
        let path = fixture_path("external_props_map.json");
        let path_str = path.to_str().expect("fixture path must be utf-8");
        let mut data = MapData::load(path_str).expect("map data should load headlessly");

        let invalid = crate::spatial::ObjectHandle(999_999);
        assert!(data.object_by_handle(invalid).is_none());
        assert!(!data.update_object_bounds_position_by_handle(
            invalid, 1.0, 2.0, 3.0, 4.0
        ));
        assert!(!data.set_object_visible_by_handle(invalid, false));
        assert!(!data.set_object_alive_by_handle(invalid, false));
        assert!(!data.remove_object_by_handle(invalid));
    }

    #[test]
    fn tile_handle_lookup_rejects_invalid_and_stale_handles_safely() {
        let path = fixture_path("external_props_map.json");
        let path_str = path.to_str().expect("fixture path must be utf-8");
        let mut data = MapData::load(path_str).expect("map data should load headlessly");

        let invalid = TileHandle(999_999);
        assert!(data.tile_by_handle(invalid).is_none());
        assert!(data.tile_runtime_by_handle(invalid).is_none());

        let (layer_idx, slot_idx, handle) = data
            .tile_state
            .runtime
            .tile_handles_by_layer
            .iter()
            .enumerate()
            .find_map(|(li, layer)| {
                layer
                    .iter()
                    .enumerate()
                    .find_map(|(si, slot)| slot.map(|h| (li, si, h)))
            })
            .expect("fixture must have at least one non-empty tile");

        assert!(data.tile_by_handle(handle).is_some());
        assert!(data.tile_runtime_by_handle(handle).is_some());

        data.tile_state.runtime.tile_handles_by_layer[layer_idx][slot_idx] = None;
        assert!(data.tile_by_handle(handle).is_none());
        assert!(data.tile_runtime_by_handle(handle).is_none());
    }

    #[test]
    fn repeated_object_move_and_remove_by_handle_is_deterministic() {
        let path = fixture_path("external_props_map.json");
        let path_str = path.to_str().expect("fixture path must be utf-8");
        let mut data = MapData::load(path_str).expect("map data should load headlessly");

        let handle = data.object_state.object_handles_by_layer[0][0].expect("fixture should have one object");
        assert!(data.object_by_handle(handle).is_some());

        assert!(data.update_object_bounds_position_by_handle(
            handle, 520.0, 40.0, 32.0, 64.0
        ));
        assert!(data.update_object_bounds_position_by_handle(
            handle, 520.0, 40.0, 32.0, 64.0
        ));

        let runtime = data
            .object_runtime_by_handle(handle)
            .expect("runtime should still be present");
        assert_eq!(runtime.x, 520.0);
        assert_eq!(runtime.y, 40.0);
        assert_eq!(runtime.width, 32.0);
        assert_eq!(runtime.height, 64.0);

        let memberships = data
            .derived_index
            .object_memberships(handle)
            .expect("memberships should exist after move");
        assert!(!memberships.is_empty());

        assert!(data.set_object_visible_by_handle(handle, false));
        assert!(
            !data
                .object_runtime_by_handle(handle)
                .expect("runtime should exist")
                .visible
        );
        assert!(data.set_object_alive_by_handle(handle, false));
        assert!(data.derived_index.object_memberships(handle).is_none());
        assert!(data.set_object_alive_by_handle(handle, true));
        assert!(data.derived_index.object_memberships(handle).is_some());

        assert!(data.remove_object_by_handle(handle));
        assert!(!data.remove_object_by_handle(handle));
        assert!(data.object_by_handle(handle).is_none());
        assert!(data.object_runtime_by_handle(handle).is_none());
        assert!(data.derived_index.object_memberships(handle).is_none());
    }

    #[test]
    fn repeated_tile_mutation_and_remove_by_handle_is_deterministic() {
        let path = fixture_path("external_props_map.json");
        let path_str = path.to_str().expect("fixture path must be utf-8");
        let mut data = MapData::load(path_str).expect("map data should load headlessly");

        let handle = data
            .tile_state
            .runtime
            .tile_handles_by_layer
            .iter()
            .find_map(|layer| layer.iter().find_map(|slot| *slot))
            .expect("fixture should have one tile handle");

        let original = data
            .tile_by_handle(handle)
            .expect("tile id should exist for handle");
        assert!(data.update_tile_gid_by_handle(handle, original));
        assert!(data.move_tile_by_handle(handle, 320.0, 96.0));
        assert!(data.move_tile_by_handle(handle, 320.0, 96.0));

        let runtime = data
            .tile_runtime_by_handle(handle)
            .expect("runtime should still exist");
        assert_eq!(runtime.x, 320.0);
        assert_eq!(runtime.y, 96.0);

        assert!(data.set_tile_visible_by_handle(handle, false));
        assert!(
            !data
                .tile_runtime_by_handle(handle)
                .expect("runtime should exist")
                .visible
        );
        assert!(data.set_tile_visible_by_handle(handle, true));

        assert!(data.set_tile_alive_by_handle(handle, false));
        assert!(data.derived_index.tile_rec(handle).is_none());
        assert!(data.set_tile_alive_by_handle(handle, true));
        assert!(data.derived_index.tile_rec(handle).is_some());

        assert!(data.remove_tile_by_handle(handle));
        assert!(!data.remove_tile_by_handle(handle));
        assert!(data.tile_by_handle(handle).is_none());
        assert!(data.tile_runtime_by_handle(handle).is_none());
        assert!(data.derived_index.tile_rec(handle).is_none());
    }

    fn tile_memberships_for_handle(
        data: &MapData,
        handle: TileHandle,
    ) -> Vec<(crate::spatial::ChunkCoord, LayerIdx, TileId, Vec2)> {
        let mut out = Vec::new();
        for (cc, chunk) in &data.derived_index.buckets {
            for (layer, bucket) in &chunk.layers {
                for rec in &bucket.tiles {
                    if rec.handle == handle {
                        out.push((*cc, *layer, rec.id, rec.rel_pos));
                    }
                }
            }
        }
        out
    }

    fn assert_tile_index_consistency(data: &MapData) {
        for (layer_idx, layer_handles) in data.tile_state.runtime.tile_handles_by_layer.iter().enumerate() {
            let bucket_layer = data.tile_state.authored.tile_layers[layer_idx].bucket_layer;
            let runtime_layer = &data.tile_state.runtime.tile_runtime_by_layer[layer_idx];
            assert_eq!(layer_handles.len(), runtime_layer.len());

            for (slot_idx, slot_handle) in layer_handles.iter().enumerate() {
                match slot_handle {
                    None => {
                        assert!(
                            runtime_layer[slot_idx].is_none(),
                            "runtime slot must be cleared when handle slot is empty"
                        );
                    }
                    Some(handle) => {
                        let Some(loc) = data
                            .tile_state
                            .runtime
                            .tile_location_by_handle
                            .get(handle.0 as usize)
                            .and_then(|v| *v)
                        else {
                            panic!("tile location missing for active handle");
                        };
                        assert_eq!(loc, (layer_idx, slot_idx));

                        let runtime = runtime_layer[slot_idx]
                            .as_ref()
                            .expect("runtime must exist for active handle");
                        let memberships = tile_memberships_for_handle(data, *handle);
                        if runtime.alive {
                            assert!(
                                !memberships.is_empty(),
                                "alive tile must be indexed in at least one chunk"
                            );
                            for (cc, layer, id, rel_pos) in memberships {
                                assert_eq!(layer, bucket_layer);
                                assert_eq!(id, runtime.id);
                                let origin =
                                    vec2((cc.x * CHUNK_SIZE) as f32, (cc.y * CHUNK_SIZE) as f32);
                                let world = origin + rel_pos;
                                assert!((world.x - runtime.x).abs() < 0.01);
                                assert!((world.y - runtime.y).abs() < 0.01);
                            }
                        } else {
                            assert!(
                                memberships.is_empty(),
                                "dead tile must have no index memberships"
                            );
                        }
                    }
                }
            }
        }

        for chunk in data.derived_index.buckets.values() {
            for bucket in chunk.layers.values() {
                for rec in &bucket.tiles {
                    let Some((li, si)) = data
                        .tile_state
                        .runtime
                        .tile_location_by_handle
                        .get(rec.handle.0 as usize)
                        .and_then(|v| *v)
                    else {
                        panic!("indexed tile handle must exist in tile location map");
                    };
                    assert_eq!(
                        data.tile_state.runtime.tile_handles_by_layer[li][si],
                        Some(rec.handle)
                    );
                    let runtime = data.tile_state.runtime.tile_runtime_by_layer[li][si]
                        .as_ref()
                        .expect("indexed tile must have runtime state");
                    assert!(runtime.alive, "indexed tile must be alive");
                }
            }
        }
    }

    fn tile_state_checksum(data: &MapData) -> u64 {
        let mut h = 1469598103934665603u64;
        for (layer_idx, layer_handles) in data.tile_state.runtime.tile_handles_by_layer.iter().enumerate() {
            for (slot_idx, slot) in layer_handles.iter().enumerate() {
                let Some(handle) = slot else {
                    continue;
                };
                let Some(runtime) = data.tile_state.runtime.tile_runtime_by_layer[layer_idx][slot_idx].as_ref()
                else {
                    continue;
                };
                h = h.wrapping_mul(1099511628211).wrapping_add(handle.0 as u64);
                h = h.wrapping_mul(1099511628211).wrapping_add(runtime.id.raw() as u64);
                h = h.wrapping_mul(1099511628211).wrapping_add(runtime.x.to_bits() as u64);
                h = h.wrapping_mul(1099511628211).wrapping_add(runtime.y.to_bits() as u64);
                h = h
                    .wrapping_mul(1099511628211)
                    .wrapping_add(runtime.alive as u64);
                h = h
                    .wrapping_mul(1099511628211)
                    .wrapping_add(runtime.visible as u64);

                let mut memberships = tile_memberships_for_handle(data, *handle);
                memberships.sort_by_key(|(cc, l, id, _)| (cc.x, cc.y, *l, id.raw()));
                h = h
                    .wrapping_mul(1099511628211)
                    .wrapping_add(memberships.len() as u64);
                for (cc, l, id, _) in memberships {
                    h = h.wrapping_mul(1099511628211).wrapping_add(cc.x as u64);
                    h = h.wrapping_mul(1099511628211).wrapping_add(cc.y as u64);
                    h = h.wrapping_mul(1099511628211).wrapping_add(l as u64);
                    h = h.wrapping_mul(1099511628211).wrapping_add(id.raw() as u64);
                }
            }
        }
        h
    }

    fn run_random_tile_mutation_sequence(seed: u64) -> Vec<u64> {
        fn next_u32(state: &mut u64) -> u32 {
            *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            (*state >> 32) as u32
        }

        let path = fixture_path("external_props_map.json");
        let path_str = path.to_str().expect("fixture path must be utf-8");
        let mut data = MapData::load(path_str).expect("map data should load headlessly");
        let valid_gids: Vec<TileId> = data
            .tile_state
            .derived
            .gid_lut
            .iter()
            .enumerate()
            .filter_map(|(gid, ts)| if *ts != u16::MAX { Some(TileId(gid as u32)) } else { None })
            .collect();
        assert!(!valid_gids.is_empty());

        let mut rng = seed;
        let mut trace = Vec::new();
        for _ in 0..220 {
            let active_handles: Vec<TileHandle> = data
                .tile_state
                .runtime
                .tile_handles_by_layer
                .iter()
                .flat_map(|layer| layer.iter().flatten().copied())
                .collect();
            if active_handles.is_empty() {
                break;
            }
            let handle = active_handles[(next_u32(&mut rng) as usize) % active_handles.len()];
            match next_u32(&mut rng) % 5 {
                0 => {
                    let gid = valid_gids[(next_u32(&mut rng) as usize) % valid_gids.len()];
                    let _ = data.update_tile_gid_by_handle(handle, gid);
                }
                1 => {
                    let x = (next_u32(&mut rng) % 2048) as f32 - 1024.0;
                    let y = (next_u32(&mut rng) % 2048) as f32 - 1024.0;
                    let _ = data.move_tile_by_handle(handle, x, y);
                }
                2 => {
                    let visible = data
                        .tile_runtime_by_handle(handle)
                        .map(|r| r.visible)
                        .unwrap_or(false);
                    let _ = data.set_tile_visible_by_handle(handle, !visible);
                }
                3 => {
                    let alive = data
                        .tile_runtime_by_handle(handle)
                        .map(|r| r.alive)
                        .unwrap_or(false);
                    let _ = data.set_tile_alive_by_handle(handle, !alive);
                }
                _ => {
                    let _ = data.remove_tile_by_handle(handle);
                }
            }

            assert_tile_index_consistency(&data);
            trace.push(tile_state_checksum(&data));
        }

        trace
    }

    #[test]
    fn tile_randomized_mutation_sequence_keeps_index_consistent() {
        let _ = run_random_tile_mutation_sequence(0xA11CE5EED_u64);
    }

    #[test]
    fn tile_randomized_mutation_sequence_is_deterministic() {
        let a = run_random_tile_mutation_sequence(0xFACEFEED_u64);
        let b = run_random_tile_mutation_sequence(0xFACEFEED_u64);
        assert_eq!(a, b);
    }

    #[test]
    fn oversized_tile_move_updates_multi_chunk_membership_correctly() {
        let ir = IrMap {
            tile_w: 16,
            tile_h: 16,
            properties: Properties::default(),
            tilesets: vec![IrTileset::Atlas {
                first_gid: 1,
                source: "mock.tsx".to_string(),
                image: "mock.png".to_string(),
                tile_w: 64,
                tile_h: 64,
                tilecount: 1,
                columns: 1,
                spacing: 0,
                margin: 0,
                properties: Properties::default(),
                tiles: vec![],
            }],
            layers: vec![IrLayer {
                name: "tiles".to_string(),
                visible: true,
                opacity: 1.0,
                offset: Vec2::ZERO,
                properties: Properties::default(),
                kind: IrLayerKind::Tiles {
                    width: 1,
                    height: 1,
                    data: vec![1],
                },
            }],
        };
        let mut data = MapData::from_ir(ir).expect("map data from IR should build");
        let handle = data.tile_state.runtime.tile_handles_by_layer[0][0].expect("tile handle");

        let _ = data.move_tile_by_handle(handle, (CHUNK_SIZE - 8) as f32, 40.0);
        let memberships_a = tile_memberships_for_handle(&data, handle);
        assert!(
            memberships_a.len() > 1,
            "oversized tile should span multiple chunks near border"
        );

        let _ = data.move_tile_by_handle(handle, 32.0, 96.0);
        let memberships_b = tile_memberships_for_handle(&data, handle);
        assert_eq!(memberships_b.len(), 1);
    }

    #[test]
    fn draw_sequence_respects_runtime_visible_and_alive_flags() {
        let mut map = Map::__new_for_stamp_overflow_test(1);
        let handle = map.data.object_state.object_handles_by_layer[0][0].expect("test object handle");

        map.set_object_visible_by_handle(handle, false);
        let seq_hidden = collect_draw_sequence_for_test(&mut map, Vec2::ZERO, vec2(64.0, 64.0));
        assert!(seq_hidden.is_empty());

        map.set_object_visible_by_handle(handle, true);
        map.set_object_alive_by_handle(handle, false);
        let seq_dead = collect_draw_sequence_for_test(&mut map, Vec2::ZERO, vec2(64.0, 64.0));
        assert!(seq_dead.is_empty());

        map.set_object_alive_by_handle(handle, true);
        let seq_live = collect_draw_sequence_for_test(&mut map, Vec2::ZERO, vec2(64.0, 64.0));
        assert_eq!(seq_live.len(), 1);
        assert_eq!(seq_live[0].0, 'O');
    }

    fn run_random_mutation_sequence(seed: u64) -> Vec<Vec<u32>> {
        fn next_u32(state: &mut u64) -> u32 {
            *state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            (*state >> 32) as u32
        }

        let mut rng = seed;
        let mut map = Map::__new_for_stamp_overflow_test(0);
        let mut next_id = 1_000u32;
        let mut trace = Vec::new();

        let mut coords = Vec::new();
        for y in -8..=8 {
            for x in -8..=8 {
                coords.push(crate::spatial::ChunkCoord { x, y });
            }
        }

        for _ in 0..180 {
            let roll = next_u32(&mut rng) % 5;
            match roll {
                0 | 1 => {
                    let x = (next_u32(&mut rng) % 1800) as f32 - 900.0;
                    let y = (next_u32(&mut rng) % 1800) as f32 - 900.0;
                    let w = ((next_u32(&mut rng) % 96) + 8) as f32;
                    let h = ((next_u32(&mut rng) % 96) + 8) as f32;
                    let obj = IrObject {
                        id: next_id,
                        name: format!("r{}", next_id),
                        class_name: String::new(),
                        x,
                        y,
                        width: w,
                        height: h,
                        rotation: 0.0,
                        visible: true,
                        shape: IrObjectShape::Rectangle,
                        properties: Properties::default(),
                    };
                    next_id += 1;
                    let _ = map.spawn_object_in_layer(0, obj);
                }
                2 => {
                    let Some(layer_handles) = map.data.object_state.object_handles_by_layer.first() else {
                        continue;
                    };
                    let live: Vec<_> = layer_handles.iter().flatten().copied().collect();
                    if live.is_empty() {
                        continue;
                    }
                    let handle = live[(next_u32(&mut rng) as usize) % live.len()];
                    let x = (next_u32(&mut rng) % 1800) as f32 - 900.0;
                    let y = (next_u32(&mut rng) % 1800) as f32 - 900.0;
                    let w = ((next_u32(&mut rng) % 96) + 8) as f32;
                    let h = ((next_u32(&mut rng) % 96) + 8) as f32;
                    let _ = map.update_object_bounds_position_by_handle(handle, x, y, w, h);
                }
                3 => {
                    let Some(layer_handles) = map.data.object_state.object_handles_by_layer.first() else {
                        continue;
                    };
                    let live: Vec<_> = layer_handles.iter().flatten().copied().collect();
                    if live.is_empty() {
                        continue;
                    }
                    let handle = live[(next_u32(&mut rng) as usize) % live.len()];
                    let alive = map
                        .object_runtime_by_handle(handle)
                        .map(|s| s.alive)
                        .unwrap_or(false);
                    let _ = map.set_object_alive_by_handle(handle, !alive);
                }
                _ => {
                    let Some(layer_handles) = map.data.object_state.object_handles_by_layer.first() else {
                        continue;
                    };
                    let live: Vec<_> = layer_handles.iter().flatten().copied().collect();
                    if live.is_empty() {
                        continue;
                    }
                    let handle = live[(next_u32(&mut rng) as usize) % live.len()];
                    let _ = map.remove_object_by_handle(handle);
                }
            }

            let got = map.query_object_handles_in_coords(0, &coords);
            let got_ids: Vec<u32> = got.iter().map(|h| h.0).collect();
            let uniq: std::collections::HashSet<u32> = got_ids.iter().copied().collect();
            assert_eq!(uniq.len(), got_ids.len(), "query returned duplicate handles");

            let mut expected = Vec::new();
            if let Some(layer) = map.data.object_state.object_layers.first() {
                for (idx, slot) in map.data.object_state.object_handles_by_layer[0].iter().enumerate() {
                    let Some(handle) = slot else {
                        continue;
                    };
                    let Some(runtime) = map
                        .data
                        .object_state.object_runtime_by_layer
                        .first()
                        .and_then(|v| v.get(idx))
                        .and_then(|s| s.as_ref())
                    else {
                        continue;
                    };
                    if !runtime.alive {
                        continue;
                    }
                    let Some(obj) = layer.objects.get(idx) else {
                        continue;
                    };
                    let (min_c, max_c) = crate::core::object_chunk_span_runtime(obj, *runtime, layer.offset);
                    let mut overlaps = false;
                    for cc in &coords {
                        if cc.x >= min_c.x && cc.x <= max_c.x && cc.y >= min_c.y && cc.y <= max_c.y {
                            overlaps = true;
                            break;
                        }
                    }
                    if overlaps {
                        expected.push(handle.0);
                    }
                }
            }
            expected.sort_unstable();
            assert_eq!(got_ids, expected, "query drifted from canonical runtime state");
            trace.push(got_ids);
        }

        trace
    }

    #[test]
    fn randomized_mutation_sequence_keeps_index_consistent() {
        let _ = run_random_mutation_sequence(0xC0FFEE_u64);
    }

    #[test]
    fn randomized_mutation_sequence_is_deterministic() {
        let a = run_random_mutation_sequence(0xDEADBEEF_u64);
        let b = run_random_mutation_sequence(0xDEADBEEF_u64);
        assert_eq!(a, b);
    }

    #[test]
    fn visible_zombies_query_by_tag_returns_expected_ids() {
        let mut map = Map::__new_for_stamp_overflow_test(0);

        let mut zombie_props = Properties::new();
        zombie_props.insert(
            "tags".to_string(),
            PropertyValue::String("enemy,zombie".to_string()),
        );
        let zombie_a = IrObject {
            id: 2001,
            name: "zombie_a".to_string(),
            class_name: "npc".to_string(),
            x: 32.0,
            y: 32.0,
            width: 16.0,
            height: 16.0,
            rotation: 0.0,
            visible: true,
            shape: IrObjectShape::Rectangle,
            properties: zombie_props.clone(),
        };
        let zombie_b = IrObject {
            id: 2002,
            name: "zombie_b".to_string(),
            class_name: "npc".to_string(),
            x: 96.0,
            y: 96.0,
            width: 16.0,
            height: 16.0,
            rotation: 0.0,
            visible: true,
            shape: IrObjectShape::Rectangle,
            properties: zombie_props,
        };
        let civilian = IrObject {
            id: 2003,
            name: "civilian".to_string(),
            class_name: "npc".to_string(),
            x: 48.0,
            y: 48.0,
            width: 16.0,
            height: 16.0,
            rotation: 0.0,
            visible: true,
            shape: IrObjectShape::Rectangle,
            properties: Properties::default(),
        };

        let _ = map.spawn_object_in_layer(0, zombie_a);
        let _ = map.spawn_object_in_layer(0, zombie_b);
        let _ = map.spawn_object_in_layer(0, civilian);

        let ids = map.query_visible_object_ids(
            0,
            vec2(0.0, 0.0),
            vec2(128.0, 128.0),
            ObjectQueryFilter {
                kind: None,
                tag: Some("zombie"),
            },
        );
        assert_eq!(ids, vec![2001, 2002]);
    }

    #[test]
    fn despawned_entries_disappear_from_filtered_visible_query() {
        let mut map = Map::__new_for_stamp_overflow_test(0);
        let mut props = Properties::new();
        props.insert(
            "tag".to_string(),
            PropertyValue::String("zombie".to_string()),
        );
        let h = map
            .spawn_object_in_layer(
                0,
                IrObject {
                    id: 2101,
                    name: "z".to_string(),
                    class_name: "npc".to_string(),
                    x: 40.0,
                    y: 40.0,
                    width: 16.0,
                    height: 16.0,
                    rotation: 0.0,
                    visible: true,
                    shape: IrObjectShape::Rectangle,
                    properties: props,
                },
            )
            .expect("spawn should succeed");

        let before = map.query_visible_object_ids(
            0,
            vec2(0.0, 0.0),
            vec2(128.0, 128.0),
            ObjectQueryFilter {
                kind: None,
                tag: Some("zombie"),
            },
        );
        assert_eq!(before, vec![2101]);

        assert!(map.set_object_alive_by_handle(h, false));
        let after = map.query_visible_object_ids(
            0,
            vec2(0.0, 0.0),
            vec2(128.0, 128.0),
            ObjectQueryFilter {
                kind: None,
                tag: Some("zombie"),
            },
        );
        assert!(after.is_empty());
    }

    #[test]
    fn moved_entries_relocate_between_query_windows() {
        let mut map = Map::__new_for_stamp_overflow_test(0);
        let h = map
            .spawn_object_in_layer(
                0,
                IrObject {
                    id: 2201,
                    name: "roamer".to_string(),
                    class_name: "zombie".to_string(),
                    x: 20.0,
                    y: 20.0,
                    width: 16.0,
                    height: 16.0,
                    rotation: 0.0,
                    visible: true,
                    shape: IrObjectShape::Rectangle,
                    properties: Properties::default(),
                },
            )
            .expect("spawn should succeed");

        let left = map.query_visible_object_ids(
            0,
            vec2(0.0, 0.0),
            vec2(64.0, 64.0),
            ObjectQueryFilter {
                kind: Some("zombie"),
                tag: None,
            },
        );
        let right = map.query_visible_object_ids(
            0,
            vec2(256.0, 0.0),
            vec2(320.0, 64.0),
            ObjectQueryFilter {
                kind: Some("zombie"),
                tag: None,
            },
        );
        assert_eq!(left, vec![2201]);
        assert!(right.is_empty());

        assert!(map.update_object_bounds_position_by_handle(
            h, 272.0, 20.0, 16.0, 16.0
        ));
        let left_after = map.query_visible_object_ids(
            0,
            vec2(0.0, 0.0),
            vec2(64.0, 64.0),
            ObjectQueryFilter {
                kind: Some("zombie"),
                tag: None,
            },
        );
        let right_after = map.query_visible_object_ids(
            0,
            vec2(256.0, 0.0),
            vec2(320.0, 64.0),
            ObjectQueryFilter {
                kind: Some("zombie"),
                tag: None,
            },
        );
        assert!(left_after.is_empty());
        assert_eq!(right_after, vec![2201]);
    }

    #[derive(Debug, Clone, PartialEq)]
    struct PersistedObjectPolicyView {
        id: u32,
        name: String,
        class_name: String,
        visible: bool,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        rotation: f32,
    }

    fn persisted_objects_for_policy_test(map: &Map, layer_idx: usize) -> Vec<PersistedObjectPolicyView> {
        let mut out = Vec::new();
        let Some(layer) = map.data.object_state.object_layers.get(layer_idx) else {
            return out;
        };
        for (i, authored) in layer.objects.iter().enumerate() {
            let Some(Some(handle)) = map
                .data
                .object_state.object_handles_by_layer
                .get(layer_idx)
                .and_then(|v| v.get(i))
            else {
                continue;
            };
            let Some(runtime) = map.object_runtime_by_handle(*handle) else {
                continue;
            };
            if !runtime.alive {
                continue;
            }
            out.push(PersistedObjectPolicyView {
                id: authored.id,
                name: authored.name.clone(),
                class_name: authored.class_name.clone(),
                visible: runtime.visible,
                x: runtime.x,
                y: runtime.y,
                width: runtime.width,
                height: runtime.height,
                rotation: authored.rotation,
            });
        }
        out
    }

    #[test]
    fn persistence_policy_saves_runtime_geometry_and_visibility() {
        let mut map = Map::__new_for_stamp_overflow_test(0);
        let h = map
            .spawn_object_in_layer(
                0,
                IrObject {
                    id: 3001,
                    name: "npc".to_string(),
                    class_name: "zombie".to_string(),
                    x: 10.0,
                    y: 20.0,
                    width: 16.0,
                    height: 16.0,
                    rotation: 0.0,
                    visible: true,
                    shape: IrObjectShape::Rectangle,
                    properties: Properties::default(),
                },
            )
            .expect("spawn should succeed");

        assert!(map.update_object_bounds_position_by_handle(h, 111.0, 222.0, 33.0, 44.0));
        assert!(map.set_object_visible_by_handle(h, false));

        let persisted = persisted_objects_for_policy_test(&map, 0);
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].id, 3001);
        assert!(!persisted[0].visible);
        assert_eq!(persisted[0].x, 111.0);
        assert_eq!(persisted[0].y, 222.0);
        assert_eq!(persisted[0].width, 33.0);
        assert_eq!(persisted[0].height, 44.0);
    }

    #[test]
    fn persistence_policy_omits_despawned_objects() {
        let mut map = Map::__new_for_stamp_overflow_test(0);
        let h = map
            .spawn_object_in_layer(
                0,
                IrObject {
                    id: 3002,
                    name: "gone".to_string(),
                    class_name: "npc".to_string(),
                    x: 10.0,
                    y: 20.0,
                    width: 16.0,
                    height: 16.0,
                    rotation: 0.0,
                    visible: true,
                    shape: IrObjectShape::Rectangle,
                    properties: Properties::default(),
                },
            )
            .expect("spawn should succeed");
        assert!(map.set_object_alive_by_handle(h, false));

        let persisted = persisted_objects_for_policy_test(&map, 0);
        assert!(persisted.is_empty());
    }

    #[test]
    fn persistence_policy_excludes_render_state_fields() {
        let mut map = Map::__new_for_stamp_overflow_test(0);
        let _ = map.spawn_object_in_layer(
            0,
            IrObject {
                id: 3003,
                name: "stable".to_string(),
                class_name: "npc".to_string(),
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
                rotation: 0.0,
                visible: true,
                shape: IrObjectShape::Rectangle,
                properties: Properties::default(),
            },
        );

        let before = persisted_objects_for_policy_test(&map, 0);
        map.set_cull_padding(999.0);
        map.set_debug_draw(true);
        map.__set_frame_stamp_for_testing(777);
        let after = persisted_objects_for_policy_test(&map, 0);
        assert_eq!(before, after);
    }

    #[test]
    fn load_mutate_save_reload_roundtrip_consistency() {
        let source = fixture_path("external_props_map.json");
        let src_str = source.to_str().expect("fixture path utf8");
        let mut data = MapData::load(src_str).expect("source map should load");

        let handle = data.object_state.object_handles_by_layer[0][0].expect("object handle exists");
        assert!(data.update_object_bounds_position_by_handle(
            handle, 144.0, 55.0, 30.0, 42.0
        ));
        assert!(data.set_object_visible_by_handle(handle, false));

        let out = temp_export_path("roundtrip_export");
        let out_str = out.to_str().expect("export path utf8");
        data.save_to_json(out_str).expect("export should succeed");
        let reloaded = MapData::load(out_str).expect("reloaded map should parse");

        let obj = &reloaded.object_layers()[0].objects[0];
        assert_eq!(obj.id, 7);
        assert_eq!(obj.x, 144.0);
        assert_eq!(obj.y, 55.0);
        assert_eq!(obj.width, 30.0);
        assert_eq!(obj.height, 42.0);
        assert!(!obj.visible);

        let _ = std::fs::remove_file(out);
    }

    #[test]
    fn export_roundtrip_preserves_id_stable_entity_attributes() {
        let source = fixture_path("external_props_map.json");
        let src_str = source.to_str().expect("fixture path utf8");
        let mut map = Map::__new_for_stamp_overflow_test(0);
        let mut props = Properties::new();
        props.insert("tag".to_string(), PropertyValue::String("zombie".to_string()));
        let _ = map.spawn_object_in_layer(
            0,
            IrObject {
                id: 9001,
                name: "boss".to_string(),
                class_name: "zombie".to_string(),
                x: 200.0,
                y: 300.0,
                width: 48.0,
                height: 64.0,
                rotation: 0.0,
                visible: true,
                shape: IrObjectShape::Rectangle,
                properties: props,
            },
        );

        // Ensure export path also works from regular loaded maps.
        let _ = MapData::load(src_str).expect("baseline fixture should load");

        let out = temp_export_path("id_stable_export");
        let out_str = out.to_str().expect("export path utf8");
        map.save_to_json(out_str).expect("export should succeed");
        let reloaded = MapData::load(out_str).expect("reloaded map should parse");

        let ids: Vec<u32> = reloaded.object_layers()[0].objects.iter().map(|o| o.id).collect();
        assert!(ids.contains(&9001));
        let boss = reloaded
            .object_layers()[0]
            .objects
            .iter()
            .find(|o| o.id == 9001)
            .expect("boss object should persist");
        assert_eq!(boss.class_name, "zombie");
        assert_eq!(boss.properties.get_string("tag"), Some("zombie"));
        assert_eq!(boss.width, 48.0);
        assert_eq!(boss.height, 64.0);

        let _ = std::fs::remove_file(out);
    }

    #[test]
    fn tiled_flip_flags_match_expected_transform_for_all_8_combinations() {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        struct M2 {
            a11: i32,
            a12: i32,
            a21: i32,
            a22: i32,
        }

        fn mul(lhs: M2, rhs: M2) -> M2 {
            M2 {
                a11: lhs.a11 * rhs.a11 + lhs.a12 * rhs.a21,
                a12: lhs.a11 * rhs.a12 + lhs.a12 * rhs.a22,
                a21: lhs.a21 * rhs.a11 + lhs.a22 * rhs.a21,
                a22: lhs.a21 * rhs.a12 + lhs.a22 * rhs.a22,
            }
        }

        // Build Tiled's transform using documented order:
        // D (axis swap), then H, then V.
        fn tiled_matrix(h: bool, v: bool, d: bool) -> M2 {
            let i = M2 {
                a11: 1,
                a12: 0,
                a21: 0,
                a22: 1,
            };
            let mh = M2 {
                a11: -1,
                a12: 0,
                a21: 0,
                a22: 1,
            };
            let mv = M2 {
                a11: 1,
                a12: 0,
                a21: 0,
                a22: -1,
            };
            let md = M2 {
                a11: 0,
                a12: 1,
                a21: 1,
                a22: 0,
            };
            let after_d = if d { md } else { i };
            let after_h = if h { mul(mh, after_d) } else { after_d };
            if v {
                mul(mv, after_h)
            } else {
                after_h
            }
        }

        // Macroquad applies flips first, then rotation.
        fn macroquad_matrix(rotation: f32, flip_x: bool, flip_y: bool) -> M2 {
            let r90 = std::f32::consts::FRAC_PI_2;
            let r180 = std::f32::consts::PI;
            let r270 = 3.0 * std::f32::consts::FRAC_PI_2;

            let r = if (rotation - 0.0).abs() < 1e-6 {
                M2 {
                    a11: 1,
                    a12: 0,
                    a21: 0,
                    a22: 1,
                }
            } else if (rotation - r90).abs() < 1e-6 {
                M2 {
                    a11: 0,
                    a12: -1,
                    a21: 1,
                    a22: 0,
                }
            } else if (rotation - r180).abs() < 1e-6 {
                M2 {
                    a11: -1,
                    a12: 0,
                    a21: 0,
                    a22: -1,
                }
            } else if (rotation - r270).abs() < 1e-6 {
                M2 {
                    a11: 0,
                    a12: 1,
                    a21: -1,
                    a22: 0,
                }
            } else {
                panic!(
                    "rotation must be a multiple of 90 degrees, got {}",
                    rotation
                );
            };

            let fx = if flip_x {
                M2 {
                    a11: -1,
                    a12: 0,
                    a21: 0,
                    a22: 1,
                }
            } else {
                M2 {
                    a11: 1,
                    a12: 0,
                    a21: 0,
                    a22: 1,
                }
            };
            let fy = if flip_y {
                M2 {
                    a11: 1,
                    a12: 0,
                    a21: 0,
                    a22: -1,
                }
            } else {
                M2 {
                    a11: 1,
                    a12: 0,
                    a21: 0,
                    a22: 1,
                }
            };

            mul(r, mul(fx, fy))
        }

        for h in [false, true] {
            for v in [false, true] {
                for d in [false, true] {
                    let mut raw = 1u32;
                    if h {
                        raw |= crate::spatial::FLIP_H;
                    }
                    if v {
                        raw |= crate::spatial::FLIP_V;
                    }
                    if d {
                        raw |= crate::spatial::FLIP_D;
                    }

                    let (rotation, flip_x, flip_y, _pivot) =
                        Map::params_for_flips_gid(TileId(raw), 16.0, 16.0);

                    let expected = tiled_matrix(h, v, d);
                    let actual = macroquad_matrix(rotation, flip_x, flip_y);
                    assert_eq!(
                        actual, expected,
                        "mismatch for flags h={} v={} d={}",
                        h, v, d
                    );
                }
            }
        }
    }
}






