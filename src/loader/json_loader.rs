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

fn validate_map_tile_dimensions(tile_w: u32, tile_h: u32) -> Result<(), MapError> {
    if tile_w == 0 || tile_h == 0 {
        return Err(MapError::InvalidMap(format!(
            "Map tile dimensions must be non-zero (tilewidth={}, tileheight={})",
            tile_w, tile_h
        )));
    }
    Ok(())
}

fn validate_external_tileset(
    ts_source: &str,
    first_gid: u32,
    ext: &ExternalTileset,
) -> Result<(), MapError> {
    if ext.tilewidth == 0 || ext.tileheight == 0 {
        return Err(MapError::InvalidMap(format!(
            "Tileset '{}' has non-positive tile size (tilewidth={}, tileheight={})",
            ts_source, ext.tilewidth, ext.tileheight
        )));
    }
    if ext.tilecount == 0 {
        return Err(MapError::InvalidMap(format!(
            "Tileset '{}' has tilecount=0 (firstgid={})",
            ts_source, first_gid
        )));
    }
    if ext.columns == 0 {
        return Err(MapError::InvalidMap(format!(
            "Tileset '{}' has columns=0 (firstgid={})",
            ts_source, first_gid
        )));
    }

    Ok(())
}

fn validate_tile_layer_shape(layer_name: &str, l: &JsonLayer) -> Result<(), MapError> {
    if l.width == 0 || l.height == 0 {
        return Err(MapError::InvalidMap(format!(
            "Tile layer '{}' must have non-zero width/height (width={}, height={})",
            layer_name, l.width, l.height
        )));
    }

    let expected = l.width.checked_mul(l.height).ok_or_else(|| {
        MapError::InvalidMap(format!(
            "Tile layer '{}' dimensions overflow usize (width={}, height={})",
            layer_name, l.width, l.height
        ))
    })?;

    if l.data.len() != expected {
        return Err(MapError::InvalidMap(format!(
            "Tile layer '{}' data length mismatch: got {}, expected {} ({}x{})",
            layer_name,
            l.data.len(),
            expected,
            l.width,
            l.height
        )));
    }

    Ok(())
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
    validate_map_tile_dimensions(j.tilewidth, j.tileheight)?;

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
        validate_external_tileset(&ts.source, ts.firstgid, &ext)?;

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
        let layer_kind = match l.kind.as_deref().unwrap_or("tilelayer") {
            "tilelayer" => {
                validate_tile_layer_shape(&layer_name, &l)?;
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
        let properties = properties_from_json(l.properties)?;
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
include!("../../tests/unit/loader_json_tests.rs");
