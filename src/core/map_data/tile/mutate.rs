//! Handle-centric tile mutation helpers.

use super::super::{MapData, TileRuntimeState, TilesetRuntimeInfo};
use super::index_sync::tile_chunk_span;
use crate::spatial::{world_to_chunk, ChunkCoord, LayerIdx, TileHandle, TileId, CHUNK_SIZE};
use macroquad::prelude::vec2;
use std::collections::HashSet;

impl MapData {
    #[allow(dead_code)]
    pub(crate) fn update_tile_gid_by_handle(&mut self, handle: TileHandle, id: TileId) -> bool {
        let Some((layer_idx, slot_idx, current_runtime)) = self.tile_runtime_snapshot(handle)
        else {
            return false;
        };
        let new_runtime = TileRuntimeState {
            id,
            ..current_runtime
        };
        self.apply_tile_runtime_update(handle, layer_idx, slot_idx, new_runtime)
    }

    #[allow(dead_code)]
    pub(crate) fn move_tile_by_handle(&mut self, handle: TileHandle, x: f32, y: f32) -> bool {
        let Some((layer_idx, slot_idx, current_runtime)) = self.tile_runtime_snapshot(handle)
        else {
            return false;
        };
        let new_runtime = TileRuntimeState {
            x,
            y,
            ..current_runtime
        };
        self.apply_tile_runtime_update(handle, layer_idx, slot_idx, new_runtime)
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
        let Some((layer_idx, slot_idx, current_runtime)) = self.tile_runtime_snapshot(handle)
        else {
            return false;
        };
        let new_runtime = TileRuntimeState {
            alive,
            ..current_runtime
        };
        self.apply_tile_runtime_update(handle, layer_idx, slot_idx, new_runtime)
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

    fn apply_tile_runtime_update(
        &mut self,
        handle: TileHandle,
        layer_idx: usize,
        slot_idx: usize,
        new_runtime: TileRuntimeState,
    ) -> bool {
        let Some(new_entries) = self.tile_index_entries_for_runtime(layer_idx, new_runtime) else {
            return false;
        };
        let Some(Some(runtime_slot)) = self
            .tile_state
            .runtime
            .tile_runtime_by_layer
            .get_mut(layer_idx)
            .and_then(|v| v.get_mut(slot_idx))
        else {
            return false;
        };
        *runtime_slot = new_runtime;

        let _ = self.derived_index.remove_tile(handle);
        for (layer, cc, world, id) in new_entries {
            self.derived_index
                .insert_tile_with_handle(handle, id, layer, cc, world);
        }
        self.debug_assert_tile_sync_consistency(handle);
        true
    }

    fn tile_runtime_snapshot(
        &self,
        handle: TileHandle,
    ) -> Option<(usize, usize, TileRuntimeState)> {
        let (layer_idx, slot_idx) = self.tile_location(handle)?;
        let runtime = self
            .tile_state
            .runtime
            .tile_runtime_by_layer
            .get(layer_idx)?
            .get(slot_idx)?
            .as_ref()?;
        Some((layer_idx, slot_idx, *runtime))
    }

    fn tile_index_entries_for_runtime(
        &self,
        layer_idx: usize,
        runtime: TileRuntimeState,
    ) -> Option<Vec<(LayerIdx, ChunkCoord, macroquad::prelude::Vec2, TileId)>> {
        if !runtime.alive {
            return Some(Vec::new());
        }
        let layer = self.tile_state.authored.tile_layers.get(layer_idx)?;
        let tileset = self.tileset_for_gid(runtime.id)?;
        let draw_origin = vec2(runtime.x, runtime.y);

        let mut out = Vec::new();
        let oversized =
            tileset.tile_w > self.source_ir.tile_w || tileset.tile_h > self.source_ir.tile_h;
        if oversized {
            let (chunk_min, chunk_max) =
                tile_chunk_span(draw_origin, tileset.tile_w as f32, tileset.tile_h as f32);
            for cy in chunk_min.y..=chunk_max.y {
                for cx in chunk_min.x..=chunk_max.x {
                    out.push((
                        layer.bucket_layer,
                        ChunkCoord { x: cx, y: cy },
                        draw_origin,
                        runtime.id,
                    ));
                }
            }
        } else {
            out.push((
                layer.bucket_layer,
                world_to_chunk(draw_origin),
                draw_origin,
                runtime.id,
            ));
        }
        Some(out)
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
        let layer_bucket = self
            .tile_state
            .authored
            .tile_layers
            .get(layer_idx)
            .map(|l| l.bucket_layer);
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

        let mut memberships =
            Vec::<(ChunkCoord, LayerIdx, TileId, macroquad::prelude::Vec2)>::new();
        for (cc, chunk) in &self.derived_index.buckets {
            for (layer, bucket) in &chunk.layers {
                for rec in &bucket.tiles {
                    if rec.handle != handle {
                        continue;
                    }
                    memberships.push((*cc, *layer, rec.id, rec.rel_pos));
                }
            }
        }
        let unique: HashSet<_> = memberships.iter().map(|(cc, l, _, _)| (*cc, *l)).collect();
        debug_assert_eq!(
            unique.len(),
            memberships.len(),
            "duplicate tile memberships for one handle"
        );

        if runtime.alive {
            debug_assert!(
                !memberships.is_empty(),
                "alive tile must have index memberships"
            );
            for (cc, layer, id, rel_pos) in memberships {
                if let Some(bucket_layer) = layer_bucket {
                    debug_assert_eq!(layer, bucket_layer, "tile membership layer mismatch");
                }
                debug_assert_eq!(id, runtime.id, "tile id drifted from runtime");
                let chunk_origin = vec2((cc.x * CHUNK_SIZE) as f32, (cc.y * CHUNK_SIZE) as f32);
                let world = chunk_origin + rel_pos;
                debug_assert!((world.x - runtime.x).abs() < 0.01);
                debug_assert!((world.y - runtime.y).abs() < 0.01);
            }
        } else {
            debug_assert!(
                memberships.is_empty(),
                "dead tile must have no index memberships"
            );
        }
    }
}
