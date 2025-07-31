use crate::tiled::RawLayer;

pub struct Layer {
    pub name: String,
    pub data: Vec<u32>,
}

impl Layer {
    pub fn from_raw(raw: RawLayer) -> Self {
        Layer {
            name: raw.name,
            data: raw.data,
        }
    }
}
