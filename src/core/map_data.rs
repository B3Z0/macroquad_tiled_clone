use crate::ir_map::*;
use crate::loader::json_loader::*;
use crate::spatial::{world_to_chunk, GlobalIndex, LayerIdx, TileId, CHUNK_SIZE};
use crate::MapError;
use macroquad::prelude::*;
use std::collections::HashMap;

/// Stable layer identifier used by the renderer draw order.
///
/// The value maps to Tiled layer array order in the loaded map.
pub type LayerId = u32;

pub struct TilesetRuntimeInfo {
    pub first_gid: u32,
    #[allow(dead_code)]
    pub tilecount: u32,
    pub cols: u32,
    pub image: String,
    pub tile_w: u32,
    pub tile_h: u32,
    pub spacing: u32,
    pub margin: u32,
}

/// A Tiled object layer parsed from the map.
///
/// Stable API: this struct is exposed for inspection/querying (`Map::object_layers`),
/// not for direct mutation of rendering internals.
pub struct ObjectLayer {
    /// Stable layer id matching Tiled layer order.
    pub id: LayerId,
    /// Layer name from Tiled.
    pub name: String,
    /// Visibility flag from Tiled.
    pub visible: bool,
    /// Opacity from Tiled (0.0..=1.0).
    pub opacity: f32,
    /// Layer offset in world coordinates.
    pub offset: Vec2,
    /// Custom layer properties.
    pub properties: Properties,
    /// Parsed objects in this layer.
    pub objects: Vec<IrObject>,
    pub(crate) bucket_layer: LayerIdx,
}

#[derive(Clone, Copy)]
pub(crate) struct TileLayerDrawInfo {
    pub(crate) layer_id: LayerIdx,
    pub(crate) visible: bool,
    pub(crate) opacity: f32,
}

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

    for (lz, layer) in layers.iter().enumerate() {
        let stable_id = lz as LayerId;
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

/// Runtime/query data for a loaded map, independent from render-frame state.
pub struct MapData {
    pub(crate) index: GlobalIndex,
    pub(crate) tilesets: Vec<TilesetRuntimeInfo>,
    pub(crate) object_layers: Vec<ObjectLayer>,
    pub(crate) gid_lut: Vec<u16>, // lookup table for tile GIDs to tileset indices
    pub(crate) tile_layers: Vec<TileLayerDrawInfo>,
    pub(crate) draw_order: Vec<LayerId>,
    pub(crate) layer_kind_by_id: HashMap<LayerId, LayerKindInfo>,
}

impl MapData {
    /// Loads runtime/query map data without binding render textures.
    pub fn load(path: &str) -> Result<Self, MapError> {
        let (ir, _) = decode_map_file_to_ir(path)?;
        Self::from_ir(ir)
    }

    /// Returns parsed object layers for inspection/querying.
    pub fn object_layers(&self) -> &[ObjectLayer] {
        &self.object_layers
    }

    /// Iterates all parsed objects across all object layers.
    pub fn objects(&self) -> impl Iterator<Item = &IrObject> {
        self.object_layers
            .iter()
            .flat_map(|layer| layer.objects.iter())
    }

    pub(crate) fn visible_coords_for_draw(
        &self,
        view_min: Vec2,
        view_max: Vec2,
        cull_padding: f32,
    ) -> Vec<crate::spatial::ChunkCoord> {
        let min = vec2(view_min.x - cull_padding, view_min.y - cull_padding);
        let max = vec2(view_max.x + cull_padding, view_max.y + cull_padding);

        let mut cx_min = (min.x as i32).div_euclid(CHUNK_SIZE);
        let mut cy_min = (min.y as i32).div_euclid(CHUNK_SIZE);
        let mut cx_max = (max.x as i32).div_euclid(CHUNK_SIZE);
        let mut cy_max = (max.y as i32).div_euclid(CHUNK_SIZE);

        if cx_min > cx_max {
            std::mem::swap(&mut cx_min, &mut cx_max);
        }
        if cy_min > cy_max {
            std::mem::swap(&mut cy_min, &mut cy_max);
        }

        let mut coords = Vec::new();
        for cy in cy_min..=cy_max {
            for cx in cx_min..=cx_max {
                coords.push(crate::spatial::ChunkCoord { x: cx, y: cy });
            }
        }
        coords
    }

    fn ts_for_gid_from<'a>(
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
        let mut tile_layers: Vec<TileLayerDrawInfo> = Vec::new();
        let (draw_order, layer_kind_by_id) = build_draw_order_and_kind(&ir.layers);

        for (lz, layer) in ir.layers.iter().enumerate() {
            match &layer.kind {
                IrLayerKind::Objects { objects } => {
                    let bucket_layer = lz as LayerIdx;
                    let layer_idx = object_layers.len();
                    object_layers.push(ObjectLayer {
                        id: lz as LayerId,
                        name: layer.name.clone(),
                        visible: layer.visible,
                        opacity: layer.opacity,
                        offset: layer.offset,
                        properties: layer.properties.clone(),
                        objects: objects.clone(),
                        bucket_layer,
                    });

                    for (object_idx, obj) in objects.iter().enumerate() {
                        let world = vec2(obj.x, obj.y) + layer.offset;
                        let (chunk_min, chunk_max) = object_chunk_span(obj, layer.offset);

                        for cy in chunk_min.y..=chunk_max.y {
                            for cx in chunk_min.x..=chunk_max.x {
                                let cc = crate::spatial::ChunkCoord { x: cx, y: cy };
                                let chunk_origin =
                                    vec2((cc.x * CHUNK_SIZE) as f32, (cc.y * CHUNK_SIZE) as f32);
                                index.insert_object(
                                    bucket_layer,
                                    cc,
                                    crate::spatial::ObjectRec {
                                        handle: crate::spatial::ObjectHandle(object_idx as u32),
                                        rel_pos: world - chunk_origin,
                                    },
                                );
                            }
                        }
                    }
                    debug_assert!(matches!(
                        layer_kind_by_id.get(&(lz as LayerId)),
                        Some(LayerKindInfo::Objects(idx)) if *idx == layer_idx
                    ));
                }
                IrLayerKind::Tiles {
                    width,
                    height: _,
                    data,
                } => {
                    let lid = lz as LayerIdx;
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
                        let Some((ts, _)) = Self::ts_for_gid_from(tile_id, &gid_lut, &tilesets)
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
                                        lid,
                                        cc,
                                        draw_origin,
                                    );
                                }
                            }
                        } else {
                            index.add_tile(tile_id, lid, draw_origin);
                        }
                    }

                    tile_layers.push(TileLayerDrawInfo {
                        layer_id: lid,
                        visible: layer.visible,
                        opacity: layer.opacity.clamp(0.0, 1.0),
                    });
                    debug_assert!(matches!(
                        layer_kind_by_id.get(&(lz as LayerId)),
                        Some(LayerKindInfo::Tiles(idx)) if *idx == tile_layer_idx
                    ));
                }
                IrLayerKind::Unsupported => {}
            }
        }

        Ok(Self {
            index,
            tilesets,
            object_layers,
            gid_lut,
            tile_layers,
            draw_order,
            layer_kind_by_id,
        })
    }
}

