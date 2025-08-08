use crate::{ir_map::{IrLayerKind, IrTileset}, render::*};
use anyhow::Context;
use macroquad::prelude::*;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use crate::ir_map::*;
use crate::loader::json_loader::*;
use crate::{spatial::CHUNK_SIZE, GlobalIndex, LayerIdx, TileId};

#[derive(Deserialize)]
struct JsonLayer {
    data: Vec<u32>,
    width: usize,
    height: usize,
}

#[derive(Deserialize)]
struct JsonMap {
    tilewidth: u32,
    tileheight: u32,
    layers: Vec<JsonLayer>,
    tilesets: Vec<JsonTilesetRef>,
}

#[derive(Deserialize)]
struct JsonTilesetRef {
    firstgid: u32,
    source: String,
}

#[derive(Deserialize)]
struct ExternalTileset {
    tilewidth: u32,
    tileheight: u32,
    tilecount: u32,
    columns: u32,
    image: String, // tileset.png
    spacing: u32,
    margin: u32,
}

fn parse_map_file(path: &str) -> anyhow::Result<(JsonMap, PathBuf)> {
    let p = Path::new(path);

    if p.extension().and_then(|e| e.to_str()) != Some("json") {
        anyhow::bail!("Map file must be a JSON file: {}", path);
    }

    let txt = std::fs::read_to_string(p).with_context(|| format!("Reading map file {}", path))?;

    let j: JsonMap =
        serde_json::from_str(&txt).with_context(|| format!("Parsing map file {}", path))?;

    let map_dir = p
        .parent()
        .map(|d| d.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("./"));

    Ok((j, map_dir))
}

async fn load_tileset_data(
    j: &JsonMap,
    map_dir: &Path,
) -> anyhow::Result<(Vec<TilesetInfo>, Vec<u16>)> {
    let mut tilesets = Vec::with_capacity(j.tilesets.len());
    for ts in &j.tilesets {
        if !ts.source.ends_with(".json") {
            anyhow::bail!("Tileset {} source must be a JSON file", ts.source);
        }

        let ext_txt = std::fs::read_to_string(map_dir.join(&ts.source))?;
        let ext: ExternalTileset = serde_json::from_str(&ext_txt)?;

        let img_rel = &ext.image;
        let img_path = map_dir.join(img_rel);

        let tex: Texture2D = load_texture(img_path.to_str().unwrap())
            .await
            .with_context(|| format!("Loading texture {}", img_rel))?;
        tex.set_filter(FilterMode::Nearest);

        tilesets.push(TilesetInfo {
            first_gid: ts.firstgid,
            tilecount: ext.tilecount,
            cols: ext.columns,
            tex,
            tile_w: ext.tilewidth,
            tile_h: ext.tileheight,
            spacing: ext.spacing,
            margin: ext.margin,
        });
    }

    tilesets.sort_unstable_by_key(|t| t.first_gid);

    let max_gid = tilesets
        .iter()
        .map(|t| t.first_gid + t.tilecount - 1)
        .max()
        .unwrap_or(0);

    let mut gid_lut = vec![u16::MAX; (max_gid + 1) as usize];

    for (i, t) in tilesets.iter().enumerate() {
        let start = t.first_gid;
        let end = t.first_gid + t.tilecount;
        for gid in start..end {
            gid_lut[gid as usize] = i as u16;
        }
    }

    Ok((tilesets, gid_lut))
}

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
    gid_lut: Vec<u16>,
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
                    margin } => {
                    max_gid = max_gid.max(*first_gid + tilecount - 1);
                } 
            }
        }

        let mut gid_lut =  vec![u16::MAX; (max_gid + 1) as usize];

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
                    margin } => {
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

        for (lz, layer) in ir.layers.iter().enumerate() {
            if !layer.visible {
                continue;
            }

            if let IrLayerKind::Tiles { width, height, data } = &layer.kind {
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

                    index.add_tile(
                        TileId(*gid),
                        lz as LayerIdx,
                        world,
                    );
                }
            }
        }

        Ok(Self {
            index,
            tilesets,
            gid_lut,
            tile_w: ir.tile_w,
            tile_h: ir.tile_h,
        })
    }


    #[inline]
    pub fn ts_for_gid(&self, gid: TileId) -> Option<(&TilesetInfo, u32)> {
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

    fn draw_chunks(&self, view: LocalView) {
        for LocalChunkView { coord: cc, layers } in view.chunks {
            let mut layer_keys: Vec<_> = layers.keys().cloned().collect();
            layer_keys.sort_unstable();

            for lid in layer_keys {
                if let Some(vec) = layers.get(&lid) {
                    for rec in vec {
                        if let Some((ts, local)) = self.ts_for_gid(rec.id) {
                            let col = local % ts.cols;
                            let row = local / ts.cols;
                            let sx = ts.margin + col * (ts.tile_w + ts.spacing);
                            let sy = ts.margin + row * (ts.tile_h + ts.spacing);

                            draw_texture_ex(
                                &ts.tex,
                                (cc.x * CHUNK_SIZE) as f32 + rec.rel_pos.x,
                                (cc.y * CHUNK_SIZE) as f32 + rec.rel_pos.y,
                                WHITE,
                                DrawTextureParams {
                                    source: Some(Rect::new(
                                        sx as f32,
                                        sy as f32,
                                        ts.tile_w as f32,
                                        ts.tile_h as f32,
                                    )),
                                    ..Default::default()
                                },
                            );
                        }
                    }
                }
            }
        }
    }
}
