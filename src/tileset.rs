use crate::tiled::RawTilesetDef;

#[derive(Debug, Clone)]
pub struct TileSet {
    pub name: String,
    pub first_gid: u32,
    pub columns: u32,
    pub tilewidth: u32,
    pub tileheight: u32,
    pub image: String,
}

impl TileSet {
    pub fn from_def(def: RawTilesetDef, first_gid: u32) -> Self {
        TileSet {
            name: def.name,
            first_gid,
            columns: def.columns,
            tilewidth: def.tilewidth,
            tileheight: def.tileheight,
            image: def.image,
        }
    }
}