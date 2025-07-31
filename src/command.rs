use crate::geom::{Rect, Vec2};

pub struct DrawCommand {
    pub layer_index: usize,
    pub tileset_index: usize,
    pub src: Rect,
    pub dest: Vec2,
}

pub struct TileRegion {
    pub start_x: u32,
    pub start_y: u32,
    pub width: u32,
    pub height: u32,
}