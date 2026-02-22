// src/ir.rs
use macroquad::prelude::*;
use std::collections::HashMap;

/// Supported property value types parsed from Tiled JSON.
#[derive(Clone, Debug, PartialEq)]
pub enum PropertyValue {
    /// Boolean property.
    Bool(bool),
    /// 64-bit integer property.
    I64(i64),
    /// 32-bit float property.
    F32(f32),
    /// String-like property (`string`, `file`, `color`, `class`).
    String(String),
}

/// Property map attached to map/layer/object/tileset/tile entities.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Properties(HashMap<String, PropertyValue>);

impl Properties {
    /// Creates an empty property map.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts/overwrites a property.
    pub fn insert(&mut self, key: String, value: PropertyValue) {
        self.0.insert(key, value);
    }

    /// Returns raw property value by key.
    pub fn get(&self, key: &str) -> Option<&PropertyValue> {
        self.0.get(key)
    }

    /// Gets a boolean property.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        match self.0.get(key) {
            Some(PropertyValue::Bool(v)) => Some(*v),
            _ => None,
        }
    }

    /// Gets an integer property as `i32` if it fits.
    pub fn get_i32(&self, key: &str) -> Option<i32> {
        match self.0.get(key) {
            Some(PropertyValue::I64(v)) => i32::try_from(*v).ok(),
            _ => None,
        }
    }

    /// Gets an integer property as `i64`.
    pub fn get_i64(&self, key: &str) -> Option<i64> {
        match self.0.get(key) {
            Some(PropertyValue::I64(v)) => Some(*v),
            _ => None,
        }
    }

    /// Gets a float property.
    pub fn get_f32(&self, key: &str) -> Option<f32> {
        match self.0.get(key) {
            Some(PropertyValue::F32(v)) => Some(*v),
            _ => None,
        }
    }

    /// Gets a string property.
    pub fn get_string(&self, key: &str) -> Option<&str> {
        match self.0.get(key) {
            Some(PropertyValue::String(v)) => Some(v.as_str()),
            _ => None,
        }
    }
}

/// Parsed Tiled object shape kind.
#[derive(Clone, Debug, PartialEq)]
pub enum IrObjectShape {
    /// Axis-aligned rectangle object.
    Rectangle,
    /// Point object.
    Point,
    /// Closed polygon in object-local coordinates.
    Polygon(Vec<Vec2>),
    /// Open polyline in object-local coordinates.
    Polyline(Vec<Vec2>),
    /// Tile object referencing a tileset GID.
    Tile {
        /// Global tile id (may include Tiled flip flags in raw JSON).
        gid: u32,
    },
}

/// Parsed object record from an object layer (or tile objectgroup metadata).
#[derive(Clone, Debug, PartialEq)]
pub struct IrObject {
    /// Tiled object id.
    pub id: u32,
    /// Object name.
    pub name: String,
    /// Object class/type name.
    pub class_name: String,
    /// X position in world/layer coordinates (pixels).
    pub x: f32,
    /// Y position in world/layer coordinates (pixels).
    pub y: f32,
    /// Object width (pixels).
    pub width: f32,
    /// Object height (pixels).
    pub height: f32,
    /// Rotation in degrees.
    pub rotation: f32,
    /// Visibility flag.
    pub visible: bool,
    /// Object shape kind/data.
    pub shape: IrObjectShape,
    /// Custom object properties.
    pub properties: Properties,
}

/// Per-tile metadata parsed from a tileset.
#[derive(Clone, Debug, PartialEq)]
pub struct IrTileMetadata {
    /// Local tile id within the tileset.
    pub id: u32,
    /// Tile-level properties.
    pub properties: Properties,
    /// Tile-local objectgroup objects (if present).
    pub objects: Vec<IrObject>,
}

/// Canonical, format-agnostic map.
pub struct IrMap {
    /// Map tile width (pixels).
    pub tile_w: u32,
    /// Map tile height (pixels).
    pub tile_h: u32,
    /// Map-level properties.
    pub properties: Properties,
    /// Parsed tilesets sorted by `first_gid`.
    pub tilesets: Vec<IrTileset>,
    /// Parsed layers in draw order (Tiled array order).
    pub layers: Vec<IrLayer>,
}

/// Parsed tileset representation.
pub enum IrTileset {
    /// One image atlas with a regular grid.
    Atlas {
        /// First global tile id assigned to this tileset.
        first_gid: u32,
        /// Tileset image path.
        image: String,
        /// Tile width (pixels).
        tile_w: u32,
        /// Tile height (pixels).
        tile_h: u32,
        /// Number of tiles in atlas.
        tilecount: u32,
        /// Atlas column count.
        columns: u32,
        /// Pixel spacing between tiles.
        spacing: u32, // 0 if not used
        /// Pixel margin around atlas.
        margin: u32, // 0 if not used
        /// Tileset-level properties.
        properties: Properties,
        /// Optional per-tile metadata.
        tiles: Vec<IrTileMetadata>,
    },
    // (later) ImagePerTile { first_gid, tiles: Vec<IrTileImage> },
}

/// Parsed layer payload kind.
pub enum IrLayerKind {
    /// Finite tile layer.
    Tiles {
        /// Width in tiles.
        width: usize,
        /// Height in tiles.
        height: usize,
        /// Raw tile gids (may include Tiled flip flags).
        data: Vec<u32>,
    },
    /// Object layer.
    Objects {
        /// Parsed objects.
        objects: Vec<IrObject>,
    },
    /// Unsupported layer kind, preserved as skipped.
    Unsupported,
    // (later) Objects { ... }, Image { ... }
}

/// Parsed layer entry in map draw order.
pub struct IrLayer {
    /// Layer name.
    pub name: String,
    /// Layer visibility.
    pub visible: bool,
    /// Layer opacity (`0.0..=1.0`).
    pub opacity: f32,
    /// Layer world offset (pixels).
    pub offset: Vec2,
    /// Layer properties.
    pub properties: Properties,
    /// Layer payload.
    pub kind: IrLayerKind,
}
