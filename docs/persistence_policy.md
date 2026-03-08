# Persistence Scope Policy (Phase C / Ticket C1)

This document defines what runtime state is in persistence scope and what is explicitly out of scope.

## Canonical Source for Save

Save/export reads from canonical runtime state (`MapData`) only.

## Persisted Fields

For object layers/objects, persistence includes:

- Layer metadata needed by authored map content:
  - `id`, `name`, `visible`, `opacity`, `offset`, `properties`
- Object authored/static identity and shape metadata:
  - `id`, `name`, `class_name`, `rotation`, `shape`, `properties`
- Runtime-mutated object fields from canonical runtime state:
  - `x`, `y`, `width`, `height`, `visible`

Runtime `alive = false` policy:

- Despawned/dead objects are omitted from exported object arrays.

## Never Persisted

Derived/query/index/cache and render-frame state are never persisted:

- Spatial/index/cache:
  - `GlobalIndex`
  - handle LUTs and membership maps
- Render-only state:
  - `RenderState` (frame stamp, cull padding, dedupe/debug buffers)
  - `MacroquadRenderAssets` (textures/atlas GPU-facing data)

These are rebuildable or runtime-only and are not authoritative gameplay/map content.

## Tiled JSON Compatibility Goals

Compatibility target is deterministic finite orthogonal Tiled JSON within current crate support:

- Preserve authored map/layer/object structure compatible with existing loader constraints.
- Persist runtime geometry/visibility by writing canonical object values into standard Tiled object fields.
- Do not require non-standard runtime-only fields for MVP compatibility.

## Determinism Goal

Given identical canonical state, export output should be deterministic in:

- layer order
- object order within layers
- numeric field values written from canonical runtime state
