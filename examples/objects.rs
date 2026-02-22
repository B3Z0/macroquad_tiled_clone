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
    let max_frames = std::env::var("MQ_FRAMES")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    let mut frame_count = 0u64;

    println!("object_layers={}", map.object_layers().len());
    println!("objects={}", map.objects().count());

    let screen_size = Vec2::new(screen_width(), screen_height());

    loop {
        clear_background(BLACK);
        let stamp = map.next_frame_stamp();

        map.draw_visible_rect(Vec2::ZERO, screen_size);
        map.draw_objects_tiles_with_stamp(Vec2::ZERO, screen_size, stamp);
        map.draw_objects_debug_with_stamp(Vec2::ZERO, screen_size, stamp);

        draw_text("objects example", 20.0, 30.0, 32.0, WHITE);
        next_frame().await;
        frame_count += 1;
        if let Some(max) = max_frames {
            if frame_count >= max {
                break;
            }
        }
    }
}
