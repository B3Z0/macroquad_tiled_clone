use crate::ir_map::*;
use crate::loader::json_loader::*;
use crate::render::*;
use crate::spatial::{world_to_chunk, GlobalIndex, LayerIdx, TileId, CHUNK_SIZE};
use crate::MapError;
use macroquad::prelude::*;
use std::collections::HashMap;
use std::path::Path;

/// Stable layer identifier used by the renderer draw order.
///
/// The value maps to Tiled layer array order in the loaded map.
pub type LayerId = u32;

pub struct TilesetInfo {
    pub first_gid: u32,
    #[allow(dead_code)]
    pub tilecount: u32,
    pub cols: u32,
    pub tex: Texture2D,
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
    bucket_layer: LayerIdx,
    // Separate dedupe buffers let tile-object rendering and debug overlay
    // each draw an object once per frame, using the same frame stamp.
    seen_stamp_tiles: Vec<u32>,
    seen_stamp_debug: Vec<u32>,
}

#[derive(Clone, Copy)]
struct TileLayerDrawInfo {
    layer_id: LayerIdx,
    visible: bool,
    opacity: f32,
}

#[derive(Clone, Copy)]
enum LayerKindInfo {
    Tiles(usize),
    Objects(usize),
    Unsupported,
}

/// Internal render state and configuration.
///
/// This is not part of the stable public API; callers should use [`Map`] methods.
struct MapRenderer {
    debug_draw: bool,
    cull_padding: f32,
    frame_stamp: u32,
}

impl MapRenderer {
    fn new() -> Self {
        Self::default()
    }

    fn next_frame_stamp(&mut self, object_layers: &mut [ObjectLayer]) -> u32 {
        for layer in object_layers.iter_mut() {
            ensure_object_layer_stamp_invariant(layer);
        }
        if self.frame_stamp == u32::MAX {
            for layer in object_layers {
                layer.seen_stamp_tiles.fill(0);
                layer.seen_stamp_debug.fill(0);
            }
            self.frame_stamp = 1;
            return 1;
        }

        self.frame_stamp += 1;
        self.frame_stamp
    }
}

impl Default for MapRenderer {
    fn default() -> Self {
        Self {
            debug_draw: false,
            cull_padding: CHUNK_SIZE as f32,
            frame_stamp: 0,
        }
    }
}

