use crate::core::LayerKindInfo;
use crate::core::{
    LayerPlan, ObjectState, TileAuthoredState, TileDerivedState, TileRuntimeStore, TileState,
};
use crate::ir_map::*;
use crate::loader::json_loader::*;
use crate::render::{MacroquadRenderAssets, RenderState};
use crate::spatial::GlobalIndex;
use crate::MapError;
use macroquad::prelude::*;
use std::collections::HashMap;
use std::path::Path;

pub use crate::core::TileQueryFilter;
#[cfg(test)]
use crate::core::{build_draw_order_and_kind, TileLayerDrawInfo};
#[cfg(test)]
use crate::core::{object_chunk_span, tile_draw_origin};
pub use crate::core::{LayerId, MapData, ObjectLayer, ObjectQueryFilter, ObjectRuntimeState};
#[cfg(test)]
use crate::render::cull::{query_visible_rect, visible_chunk_coords_rect};
#[cfg(test)]
use crate::spatial::{world_to_chunk, LayerIdx, TileId, CHUNK_SIZE};
pub use crate::spatial::{ObjectHandle, TileHandle};

/// Loaded Tiled map with rendering helpers.
///
/// `Map` is the stable facade over three internal components:
/// - [`MapData`] for canonical mutable runtime truth.
/// - `MacroquadRenderAssets` for Macroquad textures/atlas metadata.
/// - `RenderState` for frame-local draw state (stamps/culling/debug flags).
///
/// Coordinate contract:
/// - All draw APIs use world-space pixel coordinates (`Vec2`) in Macroquad's coordinate space.
/// - `view_min` and `view_max` are opposite corners of the view rectangle, not width/height.
/// - Order of corners is normalized internally (`min/max` are swapped when needed).
///
/// Stable API (recommended first):
/// - Construction: [`Map::load`].
/// - Main rendering: [`Map::draw`], [`Map::draw_visible_rect`].
/// - Object rendering: [`Map::draw_objects_tiles`], [`Map::draw_objects_debug`].
/// - Runtime queries: [`Map::object_layers`], [`Map::objects`].
/// - Render controls: [`Map::set_cull_padding`], [`Map::set_debug_draw`].
///
/// Stamp contract:
/// - Stable methods above manage stamps automatically.
/// - Advanced methods for manual frame composition:
///   [`Map::next_frame_stamp`], [`Map::draw_visible_rect_with_stamp`],
///   [`Map::draw_objects_tiles_with_stamp`], [`Map::draw_objects_debug_with_stamp`].
pub struct Map {
    pub(crate) data: MapData,
    pub(crate) assets: MacroquadRenderAssets,
    pub(crate) render_state: RenderState,
}

impl Map {
    /// Loads a Tiled map JSON file and its external tilesets/textures.
    ///
    /// This is the stable entry point for creating a [`Map`].
    pub async fn load(path: &str) -> Result<Self, MapError> {
        let (ir, base) = decode_map_file_to_ir(path)?;
        Self::from_ir(ir, &base).await
    }

    /// Saves canonical runtime state to a Tiled JSON file.
    pub fn save_to_json(&self, path: &str) -> Result<(), MapError> {
        self.data.save_to_json(path)
    }

