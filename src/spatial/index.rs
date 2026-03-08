//! Spatial index/cache structures derived from canonical runtime map content.

use macroquad::prelude::*;
use std::collections::{HashMap, HashSet};

pub const CHUNK_SIZE: i32 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileHandle(pub u32);

pub type LayerIdx = u16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkCoord {
    pub x: i32,
    pub y: i32,
}

pub const FLIP_H: u32 = 0x8000_0000; // bit 31
pub const FLIP_V: u32 = 0x4000_0000; // bit 30
pub const FLIP_D: u32 = 0x2000_0000; // bit 29
pub const GID_MASK: u32 = 0x1FFF_FFFF; // keep lower 29 bits (bit 28 is free)

impl TileId {
    #[inline]
    pub fn raw(self) -> u32 {
        self.0
    }
    #[inline]
    pub fn clean(self) -> u32 {
        self.0 & GID_MASK
    }
    #[inline]
    pub fn flip_h(self) -> bool {
        (self.0 & FLIP_H) != 0
    }
    #[inline]
    pub fn flip_v(self) -> bool {
        (self.0 & FLIP_V) != 0
    }
    #[inline]
    pub fn flip_d(self) -> bool {
        (self.0 & FLIP_D) != 0
    }
}

#[inline]
pub fn world_to_chunk(p: Vec2) -> ChunkCoord {
    ChunkCoord {
        x: (p.x as i32).div_euclid(CHUNK_SIZE),
        y: (p.y as i32).div_euclid(CHUNK_SIZE),
    }
}

#[inline]
pub fn rel(p: Vec2) -> Vec2 {
    vec2(
        (p.x as i32).rem_euclid(CHUNK_SIZE) as f32,
        (p.y as i32).rem_euclid(CHUNK_SIZE) as f32,
    )
}

#[derive(Debug, Clone)]
pub struct TileRec {
    pub handle: TileHandle,
    pub id: TileId,
    pub rel_pos: Vec2,
}

#[derive(Debug, Clone)]
pub struct ObjectRec {
    pub handle: ObjectHandle,
    pub rel_pos: Vec2,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
/// Stable handle identity for object records in the spatial index.
pub struct ObjectHandle(pub u32);

#[derive(Debug, Clone, Default)]
pub struct LayerBucket {
    pub tiles: Vec<TileRec>,
    pub objects: Vec<ObjectRec>,
}

pub struct GlobalChunk {
    pub layers: HashMap<LayerIdx, LayerBucket>,
}

impl GlobalChunk {
    pub fn new() -> Self {
        GlobalChunk {
            layers: HashMap::new(),
        }
    }
}

impl Default for GlobalChunk {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TileLoc {
    pub chunk: ChunkCoord,
    pub layer: LayerIdx,
    pub index: usize,
}

pub struct ObjectLoc {
    pub chunk: ChunkCoord,
    pub layer: LayerIdx,
    pub index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectMembership {
    pub chunk: ChunkCoord,
    pub layer: LayerIdx,
}

pub struct GlobalIndex {
    pub buckets: HashMap<ChunkCoord, GlobalChunk>,
    pub handles: Vec<Option<TileLoc>>,
    object_handles: Vec<Option<ObjectLoc>>,
    object_memberships: Vec<Vec<ObjectMembership>>,
    next_handle: u32,
    next_object_handle: u32,
}

impl GlobalIndex {
    pub fn new() -> Self {
        GlobalIndex {
            buckets: HashMap::new(),
            handles: Vec::new(),
            object_handles: Vec::new(),
            object_memberships: Vec::new(),
            next_handle: 0,
            next_object_handle: 0,
        }
    }

    pub fn alloc_handle(&mut self) -> TileHandle {
        assert!(self.next_handle != u32::MAX, "tile handle space exhausted");
        let h = TileHandle(self.next_handle);
        self.next_handle += 1;
        self.handles.push(None);
        h
    }

