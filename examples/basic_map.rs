use macroquad::prelude::*;
use macroquad_tiled_clone::map::Map;

// ❶ Override the default 800 × 450 pixels here
fn window_conf() -> Conf {
    Conf {
        window_title: "Basic Map".into(),
        window_width: 1280,     // ← any size you like
        window_height: 720,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]     // ❷ pass the window config function here
async fn main() {
    let map = Map::load_basic("assets2/map.json")
        .await
        .expect("Failed to load map");

    loop {
        clear_background(BLACK);
        map.draw();
        next_frame().await;
    }
}
