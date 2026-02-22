use macroquad::prelude::*;
use macroquad_tiled_clone::map::Map;

fn window_conf() -> Conf {
    Conf {
        window_title: "Objects Example".into(),
        window_width: 1280,
        window_height: 720,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut map = Map::load("assets2/map.json")
        .await
        .expect("Failed to load map");

    println!("object_layers={}", map.object_layers().len());
    println!("objects={}", map.objects().count());

    let screen_size = Vec2::new(screen_width(), screen_height());

    loop {
        clear_background(BLACK);

        map.draw_visible_rect(Vec2::ZERO, screen_size);
        map.draw_objects_tiles(Vec2::ZERO, screen_size);
        map.draw_objects_debug(Vec2::ZERO, screen_size);

        draw_text("objects example", 20.0, 30.0, 32.0, WHITE);
        next_frame().await;
    }
}