    pub fn alloc_object_handle(&mut self) -> ObjectHandle {
        assert!(
            self.next_object_handle != u32::MAX,
            "object handle space exhausted"
        );
        let h = ObjectHandle(self.next_object_handle);
        self.next_object_handle += 1;
        self.object_handles.push(None);
        self.object_memberships.push(Vec::new());
        h
    }
}

impl Default for GlobalIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalIndex {
    pub fn tile_loc(&self, handle: TileHandle) -> Option<&TileLoc> {
        self.handles.get(handle.0 as usize).and_then(Option::as_ref)
    }

    pub fn object_loc(&self, handle: ObjectHandle) -> Option<&ObjectLoc> {
        self.object_handles
            .get(handle.0 as usize)
            .and_then(Option::as_ref)
    }

    pub fn tile_rec(&self, handle: TileHandle) -> Option<&TileRec> {
        let loc = self.tile_loc(handle)?;
        self.buckets
            .get(&loc.chunk)?
            .layers
            .get(&loc.layer)?
            .tiles
            .get(loc.index)
            .and_then(|rec| {
                if rec.handle == handle {
                    Some(rec)
                } else {
                    None
                }
            })
    }

    pub fn object_rec(&self, handle: ObjectHandle) -> Option<&ObjectRec> {
        let loc = self.object_loc(handle)?;
        self.buckets
            .get(&loc.chunk)?
            .layers
            .get(&loc.layer)?
            .objects
            .get(loc.index)
            .and_then(|rec| {
                if rec.handle == handle {
                    Some(rec)
                } else {
                    None
                }
            })
    }

    pub fn object_memberships(&self, handle: ObjectHandle) -> Option<&[ObjectMembership]> {
        self.object_loc(handle)?;
        self.object_memberships
            .get(handle.0 as usize)
            .map(Vec::as_slice)
    }

    pub fn dedup_object_handles_in_coords(
        &self,
        coords: &[ChunkCoord],
        layer: LayerIdx,
    ) -> Vec<ObjectHandle> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        for cc in coords {
            let Some(chunk) = self.buckets.get(cc) else {
                continue;
            };
            let Some(bucket) = chunk.layers.get(&layer) else {
                continue;
            };
            for rec in &bucket.objects {
                if seen.insert(rec.handle) {
                    out.push(rec.handle);
                }
            }
        }
        out
    }

    pub fn add_tile(&mut self, id: TileId, layer: LayerIdx, world: Vec2) -> TileHandle {
        let cc = world_to_chunk(world);
        let handle = self.alloc_handle();
        self.insert_tile_with_handle(handle, id, layer, cc, world);
        handle
    }

    pub fn insert_tile_with_handle(
        &mut self,
        handle: TileHandle,
        id: TileId,
        layer: LayerIdx,
        chunk: ChunkCoord,
        world: Vec2,
    ) {
        let bucket = self.buckets.entry(chunk).or_default();
        let vec = &mut bucket.layers.entry(layer).or_default().tiles;
        let chunk_origin = vec2((chunk.x * CHUNK_SIZE) as f32, (chunk.y * CHUNK_SIZE) as f32);

        let idx = vec.len();
        vec.push(TileRec {
            handle,
            id,
            rel_pos: world - chunk_origin,
        });

        let hidx = handle.0 as usize;
        if hidx >= self.handles.len() {
            self.handles.resize_with(hidx + 1, || None);
        }

        // Keep first location as canonical metadata; a handle may exist in
        // multiple chunks for oversized tiles and still share one identity.
        if self.handles[hidx].is_none() {
            self.handles[hidx] = Some(TileLoc {
                chunk,
                layer,
                index: idx,
            });
        }
    }

