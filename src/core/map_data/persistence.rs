use super::{LayerId, LayerKindInfo, MapData, ObjectLayer, ObjectRuntimeState};
use crate::ir_map::{
    IrLayer, IrLayerKind, IrObject, IrObjectShape, IrTileset, Properties, PropertyValue,
};
use crate::MapError;
use serde_json::{json, Value as JsonValue};
use std::path::Path;

impl MapData {
    /// Saves canonical runtime state to a Tiled JSON map file.
    ///
    /// Export reads canonical state only and excludes derived index and render data.
    pub fn save_to_json(&self, path: &str) -> Result<(), MapError> {
        let p = Path::new(path);
        let layers_json = self.build_layers_json();
        let tilesets_json = self.build_tilesets_json();
        let root = self.build_root_json(layers_json, tilesets_json);
        self.write_json_file(p, &root)
    }

    fn build_layers_json(&self) -> Vec<JsonValue> {
        let mut layers_json = Vec::new();
        for (layer_z, layer_ir) in self.source_ir.layers.iter().enumerate() {
            match self.layer_kind_for_z(layer_z) {
                Some(LayerKindInfo::Tiles(_)) => {
                    if let Some(tile_layer_json) = self.tile_layer_json(layer_ir) {
                        layers_json.push(tile_layer_json);
                    }
                }
                Some(LayerKindInfo::Objects(object_layer_idx)) => {
                    if let Some(object_layer_json) =
                        self.object_layer_json(layer_ir, object_layer_idx)
                    {
                        layers_json.push(object_layer_json);
                    }
                }
                Some(LayerKindInfo::Unsupported) | None => {}
            }
        }
        layers_json
    }

    fn layer_kind_for_z(&self, layer_z: usize) -> Option<LayerKindInfo> {
        self.layer_plan
            .layer_kind_by_id
            .get(&(layer_z as LayerId))
            .copied()
    }

    fn tile_layer_json(&self, layer_ir: &IrLayer) -> Option<JsonValue> {
        let IrLayerKind::Tiles {
            width,
            height,
            data,
        } = &layer_ir.kind
        else {
            return None;
        };
        Some(json!({
            "type": "tilelayer",
            "name": layer_ir.name,
            "visible": layer_ir.visible,
            "opacity": layer_ir.opacity,
            "offsetx": layer_ir.offset.x,
            "offsety": layer_ir.offset.y,
            "width": width,
            "height": height,
            "data": data,
            "properties": properties_to_json_vec(&layer_ir.properties),
        }))
    }

    fn object_layer_json(&self, layer_ir: &IrLayer, object_layer_idx: usize) -> Option<JsonValue> {
        let layer = self.object_state.object_layers.get(object_layer_idx)?;
        let objects_json = self.object_layer_objects_json(layer, object_layer_idx);
        Some(json!({
            "type": "objectgroup",
            "name": layer_ir.name,
            "visible": layer_ir.visible,
            "opacity": layer_ir.opacity,
            "offsetx": layer_ir.offset.x,
            "offsety": layer_ir.offset.y,
            "objects": objects_json,
            "properties": properties_to_json_vec(&layer_ir.properties),
        }))
    }

    fn object_layer_objects_json(
        &self,
        layer: &ObjectLayer,
        object_layer_idx: usize,
    ) -> Vec<JsonValue> {
        let mut objects_json = Vec::new();
        for (idx, authored) in layer.objects.iter().enumerate() {
            let has_handle = self
                .object_state
                .object_handles_by_layer
                .get(object_layer_idx)
                .and_then(|v| v.get(idx))
                .and_then(|slot| slot.as_ref())
                .is_some();
            if !has_handle {
                continue;
            }

            let Some(runtime) = self
                .object_state
                .object_runtime_by_layer
                .get(object_layer_idx)
                .and_then(|v| v.get(idx))
                .and_then(|r| r.as_ref())
            else {
                continue;
            };
            if !runtime.alive {
                continue;
            }

            objects_json.push(object_entry_json(authored, runtime));
        }
        objects_json
    }

    fn build_tilesets_json(&self) -> Vec<JsonValue> {
        let mut tilesets_json = Vec::new();
        for ts in &self.source_ir.tilesets {
            match ts {
                IrTileset::Atlas {
                    first_gid, source, ..
                } => {
                    tilesets_json.push(json!({
                        "firstgid": first_gid,
                        "source": source,
                    }));
                }
            }
        }
        tilesets_json.sort_by(|a, b| {
            let af = a["firstgid"].as_u64().unwrap_or(0);
            let bf = b["firstgid"].as_u64().unwrap_or(0);
            af.cmp(&bf)
        });
        tilesets_json
    }

    fn build_root_json(
        &self,
        layers_json: Vec<JsonValue>,
        tilesets_json: Vec<JsonValue>,
    ) -> JsonValue {
        json!({
            "tilewidth": self.source_ir.tile_w,
            "tileheight": self.source_ir.tile_h,
            "properties": properties_to_json_vec(&self.source_ir.properties),
            "layers": layers_json,
            "tilesets": tilesets_json,
        })
    }

    fn write_json_file(&self, path: &Path, root: &JsonValue) -> Result<(), MapError> {
        let text = serde_json::to_string_pretty(&root).map_err(|source| MapError::Json {
            path: path.to_path_buf(),
            source,
        })?;
        std::fs::write(path, text).map_err(|source| MapError::Io {
            path: path.to_path_buf(),
            source,
        })
    }
}

fn object_entry_json(authored: &IrObject, runtime: &ObjectRuntimeState) -> JsonValue {
    let mut obj = json!({
        "id": authored.id,
        "name": authored.name,
        "type": "",
        "class": authored.class_name,
        "x": runtime.x,
        "y": runtime.y,
        "width": runtime.width,
        "height": runtime.height,
        "rotation": authored.rotation,
        "visible": runtime.visible,
        "properties": properties_to_json_vec(&authored.properties),
    });

    match &authored.shape {
        IrObjectShape::Rectangle => {}
        IrObjectShape::Point => {
            obj["point"] = JsonValue::Bool(true);
        }
        IrObjectShape::Polygon(points) => {
            obj["polygon"] =
                JsonValue::Array(points.iter().map(|p| json!({"x": p.x, "y": p.y})).collect());
        }
        IrObjectShape::Polyline(points) => {
            obj["polyline"] =
                JsonValue::Array(points.iter().map(|p| json!({"x": p.x, "y": p.y})).collect());
        }
        IrObjectShape::Tile { gid } => {
            obj["gid"] = JsonValue::Number(serde_json::Number::from(*gid));
        }
    }
    obj
}

fn properties_to_json_vec(props: &Properties) -> Vec<JsonValue> {
    let mut entries: Vec<_> = props.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));

    entries
        .into_iter()
        .map(|(name, value)| match value {
            PropertyValue::Bool(v) => json!({
                "name": name,
                "type": "bool",
                "value": v,
            }),
            PropertyValue::I64(v) => json!({
                "name": name,
                "type": "int",
                "value": v,
            }),
            PropertyValue::F32(v) => json!({
                "name": name,
                "type": "float",
                "value": v,
            }),
            PropertyValue::String(v) => json!({
                "name": name,
                "type": "string",
                "value": v,
            }),
        })
        .collect()
}
