use macroquad::prelude::*;
use serde::Deserialize;

use crate::{spatial::CHUNK_SIZE, GlobalIndex, LayerIdx, TileId};

#[derive(Deserialize)]
struct JsonLayer {
    data:   Vec<u32>,
    width:  usize,
    height: usize,
}

#[derive(Deserialize)]
struct JsonMap {
    tilewidth:  u32,
    tileheight: u32,
    layers:     Vec<JsonLayer>,
}

/// Load a one-layer orthogonal map exported from Tiled as JSON (CSV data).
///
/// • Inserts every non-empty GID into the spatial hash.
/// • Returns `(tilewidth, tileheight)` so the caller can draw correctly.
pub fn load_basic_json(map: &mut GlobalIndex, path: &str) -> anyhow::Result<(u32, u32)> {
    let txt      = std::fs::read_to_string(path)?;
    let j: JsonMap = serde_json::from_str(&txt)?;

    let tw = j.tilewidth;
    let th = j.tileheight;

    for (lz, layer) in j.layers.iter().enumerate() {
        for (idx, gid) in layer.data.iter().enumerate() {
            if *gid == 0 { continue }

            let col   = idx % layer.width;
            let row   = idx / layer.width;
            let world = vec2(col as f32 * tw as f32, row as f32 * th as f32);

            map.add_tile(TileId(*gid), lz as LayerIdx, world);
        }
    }
    Ok((tw, th))
}


pub struct Map { 
    pub index: GlobalIndex,
    pub tile_w: u32,
    pub tile_h: u32,
}

impl Map {
    pub fn load_basic(path:&str) -> anyhow::Result<Self> {
        let mut index = GlobalIndex::new();
        let (tw, th) = load_basic_json(&mut index, path)?;
        Ok(Self { index, tile_w: tw, tile_h: th })
    }

    pub fn draw(&self, texture: &Texture2D) {
        let cols = texture.width() as u32 / self.tile_w;    // sprites per row

        for (cc, bucket) in &self.index.buckets {
            for vec in bucket.layers.values() {
                for rec in vec {
                    let gid  = rec.id.0;
                    let idx  = gid - 1;                     // GID 1 → atlas 0
                    let sx   = (idx % cols) * self.tile_w;  // left  px in atlas
                    let sy   = (idx / cols) * self.tile_h;  // top   px in atlas

                    draw_texture_ex(
                        texture,
                        (cc.x * CHUNK_SIZE) as f32 + rec.rel_pos.x,
                        (cc.y * CHUNK_SIZE) as f32 + rec.rel_pos.y,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                sx as f32,
                                sy as f32,
                                self.tile_w as f32,
                                self.tile_h as f32,
                            )),
                            ..Default::default()
                        },
                    );
                }
            }
        }
    }
}
