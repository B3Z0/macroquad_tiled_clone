// src/loader/json.rs
use crate::error::MapError;
use crate::ir_map::*;
use macroquad::prelude::*;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct JsonLayer {
    #[serde(default)]
    data: Vec<u32>,
    #[serde(default)]
    width: usize,
    #[serde(default)]
    height: usize,
    #[serde(default = "default_true")]
    visible: bool,
    #[serde(default = "one")]
    opacity: f32,
    #[serde(default)]
    offsetx: f32,
    #[serde(default)]
    offsety: f32,
    #[serde(default)]
    name: String,
    #[serde(rename = "type")]
    kind: Option<String>, // "tilelayer" expected here
    #[serde(default)]
    properties: Vec<JsonProperty>,
    #[serde(default)]
    objects: Vec<JsonObject>,
}

fn default_true() -> bool {
    true
}
fn one() -> f32 {
    1.0
}

#[derive(Deserialize)]
struct JsonTilesetRef {
    firstgid: u32,
    source: String,
}

#[derive(Deserialize)]
struct JsonMap {
    tilewidth: u32,
    tileheight: u32,
    layers: Vec<JsonLayer>,
    tilesets: Vec<JsonTilesetRef>,
    #[serde(default)]
    properties: Vec<JsonProperty>,
}

#[derive(Deserialize)]
struct ExternalTileset {
    tilewidth: u32,
    tileheight: u32,
    tilecount: u32,
    columns: u32,
    image: String,
    #[serde(default)]
    spacing: u32,
    #[serde(default)]
    margin: u32,
    #[serde(default)]
    properties: Vec<JsonProperty>,
    #[serde(default)]
    tiles: Vec<JsonTile>,
}

#[derive(Deserialize)]
struct JsonProperty {
    name: String,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    value: JsonValue,
}

#[derive(Deserialize)]
struct JsonObject {
    #[serde(default)]
    id: u32,
    #[serde(default)]
    name: String,
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    class: String,
    #[serde(default)]
    x: f32,
    #[serde(default)]
    y: f32,
    #[serde(default)]
    width: f32,
    #[serde(default)]
    height: f32,
    #[serde(default)]
    rotation: f32,
    #[serde(default = "default_true")]
    visible: bool,
    #[serde(default)]
    point: bool,
    #[serde(default)]
    polygon: Vec<JsonObjectPoint>,
    #[serde(default)]
    polyline: Vec<JsonObjectPoint>,
    #[serde(default)]
    gid: Option<u32>,
    #[serde(default)]
    properties: Vec<JsonProperty>,
}

#[derive(Deserialize)]
struct JsonObjectPoint {
    x: f32,
    y: f32,
}

#[derive(Deserialize, Default)]
struct JsonObjectGroup {
    #[serde(default)]
    objects: Vec<JsonObject>,
}

#[derive(Deserialize)]
struct JsonTile {
    id: u32,
    #[serde(default)]
    properties: Vec<JsonProperty>,
    #[serde(default)]
    objectgroup: JsonObjectGroup,
}

fn json_property_to_ir(prop: JsonProperty) -> Result<Option<(String, PropertyValue)>, MapError> {
    let JsonProperty { name, kind, value } = prop;

    let parsed = match kind.as_deref() {
        Some("bool") => value.as_bool().map(PropertyValue::Bool),
        Some("int") | Some("object") => value.as_i64().map(PropertyValue::I64),
        Some("float") => value.as_f64().map(|n| PropertyValue::F32(n as f32)),
        Some("string") | Some("file") | Some("color") | Some("class") => {
            value.as_str().map(|s| PropertyValue::String(s.to_owned()))
        }
        Some(other) => {
            return Err(MapError::UnsupportedPropertyType {
                name,
                kind: other.to_owned(),
            });
        }
        None => {
            if let Some(v) = value.as_bool() {
                Some(PropertyValue::Bool(v))
            } else if let Some(v) = value.as_i64() {
                Some(PropertyValue::I64(v))
            } else if let Some(v) = value.as_f64() {
                Some(PropertyValue::F32(v as f32))
            } else {
                value.as_str().map(|s| PropertyValue::String(s.to_owned()))
            }
        }
    };

    Ok(parsed.map(|value| (name, value)))
}

fn properties_from_json(props: Vec<JsonProperty>) -> Result<Properties, MapError> {
    let mut out = Properties::new();
    for p in props {
        if let Some((name, value)) = json_property_to_ir(p)? {
            out.insert(name, value);
        }
    }
    Ok(out)
}

