// tests/map_tests.rs

use macroquad_tiled_clone::{Map, Error};

const BAD_LAYER_SIZE: &str = r#"
{
  "width": 2,
  "height": 2,
  "tilewidth": 8,
  "tileheight": 8,
  "layers": [
    { "name": "oops", "data": [1,2,3] }  // only 3 tiles, not 4
  ]
}
"#;

#[test]
fn error_on_layer_size_mismatch() {
    let err = Map::load_from_str(BAD_LAYER_SIZE).unwrap_err();
    // You’ll need to add an Error::InvalidLayerSize(String) variant
    // that carries the layer name.
    assert!(matches!(err, Error::InvalidLayerSize(name) if name=="oops"));
}


const JSON_WITH_EXTRA: &str = r#"
{
  "width":1, "height":1,
  "tilewidth":8, "tileheight":8,
  "dummyField": "ignored",
  "layers": [
    {
      "name":"L",
      "data":[0],
      "opacity": 0.5,
      "properties": []
    }
  ]
}
"#;

#[test]
fn load_ignores_extra_fields() {
    let map = Map::load_from_str(JSON_WITH_EXTRA).expect("Should ignore unknown fields");
    assert_eq!(map.layers[0].name, "L");
    assert_eq!(map.layers[0].data, vec![0]);
}

const EMPTY_NAME_JSON: &str = r#"
{
  "width":1,"height":1,"tilewidth":8,"tileheight":8,
  "layers":[ { "name":"", "data":[1] } ]
}
"#;

#[test]
fn load_allows_empty_layer_name() {
    let map = Map::load_from_str(EMPTY_NAME_JSON).unwrap();
    assert_eq!(map.layers[0].name, "");
}