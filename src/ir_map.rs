// src/ir.rs
use macroquad::prelude::*;

/// Canonical, format-agnostic map.
pub struct IrMap {
    pub tile_w: u32,
    pub tile_h: u32,
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
    },
    // (later) ImagePerTile { first_gid, tiles: Vec<IrTileImage> },
}

pub enum IrLayerKind {
    Tiles {
        width: usize,
        height: usize,
        data: Vec<u32>, // raw GIDs (including flip flags ok)
    },
    // (later) Objects { ... }, Image { ... }
}

pub struct IrLayer {
    pub name: String,
    pub visible: bool,
    pub opacity: f32,
    pub offset: Vec2, // world offset for this layer
    pub kind: IrLayerKind,
}
