// tests/load_tests.rs

use std::fs;
use std::path::PathBuf;
use macroquad_tiled_clone::{Map, Error};

#[test]
fn integration_load_from_file_and_str() {
    // Inline JSON
    let json = r#"
    {
        "width": 1,
        "height": 1,
        "tilewidth": 4,
        "tileheight": 4,
        "layers": [ { "name": "L", "data": [0] } ]
    }
    "#;
    let map = Map::load_from_str(json).expect("should parse inline JSON");
    assert_eq!(map.width, 1);

    // File-based
    let mut path = PathBuf::from(std::env::temp_dir());
    path.push("test_map_integration.json");
    fs::write(&path, json).unwrap();
    let map2 = Map::load_from_file(&path).unwrap();
    assert_eq!(map2.tilewidth, 4);
    fs::remove_file(&path).unwrap();
}

#[test]
fn integration_unsupported_format() {
    let err = Map::load_from_file("foo.tmx").unwrap_err();
    match err {
        Error::UnsupportedFormat(ext) => assert_eq!(ext, "foo.tmx"),
        other => panic!("expected UnsupportedFormat, got {:?}", other),
    }
}
