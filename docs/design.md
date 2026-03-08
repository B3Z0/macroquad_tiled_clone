# Design Notes (v0.1.0)

## Scope

This crate targets finite, orthogonal Tiled JSON maps rendered with Macroquad.
The v0.1.0 goal is a small, stable drawing API with deterministic behavior and predictable errors.

## Public API Shape

Stable public surface:

- `Map::load(path) -> Result<Map, MapError>`
- `Map::draw(view_min, view_max)` for normal rendering
- `Map::draw_visible_rect(view_min, view_max)` for tile-only advanced flow
- `Map::draw_visible_rect_with_stamp(view_min, view_max, stamp)` for tile-only manual composition
- `Map::set_debug_draw(enabled)`
- `Map::set_cull_padding(pixels)`
- Object inspection accessors (`object_layers`, `objects`)

Advanced/manual surface:

- `Map::next_frame_stamp()`
- `Map::draw_objects_tiles_with_stamp(...)`
- `Map::draw_objects_debug_with_stamp(...)`

Rule: if manual object draws are used in one frame, use one shared stamp for all object passes.

## Architecture Boundaries

Current ownership split:

- `src/core/*`
  - `MapData` is the canonical mutable runtime truth model.
  - `GlobalIndex` inside `MapData` is derived query/index cache, rebuildable from canonical object/tile state.
  - Layer ordering metadata and spatial index construction.
  - Chunk visibility coordinate helpers for view rectangles.
  - Must not import from `crate::render`.
- `src/render/*`
  - Renderer is a consumer-only view over canonical state.
  - `MacroquadRenderAssets`: texture/atlas ownership and binding.
  - `RenderState`: frame stamp + dedupe buffers + draw config flags.
  - `macroquad_renderer.rs`: draw pipeline implementation for `Map`.
- `src/map.rs`
  - Stable facade API only.
  - Holds `data: MapData`, `assets: MacroquadRenderAssets`, `render_state: RenderState`.
  - Delegates loading/query/render behavior to core/render modules.

This split exists to keep runtime/query use-cases (headless engine systems) separate from rendering concerns while preserving the current stable `Map` API.

Canonical state boundary rule:

- Exactly one canonical runtime truth model exists: `MapData`.
- Query/index structures (for example `GlobalIndex`) are derived/runtime-sync data, not authoritative gameplay truth.
- Rendering never owns gameplay truth; it consumes `MapData` plus render-only state.
- Sync mode is eager incremental: every canonical object mutation updates index state in the same operation.

## Engine Integration

### Data/Query Path

- Load/query only through `MapData::load(path)`.
- Use `object_layers()` and `objects()` for runtime systems.
- No texture loading is required on this path.

### Render Path

- Load and render through `Map::load(path)` and `Map::draw(view_min, view_max)`.
- `Map::draw_visible_rect(...)` is tile-only rendering.
- `set_cull_padding(pixels)` controls chunk culling expansion for both `draw` and `draw_visible_rect`.

### Manual Stamp Composition

When composing passes manually in one frame:

1. Call `let stamp = map.next_frame_stamp();`
2. Use the same `stamp` for all `_with_stamp` object/tile passes.
3. Do not mix multiple stamps for one logical frame.

## Coordinates and Culling

- All draw APIs consume world-space pixel coordinates (`Vec2`).
- `view_min`/`view_max` are rectangle corners, not size values.
- Culling can expand the view by `cull_padding` (in pixels).
- `Map::draw` and `Map::draw_visible_rect` use the same culling coordinate path.
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

## Persistence Policy

- Canonical save source is `MapData`.
- Persist canonical authored fields plus runtime-mutated object geometry/visibility.
- Despawned (`alive = false`) objects are omitted from persisted object arrays.
- Never persist derived/index/cache (`GlobalIndex`, LUTs) or render state/assets.
- See `docs/persistence_policy.md` for explicit policy scope.

## Known Non-Goals (v0.1.0)

- Infinite maps (`layers[].chunks`)
- Image layers
- Group layers
- Embedded tilesets
- Base64/compressed data
- Isometric/hex maps

## Contributor Migration Notes

- If adding runtime/query features, implement in `src/core/*` first, then expose through facade as needed.
- If adding draw behavior, implement in `src/render/macroquad_renderer.rs` and keep `Map` methods stable.
- Do not re-introduce dedupe or texture ownership into `ObjectLayer`/`MapData`.
- Keep regression coverage in `src/map_tests.rs` for:
  - dedupe invariants
  - stamp overflow behavior
  - oversized tile anchoring/culling
  - draw order determinism
  - cull padding consistency across draw APIs
