use crate::core::MapData;
use crate::spatial::CHUNK_SIZE;

/// Internal render state and dedupe buffers.
///
/// This is not part of the stable public API; callers should use [`crate::Map`] methods.
pub(crate) struct RenderState {
    pub(crate) debug_draw: bool,
    pub(crate) cull_padding: f32,
    pub(crate) frame_stamp: u32,
    pub(crate) seen_tiles: Vec<u32>,
    pub(crate) seen_objects_tiles: Vec<Vec<u32>>,
    pub(crate) seen_objects_debug: Vec<Vec<u32>>,
}

impl RenderState {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn sync_with_data(&mut self, data: &MapData) {
        if self.seen_tiles.len() != data.derived_index.handles.len() {
            self.seen_tiles.resize(data.derived_index.handles.len(), 0);
        }
        if self.seen_objects_tiles.len() != data.object_state.object_layers.len() {
            self.seen_objects_tiles
                .resize_with(data.object_state.object_layers.len(), Vec::new);
        }
        if self.seen_objects_debug.len() != data.object_state.object_layers.len() {
            self.seen_objects_debug
                .resize_with(data.object_state.object_layers.len(), Vec::new);
        }
        for (i, layer) in data.object_state.object_layers.iter().enumerate() {
            let n = layer.objects.len();
            if self.seen_objects_tiles[i].len() != n {
                self.seen_objects_tiles[i].resize(n, 0);
            }
            if self.seen_objects_debug[i].len() != n {
                self.seen_objects_debug[i].resize(n, 0);
            }
        }
    }

    pub(crate) fn next_frame_stamp(&mut self, data: &MapData) -> u32 {
        self.sync_with_data(data);
        if self.frame_stamp == u32::MAX {
            for seen in &mut self.seen_objects_tiles {
                seen.fill(0);
            }
            for seen in &mut self.seen_objects_debug {
                seen.fill(0);
            }
            self.seen_tiles.fill(0);
            self.frame_stamp = 1;
            return 1;
        }

        self.frame_stamp += 1;
        self.frame_stamp
    }
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            debug_draw: false,
            cull_padding: CHUNK_SIZE as f32,
            frame_stamp: 0,
            seen_tiles: vec![],
            seen_objects_tiles: vec![],
            seen_objects_debug: vec![],
        }
    }
}
