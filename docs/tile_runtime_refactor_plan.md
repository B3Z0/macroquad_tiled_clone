# Tile Runtime Refactor Plan

## Scope

This document defines the staged rollout for tile-runtime parity with object-runtime behavior.
It is a no-behavior-change planning artifact for the upcoming T1-T5 tickets.

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

## Invariants (Target)

1. Exactly one canonical runtime truth model: `MapData`.
2. `derived_index` remains derived/cache only; never canonical.
3. Tile and object handle semantics are symmetrical:
   - stable handle identity
   - O(1) handle lookup path
   - stale/invalid handles fail safely (`None`/`false`)
4. Every canonical mutation eagerly synchronizes index state in the same operation.
5. Draw order determinism, cull behavior, and dedupe semantics remain unchanged.

## File-Level Rollout Map

- Canonical type ownership:
  - `src/core/map_data/mod.rs`
- Tile canonical build + derived sync seed points:
  - `src/core/map_data/tile/load.rs`
  - `src/core/map_data/tile/index_sync.rs`
  - `src/spatial/index.rs`
- Future tile handle APIs:
  - `src/core/map_data/tile/query.rs`
  - new tile mutation compartment under `src/core/map_data/tile/` (ticketed)
- Render consumers (must stay consumer-only):
  - `src/render/macroquad_renderer.rs`
  - `src/render/state.rs`
- Tests/stress:
  - `tests/tile_mutation_stress.rs`
  - `tests/unit/map_tests.rs`

## Ticket Sequence

1. T1 foundation: canonical tile runtime containers + shape normalization.
2. T2 APIs: tile handle lookup/mutation parity.
3. T3 sync: eager atomic index sync + invariants.
4. T4 gameplay ops: region query/mutation helpers.
5. T5 docs/contract finalization.

## Out of Scope

- Save format redesign outside ticketed persistence updates.
- ECS/system integration internals.
- Multi-format export support.
