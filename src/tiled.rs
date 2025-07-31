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
}