use crate::ir_map::*;
use crate::loader::json_loader::*;
use crate::render::*;
use crate::{
    spatial::{rel, world_to_chunk, ChunkCoord, CHUNK_SIZE},
    GlobalIndex, LayerIdx, TileId,
};
use anyhow::Context;
use macroquad::prelude::*;
use std::collections::HashMap;
use std::path::Path;

pub struct TilesetInfo {
    pub first_gid: u32,
    pub tilecount: u32,
    pub cols: u32,
    pub tex: Texture2D,
    pub tile_w: u32,
    pub tile_h: u32,
    pub spacing: u32,
    pub margin: u32,
}

pub struct ObjectLayer {
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    pub offset: Vec2,
    pub properties: Properties,
    pub objects: Vec<IrObject>,
}

#[derive(Clone, Copy)]
struct ObjectRec {
    object_idx: usize,
    rel_pos: Vec2,
}

#[derive(Clone, Copy)]
struct TileLayerDrawInfo {
    layer_id: LayerIdx,
    visible: bool,
    opacity: f32,
}

#[derive(Clone, Copy)]
enum DrawLayer {
    Tiles(usize),
    Objects(usize),
}

pub struct Map {
    pub index: GlobalIndex,
    pub tilesets: Vec<TilesetInfo>,
    object_layers: Vec<ObjectLayer>,
    object_buckets: HashMap<ChunkCoord, HashMap<usize, Vec<ObjectRec>>>,
    debug_draw: bool,
    gid_lut: Vec<u16>, //lookup table for tile GIDs to tileset indices
    tile_layers: Vec<TileLayerDrawInfo>,
    draw_order: Vec<DrawLayer>,
    pub tile_w: u32,
    pub tile_h: u32,
}

impl Map {
    pub async fn load(path: &str) -> anyhow::Result<Self> {
        let (ir, base) = decode_map_file_to_ir(path)?;
        Self::from_ir(ir, &base).await
    }

    pub async fn from_ir(ir: IrMap, base_dir: &Path) -> anyhow::Result<Self> {
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
                    let tex = load_texture(img_path.to_str().unwrap())
                        .await
                        .with_context(|| format!("Loading texture {}", image))?;
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
        let mut object_buckets: HashMap<ChunkCoord, HashMap<usize, Vec<ObjectRec>>> =
            HashMap::new();
        let mut tile_layers: Vec<TileLayerDrawInfo> = Vec::new();
        let mut draw_order: Vec<DrawLayer> = Vec::new();

        for (lz, layer) in ir.layers.iter().enumerate() {
            if let IrLayerKind::Objects { objects } = &layer.kind {
                let layer_idx = object_layers.len();
                object_layers.push(ObjectLayer {
                    name: layer.name.clone(),
                    visible: layer.visible,
                    opacity: layer.opacity,
                    offset: layer.offset,
                    properties: layer.properties.clone(),
                    objects: objects.clone(),
                });

                for (object_idx, obj) in objects.iter().enumerate() {
                    let world = vec2(obj.x, obj.y) + layer.offset;
                    let (min, max) = Self::object_aabb_world(obj, layer.offset);
                    let chunk_min = world_to_chunk(min);
                    let chunk_max = world_to_chunk(max);

                    for cy in chunk_min.y..=chunk_max.y {
                        for cx in chunk_min.x..=chunk_max.x {
                            let cc = ChunkCoord { x: cx, y: cy };
                            let by_layer = object_buckets.entry(cc).or_default();
                            by_layer.entry(layer_idx).or_default().push(ObjectRec {
                                object_idx,
                                rel_pos: rel(world),
                            });
                        }
                    }
                }
                draw_order.push(DrawLayer::Objects(layer_idx));
                continue;
            }

            let lid = lz as LayerIdx;
            let mut inserted_any = false;

            let (width, data) = match &layer.kind {
                IrLayerKind::Tiles {
                    width,
                    height: _,
                    data,
                } => (width, data),
                _ => continue,
            };

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
                inserted_any = true;
            }

            if inserted_any {
                tile_layers.push(TileLayerDrawInfo {
                    layer_id: lid,
                    visible: layer.visible,
                    opacity: layer.opacity.clamp(0.0, 1.0),
                });
                draw_order.push(DrawLayer::Tiles(tile_layers.len() - 1));
            }
        }

