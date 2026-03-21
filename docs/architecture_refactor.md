# Architecture Refactor Status

## Status

This refactor is implemented in the current codebase.
The document is kept as an architecture snapshot describing the ownership boundaries that now exist, not as a future plan.

## Goal

Define hard ownership boundaries so map runtime/query logic and macroquad rendering logic stay separated internally while preserving the public `Map` API.

## Components

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
Public compatibility layer preserving the external API.
- Delegates load/query/render calls to target components
- Keeps stable call sites while internals are decoupled

## Ownership Rules
1. Canonical runtime truth belongs to `MapData` only.
2. Querying visibility/chunks belongs to `MapData`.
3. `GlobalIndex` is derived/cache state and not canonical truth.
4. Texture loading/storage belongs to `MacroquadRenderAssets`.
5. Stamp lifecycle and dedupe buffers belong to `RenderState`.
6. Draw-call orchestration belongs to `MacroquadMapRenderer`.
7. `Map` facade should remain orchestration/delegation only.
8. Canonical->index synchronization uses eager incremental updates on every mutation.

## Current Ownership Snapshot

Current `Map` fields from [src/map.rs](../src/map.rs):
- `data: MapData`
- `assets: MacroquadRenderAssets`
- `render_state: RenderState`

Current `MapData` ownership from [src/core/map_data/mod.rs](../src/core/map_data/mod.rs):
- `source_ir: IrMap` (canonical authored source)
- `derived_index: GlobalIndex` (derived query/index cache, not canonical truth)
- `object_state: ObjectState` (canonical object runtime state + handle maps)
- `tile_state: TileState` with explicit compartments:
  - `authored: TileAuthoredState`
  - `runtime: TileRuntimeStore`
  - `derived: TileDerivedState`
- `layer_plan: LayerPlan` (deterministic layer traversal plan)

`ObjectState`:
- `object_layers`
- `object_location_by_handle`
- `object_handles_by_layer`
- `object_runtime_by_layer`

`TileState`:
- `authored.tile_layers`
- `authored.tileset_runtime_info`
- `runtime.tile_location_by_handle`
- `runtime.tile_handles_by_layer`
- `runtime.tile_runtime_by_layer`
- `derived.gid_lut`
- `derived.tile_layer_draw_info`

`LayerPlan`:
- `draw_order`
- `layer_kind_by_id`

`RenderState`:
- frame stamp lifecycle
- cull padding
- debug flag
- per-pass dedupe stamp buffers

`MacroquadRenderAssets`:
- macroquad texture-backed tileset assets only

## Module Ownership Tree (Current State)

- `src/core/map_data/mod.rs`
  - type declarations, module wiring, thin orchestration entry points
- `src/core/map_data/load.rs`
  - canonical map build from IR
- `src/core/map_data/persistence.rs`
  - canonical save/export
- `src/core/map_data/object/{load,mutate,query,index_sync}.rs`
  - object state build, handle-centric mutation/query, index sync helpers
- `src/core/map_data/tile/{load,mutate,query,index_sync,draw}.rs`
  - tile state build, handle-centric mutation/query, index helpers, and draw-origin math
- `src/core/map_data/shared/{geometry,layer_plan,tags}.rs`
  - shared geometry/layer-planning/tag helpers
- `src/render/assets.rs`
  - texture/atlas ownership
- `src/render/state.rs`
  - frame-local render state and dedupe buffers
- `src/render/macroquad_renderer.rs`
  - draw execution pipeline
- `src/map.rs`
  - stable facade API over core + render components

## Non-Goals
- No runtime behavior changes in readability-only tickets
- No breaking public API changes
- No coupling of render internals back into canonical state

## Unified Handle Contract Status

- Objects and tiles now both support canonical runtime handle maps plus handle-centric mutation/query APIs.
- Both paths use eager canonical->index synchronization and deterministic dedupe behavior.
- Stale/invalid handles fail safely (`None`/`false`) for both object and tile operations.

## Acceptance Checklist
- Architecture boundary is explicit and documented.
- Every current `Map` field maps to one target owner.
- Ownership rules are unambiguous for stamps, textures, and queries.
- Naming and readability conventions are defined in `docs/code_style.md` for continued maintenance.
