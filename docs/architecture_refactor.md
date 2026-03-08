# Architecture Refactor Plan (Ticket 1)

## Goal
Define hard ownership boundaries so map runtime/query logic and macroquad rendering logic are separated internally, while preserving the current public `Map` API during transition.

## Current Problem
`Map` currently owns runtime data, render assets, and frame/render state in one struct ([src/map.rs](../src/map.rs)). This makes responsibilities unclear and increases refactor risk.

## Target Components

### 1) `MapData` (canonical mutable runtime state)
Owns canonical gameplay/runtime truth.
- Layer data, objects, properties, draw order metadata
- Query/index data and culling coordinate queries derived from canonical content
- GID lookup metadata needed for runtime tile identity

Must not own textures or perform draw calls.

### 2) `RenderState` (frame/cull/debug + dedupe)
Owns render-pass mutable state.
- `frame_stamp`
- `cull_padding`
- debug flags
- dedupe buffers (tiles and objects)

Must not own map content or textures.

### 3) `MacroquadRenderAssets` (textures/atlas)
Owns macroquad-specific GPU/asset data.
- `Texture2D`
- render atlas info needed to sample tileset images

Must not own query logic.

### 4) `MacroquadMapRenderer` (draw pipeline)
Owns draw execution.
- Tile/object draw passes
- flip/rotation rendering math
- layer-order render traversal

Consumes `MapData + RenderState + MacroquadRenderAssets`.

### 5) `Map` facade (compat API)
Public compatibility layer preserving current external API.
- Delegates load/query/render calls to target components
- Keeps stable call sites while internals are decoupled

## Ownership Rules
1. Canonical runtime truth belongs to `MapData` only.
2. Querying visibility/chunks belongs to `MapData`.
3. `GlobalIndex` is derived/cache state and not canonical truth.
4. Texture loading/storage belongs to `MacroquadRenderAssets`.
5. Stamp lifecycle and dedupe buffers belong to `RenderState`.
6. Draw-call orchestration belongs to `MacroquadMapRenderer`.
7. `Map` facade contains no business logic long-term; only orchestration/delegation.
8. Canonical->index synchronization uses eager incremental updates on every mutation.

## Mapping: Current `Map` Fields -> Target Owner

Current `Map` fields from [src/map.rs](../src/map.rs):
- `index: GlobalIndex` -> `MapData`
- `tilesets: Vec<TilesetInfo>` -> split into:
  - runtime tileset metadata -> `MapData`
  - macroquad textures/atlas draw assets -> `MacroquadRenderAssets`
- `object_layers: Vec<ObjectLayer>` -> `MapData` (object content)
- `renderer: MapRenderer` -> `RenderState`
- `gid_lut: Vec<u16>` -> `MapData`
- `tile_layers: Vec<TileLayerDrawInfo>` -> `MapData`
- `tile_seen_stamps: Vec<u32>` -> `RenderState`
- `draw_order: Vec<LayerId>` -> `MapData`
- `layer_kind_by_id: HashMap<LayerId, LayerKindInfo>` -> `MapData`

## Existing Module References (current state)
- Loader/parsing: [src/loader/json_loader.rs](../src/loader/json_loader.rs)
- Runtime/IR model: [src/ir_map.rs](../src/ir_map.rs)
- Spatial index: [src/spatial/index.rs](../src/spatial/index.rs)
- Culling helpers: [src/render/cull.rs](../src/render/cull.rs)
- Unified map/runtime/render implementation (to be split): [src/map.rs](../src/map.rs)

## Non-Goals for Ticket 1
- No runtime behavior changes
- No API changes
- No file moves yet

## Acceptance Checklist
- Architecture boundary is explicit and documented.
- Every current `Map` field maps to one target owner.
- Ownership rules are unambiguous for stamps, textures, and queries.
