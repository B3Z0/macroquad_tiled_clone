use macroquad::prelude::{vec2, Vec2};
use macroquad_tiled_clone::Map;

#[test]
fn stamp_overflow_does_not_hide_objects_or_break_dedupe() {
    let mut map = Map::__new_for_stamp_overflow_test(3);
    map.__set_frame_stamp_for_testing(u32::MAX - 1);

    map.draw(Vec2::ZERO, vec2(64.0, 64.0));
    assert_eq!(map.__frame_stamp_for_testing(), u32::MAX);
    assert_eq!(map.__seen_tiles_stamp_count_for_testing(0, u32::MAX), 3);

    map.draw(Vec2::ZERO, vec2(64.0, 64.0));
    assert_eq!(map.__frame_stamp_for_testing(), 1);
    assert_eq!(map.__seen_tiles_stamp_count_for_testing(0, 1), 3);
}
