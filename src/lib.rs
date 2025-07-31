mod error;
mod layer;
mod tiled;

pub use error::Error;
use nanoserde::DeJson;
pub use tiled::RawMap;
pub use layer::Layer;

use macroquad::prelude::*;
use std::fs;
use std::path::Path;

/// Minimal tile map representation
pub struct Map {
    pub width: u32,
    pub height: u32,
    pub tilewidth: u32,
    pub tileheight: u32,
    pub layers: Vec<Layer>,
}

impl Map {
    /// Load a Tiled JSON map from a string
    pub fn load_from_str(json: &str) -> Result<Self, Error> {
        let raw: RawMap = RawMap::deserialize_json(json)?;
        if raw.layers.is_empty() {
            return Err(Error::NoLayer);
        }

        let layers = raw
            .layers
            .into_iter()
            .map(|raw_layer| Layer::from_raw(raw_layer))
            .collect();

        Ok(Self {
            width: raw.width,
            height: raw.height,
            tilewidth: raw.tilewidth,
            tileheight: raw.tileheight,
            layers,
        })
    }

    /// Load a map from a file path, only supporting JSON for now
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let path = path.as_ref();
        match path.extension().and_then(|e| e.to_str()) {
            Some("json") => {
                let content = fs::read_to_string(path)?;
                Map::load_from_str(&content)
            }
            Some(ext) => Err(Error::UnsupportedFormat(ext.to_string())),
            None => Err(Error::UnsupportedFormat(String::new())),
        }
    }

    /// Draw all tiles using a single tileset texture
    pub fn draw(&self, texture: &Texture2D) {
        let cols = texture.width() as u32 / self.tilewidth;
        for layer in &self.layers {
            for y in 0..self.height {
                for x in 0..self.width {
                    // gid is the global ID of the tile
                    let gid = layer.data[(y * self.width + x) as usize]; // get the GID 
                    if gid == 0 {
                        continue;
                    }
                    
                    let idx = gid - 1;
                    let sx = (idx % cols) * self.tilewidth;
                    let sy = (idx / cols) * self.tileheight;

                    let rect = Some (Rect::new (
                        sx as f32,
                        sy as f32,
                        self.tilewidth as f32,
                        self.tileheight as f32,
                    ));

                    draw_texture_ex(
                        texture,
                        x as f32 * self.tilewidth as f32,
                        y as f32 * self.tileheight as f32,
                        WHITE,
                        DrawTextureParams {
                            source: rect,
                            ..Default::default()
                        },
                )
                }
            }
        }
    }
}
