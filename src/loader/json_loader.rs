// src/loader/json.rs
use crate::ir_map::*;
use anyhow::Context;
use macroquad::prelude::*;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
struct JsonLayer {
    data: Vec<u32>,
    width: usize,
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
}

pub fn decode_map_file_to_ir(path: &str) -> anyhow::Result<(IrMap, PathBuf)> {
    let p = Path::new(path);
    anyhow::ensure!(
        p.extension().and_then(|e| e.to_str()) == Some("json"),
        "Map file must be a JSON file: {path}"
    );

    let txt = std::fs::read_to_string(p).with_context(|| format!("Reading map file {path}"))?;
    let j: JsonMap =
        serde_json::from_str(&txt).with_context(|| format!("Parsing map file {path}"))?;

    let map_dir = p
        .parent()
        .map(|d| d.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("./"));

    // Build IR tilesets
    let mut ir_tilesets = Vec::with_capacity(j.tilesets.len());
    for ts in &j.tilesets {
        anyhow::ensure!(
            ts.source.ends_with(".json"),
            "External tileset must be JSON: {}",
            ts.source
        );
        let ext_txt = std::fs::read_to_string(map_dir.join(&ts.source))?;
        let ext: ExternalTileset = serde_json::from_str(&ext_txt)?;

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
        });
    }

    // Sort by first_gid to make LUT building trivial
    ir_tilesets.sort_by_key(|t| match t {
        IrTileset::Atlas { first_gid, .. } => *first_gid,
    });

    // Build IR layers (only tile layers for now)
    let mut ir_layers = Vec::with_capacity(j.layers.len());
    for l in j.layers {
        if l.kind.as_deref().unwrap_or("tilelayer") != "tilelayer" {
            // skip non-tiles for now
            continue;
        }
        ir_layers.push(IrLayer {
            name: l.name,
            visible: l.visible,
            opacity: l.opacity,
            offset: vec2(l.offsetx, l.offsety),
            kind: IrLayerKind::Tiles {
                width: l.width,
                height: l.height,
                data: l.data,
            },
        });
    }

    Ok((
        IrMap {
            tile_w: j.tilewidth,
            tile_h: j.tileheight,
            tilesets: ir_tilesets,
            layers: ir_layers,
        },
        map_dir,
    ))
}
