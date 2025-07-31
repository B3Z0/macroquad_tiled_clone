
use macroquad::prelude::*;
use macroquad_tiled_clone::Map;

#[macroquad::main("Basic Map")]
async fn main() {
    // Load the map JSON via new helper (or include_str!)
    let map = Map::load_from_file("assets/map.json").expect("Failed to load map");

    // Load the tileset texture
    let texture: Texture2D = load_texture("assets/tileset.png").await.unwrap();
    texture.set_filter(FilterMode::Nearest);

    loop {
        clear_background(WHITE);
        map.draw(&texture);
        next_frame().await;
    }
}
