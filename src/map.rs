use crate::ir_map::*;
use crate::loader::json_loader::*;
use crate::render::*;
use crate::{spatial::CHUNK_SIZE, GlobalIndex, LayerIdx, TileId};
use anyhow::Context;
use macroquad::prelude::*;
use std::path::Path;
use std::thread::yield_now;

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

pub struct Map {
    pub index: GlobalIndex,
    pub tilesets: Vec<TilesetInfo>,
    gid_lut: Vec<u16>, //lookup table for tile GIDs to tileset indices
    layer_order: Vec<LayerIdx>,
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
                    image,
                    tile_w,
                    tile_h,
                    tilecount,
                    columns,
                    spacing,
                    margin,
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
        let mut layer_order: Vec<LayerIdx> = Vec::new();

        for (lz, layer) in ir.layers.iter().enumerate() {
            let lid = lz as LayerIdx;
            let mut inserted_any = false;

            let IrLayerKind::Tiles {
                width,
                height,
                data,
            } = &layer.kind;

            {
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

                    index.add_tile(TileId(*gid), lz as LayerIdx, world);
                    inserted_any = true;
                }
            }
            if inserted_any {
                layer_order.push(lid);
            }
        }

        Ok(Self {
            index,
            tilesets,
            gid_lut,
            layer_order,
            tile_w: ir.tile_w,
            tile_h: ir.tile_h,
        })
    }

    #[inline]
    fn params_for_flips(
        &self,
        gid: TileId,
        tile_w: f32,
        tile_h: f32,
    ) -> (f32, bool, bool, Option<Vec2>) {
        let h = gid.flip_h(); // horizontal flip
        let v = gid.flip_v(); // vertical flip
        let d = gid.flip_d(); // diagonal flip

        let flip_x = h ^ d; // flip horizontally if not diagonal
        let flip_y = v;
        let pivot = Some(vec2(tile_w / 2.0, tile_h / 2.0));

        let rotation = match (h, v, d) {
            (false, _, _) => 0.0, // no flip

            (true, false, false) => std::f32::consts::FRAC_PI_2, // + 90 degrees (with flip)
            (true, false, true) => std::f32::consts::FRAC_PI_2,  // - 90 defrees
            (true, true, false) => std::f32::consts::FRAC_PI_2,  // + 90 degrees
            (true, true, true) => std::f32::consts::PI,          // 180 degrees
        };

        (rotation, flip_x, flip_y, pivot)
    }

    #[inline]
    fn ts_for_gid(&self, gid: TileId) -> Option<(&TilesetInfo, u32)> {
        // Clean the tile ID by removing flip/rotation flags, keep only the actual ID number
        let clean = gid.clean() as usize;

        // Check if the cleaned ID is within the bounds of our lookup table
        if clean >= self.gid_lut.len() {
            return None;
        }

        // Get the tileset index from the lookup table
        let idx = self.gid_lut[clean];

        // If the index is u16::MAX, this means the tile ID doesn't map to any tileset
        if idx == u16::MAX {
            return None;
        }

        // Get the tileset info from the tilesets array
        let ts = &self.tilesets[idx as usize];

        // Return the tileset info and the local ID within that tileset
        // The local ID is calculated by subtracting the tileset's first GID from the cleaned tile ID
        Some((ts, gid.clean() - ts.first_gid))
    }

    pub fn draw_visible_rect(&self, view_min: Vec2, view_max: Vec2) {
        let view = query_visible_rect(&self.index, view_min, view_max);
        self.draw_chunks(view);
    }

    fn draw_chunks(&self, view: LocalView) {
        for &layer_id in &self.layer_order {
            for LocalChunkView { coord: cc, layers } in &view.chunks {
                if let Some(vec) = layers.get(&layer_id) {
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
                            WHITE,
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
    }
}
