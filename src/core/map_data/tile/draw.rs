//! Tile draw-origin helpers shared by load/render code paths.

use macroquad::prelude::{vec2, Vec2};

pub(crate) fn tile_draw_origin(world: Vec2, map_tile_h: u32, tile_h: u32) -> Vec2 {
    // For orthogonal tile layers, tiles are bottom-aligned to the map cell.
    // This keeps oversized tiles extending upward instead of downward.
    vec2(world.x, world.y + (map_tile_h as f32 - tile_h as f32))
}
