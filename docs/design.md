# Design Notes (v0.1.0)

## Scope

This crate targets finite, orthogonal Tiled JSON maps rendered with Macroquad.
The v0.1.0 goal is a small, stable drawing API with deterministic behavior and predictable errors.

## Public API Shape

Stable public surface:

- `Map::load(path) -> Result<Map, MapError>`
- `Map::draw(view_min, view_max)` for normal rendering
- `Map::draw_visible_rect(view_min, view_max)` for tile-only advanced flow
- `Map::set_debug_draw(enabled)`
- `Map::set_cull_padding(pixels)`
- Object inspection accessors (`object_layers`, `objects`)

Advanced/manual surface:

- `Map::next_frame_stamp()`
- `Map::draw_objects_tiles_with_stamp(...)`
- `Map::draw_objects_debug_with_stamp(...)`

Rule: if manual object draws are used in one frame, use one shared stamp for all object passes.

## Coordinates and Culling

- All draw APIs consume world-space pixel coordinates (`Vec2`).
- `view_min`/`view_max` are rectangle corners, not size values.
- Culling can expand the view by `cull_padding` (in pixels).
- Visible chunks are iterated in deterministic sorted order.

## Layer and Draw Order

- Layer order follows Tiled layer array order.
- Object layers and tile layers share one draw-order plan.
- `Map::draw` renders according to that unified order.

## Object Indexing and Dedupe

- Object records in spatial buckets use `ObjectHandle` (runtime handle) instead of raw casts.
- Objects spanning multiple chunks are inserted into all overlapped chunks (AABB-based).
- Per-layer stamp buffers dedupe objects so each object is drawn once per pass.
- Stamp overflow (`u32::MAX`) is handled by buffer reset and wrap to `1`.

## Error Handling

The loading path is panic-free and returns typed `MapError` values:

- I/O and JSON parse failures with source path context
- Invalid map/tileset contracts
- Invalid tile/object gid references
- Unsupported property types
- Texture load failures

## Known Non-Goals (v0.1.0)

- Infinite maps (`layers[].chunks`)
- Image layers
- Group layers
- Embedded tilesets
- Base64/compressed data
- Isometric/hex maps