    #[doc(hidden)]
    pub fn __new_for_stamp_overflow_test(object_count: usize) -> Self {
        let mut index = GlobalIndex::new();
        let mut objects = Vec::with_capacity(object_count);
        let mut object_location_by_handle = Vec::with_capacity(object_count);
        let mut object_handles = Vec::with_capacity(object_count);
        let mut object_runtime = Vec::with_capacity(object_count);
        for i in 0..object_count {
            objects.push(IrObject {
                id: i as u32,
                name: String::new(),
                class_name: String::new(),
                x: 8.0,
                y: 8.0,
                width: 16.0,
                height: 16.0,
                rotation: 0.0,
                visible: true,
                shape: IrObjectShape::Tile { gid: 1 },
                properties: Properties::default(),
            });
            let handle = index.alloc_object_handle();
            object_location_by_handle.push(Some((0, i)));
            object_handles.push(Some(handle));
            object_runtime.push(Some(ObjectRuntimeState {
                alive: true,
                visible: true,
                x: 8.0,
                y: 8.0,
                width: 16.0,
                height: 16.0,
            }));
            index.insert_object(
                0,
                crate::spatial::ChunkCoord { x: 0, y: 0 },
                crate::spatial::ObjectRec {
                    handle,
                    rel_pos: vec2(0.0, 0.0),
                },
            );
        }

        let object_layer = ObjectLayer {
            id: 0,
            name: "test".to_string(),
            visible: true,
            opacity: 1.0,
            offset: Vec2::ZERO,
            properties: Properties::default(),
            objects,
            bucket_layer: 0,
        };
        let source_objects = object_layer.objects.clone();

        let mut layer_kind_by_id = HashMap::new();
        layer_kind_by_id.insert(0, LayerKindInfo::Objects(0));

        Self {
            data: MapData {
                source_ir: IrMap {
                    tile_w: 16,
                    tile_h: 16,
                    properties: Properties::default(),
                    tilesets: vec![],
                    layers: vec![IrLayer {
                        name: "test".to_string(),
                        visible: true,
                        opacity: 1.0,
                        offset: Vec2::ZERO,
                        properties: Properties::default(),
                        kind: IrLayerKind::Objects {
                            objects: source_objects,
                        },
                    }],
                },
                derived_index: index,
                object_state: ObjectState {
                    object_layers: vec![object_layer],
                    object_location_by_handle,
                    object_handles_by_layer: vec![object_handles],
                    object_runtime_by_layer: vec![object_runtime],
                },
                tile_state: TileState {
                    authored: TileAuthoredState {
                        tile_layers: vec![],
                        tileset_runtime_info: vec![],
                    },
                    runtime: TileRuntimeStore {
                        tile_location_by_handle: vec![],
                        tile_handles_by_layer: vec![],
                        tile_runtime_by_layer: vec![],
                    },
                    derived: TileDerivedState {
                        gid_lut: vec![],
                        tile_layer_draw_info: vec![],
                    },
                },
                layer_plan: LayerPlan {
                    draw_order: vec![0],
                    layer_kind_by_id,
                },
            },
            assets: MacroquadRenderAssets { tilesets: vec![] },
            render_state: RenderState::default(),
        }
    }

    #[doc(hidden)]
    pub fn __set_frame_stamp_for_testing(&mut self, stamp: u32) {
        self.render_state.frame_stamp = stamp;
    }

    #[doc(hidden)]
    pub fn __frame_stamp_for_testing(&self) -> u32 {
        self.render_state.frame_stamp
    }

    #[doc(hidden)]
    pub fn __seen_tiles_stamp_count_for_testing(&self, layer_idx: usize, stamp: u32) -> usize {
        self.render_state
            .seen_objects_tiles
            .get(layer_idx)
            .map(|v| v.iter().filter(|&&s| s == stamp).count())
            .unwrap_or(0)
    }

    pub(crate) async fn from_ir(ir: IrMap, base_dir: &Path) -> Result<Self, MapError> {
        let data = MapData::from_ir(ir)?;
        let assets = MacroquadRenderAssets::from_data(&data, base_dir).await?;
        Ok(Self {
            data,
            assets,
            render_state: RenderState::new(),
        })
    }

    #[cfg(test)]
    pub(crate) fn object_chunk_span(
        obj: &IrObject,
        layer_offset: Vec2,
    ) -> (crate::spatial::ChunkCoord, crate::spatial::ChunkCoord) {
        object_chunk_span(obj, layer_offset)
    }