fn object_to_ir(obj: JsonObject) -> Result<IrObject, MapError> {
    let shape = if let Some(gid) = obj.gid {
        IrObjectShape::Tile { gid }
    } else if obj.point {
        IrObjectShape::Point
    } else if !obj.polygon.is_empty() {
        IrObjectShape::Polygon(obj.polygon.into_iter().map(|p| vec2(p.x, p.y)).collect())
    } else if !obj.polyline.is_empty() {
        IrObjectShape::Polyline(obj.polyline.into_iter().map(|p| vec2(p.x, p.y)).collect())
    } else {
        IrObjectShape::Rectangle
    };

    let class_name = if !obj.class.is_empty() {
        obj.class
    } else {
        obj.kind
    };

    Ok(IrObject {
        id: obj.id,
        name: obj.name,
        class_name,
        x: obj.x,
        y: obj.y,
        width: obj.width,
        height: obj.height,
        rotation: obj.rotation,
        visible: obj.visible,
        shape,
        properties: properties_from_json(obj.properties)?,
    })
}

pub fn decode_map_file_to_ir(path: &str) -> Result<(IrMap, PathBuf), MapError> {
    let p = Path::new(path);
    if p.extension().and_then(|e| e.to_str()) != Some("json") {
        return Err(MapError::InvalidMap(format!(
            "Map file must be a JSON file: {path}"
        )));
    }

    let txt = std::fs::read_to_string(p).map_err(|source| MapError::Io {
        path: p.to_path_buf(),
        source,
    })?;
    let j: JsonMap = serde_json::from_str(&txt).map_err(|source| MapError::Json {
        path: p.to_path_buf(),
        source,
    })?;

    let map_dir = p
        .parent()
        .map(|d| d.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("./"));

    // Build IR tilesets
    let mut ir_tilesets = Vec::with_capacity(j.tilesets.len());
    for ts in &j.tilesets {
        if !ts.source.ends_with(".json") {
            return Err(MapError::InvalidMap(format!(
                "External tileset must be JSON: {}",
                ts.source
            )));
        }
        let ts_path = map_dir.join(&ts.source);
        let ext_txt = std::fs::read_to_string(&ts_path).map_err(|source| MapError::Io {
            path: ts_path.clone(),
            source,
        })?;
        let ext: ExternalTileset =
            serde_json::from_str(&ext_txt).map_err(|source| MapError::Json {
                path: ts_path,
                source,
            })?;

        // (We keep image path relative; Map::from_ir will join with map_dir)
        ir_tilesets.push(IrTileset::Atlas {
            first_gid: ts.firstgid,
            image: ext.image,
            tile_w: ext.tilewidth,
            tile_h: ext.tileheight,
            tilecount: ext.tilecount,
            columns: ext.columns,
            spacing: ext.spacing,
            margin: ext.margin,
            properties: properties_from_json(ext.properties)?,
            tiles: ext
                .tiles
                .into_iter()
                .map(|tile| -> Result<IrTileMetadata, MapError> {
                    Ok(IrTileMetadata {
                        id: tile.id,
                        properties: properties_from_json(tile.properties)?,
                        objects: tile
                            .objectgroup
                            .objects
                            .into_iter()
                            .map(object_to_ir)
                            .collect::<Result<Vec<_>, _>>()?,
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        });
    }

    // Sort by first_gid to make LUT building trivial
    ir_tilesets.sort_by_key(|t| match t {
        IrTileset::Atlas { first_gid, .. } => *first_gid,
    });

    let max_gid = ir_tilesets
        .iter()
        .map(|t| match t {
            IrTileset::Atlas {
                first_gid,
                tilecount,
                ..
            } => first_gid + tilecount - 1,
        })
        .max()
        .unwrap_or(0);

    // Build IR layers
    let mut ir_layers = Vec::with_capacity(j.layers.len());
    for l in j.layers {
        let layer_name = l.name.clone();
        let properties = properties_from_json(l.properties)?;
        let layer_kind = match l.kind.as_deref().unwrap_or("tilelayer") {
            "tilelayer" => {
                for &raw_gid in &l.data {
                    let gid = raw_gid & crate::spatial::GID_MASK;
                    if gid != 0 && gid > max_gid {
                        return Err(MapError::InvalidTileGid {
                            layer: layer_name.clone(),
                            gid,
                            max_gid,
                        });
                    }
                }
                IrLayerKind::Tiles {
                    width: l.width,
                    height: l.height,
                    data: l.data,
                }
            }
            "objectgroup" => IrLayerKind::Objects {
                objects: l
                    .objects
                    .into_iter()
                    .map(|obj| {
                        if let Some(raw_gid) = obj.gid {
                            let gid = raw_gid & crate::spatial::GID_MASK;
                            if gid == 0 || gid > max_gid {
                                return Err(MapError::InvalidObjectGid {
                                    layer: layer_name.clone(),
                                    object_id: obj.id,
                                    gid,
                                    max_gid,
                                });
                            }
                        }
                        object_to_ir(obj)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            },
            _ => IrLayerKind::Unsupported,
        };
        ir_layers.push(IrLayer {
            name: l.name,
            visible: l.visible,
            opacity: l.opacity,
            offset: vec2(l.offsetx, l.offsety),
            properties,
            kind: layer_kind,
        });
    }

    Ok((
        IrMap {
            tile_w: j.tilewidth,
            tile_h: j.tileheight,
            properties: properties_from_json(j.properties)?,
            tilesets: ir_tilesets,
            layers: ir_layers,
        },
        map_dir,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock went backwards")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("mq_tiled_props_{nanos}"));
        fs::create_dir_all(&dir).expect("failed to create temp dir");
        dir
    }

    #[test]
    fn parses_properties_for_map_layer_object_tileset_and_tile() {
        let dir = temp_dir();
        let map_path = dir.join("map.json");
        let ts_path = dir.join("tileset.json");

        let map_json = r#"{
          "tilewidth": 16,
          "tileheight": 16,
          "properties": [
            {"name":"is_night","type":"bool","value":true},
            {"name":"gravity","type":"float","value":9.8},
            {"name":"theme","type":"string","value":"forest"}
          ],
          "layers": [
            {
              "type":"tilelayer",
              "name":"ground",
              "width":2,
              "height":2,
              "data":[1,0,0,0],
              "properties":[
                {"name":"is_solid","type":"bool","value":true},
                {"name":"difficulty","type":"int","value":3}
              ]
            },
            {
              "type":"objectgroup",
              "name":"spawns",
              "objects":[
                {
                  "id": 7,
                  "name":"spawn_1",
                  "type":"spawn",
                  "properties":[{"name":"kind","type":"string","value":"player"}]
                }
              ],
              "properties":[{"name":"enabled","type":"bool","value":true}]
            }
          ],
          "tilesets":[{"firstgid":1,"source":"tileset.json"}]
        }"#;

        let tileset_json = r#"{
          "tilewidth":16,
          "tileheight":16,
          "tilecount":4,
          "columns":2,
          "image":"tiles.png",
          "properties":[{"name":"biome","type":"string","value":"forest"}],
          "tiles":[
            {
              "id":0,
              "properties":[{"name":"damage","type":"int","value":10}],
              "objectgroup":{
                "objects":[
                  {"id":1,"name":"hitbox","type":"shape","properties":[{"name":"sensor","type":"bool","value":false}]}
                ]
              }
            }
          ]
        }"#;

        fs::write(&map_path, map_json).expect("failed to write map");
        fs::write(&ts_path, tileset_json).expect("failed to write tileset");

        let (ir, _) = decode_map_file_to_ir(map_path.to_str().expect("path utf8")).expect("decode");

        assert_eq!(ir.properties.get_bool("is_night"), Some(true));
        assert_eq!(ir.properties.get_f32("gravity"), Some(9.8));
        assert_eq!(ir.properties.get_string("theme"), Some("forest"));

        assert_eq!(ir.layers[0].properties.get_bool("is_solid"), Some(true));
        assert_eq!(ir.layers[0].properties.get_i32("difficulty"), Some(3));

        match &ir.layers[1].kind {
            IrLayerKind::Objects { objects } => {
                assert_eq!(objects.len(), 1);
                assert_eq!(objects[0].properties.get_string("kind"), Some("player"));
            }
            _ => panic!("expected object layer"),
        }

        match &ir.tilesets[0] {
            IrTileset::Atlas {
                properties, tiles, ..
            } => {
                assert_eq!(properties.get_string("biome"), Some("forest"));
                assert_eq!(tiles.len(), 1);
                assert_eq!(tiles[0].properties.get_i32("damage"), Some(10));
                assert_eq!(tiles[0].objects.len(), 1);
                assert_eq!(
                    tiles[0].objects[0].properties.get_bool("sensor"),
                    Some(false)
                );
            }
        }
    }

    #[test]
    fn keeps_large_int_property_values() {
        let dir = temp_dir();
        let map_path = dir.join("map.json");
        let ts_path = dir.join("tileset.json");

        let map_json = r#"{
          "tilewidth": 16,
          "tileheight": 16,
          "properties": [
            {"name":"big_id","type":"object","value":5000000000}
          ],
          "layers": [],
          "tilesets":[{"firstgid":1,"source":"tileset.json"}]
        }"#;

        let tileset_json = r#"{
          "tilewidth":16,
          "tileheight":16,
          "tilecount":1,
          "columns":1,
          "image":"tiles.png"
        }"#;

        fs::write(&map_path, map_json).expect("failed to write map");
        fs::write(&ts_path, tileset_json).expect("failed to write tileset");

        let (ir, _) = decode_map_file_to_ir(map_path.to_str().expect("path utf8")).expect("decode");
        assert_eq!(ir.properties.get_i64("big_id"), Some(5_000_000_000));
        assert_eq!(ir.properties.get_i32("big_id"), None);
    }

    #[test]
    fn returns_typed_error_for_malformed_json() {
        let dir = temp_dir();
        let map_path = dir.join("map.json");
        fs::write(&map_path, "{ not json").expect("failed to write map");

        let err = decode_map_file_to_ir(map_path.to_str().expect("path utf8"))
            .err()
            .expect("expected decode error");
        assert!(matches!(err, MapError::Json { .. }));
    }

    #[test]
    fn returns_typed_error_for_missing_tileset_file() {
        let dir = temp_dir();
        let map_path = dir.join("map.json");
        let map_json = r#"{
          "tilewidth": 16,
          "tileheight": 16,
          "layers": [],
          "tilesets":[{"firstgid":1,"source":"missing_tileset.json"}]
        }"#;
        fs::write(&map_path, map_json).expect("failed to write map");

        let err = decode_map_file_to_ir(map_path.to_str().expect("path utf8"))
            .err()
            .expect("expected decode error");
        assert!(matches!(err, MapError::Io { .. }));
    }

    #[test]
    fn returns_typed_error_for_invalid_gid_reference() {
        let dir = temp_dir();
        let map_path = dir.join("map.json");
        let ts_path = dir.join("tileset.json");

        let map_json = r#"{
          "tilewidth": 16,
          "tileheight": 16,
          "layers": [
            {
              "type":"tilelayer",
              "name":"ground",
              "width":1,
              "height":1,
              "data":[99]
            }
          ],
          "tilesets":[{"firstgid":1,"source":"tileset.json"}]
        }"#;

        let tileset_json = r#"{
          "tilewidth":16,
          "tileheight":16,
          "tilecount":1,
          "columns":1,
          "image":"tiles.png"
        }"#;

        fs::write(&map_path, map_json).expect("failed to write map");
        fs::write(&ts_path, tileset_json).expect("failed to write tileset");

        let err = decode_map_file_to_ir(map_path.to_str().expect("path utf8"))
            .err()
            .expect("expected decode error");
        assert!(matches!(err, MapError::InvalidTileGid { .. }));
    }

    #[test]
    fn returns_typed_error_for_unknown_property_type() {
        let dir = temp_dir();
        let map_path = dir.join("map.json");
        let ts_path = dir.join("tileset.json");

        let map_json = r#"{
          "tilewidth": 16,
          "tileheight": 16,
          "properties": [
            {"name":"mystery","type":"not_supported","value":"x"}
          ],
          "layers": [],
          "tilesets":[{"firstgid":1,"source":"tileset.json"}]
        }"#;

        let tileset_json = r#"{
          "tilewidth":16,
          "tileheight":16,
          "tilecount":1,
          "columns":1,
          "image":"tiles.png"
        }"#;

        fs::write(&map_path, map_json).expect("failed to write map");
        fs::write(&ts_path, tileset_json).expect("failed to write tileset");

        let err = decode_map_file_to_ir(map_path.to_str().expect("path utf8"))
            .err()
            .expect("expected decode error");
        assert!(matches!(err, MapError::UnsupportedPropertyType { .. }));
    }
}
