use super::*;

impl MapData {
    /// Saves canonical runtime state to a Tiled JSON map file.
    ///
    /// Export reads canonical state only and excludes derived index and render data.
    pub fn save_to_json(&self, path: &str) -> Result<(), MapError> {
        let p = Path::new(path);

        let mut layers_json = Vec::new();
        for (layer_z, layer_ir) in self.source_ir.layers.iter().enumerate() {
            let props = properties_to_json_vec(&layer_ir.properties);
            match self.layer_kind_by_id.get(&(layer_z as LayerId)).copied() {
                Some(LayerKindInfo::Tiles(_tile_layer_idx)) => {
                    let IrLayerKind::Tiles {
                        width,
                        height,
                        data,
                    } = &layer_ir.kind
                    else {
                        continue;
                    };
                    layers_json.push(json!({
                        "type": "tilelayer",
                        "name": layer_ir.name,
                        "visible": layer_ir.visible,
                        "opacity": layer_ir.opacity,
                        "offsetx": layer_ir.offset.x,
                        "offsety": layer_ir.offset.y,
                        "width": width,
                        "height": height,
                        "data": data,
                        "properties": props,
                    }));
                }
                Some(LayerKindInfo::Objects(object_layer_idx)) => {
                    let Some(layer) = self.object_layers.get(object_layer_idx) else {
                        continue;
                    };
                    let mut objects_json = Vec::new();
                    for (idx, authored) in layer.objects.iter().enumerate() {
                        let Some(Some(_handle)) = self
                            .object_handles_by_layer
                            .get(object_layer_idx)
                            .and_then(|v| v.get(idx))
                        else {
                            continue;
                        };
                        let Some(runtime) = self
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
                                obj["polygon"] = JsonValue::Array(
                                    points.iter().map(|p| json!({"x": p.x, "y": p.y})).collect(),
                                );
                            }
                            IrObjectShape::Polyline(points) => {
                                obj["polyline"] = JsonValue::Array(
                                    points.iter().map(|p| json!({"x": p.x, "y": p.y})).collect(),
                                );
                            }
                            IrObjectShape::Tile { gid } => {
                                obj["gid"] = JsonValue::Number(serde_json::Number::from(*gid));
                            }
                        }
                        objects_json.push(obj);
                    }

                    layers_json.push(json!({
                        "type": "objectgroup",
                        "name": layer_ir.name,
                        "visible": layer_ir.visible,
                        "opacity": layer_ir.opacity,
                        "offsetx": layer_ir.offset.x,
                        "offsety": layer_ir.offset.y,
                        "objects": objects_json,
                        "properties": props,
                    }));
                }
                Some(LayerKindInfo::Unsupported) | None => {}
            }
        }

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

        let root = json!({
            "tilewidth": self.source_ir.tile_w,
            "tileheight": self.source_ir.tile_h,
            "properties": properties_to_json_vec(&self.source_ir.properties),
            "layers": layers_json,
            "tilesets": tilesets_json,
        });

        let text = serde_json::to_string_pretty(&root).map_err(|source| MapError::Json {
            path: p.to_path_buf(),
            source,
        })?;
        std::fs::write(p, text).map_err(|source| MapError::Io {
            path: p.to_path_buf(),
            source,
        })
    }
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
