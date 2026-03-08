use crate::core::MapData;
use crate::MapError;
use macroquad::prelude::*;
use std::path::Path;

pub(crate) struct MacroquadTilesetAsset {
    pub(crate) first_gid: u32,
    #[allow(dead_code)]
    pub(crate) tilecount: u32,
    pub(crate) cols: u32,
    pub(crate) tex: Texture2D,
    pub(crate) tile_w: u32,
    pub(crate) tile_h: u32,
    pub(crate) spacing: u32,
    pub(crate) margin: u32,
}

pub(crate) struct MacroquadRenderAssets {
    pub(crate) tilesets: Vec<MacroquadTilesetAsset>,
}

impl MacroquadRenderAssets {
    pub(crate) async fn from_data(data: &MapData, base_dir: &Path) -> Result<Self, MapError> {
        let mut tilesets = Vec::with_capacity(data.tile_state.authored.tileset_runtime_info.len());

        for ts in &data.tile_state.authored.tileset_runtime_info {
            let img_path = base_dir.join(&ts.image);
            let img_path_str = img_path
                .to_str()
                .ok_or_else(|| MapError::InvalidUtf8Path(img_path.clone()))?;
            let tex = load_texture(img_path_str)
                .await
                .map_err(|e| MapError::TextureLoad {
                    path: img_path.clone(),
                    message: e.to_string(),
                })?;
            tex.set_filter(FilterMode::Nearest);

            tilesets.push(MacroquadTilesetAsset {
                first_gid: ts.first_gid,
                tilecount: ts.tilecount,
                cols: ts.cols,
                tex,
                tile_w: ts.tile_w,
                tile_h: ts.tile_h,
                spacing: ts.spacing,
                margin: ts.margin,
            });
        }

        Ok(Self { tilesets })
    }
}
