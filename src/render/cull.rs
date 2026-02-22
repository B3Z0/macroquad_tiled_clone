use crate::spatial::{ChunkCoord, GlobalIndex, LayerBucket, LayerIdx, CHUNK_SIZE};
use macroquad::prelude::*;
use std::collections::HashMap;

const CULL_MARGIN_CHUNKS: i32 = 1;

pub struct LocalChunkView<'g> {
    pub coord: ChunkCoord,
    pub layers: &'g HashMap<LayerIdx, LayerBucket>,
}
pub struct LocalView<'g> {
    pub chunks: Vec<LocalChunkView<'g>>,
}

pub fn visible_chunk_coords_rect(view_min: Vec2, view_max: Vec2) -> Vec<ChunkCoord> {
    let mut cx_min = (view_min.x as i32).div_euclid(CHUNK_SIZE);
    let mut cy_min = (view_min.y as i32).div_euclid(CHUNK_SIZE);
    let mut cx_max = (view_max.x as i32).div_euclid(CHUNK_SIZE);
    let mut cy_max = (view_max.y as i32).div_euclid(CHUNK_SIZE);

    if cx_min > cx_max {
        std::mem::swap(&mut cx_min, &mut cx_max);
    }
    if cy_min > cy_max {
        std::mem::swap(&mut cy_min, &mut cy_max);
    }

    let mut coords = Vec::new();
    for cy in cy_min..=cy_max {
        for cx in cx_min..=cx_max {
            coords.push(ChunkCoord { x: cx, y: cy });
        }
    }
    coords
}

pub fn query_visible<'g>(g: &'g GlobalIndex, cam: &Camera2D) -> LocalView<'g> {
    let (viewport_width, viewport_height) = match cam.viewport {
        Some((_, _, w, h)) => (w as f32, h as f32),
        None => (screen_width(), screen_height()), // Fall back to screen dimensions
    };

    let half_w = viewport_width / cam.zoom.x / 2.0;
    let half_h = viewport_height / cam.zoom.y / 2.0;
    let cam_min = cam.target - Vec2::new(half_w, half_h);
    let cam_max = cam.target + Vec2::new(half_w, half_h);

    //pad by one chunk
    let pad = CHUNK_SIZE as f32;
    let min = vec2(cam_min.x - pad, cam_min.y - pad);
    let max = vec2(cam_max.x + pad, cam_max.y + pad);

    let cx_min = (min.x as i32).div_euclid(CHUNK_SIZE);
    let cy_min = (min.y as i32).div_euclid(CHUNK_SIZE);
    let cx_max = (max.x as i32).div_euclid(CHUNK_SIZE);
    let cy_max = (max.y as i32).div_euclid(CHUNK_SIZE);

    let mut chunks = Vec::new();
    for (&coord, bucket) in &g.buckets {
        if coord.x >= cx_min && coord.x <= cx_max && coord.y >= cy_min && coord.y <= cy_max {
            chunks.push(LocalChunkView {
                coord,
                layers: &bucket.layers,
            })
        }
    }
    chunks.sort_by_key(|c| (c.coord.y, c.coord.x));

    LocalView { chunks }
}

pub fn query_visible_rect<'g>(g: &'g GlobalIndex, view_min: Vec2, view_max: Vec2) -> LocalView<'g> {
    let mut cx_min = (view_min.x as i32).div_euclid(CHUNK_SIZE);
    let mut cy_min = (view_min.y as i32).div_euclid(CHUNK_SIZE);
    let mut cx_max = (view_max.x as i32).div_euclid(CHUNK_SIZE);
    let mut cy_max = (view_max.y as i32).div_euclid(CHUNK_SIZE);

    //pad by one chunk
    if cx_min > cx_max {
        std::mem::swap(&mut cx_min, &mut cx_max);
    }
    if cy_min > cy_max {
        std::mem::swap(&mut cy_min, &mut cy_max);
    }

    cx_min -= CULL_MARGIN_CHUNKS;
    cy_min -= CULL_MARGIN_CHUNKS;
    cx_max += CULL_MARGIN_CHUNKS;
    cy_max += CULL_MARGIN_CHUNKS;

    let mut chunks = Vec::new();
    for (&coord, bucket) in &g.buckets {
        if coord.x >= cx_min && coord.x <= cx_max && coord.y >= cy_min && coord.y <= cy_max {
            chunks.push(LocalChunkView {
                coord,
                layers: &bucket.layers,
            })
        }
    }
    chunks.sort_by_key(|c| (c.coord.y, c.coord.x));

    LocalView { chunks }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spatial::TileId;

    #[test]
    fn query_visible_rect_returns_chunks_in_stable_order() {
        let mut index = GlobalIndex::new();
        index.add_tile(TileId(1), 0, vec2(520.0, 520.0)); // (2,2)
        index.add_tile(TileId(1), 0, vec2(0.0, 0.0)); // (0,0)
        index.add_tile(TileId(1), 0, vec2(260.0, 0.0)); // (1,0)
        index.add_tile(TileId(1), 0, vec2(0.0, 260.0)); // (0,1)

        let view = query_visible_rect(&index, vec2(0.0, 0.0), vec2(800.0, 800.0));
        let coords: Vec<ChunkCoord> = view.chunks.iter().map(|c| c.coord).collect();

        assert!(coords
            .windows(2)
            .all(|w| (w[0].y, w[0].x) <= (w[1].y, w[1].x)));
    }
}