        Ok(Self {
            index,
            tilesets,
            object_layers,
            object_buckets,
            debug_draw: false,
            gid_lut,
            tile_layers,
            draw_order,
            tile_w: ir.tile_w,
            tile_h: ir.tile_h,
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

    pub fn object_layers(&self) -> &[ObjectLayer] {
        &self.object_layers
    }

    pub fn objects(&self) -> impl Iterator<Item = &IrObject> {
        self.object_layers
            .iter()
            .flat_map(|layer| layer.objects.iter())
    }

    #[inline]
    fn params_for_flips(
        &self,
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
    fn ts_for_gid(&self, gid: TileId) -> Option<(&TilesetInfo, u32)> {
        let clean = gid.clean() as usize;
        if clean >= self.gid_lut.len() {
            return None;
        }

        let idx = self.gid_lut[clean];
        if idx == u16::MAX {
            return None;
        }

        let ts = &self.tilesets[idx as usize];
        Some((ts, gid.clean() - ts.first_gid))
    }

    pub fn draw_visible_rect(&self, view_min: Vec2, view_max: Vec2) {
        let view = query_visible_rect(&self.index, view_min, view_max);
        self.draw_chunks(view);
    }

    pub fn draw(&self, view_min: Vec2, view_max: Vec2) {
        let view = query_visible_rect(&self.index, view_min, view_max);
        for entry in &self.draw_order {
            match entry {
                DrawLayer::Tiles(tile_layer_idx) => {
                    self.draw_tile_layer_from_view(&view, *tile_layer_idx);
                }
                DrawLayer::Objects(layer_idx) => {
                    self.draw_object_tiles_layer_from_view(&view, *layer_idx);
                    if self.debug_draw {
                        self.draw_object_debug_layer_from_view(&view, *layer_idx);
                    }
                }
            }
        }
    }

    pub fn set_debug_draw(&mut self, enabled: bool) {
        self.debug_draw = enabled;
    }

    pub fn draw_objects_debug(&self, view_min: Vec2, view_max: Vec2) {
        let view = query_visible_rect(&self.index, view_min, view_max);
        self.draw_chunk_objects_debug(view);
    }

    pub fn draw_objects_tiles(&self, view_min: Vec2, view_max: Vec2) {
        let view = query_visible_rect(&self.index, view_min, view_max);
        self.draw_chunk_objects_tiles(view);
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
            if let Some(vec) = layers.get(&layer.layer_id) {
                for rec in vec {
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

    fn draw_chunk_objects_debug(&self, view: LocalView) {
        for layer_idx in 0..self.object_layers.len() {
            self.draw_object_debug_layer_from_view(&view, layer_idx);
        }
    }

    fn draw_chunk_objects_tiles(&self, view: LocalView) {
        for layer_idx in 0..self.object_layers.len() {
            self.draw_object_tiles_layer_from_view(&view, layer_idx);
        }
    }

    fn draw_object_debug_layer_from_view(&self, view: &LocalView, layer_idx: usize) {
        let Some(layer) = self.object_layers.get(layer_idx) else {
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

        let mut drawn = vec![false; layer.objects.len()];

        for LocalChunkView { coord: cc, .. } in &view.chunks {
            let Some(by_layer) = self.object_buckets.get(cc) else {
                continue;
            };
            let Some(records) = by_layer.get(&layer_idx) else {
                continue;
            };

            for rec in records {
                if drawn[rec.object_idx] {
                    continue;
                }
                drawn[rec.object_idx] = true;

                let Some(obj) = layer.objects.get(rec.object_idx) else {
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
        }
    }

    fn draw_object_tiles_layer_from_view(&self, view: &LocalView, layer_idx: usize) {
        let Some(layer) = self.object_layers.get(layer_idx) else {
            return;
        };
        if !layer.visible {
            return;
        }
        let tint = Color::new(1.0, 1.0, 1.0, layer.opacity.clamp(0.0, 1.0));

        let mut drawn = vec![false; layer.objects.len()];

        for LocalChunkView { coord: cc, .. } in &view.chunks {
            let Some(by_layer) = self.object_buckets.get(cc) else {
                continue;
            };
            let Some(records) = by_layer.get(&layer_idx) else {
                continue;
            };

            for rec in records {
                if drawn[rec.object_idx] {
                    continue;
                }
                drawn[rec.object_idx] = true;

                let Some(obj) = layer.objects.get(rec.object_idx) else {
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
                let Some((ts, local)) = self.ts_for_gid(gid) else {
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

                let (flag_rotation, flip_x, flip_y, _) = self.params_for_flips(gid, w, h);
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
                        pivot: Some(vec2(0.0, h)),
                        ..Default::default()
                    },
                );
            }
        }
    }
}
