use crate::core::LayerKindInfo;
use crate::ir_map::IrObjectShape;
use crate::map::Map;
use crate::render::MacroquadTilesetAsset;
use crate::spatial::{GlobalIndex, LayerIdx, TileId, CHUNK_SIZE};
use macroquad::prelude::*;

impl Map {
    #[inline]
    pub(crate) fn params_for_flips_gid(
        gid: TileId,
        tile_w: f32,
        tile_h: f32,
    ) -> (f32, bool, bool, Option<Vec2>) {
        let h = gid.flip_h();
        let v = gid.flip_v();
        let d = gid.flip_d();

        // Tiled flag semantics for orthogonal maps:
        // apply diagonal swap first, then horizontal flip, then vertical flip.
        // We map that transform to macroquad's (flip_x, flip_y, rotation),
        // where macroquad applies flips before rotation.
        let (rotation, flip_x, flip_y) = match (h, v, d) {
            (false, false, false) => (0.0, false, false),
            (true, false, false) => (0.0, true, false),
            (false, true, false) => (0.0, false, true),
            (true, true, false) => (std::f32::consts::PI, false, false),
            (false, false, true) => (std::f32::consts::FRAC_PI_2, false, true),
            (true, false, true) => (std::f32::consts::FRAC_PI_2, false, false),
            (false, true, true) => (3.0 * std::f32::consts::FRAC_PI_2, false, false),
            (true, true, true) => (std::f32::consts::FRAC_PI_2, true, false),
        };
        let pivot = Some(vec2(tile_w / 2.0, tile_h / 2.0));

        (rotation, flip_x, flip_y, pivot)
    }