    pub fn remove_tile(&mut self, handle: TileHandle) -> bool {
        let hidx = handle.0 as usize;
        let mut removed_any = false;

        for (cc, chunk) in &mut self.buckets {
            for (layer_idx, layer) in &mut chunk.layers {
                let tiles = &mut layer.tiles;
                let mut i = 0usize;
                while i < tiles.len() {
                    if tiles[i].handle != handle {
                        i += 1;
                        continue;
                    }

                    removed_any = true;
                    let last = tiles.len() - 1;
                    let moved = if i != last {
                        Some(tiles[last].handle)
                    } else {
                        None
                    };
                    tiles.swap_remove(i);

                    if let Some(moved_handle) = moved {
                        let midx = moved_handle.0 as usize;
                        if let Some(Some(loc)) = self.handles.get_mut(midx) {
                            if loc.chunk == *cc && loc.layer == *layer_idx && loc.index == last {
                                loc.index = i;
                            }
                        }
                    }
                }
            }
        }

        if let Some(slot) = self.handles.get_mut(hidx) {
            *slot = None;
        }
        removed_any
    }

    pub fn add_object(
        &mut self,
        layer: LayerIdx,
        chunk: ChunkCoord,
        rel_pos: Vec2,
    ) -> ObjectHandle {
        let handle = self.alloc_object_handle();
        self.insert_object_with_handle(layer, chunk, ObjectRec { handle, rel_pos });
        handle
    }

    pub fn insert_object(&mut self, layer: LayerIdx, chunk: ChunkCoord, object_rec: ObjectRec) {
        self.insert_object_with_handle(layer, chunk, object_rec);
    }

    pub fn insert_object_with_handle(
        &mut self,
        layer: LayerIdx,
        chunk: ChunkCoord,
        object_rec: ObjectRec,
    ) {
        let handle = object_rec.handle;
        let bucket = self.buckets.entry(chunk).or_default();
        let vec = &mut bucket.layers.entry(layer).or_default().objects;
        let idx = vec.len();
        vec.push(object_rec);

        let hidx = handle.0 as usize;
        if hidx >= self.object_handles.len() {
            self.object_handles.resize_with(hidx + 1, || None);
            self.object_memberships.resize_with(hidx + 1, Vec::new);
        }
        if self.object_handles[hidx].is_none() {
            self.object_handles[hidx] = Some(ObjectLoc {
                chunk,
                layer,
                index: idx,
            });
        }
        let m = ObjectMembership { chunk, layer };
        if !self.object_memberships[hidx].contains(&m) {
            self.object_memberships[hidx].push(m);
        }
    }

    pub fn remove_object(&mut self, handle: ObjectHandle) -> bool {
        let Some(memberships) = self.object_memberships.get(handle.0 as usize).cloned() else {
            return false;
        };

        let mut removed_any = false;
        for m in memberships {
            while self.remove_one_object_entry(handle, m) {
                removed_any = true;
            }
        }

        let hidx = handle.0 as usize;
        if let Some(slot) = self.object_handles.get_mut(hidx) {
            *slot = None;
        }
        if let Some(m) = self.object_memberships.get_mut(hidx) {
            m.clear();
        }
        removed_any
    }

    pub fn update_object_memberships(
        &mut self,
        handle: ObjectHandle,
        new_memberships: &[(LayerIdx, ChunkCoord, Vec2)],
    ) -> bool {
        if (handle.0 as usize) >= self.object_handles.len() {
            return false;
        }

        self.remove_object(handle);
        for (layer, chunk, rel_pos) in new_memberships {
            self.insert_object_with_handle(
                *layer,
                *chunk,
                ObjectRec {
                    handle,
                    rel_pos: *rel_pos,
                },
            );
        }
        true
    }

