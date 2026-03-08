//! Tile-visible chunk query helpers.

use super::super::MapData;
use crate::spatial::CHUNK_SIZE;
use macroquad::prelude::{vec2, Vec2};

impl MapData {
    pub(crate) fn visible_coords_for_draw(
        &self,
        view_min: Vec2,
        view_max: Vec2,
        cull_padding: f32,
    ) -> Vec<crate::spatial::ChunkCoord> {
        let min = vec2(view_min.x - cull_padding, view_min.y - cull_padding);
        let max = vec2(view_max.x + cull_padding, view_max.y + cull_padding);

        let mut cx_min = (min.x as i32).div_euclid(CHUNK_SIZE);
        let mut cy_min = (min.y as i32).div_euclid(CHUNK_SIZE);
        let mut cx_max = (max.x as i32).div_euclid(CHUNK_SIZE);
        let mut cy_max = (max.y as i32).div_euclid(CHUNK_SIZE);

        if cx_min > cx_max {
            std::mem::swap(&mut cx_min, &mut cx_max);
        }
        if cy_min > cy_max {
            std::mem::swap(&mut cy_min, &mut cy_max);
        }

        let mut coords = Vec::new();
        for cy in cy_min..=cy_max {
            for cx in cx_min..=cx_max {
                coords.push(crate::spatial::ChunkCoord { x: cx, y: cy });
            }
        }
        coords
    }
}
