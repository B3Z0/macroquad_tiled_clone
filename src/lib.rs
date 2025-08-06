use macroquad::prelude::*;
use std::collections::HashMap;
pub mod map;
mod view;
pub use view::query_visible;

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

pub struct GlobalChunk {
    pub layers: HashMap<LayerIdx, Vec<TileRec>>,
}

impl GlobalChunk {
    pub fn new() -> Self {
        GlobalChunk {
            layers: HashMap::new(),
        }
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

impl GlobalIndex {
    pub fn add_tile(
        &mut self,
        id: TileId,
        layer: LayerIdx,
        world: Vec2) -> TileHandle {
            let cc = world_to_chunk(world);
            let handle = self.alloc_handle();
            let bucket = self.buckets
                .entry(cc)
                .or_insert_with(GlobalChunk::new);
            let vec = bucket.layers
                .entry(layer)
                .or_insert_with(Vec::new);
            
            let idx = vec.len();
            vec.push(TileRec {
                handle, id, rel_pos: rel(world)
            });
            self.handles[handle.0 as usize] = Some(TileLoc {
                chunk: cc,
                layer,
                index: idx,
            });
            handle
    }
}

pub struct Map { 
    pub index: GlobalIndex,
    pub tile_w: u32,
    pub tile_h: u32,
}

impl Map {
    pub fn load_basic(path:&str) -> anyhow::Result<Self> {
        let mut index = GlobalIndex::new();
        let (tw, th) = map::load_basic_json(&mut index, path)?;
        Ok(Self { index, tile_w: tw, tile_h: th })
    }

    pub fn draw(&self, texture: &Texture2D) {
        let cols = texture.width() as u32 / self.tile_w;    // sprites per row

        for (cc, bucket) in &self.index.buckets {
            for vec in bucket.layers.values() {
                for rec in vec {
                    let gid  = rec.id.0;
                    let idx  = gid - 1;                     // GID 1 → atlas 0
                    let sx   = (idx % cols) * self.tile_w;  // left  px in atlas
                    let sy   = (idx / cols) * self.tile_h;  // top   px in atlas

                    draw_texture_ex(
                        texture,
                        (cc.x * CHUNK_SIZE) as f32 + rec.rel_pos.x,
                        (cc.y * CHUNK_SIZE) as f32 + rec.rel_pos.y,
                        WHITE,
                        DrawTextureParams {
                            source: Some(Rect::new(
                                sx as f32,
                                sy as f32,
                                self.tile_w as f32,
                                self.tile_h as f32,
                            )),
                            ..Default::default()
                        },
                    );
                }
            }
        }
    }
}
