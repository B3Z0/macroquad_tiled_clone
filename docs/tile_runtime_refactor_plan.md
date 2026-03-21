# Tile Runtime Status

## Scope

This document records the tile-runtime parity work that now exists in the codebase.
It remains useful as a contract/status document, but the staged rollout described here has already been implemented.

## Glossary

- Canonical:
  Runtime gameplay truth owned by `MapData` and its state compartments.
  Canonical data is authoritative for mutation/query decisions.
- Derived:
  Data rebuilt or incrementally synchronized from canonical state for performance.
  In this crate the main derived store is `MapData::derived_index`.
- Render:
  Frame-local and GPU-facing data used only to draw (`RenderState`, `MacroquadRenderAssets`).
  Render data is never gameplay truth.

## Invariants

1. Exactly one canonical runtime truth model: `MapData`.
2. `derived_index` remains derived/cache only; never canonical.
3. Tile and object handle semantics are symmetrical:
   - stable handle identity
   - O(1) handle lookup path
   - stale/invalid handles fail safely (`None`/`false`)
4. Every canonical mutation eagerly synchronizes index state in the same operation.
5. Draw order determinism, cull behavior, and dedupe semantics remain unchanged.

## Current File Ownership

- Canonical type ownership:
  - `src/core/map_data/mod.rs`
- Tile canonical build + derived sync seed points:
  - `src/core/map_data/tile/load.rs`
  - `src/core/map_data/tile/index_sync.rs`
  - `src/spatial/index.rs`
- Tile handle query/mutation APIs:
  - `src/core/map_data/tile/query.rs`
  - `src/core/map_data/tile/mutate.rs`
- Render consumers (must stay consumer-only):
  - `src/render/macroquad_renderer.rs`
  - `src/render/state.rs`
- Tests/stress:
  - `tests/unit/map_tests.rs`
  - `tests/unit/render_cull_tests.rs`
  - `tests/unit/loader_json_tests.rs`
  - `tests/stamp_overflow.rs`

## Implemented Outcomes

1. Canonical tile runtime containers live in `MapData::tile_state`.
2. Tile handle lookup and mutation APIs are exposed through `MapData` and `Map`.
3. Canonical mutations eagerly synchronize derived index state.
4. Region query/mutation helpers are implemented and covered by tests.
5. Rendering consumes runtime tile state without becoming a second source of truth.

## Out of Scope

- Save format redesign outside ticketed persistence updates.
- ECS/system integration internals.
- Multi-format export support.
