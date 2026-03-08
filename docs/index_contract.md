# Unified Index Handle Contract (Phase A / Ticket A1)

This document defines the internal handle/LUT contract used by `GlobalIndex` for both tile and object records.

## Goals

- Keep one logical identity per indexed record.
- Support fast handle -> canonical location lookup.
- Keep multi-chunk membership possible without creating multiple identities.
- Make stale/invalid handle behavior explicit and safe.

## Handle Lifecycle

`GlobalIndex` manages two monotonic handle spaces:

- Tiles: `TileHandle`
- Objects: `ObjectHandle`

Allocation rules:

- Handles are allocated by internal counters (`alloc_handle`, `alloc_object_handle`).
- Handles are never reused after removal.
- A removed handle stays invalid permanently for the lifetime of that `GlobalIndex` instance.

## LUT Shape

Each handle space has a LUT:

- Tile LUT: `Vec<Option<TileLoc>>`
- Object LUT: `Vec<Option<ObjectLoc>>`
- Object membership index: `Vec<Vec<ObjectMembership>>`
- Canonical tile handle location map in `MapData::tile_state.runtime`:
  - `tile_location_by_handle`
  - `tile_handles_by_layer`
  - `tile_runtime_by_layer`
- Canonical object handle location map in `MapData::object_state`:
  - `object_location_by_handle`
  - `object_handles_by_layer`
  - `object_runtime_by_layer`

Where each location stores:

- `chunk`
- `layer`
- `index` (slot within the chunk/layer vector)

The LUT entry points to a canonical slot for that logical record.
For objects, the membership index stores all `(chunk, layer)` memberships owned by the handle.
For tiles, canonical runtime ownership is held by `tile_state.runtime`, while index memberships are derived from runtime state.

## Insert Semantics

- `insert_*_with_handle` inserts a record into a chunk/layer bucket.
- If the LUT entry for the handle is `None`, the inserted slot becomes canonical.
- If the LUT entry already exists, insertion is treated as additional membership (for example, oversized tile/object spanning multiple chunks), not a new identity.

## Remove Semantics

- `remove_tile(handle)` and `remove_object(handle)` remove every bucket entry with that handle across all chunks/layers.
- Implementation uses `swap_remove`; if another record is moved into a removed slot, that moved record's LUT canonical index is updated when needed.
- After removal, LUT entry is set to `None`.
- For objects, membership entries are also cleared from the membership index.

Return value:

- `true`: at least one entry existed and was removed.
- `false`: no entry existed (already removed or never allocated/inserted).

## Lookup Semantics

- `tile_loc` / `object_loc` return canonical location if handle is currently valid.
- `tile_rec` / `object_rec` validate that the canonical slot still points to the same handle.
- `object_memberships` returns all known chunk memberships for a live object handle.
- Stale or invalid handles always resolve to `None` and never panic.

## Update Semantics

- Objects:
  - `update_object_memberships(handle, placements)` atomically replaces all memberships for a live object handle.
  - Old memberships are removed first, then new memberships are inserted with the same handle identity.
  - Supports object move behavior across chunk boundaries while preserving one logical handle.
- Tiles:
  - Tile handle mutations (`update_tile_gid_by_handle`, `move_tile_by_handle`, `set_tile_alive_by_handle`, `remove_tile_by_handle`) update canonical runtime and derived index in one operation.
  - Mutation path validates target index entries first, then commits runtime state and index changes.
  - Dead (`alive = false`) tiles have no index memberships; revived tiles rebuild memberships from runtime position/gid.

## Multi-Chunk Membership and Dedupe

- Multiple chunk entries may share one handle.
- That handle remains one logical identity for dedupe/removal.
- Removing by handle clears all memberships for that logical identity.
- Tile render/query dedupe is by handle identity, so one logical tile spanning many chunks appears once per pass/query output.

## Stale/Invalid Handle Behavior

- Invalid handle (out-of-range / never allocated): lookup returns `None`, mutation returns `false`.
- Stale handle (allocated then removed/slot-cleared): lookup returns `None`, mutation returns `false`.
- Region/batch helpers skip stale/invalid handles and only count successful updates.

## Region Query and Mutation Contract

- Region queries (`query_visible_tile_handles`, `query_visible_tile_handles_all`) return deterministic, deduped handle outputs.
- Optional filter path (`TileQueryFilter.gid`) matches clean gid values.
- Region mutation helpers (`replace_visible_tiles_gid_in_rect`, `disable_visible_tiles_in_rect`) are data-oriented and immediately visible in subsequent query/render operations.

## Overflow Policy

Handle counters are `u32` and monotonic.

- On attempted allocation at `u32::MAX`, allocation panics with a clear "handle space exhausted" message.
- No wraparound is allowed.

This makes overflow behavior explicit and prevents accidental handle aliasing.

## Contract Tests (Current Coverage)

- Tile/object insert-get-remove stale-handle safety:
  - `spatial::index::tests::tile_insert_get_remove_and_stale_lookup_fail_safely`
  - `spatial::index::tests::object_insert_get_remove_and_stale_lookup_fail_safely`
- No stale slot reuse after remove:
  - `tile_reinsert_after_remove_has_no_stale_slot_access`
  - `object_reinsert_after_remove_has_no_stale_slot_access`
- Multi-chunk single-identity semantics:
  - `tile_multi_chunk_entries_keep_one_logical_identity`
  - `object_multi_chunk_entries_keep_one_logical_identity`
- Canonical mutation sync + determinism stress:
  - `tile_randomized_mutation_sequence_keeps_index_consistent`
  - `tile_randomized_mutation_sequence_is_deterministic`
  - `randomized_mutation_sequence_keeps_index_consistent`
  - `randomized_mutation_sequence_is_deterministic`
