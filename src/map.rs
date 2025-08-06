use macroquad::prelude::*;
use serde::Deserialize;

use crate::{GlobalIndex, TileId, LayerIdx};

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