    #[cfg(test)]
    pub(crate) fn tile_draw_origin(world: Vec2, map_tile_h: u32, tile_h: u32) -> Vec2 {
        tile_draw_origin(world, map_tile_h, tile_h)
    }

    /// Advances and returns the frame stamp used for object deduplication.
    ///
    /// Advanced API: call this once per frame when using `*_with_stamp` methods manually.
    pub fn next_frame_stamp(&mut self) -> u32 {
        self.render_state.next_frame_stamp(&self.data)
    }

    /// Returns parsed object layers for inspection/querying.
    pub fn object_layers(&self) -> &[ObjectLayer] {
        self.data.object_layers()
    }

    /// Iterates all parsed objects across all object layers.
    pub fn objects(&self) -> impl Iterator<Item = &IrObject> {
        self.data.objects()
    }

    /// Looks up an object by stable object handle.
    ///
    /// Returns `None` for invalid or removed handles.
    pub fn object_by_handle(&self, handle: ObjectHandle) -> Option<&IrObject> {
        self.data.object_by_handle(handle)
    }

    /// Looks up mutable runtime object state by stable object handle.
    ///
    /// Returns `None` for invalid or removed handles.
    pub fn object_runtime_by_handle(&self, handle: ObjectHandle) -> Option<&ObjectRuntimeState> {
        self.data.object_runtime_by_handle(handle)
    }

