use super::super::shared::geometry::object_chunk_span_runtime;
use super::super::*;

impl MapData {
    pub(super) fn object_placements_for_runtime(
        &self,
        layer_idx: usize,
        object_idx: usize,
        runtime: ObjectRuntimeState,
        bucket_layer: LayerIdx,
        layer_offset: Vec2,
    ) -> Option<Vec<(LayerIdx, crate::spatial::ChunkCoord, Vec2)>> {
        let authored = self
            .object_state
            .object_layers
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
            debug_assert!(self.derived_index.object_memberships(handle).is_none());
            return;
        };
        let Some(runtime) = self
            .object_state
            .object_runtime_by_layer
            .get(layer_idx)
            .and_then(|v| v.get(object_idx))
            .and_then(|r| r.as_ref())
        else {
            debug_assert!(self.derived_index.object_memberships(handle).is_none());
            return;
        };

        let memberships = self.derived_index.object_memberships(handle).unwrap_or(&[]);
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
