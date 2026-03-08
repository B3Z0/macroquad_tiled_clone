use super::*;

impl MapData {
    /// Returns parsed object layers for inspection/querying.
    pub fn object_layers(&self) -> &[ObjectLayer] {
        &self.object_state.layers
    }

    /// Iterates all parsed objects across all object layers.
    pub fn objects(&self) -> impl Iterator<Item = &IrObject> {
        self.object_state
            .layers
            .iter()
            .flat_map(|layer| layer.objects.iter())
    }

    /// Queries visible object handles for one object layer in a world-space rectangle.
    ///
    /// This query is data-oriented and returns stable handles for follow-up O(1)
    /// handle-based operations.
    pub fn query_visible_object_handles(
        &self,
        layer_idx: usize,
        view_min: Vec2,
        view_max: Vec2,
        filter: ObjectQueryFilter<'_>,
    ) -> Vec<ObjectHandle> {
        let Some(layer) = self.object_state.layers.get(layer_idx) else {
            return Vec::new();
        };
        if !layer.visible {
            return Vec::new();
        }

        let coords = self.visible_coords_for_draw(view_min, view_max, 0.0);
        let mut out = self.query_object_handles_in_coords(layer_idx, &coords);
        out.retain(|&handle| {
            let Some((handle_layer_idx, object_slot_idx)) = self.object_location(handle) else {
                return false;
            };
            if handle_layer_idx != layer_idx {
                return false;
            }
            let Some(runtime) = self
                .object_state
                .runtime_by_layer
                .get(handle_layer_idx)
                .and_then(|v| v.get(object_slot_idx))
                .and_then(|r| r.as_ref())
            else {
                return false;
            };
            if !runtime.alive || !runtime.visible {
                return false;
            }
            let Some(obj) = self
                .object_state
                .layers
                .get(handle_layer_idx)
                .and_then(|layer| layer.objects.get(object_slot_idx))
            else {
                return false;
            };
            if let Some(kind) = filter.kind {
                if obj.class_name != kind {
                    return false;
                }
            }
            if let Some(tag) = filter.tag {
                if !object_has_tag(obj, tag) {
                    return false;
                }
            }
            true
        });
        out
    }

    /// Queries visible authored object IDs for one object layer in a world-space rectangle.
    ///
    /// IDs follow Tiled object ids; results are deterministic and deduplicated.
    pub fn query_visible_object_ids(
        &self,
        layer_idx: usize,
        view_min: Vec2,
        view_max: Vec2,
        filter: ObjectQueryFilter<'_>,
    ) -> Vec<u32> {
        let mut ids = Vec::new();
        for handle in self.query_visible_object_handles(layer_idx, view_min, view_max, filter) {
            if let Some(obj) = self.object_by_handle(handle) {
                ids.push(obj.id);
            }
        }
        ids
    }

    pub(crate) fn object_location(&self, handle: ObjectHandle) -> Option<(usize, usize)> {
        let (layer_idx, object_idx) = self
            .object_state
            .location_by_handle
            .get(handle.0 as usize)?
            .as_ref()?;
        let slot_handle = self
            .object_state
            .handles_by_layer
            .get(*layer_idx)?
            .get(*object_idx)?
            .as_ref()?;
        if *slot_handle != handle {
            return None;
        }
        Some((*layer_idx, *object_idx))
    }

    pub(crate) fn object_by_handle(&self, handle: ObjectHandle) -> Option<&IrObject> {
        let (layer_idx, object_idx) = self.object_location(handle)?;
        self.object_state
            .layers
            .get(layer_idx)
            .and_then(|layer| layer.objects.get(object_idx))
    }

    pub(crate) fn object_runtime_by_handle(
        &self,
        handle: ObjectHandle,
    ) -> Option<&ObjectRuntimeState> {
        let (layer_idx, object_idx) = self.object_location(handle)?;
        self.object_state
            .runtime_by_layer
            .get(layer_idx)?
            .get(object_idx)?
            .as_ref()
    }

    pub(crate) fn query_object_handles_in_coords(
        &self,
        layer_idx: usize,
        coords: &[crate::spatial::ChunkCoord],
    ) -> Vec<ObjectHandle> {
        let Some(layer) = self.object_state.layers.get(layer_idx) else {
            return Vec::new();
        };
        let mut handles = self
            .index
            .dedup_object_handles_in_coords(coords, layer.bucket_layer);
        handles.sort_by_key(|h| h.0);
        handles
    }

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

fn object_has_tag(obj: &IrObject, tag: &str) -> bool {
    if let Some(v) = obj.properties.get_string("tag") {
        if v == tag {
            return true;
        }
    }
    if let Some(v) = obj.properties.get_string("tags") {
        return v.split(',').any(|t| t.trim() == tag);
    }
    false
}
