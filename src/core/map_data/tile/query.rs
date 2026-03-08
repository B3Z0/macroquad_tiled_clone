//! Tile-visible chunk query helpers.

use super::super::{MapData, TileRuntimeState};
use crate::spatial::{TileHandle, TileId, CHUNK_SIZE};
use macroquad::prelude::{vec2, Vec2};

impl MapData {
    // TODO(T2.1/T4.1): add tile handle-centric and region tile query APIs here.
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

    #[allow(dead_code)]
    pub(crate) fn tile_by_handle(&self, handle: TileHandle) -> Option<TileId> {
        let (layer_idx, slot_idx) = self.tile_location(handle)?;
        let runtime = self
            .tile_state
            .runtime
            .tile_runtime_by_layer
            .get(layer_idx)?
            .get(slot_idx)?
            .as_ref()?;
        Some(runtime.id)
    }

    pub(crate) fn tile_runtime_by_handle(&self, handle: TileHandle) -> Option<&TileRuntimeState> {
        let (layer_idx, slot_idx) = self.tile_location(handle)?;
        self.tile_state
            .runtime
            .tile_runtime_by_layer
            .get(layer_idx)?
            .get(slot_idx)?
            .as_ref()
    }

    pub(super) fn tile_location(&self, handle: TileHandle) -> Option<(usize, usize)> {
        let (layer_idx, slot_idx) = self
            .tile_state
            .runtime
            .tile_location_by_handle
            .get(handle.0 as usize)?
            .as_ref()?;
        let slot_handle = self
            .tile_state
            .runtime
            .tile_handles_by_layer
            .get(*layer_idx)?
            .get(*slot_idx)?
            .as_ref()?;
        if *slot_handle != handle {
            return None;
        }
        Some((*layer_idx, *slot_idx))
    }
}