    /// Updates object position and bounds by stable object handle.
    ///
    /// Returns `false` for invalid or removed handles.
    pub fn update_object_bounds_position_by_handle(
        &mut self,
        handle: ObjectHandle,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> bool {
        self.data
            .update_object_bounds_position_by_handle(handle, x, y, width, height)
    }

    /// Removes an object by stable object handle.
    ///
    /// Returns `false` for invalid or already removed handles.
    pub fn remove_object_by_handle(&mut self, handle: ObjectHandle) -> bool {
        self.data.remove_object_by_handle(handle)
    }

    /// Sets runtime visibility for an object by handle.
    ///
    /// Returns `false` for invalid or removed handles.
    pub fn set_object_visible_by_handle(&mut self, handle: ObjectHandle, visible: bool) -> bool {
        self.data.set_object_visible_by_handle(handle, visible)
    }

    /// Sets runtime alive/enabled flag for an object by handle.
    ///
    /// When set to `false`, object memberships are removed from query index.
    /// When set to `true`, memberships are rebuilt from current runtime bounds.
    pub fn set_object_alive_by_handle(&mut self, handle: ObjectHandle, alive: bool) -> bool {
        self.data.set_object_alive_by_handle(handle, alive)
    }

    /// Spawns a new object into an existing object layer and returns its stable handle.
    ///
    /// Returns `None` when `layer_idx` is invalid.
    pub fn spawn_object_in_layer(
        &mut self,
        layer_idx: usize,
        object: IrObject,
    ) -> Option<ObjectHandle> {
        self.data.spawn_object_in_layer(layer_idx, object)
    }

    /// Returns deduplicated object handles visible in `coords` for one object layer.
    ///
    /// Handles are returned in deterministic ascending handle order.
    pub fn query_object_handles_in_coords(
        &self,
        layer_idx: usize,
        coords: &[crate::spatial::ChunkCoord],
    ) -> Vec<ObjectHandle> {
        self.data.query_object_handles_in_coords(layer_idx, coords)
    }

    /// Queries visible object handles in a world-space view rectangle.
    ///
    /// Results are deduplicated, deterministic, and suitable for O(1) follow-up
    /// handle-based operations.
    pub fn query_visible_object_handles(
        &self,
        layer_idx: usize,
        view_min: Vec2,
        view_max: Vec2,
        filter: ObjectQueryFilter<'_>,
    ) -> Vec<ObjectHandle> {
        self.data
            .query_visible_object_handles(layer_idx, view_min, view_max, filter)
    }

    /// Queries visible authored object IDs in a world-space view rectangle.
    pub fn query_visible_object_ids(
        &self,
        layer_idx: usize,
        view_min: Vec2,
        view_max: Vec2,
        filter: ObjectQueryFilter<'_>,
    ) -> Vec<u32> {
        self.data
            .query_visible_object_ids(layer_idx, view_min, view_max, filter)
    }

    /// Queries visible tile handles for one tile layer in a world-space rectangle.
    ///
    /// Results are deduplicated and deterministic.
    pub fn query_visible_tile_handles(
        &self,
        layer_idx: usize,
        view_min: Vec2,
        view_max: Vec2,
        filter: TileQueryFilter,
    ) -> Vec<TileHandle> {
        self.data
            .query_visible_tile_handles(layer_idx, view_min, view_max, filter)
    }

    /// Queries visible tile handles across all tile layers in a world-space rectangle.
    ///
    /// Results are deterministic and sorted by `(layer_idx, handle)`.
    pub fn query_visible_tile_handles_all(
        &self,
        view_min: Vec2,
        view_max: Vec2,
        filter: TileQueryFilter,
    ) -> Vec<(usize, TileHandle)> {
        self.data
            .query_visible_tile_handles_all(view_min, view_max, filter)
    }

    /// Sets runtime visibility for multiple tiles by handle.
    ///
    /// Returns number of handles successfully updated.
    pub fn set_tiles_visible_by_handle(&mut self, handles: &[TileHandle], visible: bool) -> usize {
        self.data.set_tiles_visible_by_handle(handles, visible)
    }

    /// Sets runtime alive/enabled for multiple tiles by handle.
    ///
    /// Returns number of handles successfully updated.
    pub fn set_tiles_alive_by_handle(&mut self, handles: &[TileHandle], alive: bool) -> usize {
        self.data.set_tiles_alive_by_handle(handles, alive)
    }

    /// Replaces gid for multiple tiles by handle.
    ///
    /// `new_gid` is interpreted as raw gid value (clean gid without flip flags recommended).
    /// Returns number of handles successfully updated.
    pub fn update_tiles_gid_by_handle(&mut self, handles: &[TileHandle], new_gid: u32) -> usize {
        self.data
            .update_tiles_gid_by_handle(handles, crate::spatial::TileId(new_gid))
    }

    /// Replaces visible tiles in one tile layer and view rectangle with `new_gid`.
    ///
    /// Returns changed handles in deterministic order.
    pub fn replace_visible_tiles_gid_in_rect(
        &mut self,
        layer_idx: usize,
        view_min: Vec2,
        view_max: Vec2,
        filter: TileQueryFilter,
        new_gid: u32,
    ) -> Vec<TileHandle> {
        self.data.replace_visible_tiles_gid_in_rect(
            layer_idx,
            view_min,
            view_max,
            filter,
            crate::spatial::TileId(new_gid),
        )
    }

    /// Disables (`alive = false`) visible tiles in one tile layer and view rectangle.
    ///
    /// Returns changed handles in deterministic order.
    pub fn disable_visible_tiles_in_rect(
        &mut self,
        layer_idx: usize,
        view_min: Vec2,
        view_max: Vec2,
        filter: TileQueryFilter,
    ) -> Vec<TileHandle> {
        self.data
            .disable_visible_tiles_in_rect(layer_idx, view_min, view_max, filter)
    }

    /// Enables/disables object debug overlay drawing used by [`Map::draw`].
    ///
    /// Stable API.
    pub fn set_debug_draw(&mut self, enabled: bool) {
        self.render_state.debug_draw = enabled;
    }

    /// Sets extra culling padding in world-space pixels around the view rectangle.
    ///
    /// Stable API. `0.0` means no extra padding.
    /// Default is one chunk (`CHUNK_SIZE`).
    pub fn set_cull_padding(&mut self, padding: f32) {
        self.render_state.cull_padding = padding.max(0.0);
    }
}

#[cfg(test)]
include!("../tests/unit/map_tests.rs");
