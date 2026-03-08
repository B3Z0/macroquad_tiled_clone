# Code Style and Naming Conventions

This document defines naming and readability conventions for internal refactors that must not change behavior.

## Goals

- Standardize names across runtime, index, and render boundaries.
- Keep public API stable unless explicitly additive.
- Make ownership obvious from symbol names.

## Naming Rules

- Types (`struct`, `enum`, `trait`, type aliases): `PascalCase`
- Functions, methods, modules, files, variables, fields: `snake_case`
- Constants/statics: `SCREAMING_SNAKE_CASE`
- Test names: `snake_case` with behavior intent (`move_object_updates_memberships`)

## Domain Prefix/Suffix Rules

- Runtime handles: suffix `_handle` (`object_handle`, `tile_handle`)
- Authored identifiers: suffix `_id` (`layer_id`, `object_id`)
- Canonical runtime state containers: include `runtime` or `state`
- Derived lookup/cache containers: include `index`, `lut`, or `cache`
- Booleans: positive form where possible (`visible`, `alive`, `enabled`)
- Coordinate values: explicit space/scope (`world_x`, `world_y`, `chunk_x`, `chunk_y`)

## Canonical vs Derived Naming

- Canonical gameplay truth symbols must avoid cache-oriented names.
- Derived query/index symbols must be clearly marked as derived.

Examples:

- Canonical: `object_runtime_by_layer`, `object_layers`
- Derived: `object_loc_by_handle`, `gid_lut`, `index`

## Invalid/Removed Handle Naming

- Use `invalid` and `stale` explicitly in tests/docs.
- APIs should return `Option`/`bool` for missing or stale handles; never panic.

## Readability Refactor Scope (No Behavior Change)

The following sequence is required for large files (for example `src/core/map_data/mod.rs`):

1. Move-only split by concern (no symbol renames in same change).
2. Mechanical renames (one family at a time, tests green after each).
3. Private helper extraction for long methods.
4. Module-level docs explaining ownership and invariants.

Constraints:

- No changes to algorithmic behavior or ordering.
- No changes to public API signatures unless explicitly additive.
- Keep determinism for draw order, cull behavior, and stamp dedupe.

## `MapData` Readability Split Plan

Target internal modules:

- `src/core/map_data/mod.rs`: type exports and shared internal helpers
- `src/core/map_data/load.rs`: loading/build from IR
- `src/core/map_data/persistence.rs`: save/export from canonical state
- `src/core/map_data/object_runtime.rs`: object runtime mutations by handle
- `src/core/map_data/query.rs`: visible/context query entrypoints
- `src/core/map_data/index_sync.rs`: canonical-to-index synchronization helpers

Migration rule:

- Keep `pub use` re-exports so existing crate users and examples remain unchanged.

## Review Checklist

- Names communicate ownership (`canonical` vs `derived` vs `render`).
- No ambiguous abbreviations (`loc` and `lut` are acceptable; avoid new unclear acronyms).
- Methods that mutate canonical state mention what is synchronized (`update_*_by_handle`, `set_*_by_handle`).
- Docs updated for any new public symbol.
