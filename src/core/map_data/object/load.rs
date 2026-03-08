use super::super::shared::geometry::object_chunk_span_runtime;
use super::super::*;

pub(crate) fn build_object_state_from_layers(
    layers: &[IrLayer],
    layer_kind_by_id: &HashMap<LayerId, LayerKindInfo>,
    index: &mut GlobalIndex,
) -> ObjectState {
    let mut object_layers = Vec::new();
    let mut object_location_by_handle = Vec::new();
    let mut object_handles_by_layer = Vec::new();
    let mut object_runtime_by_layer = Vec::new();

    for (layer_z, layer) in layers.iter().enumerate() {
        let IrLayerKind::Objects { objects } = &layer.kind else {
            continue;
        };

        let bucket_layer = layer_z as LayerIdx;
        let layer_idx = object_layers.len();
        object_layers.push(ObjectLayer {
            id: layer_z as LayerId,
            name: layer.name.clone(),
            visible: layer.visible,
            opacity: layer.opacity,
            offset: layer.offset,
            properties: layer.properties.clone(),
            objects: objects.clone(),
            bucket_layer,
        });
        let mut handles_in_layer = Vec::with_capacity(objects.len());
        let mut runtime_in_layer = Vec::with_capacity(objects.len());

        for (object_idx, obj) in objects.iter().enumerate() {
            let handle = index.alloc_object_handle();
            let handle_idx = handle.0 as usize;
            if handle_idx >= object_location_by_handle.len() {
                object_location_by_handle.resize(handle_idx + 1, None);
            }
            object_location_by_handle[handle_idx] = Some((layer_idx, object_idx));
            handles_in_layer.push(Some(handle));
            runtime_in_layer.push(Some(ObjectRuntimeState {
                alive: true,
                visible: obj.visible,
                x: obj.x,
                y: obj.y,
                width: obj.width,
                height: obj.height,
            }));

            let runtime = runtime_in_layer[object_idx].expect("runtime must exist during build");
            let world = vec2(runtime.x, runtime.y) + layer.offset;
            let (chunk_min, chunk_max) = object_chunk_span_runtime(obj, runtime, layer.offset);

            for cy in chunk_min.y..=chunk_max.y {
                for cx in chunk_min.x..=chunk_max.x {
                    let cc = crate::spatial::ChunkCoord { x: cx, y: cy };
                    let chunk_origin = vec2((cc.x * CHUNK_SIZE) as f32, (cc.y * CHUNK_SIZE) as f32);
                    index.insert_object(
                        bucket_layer,
                        cc,
                        crate::spatial::ObjectRec {
                            handle,
                            rel_pos: world - chunk_origin,
                        },
                    );
                }
            }
        }
        object_handles_by_layer.push(handles_in_layer);
        object_runtime_by_layer.push(runtime_in_layer);
        debug_assert!(matches!(
            layer_kind_by_id.get(&(layer_z as LayerId)),
            Some(LayerKindInfo::Objects(idx)) if *idx == layer_idx
        ));
    }

    ObjectState {
        object_layers,
        object_location_by_handle,
        object_handles_by_layer,
        object_runtime_by_layer,
    }
}