    fn remove_one_object_entry(
        &mut self,
        handle: ObjectHandle,
        membership: ObjectMembership,
    ) -> bool {
        let Some(chunk) = self.buckets.get_mut(&membership.chunk) else {
            return false;
        };
        let Some(layer) = chunk.layers.get_mut(&membership.layer) else {
            return false;
        };
        let objects = &mut layer.objects;
        let Some(idx) = objects.iter().position(|r| r.handle == handle) else {
            return false;
        };

        let last = objects.len() - 1;
        let moved = if idx != last {
            Some(objects[last].handle)
        } else {
            None
        };
        objects.swap_remove(idx);

        if let Some(moved_handle) = moved {
            let midx = moved_handle.0 as usize;
            if let Some(Some(loc)) = self.object_handles.get_mut(midx) {
                if loc.chunk == membership.chunk
                    && loc.layer == membership.layer
                    && loc.index == last
                {
                    loc.index = idx;
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_insert_get_remove_and_stale_lookup_fail_safely() {
        let mut index = GlobalIndex::new();
        let handle = index.add_tile(TileId(7), 1, vec2(16.0, 32.0));

        let loc = index.tile_loc(handle).expect("tile location must exist");
        assert_eq!(loc.layer, 1);
        assert_eq!(
            index
                .tile_rec(handle)
                .expect("tile record must exist")
                .id
                .clean(),
            7
        );

        assert!(index.remove_tile(handle));
        assert!(index.tile_loc(handle).is_none());
        assert!(index.tile_rec(handle).is_none());
        assert!(!index.remove_tile(handle));
    }

    #[test]
    fn tile_reinsert_after_remove_has_no_stale_slot_access() {
        let mut index = GlobalIndex::new();
        let old_handle = index.add_tile(TileId(1), 0, vec2(0.0, 0.0));
        assert!(index.remove_tile(old_handle));

        let new_handle = index.add_tile(TileId(2), 0, vec2(4.0, 4.0));
        assert_ne!(old_handle, new_handle);
        assert!(index.tile_rec(old_handle).is_none());
        assert_eq!(
            index
                .tile_rec(new_handle)
                .expect("new tile must exist")
                .id
                .clean(),
            2
        );
    }

    #[test]
    fn tile_multi_chunk_entries_keep_one_logical_identity() {
        let mut index = GlobalIndex::new();
        let handle = index.alloc_handle();
        let world = vec2((CHUNK_SIZE - 8) as f32, 12.0);
        index.insert_tile_with_handle(handle, TileId(9), 0, ChunkCoord { x: 0, y: 0 }, world);
        index.insert_tile_with_handle(handle, TileId(9), 0, ChunkCoord { x: 1, y: 0 }, world);

        assert!(index.tile_rec(handle).is_some());
        assert!(index.remove_tile(handle));
        assert!(index.tile_rec(handle).is_none());
        let chunk0 = index
            .buckets
            .get(&ChunkCoord { x: 0, y: 0 })
            .and_then(|c| c.layers.get(&0))
            .map(|b| b.tiles.len())
            .unwrap_or(0);
        let chunk1 = index
            .buckets
            .get(&ChunkCoord { x: 1, y: 0 })
            .and_then(|c| c.layers.get(&0))
            .map(|b| b.tiles.len())
            .unwrap_or(0);
        assert_eq!(chunk0 + chunk1, 0);
    }

    #[test]
    fn object_insert_get_remove_and_stale_lookup_fail_safely() {
        let mut index = GlobalIndex::new();
        let handle = index.add_object(2, ChunkCoord { x: 0, y: 0 }, vec2(3.0, 5.0));

        let loc = index
            .object_loc(handle)
            .expect("object location must exist");
        assert_eq!(loc.layer, 2);
        assert_eq!(
            index
                .object_rec(handle)
                .expect("object record must exist")
                .rel_pos,
            vec2(3.0, 5.0)
        );

        assert!(index.remove_object(handle));
        assert!(index.object_loc(handle).is_none());
        assert!(index.object_rec(handle).is_none());
        assert!(!index.remove_object(handle));
    }

    #[test]
    fn object_reinsert_after_remove_has_no_stale_slot_access() {
        let mut index = GlobalIndex::new();
        let old_handle = index.add_object(0, ChunkCoord { x: 0, y: 0 }, vec2(1.0, 1.0));
        assert!(index.remove_object(old_handle));

        let new_handle = index.add_object(0, ChunkCoord { x: 0, y: 0 }, vec2(8.0, 13.0));
        assert_ne!(old_handle, new_handle);
        assert!(index.object_rec(old_handle).is_none());
        assert_eq!(
            index
                .object_rec(new_handle)
                .expect("new object must exist")
                .rel_pos,
            vec2(8.0, 13.0)
        );
    }

    #[test]
    fn object_multi_chunk_entries_keep_one_logical_identity() {
        let mut index = GlobalIndex::new();
        let handle = index.alloc_object_handle();
        index.insert_object_with_handle(
            3,
            ChunkCoord { x: 0, y: 0 },
            ObjectRec {
                handle,
                rel_pos: vec2(250.0, 15.0),
            },
        );
        index.insert_object_with_handle(
            3,
            ChunkCoord { x: 1, y: 0 },
            ObjectRec {
                handle,
                rel_pos: vec2(-6.0, 15.0),
            },
        );

        let memberships = index
            .object_memberships(handle)
            .expect("memberships should exist");
        assert_eq!(memberships.len(), 2);
        assert!(index.object_rec(handle).is_some());
        assert!(index.remove_object(handle));
        assert!(index.object_rec(handle).is_none());
        let chunk0 = index
            .buckets
            .get(&ChunkCoord { x: 0, y: 0 })
            .and_then(|c| c.layers.get(&3))
            .map(|b| b.objects.len())
            .unwrap_or(0);
        let chunk1 = index
            .buckets
            .get(&ChunkCoord { x: 1, y: 0 })
            .and_then(|c| c.layers.get(&3))
            .map(|b| b.objects.len())
            .unwrap_or(0);
        assert_eq!(chunk0 + chunk1, 0);
    }

    #[test]
    fn object_spanning_multiple_chunks_is_deduped_in_query() {
        let mut index = GlobalIndex::new();
        let handle = index.alloc_object_handle();
        index.insert_object_with_handle(
            7,
            ChunkCoord { x: 0, y: 0 },
            ObjectRec {
                handle,
                rel_pos: vec2(252.0, 20.0),
            },
        );
        index.insert_object_with_handle(
            7,
            ChunkCoord { x: 1, y: 0 },
            ObjectRec {
                handle,
                rel_pos: vec2(-4.0, 20.0),
            },
        );

        let handles = index.dedup_object_handles_in_coords(
            &[ChunkCoord { x: 0, y: 0 }, ChunkCoord { x: 1, y: 0 }],
            7,
        );
        assert_eq!(handles, vec![handle]);
    }

    #[test]
    fn move_object_updates_old_and_new_chunk_memberships() {
        let mut index = GlobalIndex::new();
        let handle = index.alloc_object_handle();
        index.insert_object_with_handle(
            4,
            ChunkCoord { x: 0, y: 0 },
            ObjectRec {
                handle,
                rel_pos: vec2(16.0, 16.0),
            },
        );
        index.insert_object_with_handle(
            4,
            ChunkCoord { x: 1, y: 0 },
            ObjectRec {
                handle,
                rel_pos: vec2(-8.0, 16.0),
            },
        );

        assert!(index.update_object_memberships(
            handle,
            &[
                (4, ChunkCoord { x: 2, y: 0 }, vec2(12.0, 10.0)),
                (4, ChunkCoord { x: 2, y: 1 }, vec2(12.0, -20.0)),
            ],
        ));

        let old0 = index
            .buckets
            .get(&ChunkCoord { x: 0, y: 0 })
            .and_then(|c| c.layers.get(&4))
            .map(|b| b.objects.iter().filter(|o| o.handle == handle).count())
            .unwrap_or(0);
        let old1 = index
            .buckets
            .get(&ChunkCoord { x: 1, y: 0 })
            .and_then(|c| c.layers.get(&4))
            .map(|b| b.objects.iter().filter(|o| o.handle == handle).count())
            .unwrap_or(0);
        assert_eq!(old0 + old1, 0);

        let new_handles = index.dedup_object_handles_in_coords(
            &[ChunkCoord { x: 2, y: 0 }, ChunkCoord { x: 2, y: 1 }],
            4,
        );
        assert_eq!(new_handles, vec![handle]);
        let memberships = index
            .object_memberships(handle)
            .expect("memberships should still exist");
        assert_eq!(memberships.len(), 2);
    }
}
