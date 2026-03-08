//! Handle-centric tile mutation helpers.

use super::super::{MapData, TileRuntimeState, TilesetRuntimeInfo};
use super::index_sync::tile_chunk_span;
use crate::spatial::{world_to_chunk, TileHandle, TileId};
use macroquad::prelude::vec2;

impl MapData {
    #[allow(dead_code)]
    pub(crate) fn update_tile_gid_by_handle(&mut self, handle: TileHandle, id: TileId) -> bool {
        let Some((layer_idx, slot_idx)) = self.tile_location(handle) else {
            return false;
        };
        let runtime_snapshot = {
            let Some(Some(runtime)) = self
                .tile_state
                .runtime
                .tile_runtime_by_layer
                .get_mut(layer_idx)
                .and_then(|v| v.get_mut(slot_idx))
            else {
                return false;
            };
            runtime.id = id;
            *runtime
        };

        let ok = self.sync_tile_index_for_runtime(handle, layer_idx, runtime_snapshot);
        self.debug_assert_tile_sync_consistency(handle);
        ok
    }

    #[allow(dead_code)]
    pub(crate) fn move_tile_by_handle(&mut self, handle: TileHandle, x: f32, y: f32) -> bool {
        let Some((layer_idx, slot_idx)) = self.tile_location(handle) else {
            return false;
        };
        let runtime_snapshot = {
            let Some(Some(runtime)) = self
                .tile_state
                .runtime
                .tile_runtime_by_layer
                .get_mut(layer_idx)
                .and_then(|v| v.get_mut(slot_idx))
            else {
                return false;
            };
            runtime.x = x;
            runtime.y = y;
            *runtime
        };

        let ok = self.sync_tile_index_for_runtime(handle, layer_idx, runtime_snapshot);
        self.debug_assert_tile_sync_consistency(handle);
        ok
    }

    #[allow(dead_code)]
    pub(crate) fn set_tile_visible_by_handle(&mut self, handle: TileHandle, visible: bool) -> bool {
        let Some((layer_idx, slot_idx)) = self.tile_location(handle) else {
            return false;
        };
        let Some(Some(runtime)) = self
            .tile_state
            .runtime
            .tile_runtime_by_layer
            .get_mut(layer_idx)
            .and_then(|v| v.get_mut(slot_idx))
        else {
            return false;
        };
        runtime.visible = visible;
        self.debug_assert_tile_sync_consistency(handle);
        true
    }

    #[allow(dead_code)]
    pub(crate) fn set_tile_alive_by_handle(&mut self, handle: TileHandle, alive: bool) -> bool {
        let Some((layer_idx, slot_idx)) = self.tile_location(handle) else {
            return false;
        };
        let runtime_snapshot = {
            let Some(Some(runtime)) = self
                .tile_state
                .runtime
                .tile_runtime_by_layer
                .get_mut(layer_idx)
                .and_then(|v| v.get_mut(slot_idx))
            else {
                return false;
            };
            runtime.alive = alive;
            *runtime
        };

        let ok = self.sync_tile_index_for_runtime(handle, layer_idx, runtime_snapshot);
        self.debug_assert_tile_sync_consistency(handle);
        ok
    }

    #[allow(dead_code)]
    pub(crate) fn remove_tile_by_handle(&mut self, handle: TileHandle) -> bool {
        let Some((layer_idx, slot_idx)) = self.tile_location(handle) else {
            return false;
        };
        if !self.derived_index.remove_tile(handle) {
            return false;
        }
        if let Some(slot) = self
            .tile_state
            .runtime
            .tile_location_by_handle
            .get_mut(handle.0 as usize)
        {
            *slot = None;
        }
        if let Some(layer_handles) = self
            .tile_state
            .runtime
            .tile_handles_by_layer
            .get_mut(layer_idx)
        {
            if let Some(slot) = layer_handles.get_mut(slot_idx) {
                *slot = None;
            }
        }
        if let Some(layer_runtime) = self
            .tile_state
            .runtime
            .tile_runtime_by_layer
            .get_mut(layer_idx)
        {
            if let Some(slot) = layer_runtime.get_mut(slot_idx) {
                *slot = None;
            }
        }
        self.debug_assert_tile_sync_consistency(handle);
        true
    }

    fn sync_tile_index_for_runtime(
        &mut self,
        handle: TileHandle,
        layer_idx: usize,
        runtime: TileRuntimeState,
    ) -> bool {
        let _ = self.derived_index.remove_tile(handle);
        if !runtime.alive {
            return true;
        }

        let Some(layer) = self.tile_state.authored.tile_layers.get(layer_idx) else {
            return false;
        };
        let Some(tileset) = self.tileset_for_gid(runtime.id) else {
            return false;
        };

        let draw_origin = vec2(runtime.x, runtime.y);
        let oversized =
            tileset.tile_w > self.source_ir.tile_w || tileset.tile_h > self.source_ir.tile_h;
        if oversized {
            let (chunk_min, chunk_max) =
                tile_chunk_span(draw_origin, tileset.tile_w as f32, tileset.tile_h as f32);
            for cy in chunk_min.y..=chunk_max.y {
                for cx in chunk_min.x..=chunk_max.x {
                    let cc = crate::spatial::ChunkCoord { x: cx, y: cy };
                    self.derived_index.insert_tile_with_handle(
                        handle,
                        runtime.id,
                        layer.bucket_layer,
                        cc,
                        draw_origin,
                    );
                }
            }
        } else {
            let cc = world_to_chunk(draw_origin);
            self.derived_index.insert_tile_with_handle(
                handle,
                runtime.id,
                layer.bucket_layer,
                cc,
                draw_origin,
            );
        }
        true
    }

    fn tileset_for_gid(&self, id: TileId) -> Option<&TilesetRuntimeInfo> {
        let clean = id.clean() as usize;
        let idx = *self.tile_state.derived.gid_lut.get(clean)?;
        if idx == u16::MAX {
            return None;
        }
        self.tile_state
            .authored
            .tileset_runtime_info
            .get(idx as usize)
    }

    fn debug_assert_tile_sync_consistency(&self, handle: TileHandle) {
        let Some((layer_idx, slot_idx)) = self.tile_location(handle) else {
            debug_assert!(self.derived_index.tile_rec(handle).is_none());
            return;
        };
        let Some(runtime) = self
            .tile_state
            .runtime
            .tile_runtime_by_layer
            .get(layer_idx)
            .and_then(|v| v.get(slot_idx))
            .and_then(|r| r.as_ref())
        else {
            debug_assert!(self.derived_index.tile_rec(handle).is_none());
            return;
        };

        if runtime.alive {
            debug_assert!(self.derived_index.tile_rec(handle).is_some());
        } else {
            debug_assert!(self.derived_index.tile_rec(handle).is_none());
        }
    }
}