fn build_draw_order_and_kind(
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

fn ensure_object_layer_stamp_invariant(layer: &mut ObjectLayer) {
    let object_count = layer.objects.len();
    if layer.seen_stamp_tiles.len() != object_count {
        layer.seen_stamp_tiles.resize(object_count, 0);
    }
    if layer.seen_stamp_debug.len() != object_count {
        layer.seen_stamp_debug.resize(object_count, 0);
    }

    debug_assert_eq!(layer.seen_stamp_tiles.len(), object_count);
    debug_assert_eq!(layer.seen_stamp_debug.len(), object_count);
}

/// Loaded Tiled map with rendering helpers.
///
/// Coordinate contract:
/// - All draw APIs use world-space pixel coordinates (`Vec2`) in Macroquad's coordinate space.
/// - `view_min` and `view_max` are opposite corners of the view rectangle, not width/height.
/// - Order of corners is normalized internally (`min/max` are swapped when needed).
///
/// Stamp contract:
/// - [`Map::draw`] and non-`_with_stamp` object methods manage stamps automatically.
/// - `_with_stamp` methods are advanced APIs for manual pass composition in one frame.
pub struct Map {
    index: GlobalIndex,
    tilesets: Vec<TilesetInfo>,
    object_layers: Vec<ObjectLayer>,
    renderer: MapRenderer,
    gid_lut: Vec<u16>, //lookup table for tile GIDs to tileset indices
    tile_layers: Vec<TileLayerDrawInfo>,
    draw_order: Vec<LayerId>,
    layer_kind_by_id: HashMap<LayerId, LayerKindInfo>,
}

impl Map {
    /// Loads a Tiled map JSON file and its external tilesets/textures.
    ///
    /// This is the stable entry point for creating a [`Map`].
    pub async fn load(path: &str) -> Result<Self, MapError> {
        let (ir, base) = decode_map_file_to_ir(path)?;
        Self::from_ir(ir, &base).await
    }

    #[doc(hidden)]
    pub fn __new_for_stamp_overflow_test(object_count: usize) -> Self {
        let mut index = GlobalIndex::new();
        let mut objects = Vec::with_capacity(object_count);
        for i in 0..object_count {
            objects.push(IrObject {
                id: i as u32,
                name: String::new(),
                class_name: String::new(),
                x: 8.0,
                y: 8.0,
                width: 16.0,
                height: 16.0,
                rotation: 0.0,
                visible: true,
                shape: IrObjectShape::Tile { gid: 1 },
                properties: Properties::default(),
            });
            index.insert_object(
                0,
                crate::spatial::ChunkCoord { x: 0, y: 0 },
                crate::spatial::ObjectRec {
                    handle: crate::spatial::ObjectHandle(i as u32),
                    rel_pos: vec2(0.0, 0.0),
                },
            );
        }

        let mut object_layer = ObjectLayer {
            id: 0,
            name: "test".to_string(),
            visible: true,
            opacity: 1.0,
            offset: Vec2::ZERO,
            properties: Properties::default(),
            objects,
            bucket_layer: 0,
            seen_stamp_tiles: vec![],
            seen_stamp_debug: vec![],
        };
        ensure_object_layer_stamp_invariant(&mut object_layer);

        let mut layer_kind_by_id = HashMap::new();
        layer_kind_by_id.insert(0, LayerKindInfo::Objects(0));

        Self {
            index,
            tilesets: vec![],
            object_layers: vec![object_layer],
            renderer: MapRenderer::default(),
            gid_lut: vec![],
            tile_layers: vec![],
            draw_order: vec![0],
            layer_kind_by_id,
        }
    }

    #[doc(hidden)]
    pub fn __set_frame_stamp_for_testing(&mut self, stamp: u32) {
        self.renderer.frame_stamp = stamp;
    }

    #[doc(hidden)]
    pub fn __frame_stamp_for_testing(&self) -> u32 {
        self.renderer.frame_stamp
    }

    #[doc(hidden)]
    pub fn __seen_tiles_stamp_count_for_testing(&self, layer_idx: usize, stamp: u32) -> usize {
        self.object_layers
            .get(layer_idx)
            .map(|l| l.seen_stamp_tiles.iter().filter(|&&v| v == stamp).count())
            .unwrap_or(0)
    }

    pub(crate) async fn from_ir(ir: IrMap, base_dir: &Path) -> Result<Self, MapError> {
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
                    let img_path = base_dir.join(image);
                    let img_path_str = img_path
                        .to_str()
                        .ok_or_else(|| MapError::InvalidUtf8Path(img_path.clone()))?;
                    let tex =
                        load_texture(img_path_str)
                            .await
                            .map_err(|e| MapError::TextureLoad {
                                path: img_path.clone(),
                                message: e.to_string(),
                            })?;
                    tex.set_filter(FilterMode::Nearest);

                    tilesets.push(TilesetInfo {
                        first_gid: *first_gid,
                        tilecount: *tilecount,
                        cols: *columns,
                        tex,
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
                        seen_stamp_tiles: vec![0; objects.len()],
                        seen_stamp_debug: vec![0; objects.len()],
                    });
                    if let Some(last) = object_layers.last_mut() {
                        ensure_object_layer_stamp_invariant(last);
                    }

                    for (object_idx, obj) in objects.iter().enumerate() {
                        let world = vec2(obj.x, obj.y) + layer.offset;
                        let (chunk_min, chunk_max) = Self::object_chunk_span(obj, layer.offset);

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
                                        // Objects may be inserted into multiple chunks. Store
                                        // position relative to each inserted chunk so world
                                        // reconstruction is stable regardless of which bucket
                                        // is visited first for deduped rendering.
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
                        index.add_tile(TileId(*gid), lid, world);
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
            renderer: MapRenderer::new(),
            gid_lut,
            tile_layers,
            draw_order,
            layer_kind_by_id,
        })
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

    fn object_chunk_span(
        obj: &IrObject,
        layer_offset: Vec2,
    ) -> (crate::spatial::ChunkCoord, crate::spatial::ChunkCoord) {
        let (min, max) = Self::object_aabb_world(obj, layer_offset);
        (world_to_chunk(min), world_to_chunk(max))
    }

    /// Advances and returns the frame stamp used for object deduplication.
    ///
    /// Advanced API: call this once per frame when using `*_with_stamp` methods manually.
    pub fn next_frame_stamp(&mut self) -> u32 {
        self.renderer.next_frame_stamp(&mut self.object_layers)
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

    #[inline]
    fn params_for_flips_gid(
        gid: TileId,
        tile_w: f32,
        tile_h: f32,
    ) -> (f32, bool, bool, Option<Vec2>) {
        let h = gid.flip_h();
        let v = gid.flip_v();
        let d = gid.flip_d();

        let flip_x = h ^ d;
        let flip_y = v;
        let pivot = Some(vec2(tile_w / 2.0, tile_h / 2.0));

        let rotation = match (h, v, d) {
            (false, _, _) => 0.0,
            (true, false, false) => std::f32::consts::FRAC_PI_2,
            (true, false, true) => std::f32::consts::FRAC_PI_2,
            (true, true, false) => std::f32::consts::FRAC_PI_2,
            (true, true, true) => std::f32::consts::PI,
        };

        (rotation, flip_x, flip_y, pivot)
    }

    #[inline]
    fn params_for_flips(
        &self,
        gid: TileId,
        tile_w: f32,
        tile_h: f32,
    ) -> (f32, bool, bool, Option<Vec2>) {
        Self::params_for_flips_gid(gid, tile_w, tile_h)
    }

    #[inline]
    fn ts_for_gid_from<'a>(
        gid: TileId,
        gid_lut: &'a [u16],
        tilesets: &'a [TilesetInfo],
    ) -> Option<(&'a TilesetInfo, u32)> {
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

    #[inline]
    fn ts_for_gid(&self, gid: TileId) -> Option<(&TilesetInfo, u32)> {
        Self::ts_for_gid_from(gid, &self.gid_lut, &self.tilesets)
    }

    /// Draws only tile layers inside the visible rectangle.
    ///
    /// Stable API for tile-only rendering. Object layers are not drawn here.
    ///
    /// `view_min`/`view_max` are world-space pixel corners.
    pub fn draw_visible_rect(&self, view_min: Vec2, view_max: Vec2) {
        let view = query_visible_rect(&self.index, view_min, view_max);
        self.draw_chunks(view);
    }

    /// Draws the full map in configured layer order.
    ///
    /// Stable API: draws visible tile layers and tile-objects.
    /// If debug drawing is enabled, object debug overlays are drawn too.
    ///
    /// `view_min`/`view_max` are world-space pixel corners.
    /// Culling uses [`Map::set_cull_padding`] in world-space pixels.
    pub fn draw(&mut self, view_min: Vec2, view_max: Vec2) {
        let coords = self.visible_coords_for_draw(view_min, view_max);
        let stamp = self.next_frame_stamp();
        for i in 0..self.draw_order.len() {
            let layer_id = self.draw_order[i];
            let Some(kind) = self.layer_kind_by_id.get(&layer_id).copied() else {
                continue;
            };
            match kind {
                LayerKindInfo::Tiles(tile_layer_idx) => {
                    self.draw_tile_layer_from_coords(&coords, tile_layer_idx);
                }
                LayerKindInfo::Objects(object_layer_idx) => {
                    self.draw_object_tiles_layer_from_coords(&coords, object_layer_idx, stamp);
                    if self.renderer.debug_draw {
                        self.draw_object_debug_layer_from_coords(&coords, object_layer_idx, stamp);
                    }
                }
                LayerKindInfo::Unsupported => {}
            }
        }
    }

    /// Enables/disables object debug overlay drawing used by [`Map::draw`].
    ///
    /// Stable API.
    pub fn set_debug_draw(&mut self, enabled: bool) {
        self.renderer.debug_draw = enabled;
    }

    /// Sets extra culling padding in world-space pixels around the view rectangle.
    ///
    /// Stable API. `0.0` means no extra padding.
    pub fn set_cull_padding(&mut self, padding: f32) {
        self.renderer.cull_padding = padding.max(0.0);
    }

    /// Draws debug shapes for visible object layers.
    ///
    /// Stable convenience API: acquires an internal frame stamp automatically.
    ///
    /// Use this when object drawing is a standalone call for the frame.
    pub fn draw_objects_debug(&mut self, view_min: Vec2, view_max: Vec2) {
        let stamp = self.next_frame_stamp();
        self.draw_objects_debug_with_stamp(view_min, view_max, stamp);
    }

    /// Advanced API: draws debug shapes for visible object layers using a caller-provided stamp.
    ///
    /// Use this when you want frame-coherent manual composition (for example:
    /// tile-object pass + debug pass in the same frame using one shared stamp).
    ///
    /// Stamp rule: pass the same `stamp` to all object passes in a frame.
    pub fn draw_objects_debug_with_stamp(&mut self, view_min: Vec2, view_max: Vec2, stamp: u32) {
        let coords = self.visible_coords_for_draw(view_min, view_max);
        self.draw_object_layers_debug_from_coords(&coords, stamp);
    }

    /// Draws tile-objects from visible object layers.
    ///
    /// Stable convenience API: acquires an internal frame stamp automatically.
    ///
    /// Use this when object drawing is a standalone call for the frame.
    pub fn draw_objects_tiles(&mut self, view_min: Vec2, view_max: Vec2) {
        let stamp = self.next_frame_stamp();
        self.draw_objects_tiles_with_stamp(view_min, view_max, stamp);
    }

    /// Advanced API: draws tile-objects using a caller-provided stamp.
    ///
    /// This exists to support explicit control of object deduplication across
    /// multiple manual object passes in one frame.
    ///
    /// Stamp rule: pass the same `stamp` to all object passes in a frame.
    pub fn draw_objects_tiles_with_stamp(&mut self, view_min: Vec2, view_max: Vec2, stamp: u32) {
        let coords = self.visible_coords_for_draw(view_min, view_max);
        self.draw_object_layers_tiles_from_coords(&coords, stamp);
    }

    fn draw_chunks(&self, view: LocalView) {
        for tile_layer_idx in 0..self.tile_layers.len() {
            self.draw_tile_layer_from_view(&view, tile_layer_idx);
        }
    }

    fn draw_tile_layer_from_view(&self, view: &LocalView, tile_layer_idx: usize) {
        let Some(layer) = self.tile_layers.get(tile_layer_idx) else {
            return;
        };
        if !layer.visible {
            return;
        }
        let tint = Color::new(1.0, 1.0, 1.0, layer.opacity);

        for LocalChunkView { coord: cc, layers } in &view.chunks {
            if let Some(bucket) = layers.get(&layer.layer_id) {
                for rec in &bucket.tiles {
                    let (ts, local) = match self.ts_for_gid(rec.id) {
                        Some(x) => x,
                        None => continue,
                    };

                    let col = local % ts.cols;
                    let row = local / ts.cols;
                    let sx = ts.margin + col * (ts.tile_w + ts.spacing);
                    let sy = ts.margin + row * (ts.tile_h + ts.spacing);

                    let x = ((cc.x * CHUNK_SIZE) as f32 + rec.rel_pos.x).round();
                    let y = ((cc.y * CHUNK_SIZE) as f32 + rec.rel_pos.y).round();

                    let (rotation, flip_x, flip_y, pivot) =
                        self.params_for_flips(rec.id, ts.tile_w as f32, ts.tile_h as f32);

                    draw_texture_ex(
                        &ts.tex,
                        x,
                        y,
                        tint,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                sx as f32,
                                sy as f32,
                                ts.tile_w as f32,
                                ts.tile_h as f32,
                            )),
                            rotation,
                            flip_x,
                            flip_y,
                            pivot,
                            ..Default::default()
                        },
                    );
                }
            }
        }
    }

    fn draw_tile_layer_from_coords(
        &self,
        coords: &[crate::spatial::ChunkCoord],
        tile_layer_idx: usize,
    ) {
        let Some(layer) = self.tile_layers.get(tile_layer_idx) else {
            return;
        };
        if !layer.visible {
            return;
        }
        let tint = Color::new(1.0, 1.0, 1.0, layer.opacity);

        Self::for_each_visible_layer_bucket(&self.index, coords, layer.layer_id, |cc, bucket| {
            for rec in &bucket.tiles {
                let (ts, local) = match self.ts_for_gid(rec.id) {
                    Some(x) => x,
                    None => continue,
                };

                let col = local % ts.cols;
                let row = local / ts.cols;
                let sx = ts.margin + col * (ts.tile_w + ts.spacing);
                let sy = ts.margin + row * (ts.tile_h + ts.spacing);

                let x = ((cc.x * CHUNK_SIZE) as f32 + rec.rel_pos.x).round();
                let y = ((cc.y * CHUNK_SIZE) as f32 + rec.rel_pos.y).round();

                let (rotation, flip_x, flip_y, pivot) =
                    self.params_for_flips(rec.id, ts.tile_w as f32, ts.tile_h as f32);

                draw_texture_ex(
                    &ts.tex,
                    x,
                    y,
                    tint,
                    DrawTextureParams {
                        source: Some(Rect::new(
                            sx as f32,
                            sy as f32,
                            ts.tile_w as f32,
                            ts.tile_h as f32,
                        )),
                        rotation,
                        flip_x,
                        flip_y,
                        pivot,
                        ..Default::default()
                    },
                );
            }
        });
    }

    fn draw_object_layers_debug_from_coords(
        &mut self,
        coords: &[crate::spatial::ChunkCoord],
        stamp: u32,
    ) {
        for layer_idx in 0..self.object_layers.len() {
            self.draw_object_debug_layer_from_coords(coords, layer_idx, stamp);
        }
    }

    fn draw_object_layers_tiles_from_coords(
        &mut self,
        coords: &[crate::spatial::ChunkCoord],
        stamp: u32,
    ) {
        for layer_idx in 0..self.object_layers.len() {
            self.draw_object_tiles_layer_from_coords(coords, layer_idx, stamp);
        }
    }

    fn draw_object_debug_layer_from_coords(
        &mut self,
        coords: &[crate::spatial::ChunkCoord],
        layer_idx: usize,
        stamp: u32,
    ) {
        let Some(layer) = self.object_layers.get_mut(layer_idx) else {
            return;
        };
        ensure_object_layer_stamp_invariant(layer);
        if !layer.visible {
            return;
        }
        let alpha = layer.opacity.clamp(0.0, 1.0);
        let rect_color = Color::new(YELLOW.r, YELLOW.g, YELLOW.b, alpha);
        let point_color = Color::new(GREEN.r, GREEN.g, GREEN.b, alpha);
        let polygon_color = Color::new(SKYBLUE.r, SKYBLUE.g, SKYBLUE.b, alpha);
        let polyline_color = Color::new(PINK.r, PINK.g, PINK.b, alpha);
        let tile_color = Color::new(MAGENTA.r, MAGENTA.g, MAGENTA.b, alpha);
        let bucket_layer = layer.bucket_layer;

        Self::for_each_visible_layer_bucket(
            &self.index,
            coords,
            bucket_layer,
            |cc, layer_bucket| {
                let records = &layer_bucket.objects;
                for rec in records {
                    let object_idx = rec.handle.0 as usize;
                    if object_idx >= layer.objects.len() {
                        debug_assert!(false, "ObjectHandle out of bounds for debug draw");
                        continue;
                    }
                    debug_assert!(object_idx < layer.seen_stamp_debug.len());
                    if layer.seen_stamp_debug[object_idx] == stamp {
                        continue;
                    }
                    layer.seen_stamp_debug[object_idx] = stamp;

                    let Some(obj) = layer.objects.get(object_idx) else {
                        continue;
                    };
                    if !obj.visible {
                        continue;
                    }

                    let origin = vec2(
                        (cc.x * CHUNK_SIZE) as f32 + rec.rel_pos.x,
                        (cc.y * CHUNK_SIZE) as f32 + rec.rel_pos.y,
                    );

                    match &obj.shape {
                        IrObjectShape::Rectangle => {
                            draw_rectangle_lines(
                                origin.x,
                                origin.y,
                                obj.width.max(2.0),
                                obj.height.max(2.0),
                                2.0,
                                rect_color,
                            );
                        }
                        IrObjectShape::Point => {
                            draw_circle(origin.x, origin.y, 5.0, point_color);
                        }
                        IrObjectShape::Polygon(points) => {
                            if points.len() < 2 {
                                continue;
                            }
                            for i in 0..points.len() {
                                let a = origin + points[i];
                                let b = origin + points[(i + 1) % points.len()];
                                draw_line(a.x, a.y, b.x, b.y, 2.0, polygon_color);
                            }
                        }
                        IrObjectShape::Polyline(points) => {
                            for seg in points.windows(2) {
                                let a = origin + seg[0];
                                let b = origin + seg[1];
                                draw_line(a.x, a.y, b.x, b.y, 2.0, polyline_color);
                            }
                        }
                        IrObjectShape::Tile { .. } => {
                            draw_rectangle_lines(
                                origin.x,
                                origin.y - obj.height,
                                obj.width.max(16.0),
                                obj.height.max(16.0),
                                2.0,
                                tile_color,
                            );
                        }
                    }
                }
            },
        );
    }

    fn draw_object_tiles_layer_from_coords(
        &mut self,
        coords: &[crate::spatial::ChunkCoord],
        layer_idx: usize,
        stamp: u32,
    ) {
        let gid_lut = &self.gid_lut;
        let tilesets = &self.tilesets;
        let Some(layer) = self.object_layers.get_mut(layer_idx) else {
            return;
        };
        ensure_object_layer_stamp_invariant(layer);
        if !layer.visible {
            return;
        }
        let tint = Color::new(1.0, 1.0, 1.0, layer.opacity.clamp(0.0, 1.0));
        let bucket_layer = layer.bucket_layer;

        Self::for_each_visible_layer_bucket(
            &self.index,
            coords,
            bucket_layer,
            |cc, layer_bucket| {
                let records = &layer_bucket.objects;
                for rec in records {
                    let object_idx = rec.handle.0 as usize;
                    if object_idx >= layer.objects.len() {
                        debug_assert!(false, "ObjectHandle out of bounds for tile draw");
                        continue;
                    }
                    debug_assert!(object_idx < layer.seen_stamp_tiles.len());
                    if layer.seen_stamp_tiles[object_idx] == stamp {
                        continue;
                    }
                    layer.seen_stamp_tiles[object_idx] = stamp;

                    let Some(obj) = layer.objects.get(object_idx) else {
                        continue;
                    };
                    if !obj.visible {
                        continue;
                    }

                    let IrObjectShape::Tile { gid } = obj.shape else {
                        continue;
                    };

                    let origin = vec2(
                        (cc.x * CHUNK_SIZE) as f32 + rec.rel_pos.x,
                        (cc.y * CHUNK_SIZE) as f32 + rec.rel_pos.y,
                    );

                    let gid = TileId(gid);
                    let Some((ts, local)) = Self::ts_for_gid_from(gid, gid_lut, tilesets) else {
                        continue;
                    };

                    let col = local % ts.cols;
                    let row = local / ts.cols;
                    let sx = ts.margin + col * (ts.tile_w + ts.spacing);
                    let sy = ts.margin + row * (ts.tile_h + ts.spacing);

                    let w = if obj.width > 0.0 {
                        obj.width
                    } else {
                        ts.tile_w as f32
                    };
                    let h = if obj.height > 0.0 {
                        obj.height
                    } else {
                        ts.tile_h as f32
                    };

                    let (flag_rotation, flip_x, flip_y, _) = Self::params_for_flips_gid(gid, w, h);
                    let rotation = obj.rotation.to_radians() + flag_rotation;
                    draw_texture_ex(
                        &ts.tex,
                        origin.x,
                        origin.y - h,
                        tint,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                sx as f32,
                                sy as f32,
                                ts.tile_w as f32,
                                ts.tile_h as f32,
                            )),
                            dest_size: Some(vec2(w, h)),
                            rotation,
                            flip_x,
                            flip_y,
                            // Macroquad expects pivot in screen-space coordinates.
                            // Keep Tiled-style bottom-left anchoring at the object's (x, y).
                            pivot: Some(origin),
                        },
                    );
                }
            },
        );
    }

    fn for_each_visible_layer_bucket<F>(
        index: &GlobalIndex,
        coords: &[crate::spatial::ChunkCoord],
        bucket_layer: LayerIdx,
        mut f: F,
    ) where
        F: FnMut(crate::spatial::ChunkCoord, &crate::spatial::LayerBucket),
    {
        for cc in coords {
            let Some(chunk) = index.buckets.get(cc) else {
                continue;
            };
            let Some(bucket) = chunk.layers.get(&bucket_layer) else {
                continue;
            };
            f(*cc, bucket);
        }
    }

    fn visible_coords_for_draw(
        &self,
        view_min: Vec2,
        view_max: Vec2,
    ) -> Vec<crate::spatial::ChunkCoord> {
        let pad = self.renderer.cull_padding;
        visible_chunk_coords_rect(
            vec2(view_min.x - pad, view_min.y - pad),
            vec2(view_max.x + pad, view_max.y + pad),
        )
    }
}

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

        let mut layer = ObjectLayer {
            id: 0,
            name: "objects".to_string(),
            visible: true,
            opacity: 1.0,
            offset: Vec2::ZERO,
            properties: Properties::default(),
            objects: vec![make_object(1), make_object(2), make_object(3)],
            bucket_layer: 0,
            seen_stamp_tiles: vec![0],
            seen_stamp_debug: vec![],
        };

        ensure_object_layer_stamp_invariant(&mut layer);
        assert_eq!(layer.seen_stamp_tiles.len(), layer.objects.len());
        assert_eq!(layer.seen_stamp_debug.len(), layer.objects.len());

        let stamp = 42;
        let mut first_pass_drawn = 0usize;
        for object_idx in 0..layer.objects.len() {
            if layer.seen_stamp_tiles[object_idx] == stamp {
                continue;
            }
            layer.seen_stamp_tiles[object_idx] = stamp;
            first_pass_drawn += 1;
        }

        let mut second_pass_drawn = 0usize;
        for object_idx in 0..layer.objects.len() {
            if layer.seen_stamp_tiles[object_idx] == stamp {
                continue;
            }
            layer.seen_stamp_tiles[object_idx] = stamp;
            second_pass_drawn += 1;
        }

        assert_eq!(first_pass_drawn, 3);
        assert_eq!(second_pass_drawn, 0);
    }

    fn collect_draw_sequence_for_test(
        map: &mut Map,
        view_min: Vec2,
        view_max: Vec2,
    ) -> Vec<(char, LayerId, i32, i32, u32)> {
        let coords = map.visible_coords_for_draw(view_min, view_max);
        let stamp = map.next_frame_stamp();
        let mut out = Vec::new();

        for i in 0..map.draw_order.len() {
            let layer_id = map.draw_order[i];
            let Some(kind) = map.layer_kind_by_id.get(&layer_id).copied() else {
                continue;
            };

            match kind {
                LayerKindInfo::Tiles(tile_layer_idx) => {
                    let Some(layer) = map.tile_layers.get(tile_layer_idx) else {
                        continue;
                    };
                    if !layer.visible {
                        continue;
                    }

                    Map::for_each_visible_layer_bucket(
                        &map.index,
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
                    let Some(layer) = map.object_layers.get_mut(object_layer_idx) else {
                        continue;
                    };
                    ensure_object_layer_stamp_invariant(layer);
                    if !layer.visible {
                        continue;
                    }
                    let bucket_layer = layer.bucket_layer;

                    Map::for_each_visible_layer_bucket(
                        &map.index,
                        &coords,
                        bucket_layer,
                        |cc, bucket| {
                            for rec in &bucket.objects {
                                let object_idx = rec.handle.0 as usize;
                                if object_idx >= layer.objects.len() {
                                    continue;
                                }
                                if layer.seen_stamp_tiles[object_idx] == stamp {
                                    continue;
                                }
                                layer.seen_stamp_tiles[object_idx] = stamp;

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
                seen_stamp_tiles: vec![0],
                seen_stamp_debug: vec![0],
            }],
            renderer: MapRenderer {
                debug_draw: false,
                cull_padding: CHUNK_SIZE as f32,
                frame_stamp: 0,
            },
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
        };

        let seq1 = collect_draw_sequence_for_test(&mut map, Vec2::ZERO, vec2(520.0, 520.0));
        let seq2 = collect_draw_sequence_for_test(&mut map, Vec2::ZERO, vec2(520.0, 520.0));
        assert_eq!(seq1, seq2);
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
        let wrong_origin = vec2((cc_other.x * CHUNK_SIZE) as f32, (cc_other.y * CHUNK_SIZE) as f32)
            + rel_home;
        assert_ne!(wrong_origin.y, world.y);

        let correct_rel =
            world - vec2((cc_other.x * CHUNK_SIZE) as f32, (cc_other.y * CHUNK_SIZE) as f32);
        let rebuilt =
            vec2((cc_other.x * CHUNK_SIZE) as f32, (cc_other.y * CHUNK_SIZE) as f32) + correct_rel;
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

        let layer = &map.object_layers[0];
        assert_eq!(layer.seen_stamp_tiles.len(), layer.objects.len());
        assert_eq!(layer.seen_stamp_debug.len(), layer.objects.len());
    }
}
