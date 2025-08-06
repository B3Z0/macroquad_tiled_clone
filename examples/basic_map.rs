use macroquad::prelude::*;
use macroquad_tiled_clone::map::{self, Map};

// ❶ Override the default 800 × 450 pixels here
fn window_conf() -> Conf {
    Conf {
        window_title: "Basic Map".into(),
        window_width: 1280,     // ← any size you like
        window_height: 720,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]     // ❷ pass the conf fn here
async fn main() {
    // everything else stays the same
    let map = Map::load_basic("assets/map.json").expect("load");
    let tex = load_texture("assets/tileset.png").await.unwrap();
    tex.set_filter(FilterMode::Nearest);

    loop {
        clear_background(BLACK);
        map.draw(&tex);          // your draw already uses world coords
        next_frame().await;
    }
}
