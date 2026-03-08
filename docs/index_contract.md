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

Where each location stores:

- `chunk`
- `layer`
- `index` (slot within the chunk/layer vector)

The LUT entry points to a canonical slot for that logical record.
For objects, the membership index stores all `(chunk, layer)` memberships owned by the handle.

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

- `update_object_memberships(handle, placements)` atomically replaces all memberships for a live object handle.
- Old memberships are removed first, then new memberships are inserted with the same handle identity.
- This supports object "move" behavior across chunk boundaries while preserving one logical handle.

## Multi-Chunk Membership and Dedupe

- Multiple chunk entries may share one handle.
- That handle remains one logical identity for dedupe/removal.
- Removing by handle clears all memberships for that logical identity.

## Overflow Policy

Handle counters are `u32` and monotonic.

- On attempted allocation at `u32::MAX`, allocation panics with a clear "handle space exhausted" message.
- No wraparound is allowed.

This makes overflow behavior explicit and prevents accidental handle aliasing.
