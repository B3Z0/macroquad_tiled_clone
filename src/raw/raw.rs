pub struct RawMap {
    pub width: u32,
    pub height: u32,
    pub tilewidth: u32,
    pub tileheight: u32,
    pub layers: Vec<RawLayer>,
    pub tilesets: Vec<RawTilesetRef>,
}