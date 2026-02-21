// src/ir.rs
use macroquad::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum PropertyValue {
    Bool(bool),
    I64(i64),
    F32(f32),
    String(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Properties(HashMap<String, PropertyValue>);

impl Properties {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: String, value: PropertyValue) {
        self.0.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<&PropertyValue> {
        self.0.get(key)
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.0.get(key) {
            Some(PropertyValue::Bool(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn get_i32(&self, key: &str) -> Option<i32> {
        match self.0.get(key) {
            Some(PropertyValue::I64(v)) => i32::try_from(*v).ok(),
            _ => None,
        }
    }

    pub fn get_i64(&self, key: &str) -> Option<i64> {
        match self.0.get(key) {
            Some(PropertyValue::I64(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn get_f32(&self, key: &str) -> Option<f32> {
        match self.0.get(key) {
            Some(PropertyValue::F32(v)) => Some(*v),
            _ => None,
        }
    }

    pub fn get_string(&self, key: &str) -> Option<&str> {
        match self.0.get(key) {
            Some(PropertyValue::String(v)) => Some(v.as_str()),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum IrObjectShape {
    Rectangle,
    Point,
    Polygon(Vec<Vec2>),
    Polyline(Vec<Vec2>),
    Tile { gid: u32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct IrObject {
    pub id: u32,
    pub name: String,
    pub class_name: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub rotation: f32,
    pub visible: bool,
    pub shape: IrObjectShape,
    pub properties: Properties,
}

#[derive(Clone, Debug, PartialEq)]
pub struct IrTileMetadata {
    pub id: u32,
    pub properties: Properties,
    pub objects: Vec<IrObject>,
}

/// Canonical, format-agnostic map.
pub struct IrMap {
    pub tile_w: u32,
    pub tile_h: u32,
    pub properties: Properties,
    pub tilesets: Vec<IrTileset>, // must be sorted by first_gid
    pub layers: Vec<IrLayer>,     // draw order: array order
}

pub enum IrTileset {
    /// One image atlas with a regular grid.
    Atlas {
        first_gid: u32,
        image: String,
        tile_w: u32,
        tile_h: u32,
        tilecount: u32,
        columns: u32,
        spacing: u32, // 0 if not used
        margin: u32,  // 0 if not used
        properties: Properties,
        tiles: Vec<IrTileMetadata>,
    },
    // (later) ImagePerTile { first_gid, tiles: Vec<IrTileImage> },
}

pub enum IrLayerKind {
    Tiles {
        width: usize,
        height: usize,
        data: Vec<u32>, // raw GIDs (including flip flags ok)
    },
    Objects {
        objects: Vec<IrObject>,
    },
    Unsupported,
    // (later) Objects { ... }, Image { ... }
}

pub struct IrLayer {
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    pub offset: Vec2, // world offset for this layer
    pub properties: Properties,
    pub kind: IrLayerKind,
}
