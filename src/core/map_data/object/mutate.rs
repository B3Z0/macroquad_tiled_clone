use super::super::*;

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
            .object_runtime_by_layer
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
        let Some((bucket_layer, offset)) = self.object_layer_context(layer_idx) else {
            return false;
        };

        let runtime_snapshot = {
            let Some(Some(runtime)) = self
                .object_state
                .object_runtime_by_layer
                .get_mut(layer_idx)
                .and_then(|v| v.get_mut(object_idx))
            else {
                return false;
            };
            runtime.alive = alive;
            *runtime
        };

        if !alive {
            let _ = self.derived_index.remove_object(handle);
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

        let ok = self
            .derived_index
            .update_object_memberships(handle, &placements);
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
        let Some(layer) = self.object_state.object_layers.get(layer_idx) else {
            return false;
        };
        if layer.objects.get(object_idx).is_none() {
            return false;
        }
        let runtime_snapshot = {
            let Some(Some(runtime)) = self
                .object_state
                .object_runtime_by_layer
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

        let ok = self
            .derived_index
            .update_object_memberships(handle, &placements);
        self.debug_assert_object_sync_consistency(handle);
        ok
    }

    pub(crate) fn remove_object_by_handle(&mut self, handle: ObjectHandle) -> bool {
        let Some((layer_idx, object_idx)) = self.object_location(handle) else {
            return false;
        };

        if !self.derived_index.remove_object(handle) {
            return false;
        }
        self.clear_object_slots(handle, layer_idx, object_idx);
        self.debug_assert_object_sync_consistency(handle);
        true
    }

    pub(crate) fn spawn_object_in_layer(
        &mut self,
        layer_idx: usize,
        object: IrObject,
    ) -> Option<ObjectHandle> {
        let (object_idx, bucket_layer, layer_offset) = {
            let layer = self.object_state.object_layers.get_mut(layer_idx)?;
            let object_idx = layer.objects.len();
            layer.objects.push(object.clone());
            (object_idx, layer.bucket_layer, layer.offset)
        };

        self.ensure_object_layer_slot_vectors(layer_idx);

        let handle = self.derived_index.alloc_object_handle();
        let hidx = handle.0 as usize;
        if self.object_state.object_location_by_handle.len() <= hidx {
            self.object_state
                .object_location_by_handle
                .resize(hidx + 1, None);
        }
        self.object_state.object_location_by_handle[hidx] = Some((layer_idx, object_idx));
        self.object_state.object_handles_by_layer[layer_idx].push(Some(handle));

        let runtime = ObjectRuntimeState {
            alive: true,
            visible: object.visible,
            x: object.x,
            y: object.y,
            width: object.width,
            height: object.height,
        };
        self.object_state.object_runtime_by_layer[layer_idx].push(Some(runtime));

        let placements = self.object_placements_for_runtime(
            layer_idx,
            object_idx,
            runtime,
            bucket_layer,
            layer_offset,
        )?;
        let _ = self
            .derived_index
            .update_object_memberships(handle, &placements);
        self.debug_assert_object_sync_consistency(handle);
        Some(handle)
    }

    fn object_layer_context(&self, layer_idx: usize) -> Option<(LayerIdx, Vec2)> {
        let layer = self.object_state.object_layers.get(layer_idx)?;
        Some((layer.bucket_layer, layer.offset))
    }

    fn clear_object_slots(&mut self, handle: ObjectHandle, layer_idx: usize, object_idx: usize) {
        if let Some(slot) = self
            .object_state
            .object_location_by_handle
            .get_mut(handle.0 as usize)
        {
            *slot = None;
        }
        if let Some(layer_handles) = self.object_state.object_handles_by_layer.get_mut(layer_idx) {
            if let Some(slot) = layer_handles.get_mut(object_idx) {
                *slot = None;
            }
        }
        if let Some(runtime_layer) = self.object_state.object_runtime_by_layer.get_mut(layer_idx) {
            if let Some(slot) = runtime_layer.get_mut(object_idx) {
                *slot = None;
            }
        }
    }

    fn ensure_object_layer_slot_vectors(&mut self, layer_idx: usize) {
        if self.object_state.object_handles_by_layer.len() <= layer_idx {
            self.object_state
                .object_handles_by_layer
                .resize_with(layer_idx + 1, Vec::new);
        }
        if self.object_state.object_runtime_by_layer.len() <= layer_idx {
            self.object_state
                .object_runtime_by_layer
                .resize_with(layer_idx + 1, Vec::new);
        }
    }
}
