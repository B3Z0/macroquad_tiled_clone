mod error;
mod layer;
mod tiled;
mod geom;
mod command;

pub use error::Error;
use nanoserde::DeJson;
pub use tiled::RawMap;
pub use layer::Layer;
pub use geom::{Rect, Vec2};
pub use command::{DrawCommand, TileRegion};

use macroquad::prelude::*;
use std::fs;
use std::path::Path;

/// Minimal tile map representation
#[derive(Debug)]
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
        if raw.width == 0 || raw.height == 0 {
            return Err(Error::InvalidLayerSize("<map>".to_string()));
        }
        if raw.layers.is_empty() {
            return Err(Error::NoLayer);
        }
        for layer in &raw.layers {
            if layer.data.len() != (raw.width * raw.height) as usize {
                return Err(Error::InvalidLayerSize(layer.name.clone()));
            }
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
        let path_ref = path.as_ref();
        let path_str = path_ref.display().to_string();
        let ext_opt = path_ref.extension().and_then(|e| e.to_str());

        match ext_opt {
            Some("json") => {
                let content = fs::read_to_string(path)?;
                Map::load_from_str(&content)
            }
            // Any other extension is considered unsupported
            Some(_) => Err(Error::UnsupportedFormat(path_str)),

            // If no extension, also unsupported
            None => Err(Error::UnsupportedFormat(path_str)),
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

                    let rect = Some (macroquad::prelude::Rect::new (
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




#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    // A set of JSON snippets for testing
    const VALID_JSON_SINGLE_LAYER: &str = r#"
    {
        "width": 2,
        "height": 2,
        "tilewidth": 8,
        "tileheight": 8,
        "layers": [
            { "name": "layer1", "data": [1, 0, 0, 1] }
        ]
    }
    "#;

    const VALID_JSON_MULTI_LAYER: &str = r#"
    {
        "width": 3,
        "height": 1,
        "tilewidth": 8,
        "tileheight": 8,
        "layers": [
            { "name": "bg", "data": [1, 1, 1] },
            { "name": "fg", "data": [0, 2, 0] }
        ]
    }
    "#;

    const EMPTY_LAYERS_JSON: &str = r#"
    {
        "width": 1,
        "height": 1,
        "tilewidth": 8,
        "tileheight": 8,
        "layers": []
    }
    "#;

    const MALFORMED_JSON: &str = "{ not valid json";

    #[test]
    fn load_valid_single_layer_json() {
        let map = Map::load_from_str(VALID_JSON_SINGLE_LAYER).expect("Should load valid single-layer JSON");
        assert_eq!(map.width, 2);
        assert_eq!(map.height, 2);
        assert_eq!(map.layers.len(), 1);
        let layer = &map.layers[0];
        assert_eq!(layer.name, "layer1");
        assert_eq!(layer.data, vec![1, 0, 0, 1]);
    }

    #[test]
    fn load_valid_multi_layer_json() {
        let map = Map::load_from_str(VALID_JSON_MULTI_LAYER).expect("Should load valid multi-layer JSON");
        assert_eq!(map.width, 3);
        assert_eq!(map.height, 1);
        assert_eq!(map.layers.len(), 2);
        assert_eq!(map.layers[0].name, "bg");
        assert_eq!(map.layers[1].name, "fg");
    }

    #[test]
    fn error_on_empty_layers() {
        let err = Map::load_from_str(EMPTY_LAYERS_JSON).unwrap_err();
        assert!(matches!(err, Error::NoLayer));
    }

    #[test]
    fn error_on_malformed_json() {
        let err = Map::load_from_str(MALFORMED_JSON).unwrap_err();
        assert!(matches!(err, Error::Parse(_)));
    }

    #[test]
    fn load_from_file_valid_json() {
        // Write a temporary JSON file
        let path = Path::new("test_map.json");
        fs::write(&path, VALID_JSON_SINGLE_LAYER).expect("Failed to write temp JSON");

        let map = Map::load_from_file(&path).expect("Should load map from file");
        assert_eq!(map.width, 2);
        assert_eq!(map.layers.len(), 1);

        // Clean up
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn error_on_unsupported_extension() {
        let err = Map::load_from_file("level.tmx").unwrap_err();
        assert!(matches!(err, Error::UnsupportedFormat(ext) if ext == "level.tmx"));
    }

    #[test]
    fn error_on_missing_file() {
        let err = Map::load_from_file("nonexistent.json").unwrap_err();
        assert!(matches!(err, Error::Io(_)));
    }
}
