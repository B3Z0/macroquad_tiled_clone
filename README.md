# macroquad_tiled_clone

Minimal Tiled JSON loader and renderer for Macroquad.

## Internals Model

The crate is now organized around three responsibilities:

- `core/*`: runtime/query data (`MapData`) and spatial indexing logic. No Macroquad rendering dependency in core modules.
- `render/*`: Macroquad renderer, render assets, and frame-local render state (stamps/culling/debug toggles).
- `map.rs`: stable public facade (`Map`) that delegates to core + render internals.

## Supported

- Tiled JSON maps (orthogonal) with external tilesets (`source` .json)
- Tile layers (finite) with `data` arrays
- Object layers (`objectgroup`)
- Tile objects (`gid`)
- Multiple tilesets (firstgid mapping)
- Per-layer offsets
- Properties on map/layer/object/tileset/tile
- Tile flip/rotation flags from Tiled GIDs
- Rendering via `draw_texture_ex` with nearest filtering
- Universal draw API: `map.draw(view_min, view_max)` (tiles + tile-objects)
- Optional debug outlines via `set_debug_draw(true)`

## Not Supported

- Infinite maps (chunked layers)
- Image layers
- Group layers
- Embedded tilesets
- Base64/compressed layer data
- Isometric or hex maps
- Tile animations

## Rendering API

- `draw(view_min, view_max)`: draws tiles + tile-objects, and draws debug outlines when `debug_draw` is enabled.
- `draw_visible_rect(view_min, view_max)`: draws tiles only (convenience flow).
- `draw_visible_rect_with_stamp(view_min, view_max, stamp)`: tile-only pass with caller stamp for manual composition.
- Both draw APIs use the same culling policy configured with `set_cull_padding(pixels)`.
- Default culling padding is one chunk (256 px).
- Stable usage pattern: call `map.draw(Vec2::ZERO, vec2(screen_width(), screen_height()))` once per frame.
- Advanced/manual object composition:
  - `let stamp = map.next_frame_stamp();`
  - `map.draw_objects_tiles_with_stamp(view_min, view_max, stamp);`
  - `map.draw_objects_debug_with_stamp(view_min, view_max, stamp);`

## Engine Integration

### Data/Query Path (Headless Runtime)

Use `MapData` when you want loading/query without binding textures or calling draw:

```rust
use macroquad_tiled_clone::MapData;

let data = MapData::load("assets2/map.json")?;
let object_layer_count = data.object_layers().len();
let object_count = data.objects().count();
```

### Render Path (Macroquad)

Use `Map` when you want the full load + render flow:

```rust
use macroquad::prelude::*;
use macroquad_tiled_clone::Map;

let mut map = Map::load("assets2/map.json").await?;
map.draw(Vec2::ZERO, vec2(screen_width(), screen_height()));
```

### Manual Stamp Composition

For explicit multi-pass composition in one frame, share one stamp:

```rust
let stamp = map.next_frame_stamp();
map.draw_visible_rect_with_stamp(view_min, view_max, stamp);
map.draw_objects_tiles_with_stamp(view_min, view_max, stamp);
map.draw_objects_debug_with_stamp(view_min, view_max, stamp);
```

## Examples

- `examples/basic_map.rs`: one-call `map.draw(...)` flow.
- `examples/objects.rs`: manual stamp composition flow.
- `examples/split_screen.rs`: two cameras/viewports drawing different world rectangles.

## Quickstart

1. Add to your project:
   ```toml
   macroquad_tiled_clone = { git = "https://github.com/B3Z0/macroquad_tiled_clone.git" }
   ```
2. Run the example:
   ```bash
   cargo run --example basic_map
   ```
   To run examples for a fixed number of frames (useful for CI/local checks):
   ```bash
   MQ_FRAMES=5 cargo run --example basic_map
   ```
   PowerShell:
   ```powershell
   $env:MQ_FRAMES='5'; cargo run --example basic_map
   ```
3. Load and draw a map:
   ```rust
   use macroquad::prelude::*;
   use macroquad_tiled_clone::Map;

   #[macroquad::main("My Game")]
   async fn main() {
       let mut map = Map::load("assets2/map.json")
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

## Migration Notes (Contributors)

- Keep public API additions on `Map`/`MapData`; avoid exposing render internals.
- Place runtime/query logic in `src/core/*` and keep it free from render imports.
- Place Macroquad-specific draw/texture/state logic in `src/render/*`.
- Keep `src/map.rs` as a facade/delegation layer, not a large implementation file.
- When changing draw behavior, update regression tests in `src/map_tests.rs` (dedupe, stamps, culling, draw order).
