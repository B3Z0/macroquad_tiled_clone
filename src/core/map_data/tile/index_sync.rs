//! Tile chunk-span helpers used during index population.

use crate::spatial::world_to_chunk;
use macroquad::prelude::{vec2, Vec2};

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
