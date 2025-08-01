use nanoserde::DeJson;

#[derive(DeJson)]
pub struct RawLayer {
    pub name: String,
    pub data: Vec<u32>,
}

#[derive(DeJson)]
pub struct RawMap {
    pub width: u32,
    pub height: u32,
    pub tilewidth: u32,
    pub tileheight: u32,
    pub layers: Vec<RawLayer>,
    pub tilesets: Vec<RawTilesetRef>,
}

#[derive(DeJson)]
pub struct RawTilesetRef {
    pub firstgid: u32,
    pub source: String,
}

#[derive(DeJson)]
pub struct RawTilesetDef {
    pub name: String,
    pub columns: u32,
    pub tilewidth: u32,
    pub tileheight: u32,
    pub image: String,
}