fn object_aabb_world(obj: &IrObject, layer_offset: Vec2) -> (Vec2, Vec2) {
    let origin = vec2(obj.x, obj.y) + layer_offset;

    match &obj.shape {
        IrObjectShape::Rectangle => {
            let x2 = origin.x + obj.width;
            let y2 = origin.y + obj.height;
            (
                vec2(origin.x.min(x2), origin.y.min(y2)),
                vec2(origin.x.max(x2), origin.y.max(y2)),
            )
        }
        IrObjectShape::Point => (origin - vec2(0.5, 0.5), origin + vec2(0.5, 0.5)),
        IrObjectShape::Polygon(points) | IrObjectShape::Polyline(points) => {
            if points.is_empty() {
                return (origin - vec2(0.5, 0.5), origin + vec2(0.5, 0.5));
            }

            let mut min_x = origin.x;
            let mut min_y = origin.y;
            let mut max_x = origin.x;
            let mut max_y = origin.y;

            for p in points {
                let wp = origin + *p;
                min_x = min_x.min(wp.x);
                min_y = min_y.min(wp.y);
                max_x = max_x.max(wp.x);
                max_y = max_y.max(wp.y);
            }

            (vec2(min_x, min_y), vec2(max_x, max_y))
        }
        IrObjectShape::Tile { .. } => {
            // Tile objects are drawn at (x, y - h), so AABB must match that.
            let w = if obj.width > 0.0 { obj.width } else { 1.0 };
            let h = if obj.height > 0.0 { obj.height } else { 1.0 };
            (vec2(origin.x, origin.y - h), vec2(origin.x + w, origin.y))
        }
    }
}

pub(crate) fn object_chunk_span(
    obj: &IrObject,
    layer_offset: Vec2,
) -> (crate::spatial::ChunkCoord, crate::spatial::ChunkCoord) {
    let (min, max) = object_aabb_world(obj, layer_offset);
    (world_to_chunk(min), world_to_chunk(max))
}

pub(crate) fn tile_chunk_span(
    world: Vec2,
    draw_w: f32,
    draw_h: f32,
) -> (crate::spatial::ChunkCoord, crate::spatial::ChunkCoord) {
    let max = vec2(
        world.x + draw_w.max(1.0) - f32::EPSILON,
        world.y + draw_h.max(1.0) - f32::EPSILON,
    );
    (world_to_chunk(world), world_to_chunk(max))
}

pub(crate) fn tile_draw_origin(world: Vec2, map_tile_h: u32, tile_h: u32) -> Vec2 {
    // For orthogonal tile layers, tiles are bottom-aligned to the map cell.
    // This keeps oversized tiles extending upward instead of downward.
    vec2(world.x, world.y + (map_tile_h as f32 - tile_h as f32))
}
