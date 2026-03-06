use macroquad::prelude::*;
use macroquad_tiled_clone::Map;

fn window_conf() -> Conf {
    Conf {
        window_title: "Split Screen Map".into(),
        window_width: 1280,
        window_height: 720,
        ..Default::default()
    }
}

fn highlight_camera_bounds(view_rect: Rect, color: Color, thickness: f32) {
    draw_rectangle_lines(
        view_rect.x,
        view_rect.y,
        view_rect.w,
        view_rect.h,
        thickness,
        color,
    );
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut map = Map::load("assets2/map.json")
        .await
        .expect("Failed to load map");

    // Keep one-chunk default culling buffer for stable edges.
    map.set_cull_padding(256.0);

    let max_frames = std::env::var("MQ_FRAMES")
        .ok()
        .and_then(|s| s.parse::<u64>().ok());
    let mut frame_count = 0u64;
    let debug_camera_bounds = true;

    // Demo player positions in world space.
    let mut p1 = vec2(300.0, 300.0);
    let mut p2 = vec2(700.0, 500.0);
    // Keep the same world presentation as the original 1280x720 example (16:9).
    // Each split viewport is letterboxed to 16:9 to avoid stretching.
    let cam_world_size = vec2(640.0, 360.0);

    loop {
        clear_background(BLACK);

        // WASD controls player 1 camera target.
        if is_key_down(KeyCode::A) {
            p1.x -= 2.0;
        }
        if is_key_down(KeyCode::D) {
            p1.x += 2.0;
        }
        if is_key_down(KeyCode::W) {
            p1.y -= 2.0;
        }
        if is_key_down(KeyCode::S) {
            p1.y += 2.0;
        }

        // Arrow keys control player 2 camera target.
        if is_key_down(KeyCode::Left) {
            p2.x -= 2.0;
        }
        if is_key_down(KeyCode::Right) {
            p2.x += 2.0;
        }
        if is_key_down(KeyCode::Up) {
            p2.y -= 2.0;
        }
        if is_key_down(KeyCode::Down) {
            p2.y += 2.0;
        }

        let sw = screen_width() as i32;
        let sh = screen_height() as i32;
        let half_w = sw / 2;

        // Letterbox each half to 16:9 so pixels are not stretched.
        let target_h = (half_w * 9) / 16;
        let vp_h = target_h.min(sh);
        let vp_y = (sh - vp_h) / 2;

        let left_viewport = (0, vp_y, half_w, vp_h);
        let right_viewport = (half_w, vp_y, half_w, vp_h);

        // Left viewport (player 1).
        let left_rect = Rect::new(
            p1.x - cam_world_size.x * 0.5,
            p1.y - cam_world_size.y * 0.5,
            cam_world_size.x,
            cam_world_size.y,
        );
        let cam1 = Camera2D {
            target: p1,
            // Positive y zoom here keeps world y-down with screen rendering.
            zoom: vec2(2.0 / cam_world_size.x, 2.0 / cam_world_size.y),
            viewport: Some(left_viewport),
            ..Default::default()
        };

        set_camera(&cam1);
        map.draw(left_rect.point(), left_rect.point() + left_rect.size());
        if debug_camera_bounds {
            highlight_camera_bounds(left_rect, RED, 3.0);
        }

        // Right viewport (player 2).
        let right_rect = Rect::new(
            p2.x - cam_world_size.x * 0.5,
            p2.y - cam_world_size.y * 0.5,
            cam_world_size.x,
            cam_world_size.y,
        );
        let cam2 = Camera2D {
            target: p2,
            zoom: vec2(2.0 / cam_world_size.x, 2.0 / cam_world_size.y),
            viewport: Some(right_viewport),
            ..Default::default()
        };

        set_camera(&cam2);
        map.draw(right_rect.point(), right_rect.point() + right_rect.size());
        if debug_camera_bounds {
            highlight_camera_bounds(right_rect, RED, 3.0);
        }

        // UI in screen space.
        set_default_camera();
        draw_line(sw as f32 / 2.0, 0.0, sw as f32 / 2.0, sh as f32, 2.0, WHITE);
        draw_text("P1: WASD | P2: Arrows", 20.0, 30.0, 28.0, WHITE);
        draw_text(&format!("FPS: {}", get_fps()), 20.0, 62.0, 28.0, RED);

        next_frame().await;
        frame_count += 1;
        if let Some(max) = max_frames {
            if frame_count >= max {
                break;
            }
        }
    }
}
