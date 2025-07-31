// tests/integration_example.rs

use std::path::PathBuf;
use macroquad_tiled_clone::Map;

#[test]
fn example_load_assets() {
    let mut assets = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    assets.push("assets");
    assets.push("map.json");
    // Should not panic
    let _ = Map::load_from_file(&assets).expect("Example assets should load");
}