    #[inline]
    pub(crate) fn tileset_for_gid_from<'a>(
        gid: TileId,
        gid_lut: &'a [u16],
        tilesets: &'a [MacroquadTilesetAsset],
    ) -> Option<(&'a MacroquadTilesetAsset, u32)> {
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

    /// Draws only tile layers inside the visible rectangle.
    ///
    /// Stable API for tile-only rendering. Object layers are not drawn here.
    ///
    /// `view_min`/`view_max` are world-space pixel corners.
    /// Culling uses the same [`Map::set_cull_padding`] policy as [`Map::draw`].
    pub fn draw_visible_rect(&mut self, view_min: Vec2, view_max: Vec2) {
        let stamp = self.next_frame_stamp();
        self.draw_visible_rect_with_stamp(view_min, view_max, stamp);
    }

    /// Advanced API: draws tile layers using a caller-provided stamp.
    ///
    /// Use this for manual frame composition alongside object `_with_stamp` passes.
    pub fn draw_visible_rect_with_stamp(&mut self, view_min: Vec2, view_max: Vec2, stamp: u32) {
        self.render_state.sync_with_data(&self.data);
        let coords = self.visible_coords_for_draw(view_min, view_max);
        for tile_layer_idx in 0..self.data.tile_state.tile_layer_draw_info.len() {
            self.draw_tile_layer_from_coords(&coords, tile_layer_idx, stamp);
        }
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
        for i in 0..self.data.layer_plan.draw_order.len() {
            let layer_id = self.data.layer_plan.draw_order[i];
            let Some(kind) = self
                .data
                .layer_plan
                .layer_kind_by_id
                .get(&layer_id)
                .copied()
            else {
                continue;
            };
            match kind {
                LayerKindInfo::Tiles(tile_layer_idx) => {
                    self.draw_tile_layer_from_coords(&coords, tile_layer_idx, stamp);
                }
                LayerKindInfo::Objects(object_layer_idx) => {
                    self.draw_object_tiles_layer_from_coords(&coords, object_layer_idx, stamp);
                    if self.render_state.debug_draw {
                        self.draw_object_debug_layer_from_coords(&coords, object_layer_idx, stamp);
                    }
                }
                LayerKindInfo::Unsupported => {}
            }
        }
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
        self.render_state.sync_with_data(&self.data);
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
        self.render_state.sync_with_data(&self.data);
        let coords = self.visible_coords_for_draw(view_min, view_max);
        self.draw_object_layers_tiles_from_coords(&coords, stamp);
    }

    pub(crate) fn draw_tile_layer_from_coords(
        &mut self,
        coords: &[crate::spatial::ChunkCoord],
        tile_layer_idx: usize,
        stamp: u32,
    ) {
        let Some(layer) = self
            .data
            .tile_state
            .tile_layer_draw_info
            .get(tile_layer_idx)
            .copied()
        else {
            return;
        };
        if !layer.visible {
            return;
        }
        let tint = Color::new(1.0, 1.0, 1.0, layer.opacity);
        let gid_lut = &self.data.tile_state.gid_lut;
        let tilesets = &self.assets.tilesets;
        let seen = &mut self.render_state.seen_tiles;

        Self::for_each_visible_layer_bucket(
            &self.data.derived_index,
            coords,
            layer.layer_id,
            |cc, bucket| {
                for rec in &bucket.tiles {
                    let handle_idx = rec.handle.0 as usize;
                    if handle_idx >= seen.len() {
                        debug_assert!(false, "TileHandle out of bounds for tile draw");
                        continue;
                    }
                    if seen[handle_idx] == stamp {
                        continue;
                    }
                    seen[handle_idx] = stamp;

                    let (ts, local) = match Self::tileset_for_gid_from(rec.id, gid_lut, tilesets) {
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
                        Self::params_for_flips_gid(rec.id, ts.tile_w as f32, ts.tile_h as f32);

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
            },
        );
    }

    fn draw_object_layers_debug_from_coords(
        &mut self,
        coords: &[crate::spatial::ChunkCoord],
        stamp: u32,
    ) {
        for layer_idx in 0..self.data.object_state.object_layers.len() {
            self.draw_object_debug_layer_from_coords(coords, layer_idx, stamp);
        }
    }

    fn draw_object_layers_tiles_from_coords(
        &mut self,
        coords: &[crate::spatial::ChunkCoord],
        stamp: u32,
    ) {
        for layer_idx in 0..self.data.object_state.object_layers.len() {
            self.draw_object_tiles_layer_from_coords(coords, layer_idx, stamp);
        }
    }

    fn draw_object_debug_layer_from_coords(
        &mut self,
        coords: &[crate::spatial::ChunkCoord],
        layer_idx: usize,
        stamp: u32,
    ) {
        self.render_state.sync_with_data(&self.data);
        let Some(layer) = self.data.object_state.object_layers.get(layer_idx) else {
            return;
        };
        let Some(seen_debug) = self.render_state.seen_objects_debug.get_mut(layer_idx) else {
            return;
        };
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
            &self.data.derived_index,
            coords,
            bucket_layer,
            |cc, layer_bucket| {
                let records = &layer_bucket.objects;
                for rec in records {
                    let Some((handle_layer_idx, object_idx)) =
                        self.data.object_location(rec.handle)
                    else {
                        debug_assert!(false, "ObjectHandle out of bounds for debug draw");
                        continue;
                    };
                    if handle_layer_idx != layer_idx || object_idx >= layer.objects.len() {
                        debug_assert!(false, "ObjectHandle layer mismatch for debug draw");
                        continue;
                    }
                    debug_assert!(object_idx < seen_debug.len());
                    if seen_debug[object_idx] == stamp {
                        continue;
                    }
                    seen_debug[object_idx] = stamp;

                    let Some(obj) = layer.objects.get(object_idx) else {
                        continue;
                    };
                    let Some(runtime) = self
                        .data
                        .object_state
                        .object_runtime_by_layer
                        .get(layer_idx)
                        .and_then(|v| v.get(object_idx))
                        .and_then(|r| r.as_ref())
                    else {
                        continue;
                    };
                    if !runtime.alive || !runtime.visible {
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
                                runtime.width.max(2.0),
                                runtime.height.max(2.0),
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
                                origin.y - runtime.height,
                                runtime.width.max(16.0),
                                runtime.height.max(16.0),
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
        self.render_state.sync_with_data(&self.data);
        let gid_lut = &self.data.tile_state.gid_lut;
        let tilesets = &self.assets.tilesets;
        let Some(layer) = self.data.object_state.object_layers.get(layer_idx) else {
            return;
        };
        let Some(seen_tiles) = self.render_state.seen_objects_tiles.get_mut(layer_idx) else {
            return;
        };
        if !layer.visible {
            return;
        }
        let tint = Color::new(1.0, 1.0, 1.0, layer.opacity.clamp(0.0, 1.0));
        let bucket_layer = layer.bucket_layer;

        Self::for_each_visible_layer_bucket(
            &self.data.derived_index,
            coords,
            bucket_layer,
            |cc, layer_bucket| {
                let records = &layer_bucket.objects;
                for rec in records {
                    let Some((handle_layer_idx, object_idx)) =
                        self.data.object_location(rec.handle)
                    else {
                        debug_assert!(false, "ObjectHandle out of bounds for tile draw");
                        continue;
                    };
                    if handle_layer_idx != layer_idx || object_idx >= layer.objects.len() {
                        debug_assert!(false, "ObjectHandle layer mismatch for tile draw");
                        continue;
                    }
                    debug_assert!(object_idx < seen_tiles.len());
                    if seen_tiles[object_idx] == stamp {
                        continue;
                    }
                    seen_tiles[object_idx] = stamp;

                    let Some(obj) = layer.objects.get(object_idx) else {
                        continue;
                    };
                    let Some(runtime) = self
                        .data
                        .object_state
                        .object_runtime_by_layer
                        .get(layer_idx)
                        .and_then(|v| v.get(object_idx))
                        .and_then(|r| r.as_ref())
                    else {
                        continue;
                    };
                    if !runtime.alive || !runtime.visible {
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
                    let Some((ts, local)) = Self::tileset_for_gid_from(gid, gid_lut, tilesets)
                    else {
                        continue;
                    };

                    let col = local % ts.cols;
                    let row = local / ts.cols;
                    let sx = ts.margin + col * (ts.tile_w + ts.spacing);
                    let sy = ts.margin + row * (ts.tile_h + ts.spacing);

                    let w = if runtime.width > 0.0 {
                        runtime.width
                    } else {
                        ts.tile_w as f32
                    };
                    let h = if runtime.height > 0.0 {
                        runtime.height
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

    pub(crate) fn for_each_visible_layer_bucket<F>(
        derived_index: &GlobalIndex,
        coords: &[crate::spatial::ChunkCoord],
        bucket_layer: LayerIdx,
        mut f: F,
    ) where
        F: FnMut(crate::spatial::ChunkCoord, &crate::spatial::LayerBucket),
    {
        for cc in coords {
            let Some(chunk) = derived_index.buckets.get(cc) else {
                continue;
            };
            let Some(bucket) = chunk.layers.get(&bucket_layer) else {
                continue;
            };
            f(*cc, bucket);
        }
    }

    pub(crate) fn visible_coords_for_draw(
        &self,
        view_min: Vec2,
        view_max: Vec2,
    ) -> Vec<crate::spatial::ChunkCoord> {
        self.data
            .visible_coords_for_draw(view_min, view_max, self.render_state.cull_padding)
    }
}
