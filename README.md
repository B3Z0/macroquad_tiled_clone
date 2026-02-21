# macroquad_tiled_clone

Minimal Tiled JSON loader and renderer for Macroquad.

## Supports

- Tiled JSON maps (orthogonal) with external tilesets (`source` .json)
- Tile layers with `data` arrays
- Object layers (`objectgroup`)
- Multiple tilesets (firstgid mapping)
- Per-layer offsets
- Properties on map/layer/object/tileset/tile
- Tile flip/rotation flags from Tiled GIDs
- Rendering via `draw_texture_ex` with nearest filtering
- Universal draw API: `map.draw(view_min, view_max)` (tiles + tile-objects)

## Not yet

- Inline tilesets
- Image layers
- Infinite maps (chunked layers)
- Isometric or hex maps
- Tile animations
- Layer visibility/opacity in rendering

## Quickstart

1. Add to your project:
   ```toml
   macroquad_tiled_clone = { git = "https://github.com/B3Z0/macroquad_tiled_clone.git" }
   ```
2. Run the example:
   ```bash
   cargo run --example basic_map
   ```
3. Load and draw a map:
   ```rust
   use macroquad::prelude::*;
   use macroquad_tiled_clone::map::Map;

   #[macroquad::main("My Game")]
   async fn main() {
       let map = Map::load("assets2/map.json")
           .await
           .expect("Failed to load map");

       loop {
           clear_background(BLACK);
           map.draw(Vec2::ZERO, vec2(screen_width(), screen_height()));
           next_frame().await;
       }
   }
   ```

## Limitations

- Map files must be `.json` exported from Tiled.
- Tilesets must be external JSON tilesets with a single atlas image.
- Unsupported layer kinds are skipped.
- Infinite maps are not supported (no chunked `layers[].chunks`).
- Layer `visible` and `opacity` are loaded but not applied at draw time.
