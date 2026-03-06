#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock went backwards")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("mq_tiled_props_{nanos}"));
        fs::create_dir_all(&dir).expect("failed to create temp dir");
        dir
    }

    #[test]
    fn parses_properties_for_map_layer_object_tileset_and_tile() {
        let dir = temp_dir();
        let map_path = dir.join("map.json");
        let ts_path = dir.join("tileset.json");

        let map_json = r#"{
          "tilewidth": 16,
          "tileheight": 16,
          "properties": [
            {"name":"is_night","type":"bool","value":true},
            {"name":"gravity","type":"float","value":9.8},
            {"name":"theme","type":"string","value":"forest"}
          ],
          "layers": [
            {
              "type":"tilelayer",
              "name":"ground",
              "width":2,
              "height":2,
              "data":[1,0,0,0],
              "properties":[
                {"name":"is_solid","type":"bool","value":true},
                {"name":"difficulty","type":"int","value":3}
              ]
            },
            {
              "type":"objectgroup",
              "name":"spawns",
              "objects":[
                {
                  "id": 7,
                  "name":"spawn_1",
                  "type":"spawn",
                  "properties":[{"name":"kind","type":"string","value":"player"}]
                }
              ],
              "properties":[{"name":"enabled","type":"bool","value":true}]
            }
          ],
          "tilesets":[{"firstgid":1,"source":"tileset.json"}]
        }"#;

        let tileset_json = r#"{
          "tilewidth":16,
          "tileheight":16,
          "tilecount":4,
          "columns":2,
          "image":"tiles.png",
          "properties":[{"name":"biome","type":"string","value":"forest"}],
          "tiles":[
            {
              "id":0,
              "properties":[{"name":"damage","type":"int","value":10}],
              "objectgroup":{
                "objects":[
                  {"id":1,"name":"hitbox","type":"shape","properties":[{"name":"sensor","type":"bool","value":false}]}
                ]
              }
            }
          ]
        }"#;

        fs::write(&map_path, map_json).expect("failed to write map");
        fs::write(&ts_path, tileset_json).expect("failed to write tileset");

        let (ir, _) = decode_map_file_to_ir(map_path.to_str().expect("path utf8")).expect("decode");

        assert_eq!(ir.properties.get_bool("is_night"), Some(true));
        assert_eq!(ir.properties.get_f32("gravity"), Some(9.8));
        assert_eq!(ir.properties.get_string("theme"), Some("forest"));

        assert_eq!(ir.layers[0].properties.get_bool("is_solid"), Some(true));
        assert_eq!(ir.layers[0].properties.get_i32("difficulty"), Some(3));

        match &ir.layers[1].kind {
            IrLayerKind::Objects { objects } => {
                assert_eq!(objects.len(), 1);
                assert_eq!(objects[0].properties.get_string("kind"), Some("player"));
            }
            _ => panic!("expected object layer"),
        }

        match &ir.tilesets[0] {
            IrTileset::Atlas {
                properties, tiles, ..
            } => {
                assert_eq!(properties.get_string("biome"), Some("forest"));
                assert_eq!(tiles.len(), 1);
                assert_eq!(tiles[0].properties.get_i32("damage"), Some(10));
                assert_eq!(tiles[0].objects.len(), 1);
                assert_eq!(
                    tiles[0].objects[0].properties.get_bool("sensor"),
                    Some(false)
                );
            }
        }
    }

    #[test]
    fn keeps_large_int_property_values() {
        let dir = temp_dir();
        let map_path = dir.join("map.json");
        let ts_path = dir.join("tileset.json");

        let map_json = r#"{
          "tilewidth": 16,
          "tileheight": 16,
          "properties": [
            {"name":"big_id","type":"object","value":5000000000}
          ],
          "layers": [],
          "tilesets":[{"firstgid":1,"source":"tileset.json"}]
        }"#;

        let tileset_json = r#"{
          "tilewidth":16,
          "tileheight":16,
          "tilecount":1,
          "columns":1,
          "image":"tiles.png"
        }"#;

        fs::write(&map_path, map_json).expect("failed to write map");
        fs::write(&ts_path, tileset_json).expect("failed to write tileset");

        let (ir, _) = decode_map_file_to_ir(map_path.to_str().expect("path utf8")).expect("decode");
        assert_eq!(ir.properties.get_i64("big_id"), Some(5_000_000_000));
        assert_eq!(ir.properties.get_i32("big_id"), None);
    }

    #[test]
    fn returns_typed_error_for_malformed_json() {
        let dir = temp_dir();
        let map_path = dir.join("map.json");
        fs::write(&map_path, "{ not json").expect("failed to write map");

        let err = decode_map_file_to_ir(map_path.to_str().expect("path utf8"))
            .err()
            .expect("expected decode error");
        assert!(matches!(err, MapError::Json { .. }));
    }

    #[test]
    fn returns_typed_error_for_missing_tileset_file() {
        let dir = temp_dir();
        let map_path = dir.join("map.json");
        let map_json = r#"{
          "tilewidth": 16,
          "tileheight": 16,
          "layers": [],
          "tilesets":[{"firstgid":1,"source":"missing_tileset.json"}]
        }"#;
        fs::write(&map_path, map_json).expect("failed to write map");

        let err = decode_map_file_to_ir(map_path.to_str().expect("path utf8"))
            .err()
            .expect("expected decode error");
        assert!(matches!(err, MapError::Io { .. }));
    }

    #[test]
    fn returns_typed_error_for_invalid_gid_reference() {
        let dir = temp_dir();
        let map_path = dir.join("map.json");
        let ts_path = dir.join("tileset.json");

        let map_json = r#"{
          "tilewidth": 16,
          "tileheight": 16,
          "layers": [
            {
              "type":"tilelayer",
              "name":"ground",
              "width":1,
              "height":1,
              "data":[99]
            }
          ],
          "tilesets":[{"firstgid":1,"source":"tileset.json"}]
        }"#;

        let tileset_json = r#"{
          "tilewidth":16,
          "tileheight":16,
          "tilecount":1,
          "columns":1,
          "image":"tiles.png"
        }"#;

        fs::write(&map_path, map_json).expect("failed to write map");
        fs::write(&ts_path, tileset_json).expect("failed to write tileset");

        let err = decode_map_file_to_ir(map_path.to_str().expect("path utf8"))
            .err()
            .expect("expected decode error");
        assert!(matches!(err, MapError::InvalidTileGid { .. }));
    }

    #[test]
    fn returns_typed_error_for_unknown_property_type() {
        let dir = temp_dir();
        let map_path = dir.join("map.json");
        let ts_path = dir.join("tileset.json");

        let map_json = r#"{
          "tilewidth": 16,
          "tileheight": 16,
          "properties": [
            {"name":"mystery","type":"not_supported","value":"x"}
          ],
          "layers": [],
          "tilesets":[{"firstgid":1,"source":"tileset.json"}]
        }"#;

        let tileset_json = r#"{
          "tilewidth":16,
          "tileheight":16,
          "tilecount":1,
          "columns":1,
          "image":"tiles.png"
        }"#;

        fs::write(&map_path, map_json).expect("failed to write map");
        fs::write(&ts_path, tileset_json).expect("failed to write tileset");

        let err = decode_map_file_to_ir(map_path.to_str().expect("path utf8"))
            .err()
            .expect("expected decode error");
        assert!(matches!(err, MapError::UnsupportedPropertyType { .. }));
    }

    #[test]
    fn returns_invalid_map_for_zero_tileset_tilecount() {
        let dir = temp_dir();
        let map_path = dir.join("map.json");
        let ts_path = dir.join("tileset.json");

        let map_json = r#"{
          "tilewidth":16,
          "tileheight":16,
          "layers": [],
          "tilesets":[{"firstgid":1,"source":"tileset.json"}]
        }"#;

        let tileset_json = r#"{
          "tilewidth":16,
          "tileheight":16,
          "tilecount":0,
          "columns":1,
          "image":"tiles.png"
        }"#;

        fs::write(&map_path, map_json).expect("failed to write map");
        fs::write(&ts_path, tileset_json).expect("failed to write tileset");

        let err = decode_map_file_to_ir(map_path.to_str().expect("path utf8"))
            .err()
            .expect("expected decode error");
        assert!(matches!(err, MapError::InvalidMap(_)));
    }

    #[test]
    fn returns_invalid_map_for_zero_tileset_columns() {
        let dir = temp_dir();
        let map_path = dir.join("map.json");
        let ts_path = dir.join("tileset.json");

        let map_json = r#"{
          "tilewidth":16,
          "tileheight":16,
          "layers": [],
          "tilesets":[{"firstgid":1,"source":"tileset.json"}]
        }"#;

        let tileset_json = r#"{
          "tilewidth":16,
          "tileheight":16,
          "tilecount":1,
          "columns":0,
          "image":"tiles.png"
        }"#;

        fs::write(&map_path, map_json).expect("failed to write map");
        fs::write(&ts_path, tileset_json).expect("failed to write tileset");

        let err = decode_map_file_to_ir(map_path.to_str().expect("path utf8"))
            .err()
            .expect("expected decode error");
        assert!(matches!(err, MapError::InvalidMap(_)));
    }

    #[test]
    fn returns_invalid_map_for_zero_tileset_tile_size() {
        let dir = temp_dir();
        let map_path = dir.join("map.json");
        let ts_path = dir.join("tileset.json");

        let map_json = r#"{
          "tilewidth":16,
          "tileheight":16,
          "layers": [],
          "tilesets":[{"firstgid":1,"source":"tileset.json"}]
        }"#;

        let tileset_json = r#"{
          "tilewidth":0,
          "tileheight":16,
          "tilecount":1,
          "columns":1,
          "image":"tiles.png"
        }"#;

        fs::write(&map_path, map_json).expect("failed to write map");
        fs::write(&ts_path, tileset_json).expect("failed to write tileset");

        let err = decode_map_file_to_ir(map_path.to_str().expect("path utf8"))
            .err()
            .expect("expected decode error");
        assert!(matches!(err, MapError::InvalidMap(_)));
    }

    #[test]
    fn returns_invalid_map_for_zero_map_tile_size() {
        let dir = temp_dir();
        let map_path = dir.join("map.json");
        let ts_path = dir.join("tileset.json");

        let map_json = r#"{
          "tilewidth":0,
          "tileheight":16,
          "layers": [],
          "tilesets":[{"firstgid":1,"source":"tileset.json"}]
        }"#;

        let tileset_json = r#"{
          "tilewidth":16,
          "tileheight":16,
          "tilecount":1,
          "columns":1,
          "image":"tiles.png"
        }"#;

        fs::write(&map_path, map_json).expect("failed to write map");
        fs::write(&ts_path, tileset_json).expect("failed to write tileset");

        let err = decode_map_file_to_ir(map_path.to_str().expect("path utf8"))
            .err()
            .expect("expected decode error");
        assert!(matches!(err, MapError::InvalidMap(_)));
    }

    #[test]
    fn returns_invalid_map_for_zero_tile_layer_dimensions() {
        let dir = temp_dir();
        let map_path = dir.join("map.json");
        let ts_path = dir.join("tileset.json");

        let map_json = r#"{
          "tilewidth":16,
          "tileheight":16,
          "layers":[
            {
              "type":"tilelayer",
              "name":"ground",
              "width":0,
              "height":1,
              "data":[]
            }
          ],
          "tilesets":[{"firstgid":1,"source":"tileset.json"}]
        }"#;

        let tileset_json = r#"{
          "tilewidth":16,
          "tileheight":16,
          "tilecount":1,
          "columns":1,
          "image":"tiles.png"
        }"#;

        fs::write(&map_path, map_json).expect("failed to write map");
        fs::write(&ts_path, tileset_json).expect("failed to write tileset");

        let err = decode_map_file_to_ir(map_path.to_str().expect("path utf8"))
            .err()
            .expect("expected decode error");
        assert!(matches!(err, MapError::InvalidMap(_)));
    }

    #[test]
    fn returns_invalid_map_for_tile_layer_data_length_mismatch() {
        let dir = temp_dir();
        let map_path = dir.join("map.json");
        let ts_path = dir.join("tileset.json");

        let map_json = r#"{
          "tilewidth":16,
          "tileheight":16,
          "layers":[
            {
              "type":"tilelayer",
              "name":"ground",
              "width":2,
              "height":2,
              "data":[1,0,0]
            }
          ],
          "tilesets":[{"firstgid":1,"source":"tileset.json"}]
        }"#;

        let tileset_json = r#"{
          "tilewidth":16,
          "tileheight":16,
          "tilecount":1,
          "columns":1,
          "image":"tiles.png"
        }"#;

        fs::write(&map_path, map_json).expect("failed to write map");
        fs::write(&ts_path, tileset_json).expect("failed to write tileset");

        let err = decode_map_file_to_ir(map_path.to_str().expect("path utf8"))
            .err()
            .expect("expected decode error");
        assert!(matches!(err, MapError::InvalidMap(_)));
    }
}
