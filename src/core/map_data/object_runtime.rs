use super::*;

impl MapData {
    pub(crate) fn set_object_visible_by_handle(
        &mut self,
        handle: ObjectHandle,
        visible: bool,
    ) -> bool {
        let Some((layer_idx, object_idx)) = self.object_location(handle) else {
            return false;
        };
        let Some(Some(runtime)) = self
            .object_state
            .runtime_by_layer
            .get_mut(layer_idx)
            .and_then(|v| v.get_mut(object_idx))
        else {
            return false;
        };
        runtime.visible = visible;
        self.debug_assert_object_sync_consistency(handle);
        true
    }

    pub(crate) fn set_object_alive_by_handle(&mut self, handle: ObjectHandle, alive: bool) -> bool {
        let Some((layer_idx, object_idx)) = self.object_location(handle) else {
            return false;
        };
        let (bucket_layer, offset) = {
            let Some(layer) = self.object_state.layers.get(layer_idx) else {
                return false;
            };
            (layer.bucket_layer, layer.offset)
        };

        let runtime_snapshot = {
            let Some(Some(runtime)) = self
                .object_state
                .runtime_by_layer
                .get_mut(layer_idx)
                .and_then(|v| v.get_mut(object_idx))
            else {
                return false;
            };
            runtime.alive = alive;
            *runtime
        };

        if !alive {
            let _ = self.index.remove_object(handle);
            self.debug_assert_object_sync_consistency(handle);
            return true;
        }

        let Some(placements) = self.object_placements_for_runtime(
            layer_idx,
            object_idx,
            runtime_snapshot,
            bucket_layer,
            offset,
        ) else {
            return false;
        };

        let ok = self.index.update_object_memberships(handle, &placements);
        self.debug_assert_object_sync_consistency(handle);
        ok
    }

    pub(crate) fn update_object_bounds_position_by_handle(
        &mut self,
        handle: ObjectHandle,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> bool {
        let Some((layer_idx, object_idx)) = self.object_location(handle) else {
            return false;
        };
        let Some(layer) = self.object_state.layers.get(layer_idx) else {
            return false;
        };
        if layer.objects.get(object_idx).is_none() {
            return false;
        }
        let runtime_snapshot = {
            let Some(Some(runtime)) = self
                .object_state
                .runtime_by_layer
                .get_mut(layer_idx)
                .and_then(|v| v.get_mut(object_idx))
            else {
                return false;
            };

            runtime.x = x;
            runtime.y = y;
            runtime.width = width;
            runtime.height = height;
            *runtime
        };

        if !runtime_snapshot.alive {
            return true;
        }

        let Some(placements) = self.object_placements_for_runtime(
            layer_idx,
            object_idx,
            runtime_snapshot,
            layer.bucket_layer,
            layer.offset,
        ) else {
            return false;
        };

        let ok = self.index.update_object_memberships(handle, &placements);
        self.debug_assert_object_sync_consistency(handle);
        ok
    }

    pub(crate) fn remove_object_by_handle(&mut self, handle: ObjectHandle) -> bool {
        let Some((layer_idx, object_idx)) = self.object_location(handle) else {
            return false;
        };

        if !self.index.remove_object(handle) {
            return false;
        }
        if let Some(slot) = self
            .object_state
            .location_by_handle
            .get_mut(handle.0 as usize)
        {
            *slot = None;
        }
        if let Some(layer_handles) = self.object_state.handles_by_layer.get_mut(layer_idx) {
            if let Some(slot) = layer_handles.get_mut(object_idx) {
                *slot = None;
            }
        }
        if let Some(runtime_layer) = self.object_state.runtime_by_layer.get_mut(layer_idx) {
            if let Some(slot) = runtime_layer.get_mut(object_idx) {
                *slot = None;
            }
        }
        self.debug_assert_object_sync_consistency(handle);
        true
    }

    pub(crate) fn spawn_object_in_layer(
        &mut self,
        layer_idx: usize,
        object: IrObject,
    ) -> Option<ObjectHandle> {
        let (object_idx, bucket_layer, layer_offset) = {
            let layer = self.object_state.layers.get_mut(layer_idx)?;
            let object_idx = layer.objects.len();
            layer.objects.push(object.clone());
            (object_idx, layer.bucket_layer, layer.offset)
        };

        if self.object_state.handles_by_layer.len() <= layer_idx {
            self.object_state
                .handles_by_layer
                .resize_with(layer_idx + 1, Vec::new);
        }
        if self.object_state.runtime_by_layer.len() <= layer_idx {
            self.object_state
                .runtime_by_layer
                .resize_with(layer_idx + 1, Vec::new);
        }

        let handle = self.index.alloc_object_handle();
        let hidx = handle.0 as usize;
        if self.object_state.location_by_handle.len() <= hidx {
            self.object_state.location_by_handle.resize(hidx + 1, None);
        }
        self.object_state.location_by_handle[hidx] = Some((layer_idx, object_idx));
        self.object_state.handles_by_layer[layer_idx].push(Some(handle));

        let runtime = ObjectRuntimeState {
            alive: true,
            visible: object.visible,
            x: object.x,
            y: object.y,
            width: object.width,
            height: object.height,
        };
        self.object_state.runtime_by_layer[layer_idx].push(Some(runtime));

        let placements = self.object_placements_for_runtime(
            layer_idx,
            object_idx,
            runtime,
            bucket_layer,
            layer_offset,
        )?;
        let _ = self.index.update_object_memberships(handle, &placements);
        self.debug_assert_object_sync_consistency(handle);
        Some(handle)
    }

    fn object_placements_for_runtime(
        &self,
        layer_idx: usize,
        object_idx: usize,
        runtime: ObjectRuntimeState,
        bucket_layer: LayerIdx,
        layer_offset: Vec2,
    ) -> Option<Vec<(LayerIdx, crate::spatial::ChunkCoord, Vec2)>> {
        let authored = self
            .object_state
            .layers
            .get(layer_idx)
            .and_then(|layer| layer.objects.get(object_idx))?;

        let (chunk_min, chunk_max) = object_chunk_span_runtime(authored, runtime, layer_offset);
        let world = vec2(runtime.x, runtime.y) + layer_offset;
        let mut placements = Vec::new();
        for cy in chunk_min.y..=chunk_max.y {
            for cx in chunk_min.x..=chunk_max.x {
                let cc = crate::spatial::ChunkCoord { x: cx, y: cy };
                let chunk_origin = vec2((cc.x * CHUNK_SIZE) as f32, (cc.y * CHUNK_SIZE) as f32);
                placements.push((bucket_layer, cc, world - chunk_origin));
            }
        }
        Some(placements)
    }

    pub(crate) fn debug_assert_object_sync_consistency(&self, handle: ObjectHandle) {
        let Some((layer_idx, object_idx)) = self.object_location(handle) else {
            debug_assert!(self.index.object_memberships(handle).is_none());
            return;
        };
        let Some(runtime) = self
            .object_state
            .runtime_by_layer
            .get(layer_idx)
            .and_then(|v| v.get(object_idx))
            .and_then(|r| r.as_ref())
        else {
            debug_assert!(self.index.object_memberships(handle).is_none());
            return;
        };

        let memberships = self.index.object_memberships(handle).unwrap_or(&[]);
        let unique: HashSet<_> = memberships.iter().copied().collect();
        debug_assert_eq!(
            unique.len(),
            memberships.len(),
            "duplicate index memberships for one object handle"
        );

        if runtime.alive {
            debug_assert!(
                !memberships.is_empty(),
                "alive object must have at least one index membership"
            );
        }
    }
}
