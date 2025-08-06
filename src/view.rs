use std::collections::HashMap;
use macroquad::prelude::*;
use crate::{GlobalIndex, ChunkCoord, TileRec, LayerIdx};

pub struct LocalChunkView<'g> {
    pub coord: ChunkCoord,
    pub layers: &'g HashMap<LayerIdx, Vec<TileRec>>,
}
pub struct LocalView<'g> { pub chunks: Vec<LocalChunkView<'g>> }

/// TEMP: return every bucket.  Replace with real ≤16-bucket culler later.
pub fn query_visible<'g>(g:&'g GlobalIndex, _cam:&Camera2D) -> LocalView<'g> {
    LocalView {
        chunks: g.buckets
                 .iter()
                 .map(|(c,b)| LocalChunkView{ coord:*c, layers:&b.layers })
                 .collect()
    }
}
