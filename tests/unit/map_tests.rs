#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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
            index: GlobalIndex::new(),
            tilesets: vec![],
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
            gid_lut: vec![],
            tile_layers: vec![],
            draw_order: vec![],
            layer_kind_by_id: HashMap::new(),
        };
        let mut state = RenderState::default();
        state.sync_with_data(&data);

        assert_eq!(state.seen_objects_tiles[0].len(), 3);
        assert_eq!(state.seen_objects_debug[0].len(), 3);

        let stamp = 42;
        let mut first_pass_drawn = 0usize;
        for object_idx in 0..data.object_layers[0].objects.len() {
            if state.seen_objects_tiles[0][object_idx] == stamp {
                continue;
            }
            state.seen_objects_tiles[0][object_idx] = stamp;
            first_pass_drawn += 1;
        }

        let mut second_pass_drawn = 0usize;
        for object_idx in 0..data.object_layers[0].objects.len() {
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
        let Some(layer) = map.data.tile_layers.get(tile_layer_idx).copied() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let seen = &mut map.render_state.seen_tiles;

        Map::for_each_visible_layer_bucket(
            &map.data.index,
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

    fn collect_draw_sequence_for_test(
        map: &mut Map,
        view_min: Vec2,
        view_max: Vec2,
    ) -> Vec<(char, LayerId, i32, i32, u32)> {
        let coords = map.visible_coords_for_draw(view_min, view_max);
        let stamp = map.next_frame_stamp();
        let mut out = Vec::new();

        for i in 0..map.data.draw_order.len() {
            let layer_id = map.data.draw_order[i];
            let Some(kind) = map.data.layer_kind_by_id.get(&layer_id).copied() else {
                continue;
            };

            match kind {
                LayerKindInfo::Tiles(tile_layer_idx) => {
                    let Some(layer) = map.data.tile_layers.get(tile_layer_idx) else {
                        continue;
                    };
                    if !layer.visible {
                        continue;
                    }

                    Map::for_each_visible_layer_bucket(
                        &map.data.index,
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
                    let Some(layer) = map.data.object_layers.get_mut(object_layer_idx) else {
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
                        &map.data.index,
                        &coords,
                        bucket_layer,
                        |cc, bucket| {
                            for rec in &bucket.objects {
                                let object_idx = rec.handle.0 as usize;
                                if object_idx >= layer.objects.len() {
                                    continue;
                                }
                                if seen_layer[object_idx] == stamp {
                                    continue;
                                }
                                seen_layer[object_idx] = stamp;

                                let Some(obj) = layer.objects.get(object_idx) else {
                                    continue;
                                };
                                if !obj.visible {
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
                index,
                tilesets: vec![],
                object_layers: vec![],
                gid_lut: vec![],
                tile_layers: vec![TileLayerDrawInfo {
                    layer_id: 0,
                    visible: true,
                    opacity: 1.0,
                }],
                draw_order: vec![0],
                layer_kind_by_id: {
                    let mut m = HashMap::new();
                    m.insert(0, LayerKindInfo::Tiles(0));
                    m
                },
            },
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
                handle: crate::spatial::ObjectHandle(0),
                rel_pos: vec2((CHUNK_SIZE - 8) as f32, 32.0),
            },
        );
        index.insert_object(
            1,
            crate::spatial::ChunkCoord { x: 1, y: 0 },
            crate::spatial::ObjectRec {
                handle: crate::spatial::ObjectHandle(0),
                rel_pos: vec2(0.0, 32.0),
            },
        );

        let mut map = Map {
            data: MapData {
                index,
                tilesets: vec![],
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
                gid_lut: vec![],
                tile_layers: vec![
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
                draw_order: vec![0, 1, 2],
                layer_kind_by_id: {
                    let mut m = HashMap::new();
                    m.insert(0, LayerKindInfo::Tiles(0));
                    m.insert(1, LayerKindInfo::Objects(0));
                    m.insert(2, LayerKindInfo::Tiles(1));
                    m
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
                index,
                tilesets: vec![],
                object_layers: vec![],
                gid_lut: vec![],
                tile_layers: vec![TileLayerDrawInfo {
                    layer_id: 0,
                    visible: true,
                    opacity: 1.0,
                }],
                draw_order: vec![0],
                layer_kind_by_id: {
                    let mut m = HashMap::new();
                    m.insert(0, LayerKindInfo::Tiles(0));
                    m
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
        assert_eq!(data.object_layers.len(), 1);
        assert_eq!(data.tile_layers.len(), 2);
        assert_eq!(data.draw_order, vec![0, 1, 2]);
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

        let layer = &map.data.object_layers[0];
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
