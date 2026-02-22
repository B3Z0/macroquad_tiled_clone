use macroquad::prelude::*;
use std::collections::HashMap;

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

pub struct GlobalIndex {
    pub buckets: HashMap<ChunkCoord, GlobalChunk>,
    pub handles: Vec<Option<TileLoc>>,
    next_handle: u32,
}

impl GlobalIndex {
    pub fn new() -> Self {
        GlobalIndex {
            buckets: HashMap::new(),
            handles: Vec::new(),
            next_handle: 0,
        }
    }

    fn alloc_handle(&mut self) -> TileHandle {
        let h = TileHandle(self.next_handle);
        self.next_handle += 1;
        self.handles.push(None);
        h
    }
}

impl Default for GlobalIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalIndex {
    pub fn add_tile(&mut self, id: TileId, layer: LayerIdx, world: Vec2) -> TileHandle {
        let cc = world_to_chunk(world);
        let handle = self.alloc_handle();
        let bucket = self.buckets.entry(cc).or_default();
        let vec = &mut bucket.layers.entry(layer).or_default().tiles;

        let idx = vec.len();
        vec.push(TileRec {
            handle,
            id,
            rel_pos: rel(world),
        });
        self.handles[handle.0 as usize] = Some(TileLoc {
            chunk: cc,
            layer,
            index: idx,
        });
        handle
    }

    pub fn insert_object(&mut self, layer: LayerIdx, chunk: ChunkCoord, object_rec: ObjectRec) {
        let bucket = self.buckets.entry(chunk).or_default();
        bucket
            .layers
            .entry(layer)
            .or_default()
            .objects
            .push(object_rec);
    }
}
