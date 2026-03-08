//! Canonical mutable map runtime model.
//!
//! This module currently contains loading, runtime mutation, query, and persistence helpers.
//! It is the next readability split target into concern-focused submodules without behavior changes.

use crate::ir_map::*;
use crate::loader::json_loader::*;
use crate::spatial::{world_to_chunk, GlobalIndex, LayerIdx, ObjectHandle, TileId, CHUNK_SIZE};
use crate::MapError;
use macroquad::prelude::*;
use serde_json::{json, Value as JsonValue};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Stable layer identifier used by the renderer draw order.
///
/// The value maps to Tiled layer array order in the loaded map.
pub type LayerId = u32;

pub struct TilesetRuntimeInfo {
    pub first_gid: u32,
    #[allow(dead_code)]
    pub tilecount: u32,
    pub cols: u32,
    pub image: String,
    pub tile_w: u32,
    pub tile_h: u32,
    pub spacing: u32,
    pub margin: u32,
}

/// A Tiled object layer parsed from the map.
///
/// Stable API: this struct is exposed for inspection/querying (`Map::object_layers`),
/// not for direct mutation of rendering internals.
pub struct ObjectLayer {
    /// Stable layer id matching Tiled layer order.
    pub id: LayerId,
    /// Layer name from Tiled.
    pub name: String,
    /// Visibility flag from Tiled.
    pub visible: bool,
    /// Opacity from Tiled (0.0..=1.0).
    pub opacity: f32,
    /// Layer offset in world coordinates.
    pub offset: Vec2,
    /// Custom layer properties.
    pub properties: Properties,
    /// Parsed objects in this layer.
    pub objects: Vec<IrObject>,
    pub(crate) bucket_layer: LayerIdx,
}

/// Mutable runtime state for one authored object.
#[derive(Clone, Copy)]
pub struct ObjectRuntimeState {
    /// Runtime alive/enabled flag.
    pub alive: bool,
    /// Runtime visibility flag used by rendering.
    pub visible: bool,
    /// Runtime X position in world object space.
    pub x: f32,
    /// Runtime Y position in world object space.
    pub y: f32,
    /// Runtime object width.
    pub width: f32,
    /// Runtime object height.
    pub height: f32,
}

/// Optional filters for visible object queries.
#[derive(Clone, Copy, Default)]
pub struct ObjectQueryFilter<'a> {
    /// Exact match against `IrObject::class_name`.
    pub kind: Option<&'a str>,
    /// Tag value matched against object `tag` or comma-separated `tags` properties.
    pub tag: Option<&'a str>,
}

#[derive(Clone, Copy)]
pub(crate) struct TileLayerDrawInfo {
    pub(crate) layer_id: LayerIdx,
    pub(crate) visible: bool,
    pub(crate) opacity: f32,
}

#[derive(Clone, Copy)]
pub(crate) enum LayerKindInfo {
    Tiles(usize),
    Objects(usize),
    Unsupported,
}

pub(crate) fn build_draw_order_and_kind(
    layers: &[IrLayer],
) -> (Vec<LayerId>, HashMap<LayerId, LayerKindInfo>) {
    let mut draw_order = Vec::with_capacity(layers.len());
    let mut layer_kind_by_id = HashMap::with_capacity(layers.len());
    let mut tile_layer_idx = 0usize;
    let mut object_layer_idx = 0usize;

    for (lz, layer) in layers.iter().enumerate() {
        let stable_id = lz as LayerId;
        draw_order.push(stable_id);
        match layer.kind {
            IrLayerKind::Tiles { .. } => {
                layer_kind_by_id.insert(stable_id, LayerKindInfo::Tiles(tile_layer_idx));
                tile_layer_idx += 1;
            }
            IrLayerKind::Objects { .. } => {
                layer_kind_by_id.insert(stable_id, LayerKindInfo::Objects(object_layer_idx));
                object_layer_idx += 1;
            }
            IrLayerKind::Unsupported => {
                layer_kind_by_id.insert(stable_id, LayerKindInfo::Unsupported);
            }
        }
    }

    (draw_order, layer_kind_by_id)
}

/// Runtime/query data for a loaded map, independent from render-frame state.
///
/// Canonical boundary:
/// - Canonical mutable runtime truth is stored in layer/object/tile metadata fields.
/// - `index` is derived query/cache state synchronized from canonical content.
pub struct MapData {
    pub(crate) source_ir: IrMap,
    pub(crate) index: GlobalIndex,
    pub(crate) tilesets: Vec<TilesetRuntimeInfo>,
    pub(crate) object_layers: Vec<ObjectLayer>,
    pub(crate) object_loc_by_handle: Vec<Option<(usize, usize)>>,
    pub(crate) object_handles_by_layer: Vec<Vec<Option<ObjectHandle>>>,
    pub(crate) object_runtime_by_layer: Vec<Vec<Option<ObjectRuntimeState>>>,
    pub(crate) gid_lut: Vec<u16>, // lookup table for tile GIDs to tileset indices
    pub(crate) tile_layers: Vec<TileLayerDrawInfo>,
    pub(crate) draw_order: Vec<LayerId>,
    pub(crate) layer_kind_by_id: HashMap<LayerId, LayerKindInfo>,
}

impl MapData {
    /// Loads runtime/query map data without binding render textures.
    pub fn load(path: &str) -> Result<Self, MapError> {
        let (ir, _) = decode_map_file_to_ir(path)?;
        Self::from_ir(ir)
    }

    /// Saves canonical runtime state to a Tiled JSON map file.
    ///
    /// Export reads canonical state only and excludes derived index and render data.
    pub fn save_to_json(&self, path: &str) -> Result<(), MapError> {
        let p = Path::new(path);

        let mut layers_json = Vec::new();
        for (lz, layer_ir) in self.source_ir.layers.iter().enumerate() {
            let props = properties_to_json_vec(&layer_ir.properties);
            match self.layer_kind_by_id.get(&(lz as LayerId)).copied() {
                Some(LayerKindInfo::Tiles(_tile_layer_idx)) => {
                    let IrLayerKind::Tiles {
                        width,
                        height,
                        data,
                    } = &layer_ir.kind
                    else {
                        continue;
                    };
                    layers_json.push(json!({
                        "type": "tilelayer",
                        "name": layer_ir.name,
                        "visible": layer_ir.visible,
                        "opacity": layer_ir.opacity,
                        "offsetx": layer_ir.offset.x,
                        "offsety": layer_ir.offset.y,
                        "width": width,
                        "height": height,
                        "data": data,
                        "properties": props,
                    }));
                }
                Some(LayerKindInfo::Objects(object_layer_idx)) => {
                    let Some(layer) = self.object_layers.get(object_layer_idx) else {
                        continue;
                    };
                    let mut objects_json = Vec::new();
                    for (idx, authored) in layer.objects.iter().enumerate() {
                        let Some(Some(_handle)) = self
                            .object_handles_by_layer
                            .get(object_layer_idx)
                            .and_then(|v| v.get(idx))
                        else {
                            continue;
                        };
                        let Some(runtime) = self
                            .object_runtime_by_layer
                            .get(object_layer_idx)
                            .and_then(|v| v.get(idx))
                            .and_then(|r| r.as_ref())
                        else {
                            continue;
                        };
                        if !runtime.alive {
                            continue;
                        }

                        let mut obj = json!({
                            "id": authored.id,
                            "name": authored.name,
                            "type": "",
                            "class": authored.class_name,
                            "x": runtime.x,
                            "y": runtime.y,
                            "width": runtime.width,
                            "height": runtime.height,
                            "rotation": authored.rotation,
                            "visible": runtime.visible,
                            "properties": properties_to_json_vec(&authored.properties),
                        });

                        match &authored.shape {
                            IrObjectShape::Rectangle => {}
                            IrObjectShape::Point => {
                                obj["point"] = JsonValue::Bool(true);
                            }
                            IrObjectShape::Polygon(points) => {
                                obj["polygon"] = JsonValue::Array(
                                    points.iter().map(|p| json!({"x": p.x, "y": p.y})).collect(),
                                );
                            }
                            IrObjectShape::Polyline(points) => {
                                obj["polyline"] = JsonValue::Array(
                                    points.iter().map(|p| json!({"x": p.x, "y": p.y})).collect(),
                                );
                            }
                            IrObjectShape::Tile { gid } => {
                                obj["gid"] = JsonValue::Number(serde_json::Number::from(*gid));
                            }
                        }
                        objects_json.push(obj);
                    }

                    layers_json.push(json!({
                        "type": "objectgroup",
                        "name": layer_ir.name,
                        "visible": layer_ir.visible,
                        "opacity": layer_ir.opacity,
                        "offsetx": layer_ir.offset.x,
                        "offsety": layer_ir.offset.y,
                        "objects": objects_json,
                        "properties": props,
                    }));
                }
                Some(LayerKindInfo::Unsupported) | None => {}
            }
        }

        let mut tilesets_json = Vec::new();
        for ts in &self.source_ir.tilesets {
            match ts {
                IrTileset::Atlas {
                    first_gid, source, ..
                } => {
                    tilesets_json.push(json!({
                        "firstgid": first_gid,
                        "source": source,
                    }));
                }
            }
        }
        tilesets_json.sort_by(|a, b| {
            let af = a["firstgid"].as_u64().unwrap_or(0);
            let bf = b["firstgid"].as_u64().unwrap_or(0);
            af.cmp(&bf)
        });

        let root = json!({
            "tilewidth": self.source_ir.tile_w,
            "tileheight": self.source_ir.tile_h,
            "properties": properties_to_json_vec(&self.source_ir.properties),
            "layers": layers_json,
            "tilesets": tilesets_json,
        });

        let text = serde_json::to_string_pretty(&root).map_err(|source| MapError::Json {
            path: p.to_path_buf(),
            source,
        })?;
        std::fs::write(p, text).map_err(|source| MapError::Io {
            path: p.to_path_buf(),
            source,
        })
    }

    /// Returns parsed object layers for inspection/querying.
    pub fn object_layers(&self) -> &[ObjectLayer] {
        &self.object_layers
    }

    /// Iterates all parsed objects across all object layers.
    pub fn objects(&self) -> impl Iterator<Item = &IrObject> {
        self.object_layers
            .iter()
            .flat_map(|layer| layer.objects.iter())
    }

    /// Queries visible object handles for one object layer in a world-space rectangle.
    ///
    /// This query is data-oriented and returns stable handles for follow-up O(1)
    /// handle-based operations.
    pub fn query_visible_object_handles(
        &self,
        layer_idx: usize,
        view_min: Vec2,
        view_max: Vec2,
        filter: ObjectQueryFilter<'_>,
    ) -> Vec<ObjectHandle> {
        let Some(layer) = self.object_layers.get(layer_idx) else {
            return Vec::new();
        };
        if !layer.visible {
            return Vec::new();
        }

        let coords = self.visible_coords_for_draw(view_min, view_max, 0.0);
        let mut out = self.query_object_handles_in_coords(layer_idx, &coords);
        out.retain(|&handle| {
            let Some((li, oi)) = self.object_location(handle) else {
                return false;
            };
            if li != layer_idx {
                return false;
            }
            let Some(runtime) = self
                .object_runtime_by_layer
                .get(li)
                .and_then(|v| v.get(oi))
                .and_then(|r| r.as_ref())
            else {
                return false;
            };
            if !runtime.alive || !runtime.visible {
                return false;
            }
            let Some(obj) = self.object_layers.get(li).and_then(|l| l.objects.get(oi)) else {
                return false;
            };
            if let Some(kind) = filter.kind {
                if obj.class_name != kind {
                    return false;
                }
            }
            if let Some(tag) = filter.tag {
                if !object_has_tag(obj, tag) {
                    return false;
                }
            }
            true
        });
        out
    }

    /// Queries visible authored object IDs for one object layer in a world-space rectangle.
    ///
    /// IDs follow Tiled object ids; results are deterministic and deduplicated.
    pub fn query_visible_object_ids(
        &self,
        layer_idx: usize,
        view_min: Vec2,
        view_max: Vec2,
        filter: ObjectQueryFilter<'_>,
    ) -> Vec<u32> {
        let mut ids = Vec::new();
        for handle in self.query_visible_object_handles(layer_idx, view_min, view_max, filter) {
            if let Some(obj) = self.object_by_handle(handle) {
                ids.push(obj.id);
            }
        }
        ids
    }

    pub(crate) fn object_location(&self, handle: ObjectHandle) -> Option<(usize, usize)> {
        let (layer_idx, object_idx) = self.object_loc_by_handle.get(handle.0 as usize)?.as_ref()?;
        let slot_handle = self
            .object_handles_by_layer
            .get(*layer_idx)?
            .get(*object_idx)?
            .as_ref()?;
        if *slot_handle != handle {
            return None;
        }
        Some((*layer_idx, *object_idx))
    }

    pub(crate) fn object_by_handle(&self, handle: ObjectHandle) -> Option<&IrObject> {
        let (layer_idx, object_idx) = self.object_location(handle)?;
        self.object_layers
            .get(layer_idx)
            .and_then(|layer| layer.objects.get(object_idx))
    }

    pub(crate) fn object_runtime_by_handle(
        &self,
        handle: ObjectHandle,
    ) -> Option<&ObjectRuntimeState> {
        let (layer_idx, object_idx) = self.object_location(handle)?;
        self.object_runtime_by_layer
            .get(layer_idx)?
            .get(object_idx)?
            .as_ref()
    }

    pub(crate) fn query_object_handles_in_coords(
        &self,
        layer_idx: usize,
        coords: &[crate::spatial::ChunkCoord],
    ) -> Vec<ObjectHandle> {
        let Some(layer) = self.object_layers.get(layer_idx) else {
            return Vec::new();
        };
        let mut handles = self
            .index
            .dedup_object_handles_in_coords(coords, layer.bucket_layer);
        handles.sort_by_key(|h| h.0);
        handles
    }

    pub(crate) fn set_object_visible_by_handle(
        &mut self,
        handle: ObjectHandle,
        visible: bool,
    ) -> bool {
        let Some((layer_idx, object_idx)) = self.object_location(handle) else {
            return false;
        };
        let Some(Some(runtime)) = self
            .object_runtime_by_layer
            .get_mut(layer_idx)
            .and_then(|v| v.get_mut(object_idx))
        else {
            return false;
        };
        runtime.visible = visible;
        self.debug_assert_object_sync_consistency(handle);
        true
    }

    pub(crate) fn set_object_alive_by_handle(&mut self, handle: ObjectHandle, alive: bool) -> bool {
        let Some((layer_idx, object_idx)) = self.object_location(handle) else {
            return false;
        };
        let (bucket_layer, offset) = {
            let Some(layer) = self.object_layers.get(layer_idx) else {
                return false;
            };
            (layer.bucket_layer, layer.offset)
        };

        let runtime_snapshot = {
            let Some(Some(runtime)) = self
                .object_runtime_by_layer
                .get_mut(layer_idx)
                .and_then(|v| v.get_mut(object_idx))
            else {
                return false;
            };
            runtime.alive = alive;
            *runtime
        };

        if !alive {
            let _ = self.index.remove_object(handle);
            self.debug_assert_object_sync_consistency(handle);
            return true;
        }

        let Some(placements) = self.object_placements_for_runtime(
            layer_idx,
            object_idx,
            runtime_snapshot,
            bucket_layer,
            offset,
        ) else {
            return false;
        };

        let ok = self.index.update_object_memberships(handle, &placements);
        self.debug_assert_object_sync_consistency(handle);
        ok
    }

    pub(crate) fn update_object_bounds_position_by_handle(
        &mut self,
        handle: ObjectHandle,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) -> bool {
        let Some((layer_idx, object_idx)) = self.object_location(handle) else {
            return false;
        };
        let Some(layer) = self.object_layers.get(layer_idx) else {
            return false;
        };
        if layer.objects.get(object_idx).is_none() {
            return false;
        }
        let runtime_snapshot = {
            let Some(Some(runtime)) = self
                .object_runtime_by_layer
                .get_mut(layer_idx)
                .and_then(|v| v.get_mut(object_idx))
            else {
                return false;
            };

            runtime.x = x;
            runtime.y = y;
            runtime.width = width;
            runtime.height = height;
            *runtime
        };

        if !runtime_snapshot.alive {
            return true;
        }

        let Some(placements) = self.object_placements_for_runtime(
            layer_idx,
            object_idx,
            runtime_snapshot,
            layer.bucket_layer,
            layer.offset,
        ) else {
            return false;
        };

        let ok = self.index.update_object_memberships(handle, &placements);
        self.debug_assert_object_sync_consistency(handle);
        ok
    }

    pub(crate) fn remove_object_by_handle(&mut self, handle: ObjectHandle) -> bool {
        let Some((layer_idx, object_idx)) = self.object_location(handle) else {
            return false;
        };

        if !self.index.remove_object(handle) {
            return false;
        }
        if let Some(slot) = self.object_loc_by_handle.get_mut(handle.0 as usize) {
            *slot = None;
        }
        if let Some(layer_handles) = self.object_handles_by_layer.get_mut(layer_idx) {
            if let Some(slot) = layer_handles.get_mut(object_idx) {
                *slot = None;
            }
        }
        if let Some(runtime_layer) = self.object_runtime_by_layer.get_mut(layer_idx) {
            if let Some(slot) = runtime_layer.get_mut(object_idx) {
                *slot = None;
            }
        }
        self.debug_assert_object_sync_consistency(handle);
        true
    }

    pub(crate) fn spawn_object_in_layer(
        &mut self,
        layer_idx: usize,
        object: IrObject,
    ) -> Option<ObjectHandle> {
        let (object_idx, bucket_layer, layer_offset) = {
            let layer = self.object_layers.get_mut(layer_idx)?;
            let object_idx = layer.objects.len();
            layer.objects.push(object.clone());
            (object_idx, layer.bucket_layer, layer.offset)
        };

        if self.object_handles_by_layer.len() <= layer_idx {
            self.object_handles_by_layer
                .resize_with(layer_idx + 1, Vec::new);
        }
        if self.object_runtime_by_layer.len() <= layer_idx {
            self.object_runtime_by_layer
                .resize_with(layer_idx + 1, Vec::new);
        }

        let handle = self.index.alloc_object_handle();
        let hidx = handle.0 as usize;
        if self.object_loc_by_handle.len() <= hidx {
            self.object_loc_by_handle.resize(hidx + 1, None);
        }
        self.object_loc_by_handle[hidx] = Some((layer_idx, object_idx));
        self.object_handles_by_layer[layer_idx].push(Some(handle));

        let runtime = ObjectRuntimeState {
            alive: true,
            visible: object.visible,
            x: object.x,
            y: object.y,
            width: object.width,
            height: object.height,
        };
        self.object_runtime_by_layer[layer_idx].push(Some(runtime));

        let placements = self.object_placements_for_runtime(
            layer_idx,
            object_idx,
            runtime,
            bucket_layer,
            layer_offset,
        )?;
        let _ = self.index.update_object_memberships(handle, &placements);
        self.debug_assert_object_sync_consistency(handle);
        Some(handle)
    }

    fn object_placements_for_runtime(
        &self,
        layer_idx: usize,
        object_idx: usize,
        runtime: ObjectRuntimeState,
        bucket_layer: LayerIdx,
        layer_offset: Vec2,
    ) -> Option<Vec<(LayerIdx, crate::spatial::ChunkCoord, Vec2)>> {
        let authored = self
            .object_layers
            .get(layer_idx)
            .and_then(|layer| layer.objects.get(object_idx))?;

        let (chunk_min, chunk_max) = object_chunk_span_runtime(authored, runtime, layer_offset);
        let world = vec2(runtime.x, runtime.y) + layer_offset;
        let mut placements = Vec::new();
        for cy in chunk_min.y..=chunk_max.y {
            for cx in chunk_min.x..=chunk_max.x {
                let cc = crate::spatial::ChunkCoord { x: cx, y: cy };
                let chunk_origin = vec2((cc.x * CHUNK_SIZE) as f32, (cc.y * CHUNK_SIZE) as f32);
                placements.push((bucket_layer, cc, world - chunk_origin));
            }
        }
        Some(placements)
    }

    pub(crate) fn debug_assert_object_sync_consistency(&self, handle: ObjectHandle) {
        let Some((layer_idx, object_idx)) = self.object_location(handle) else {
            debug_assert!(self.index.object_memberships(handle).is_none());
            return;
        };
        let Some(runtime) = self
            .object_runtime_by_layer
            .get(layer_idx)
            .and_then(|v| v.get(object_idx))
            .and_then(|r| r.as_ref())
        else {
            debug_assert!(self.index.object_memberships(handle).is_none());
            return;
        };

        let memberships = self.index.object_memberships(handle).unwrap_or(&[]);
        let unique: HashSet<_> = memberships.iter().copied().collect();
        debug_assert_eq!(
            unique.len(),
            memberships.len(),
            "duplicate index memberships for one object handle"
        );

        if runtime.alive {
            debug_assert!(
                !memberships.is_empty(),
                "alive object must have at least one index membership"
            );
        }
    }

    pub(crate) fn visible_coords_for_draw(
        &self,
        view_min: Vec2,
        view_max: Vec2,
        cull_padding: f32,
    ) -> Vec<crate::spatial::ChunkCoord> {
        let min = vec2(view_min.x - cull_padding, view_min.y - cull_padding);
        let max = vec2(view_max.x + cull_padding, view_max.y + cull_padding);

        let mut cx_min = (min.x as i32).div_euclid(CHUNK_SIZE);
        let mut cy_min = (min.y as i32).div_euclid(CHUNK_SIZE);
        let mut cx_max = (max.x as i32).div_euclid(CHUNK_SIZE);
        let mut cy_max = (max.y as i32).div_euclid(CHUNK_SIZE);

        if cx_min > cx_max {
            std::mem::swap(&mut cx_min, &mut cx_max);
        }
        if cy_min > cy_max {
            std::mem::swap(&mut cy_min, &mut cy_max);
        }

        let mut coords = Vec::new();
        for cy in cy_min..=cy_max {
            for cx in cx_min..=cx_max {
                coords.push(crate::spatial::ChunkCoord { x: cx, y: cy });
            }
        }
        coords
    }

    fn ts_for_gid_from<'a>(
        gid: TileId,
        gid_lut: &'a [u16],
        tilesets: &'a [TilesetRuntimeInfo],
    ) -> Option<(&'a TilesetRuntimeInfo, u32)> {
        let clean = gid.clean() as usize;
        if clean >= gid_lut.len() {
            return None;
        }

        let idx = gid_lut[clean];
        if idx == u16::MAX {
            return None;
        }

        let ts = &tilesets[idx as usize];
        Some((ts, gid.clean() - ts.first_gid))
    }

    pub(crate) fn from_ir(ir: IrMap) -> Result<Self, MapError> {
        let mut tilesets = Vec::new();

        let mut max_gid = 0u32;
        for t in &ir.tilesets {
            match t {
                IrTileset::Atlas {
                    first_gid,
                    tilecount,
                    ..
                } => {
                    max_gid = max_gid.max(*first_gid + tilecount - 1);
                }
            }
        }

        let mut gid_lut = vec![u16::MAX; (max_gid + 1) as usize];

        for (i, t) in ir.tilesets.iter().enumerate() {
            match t {
                IrTileset::Atlas {
                    first_gid,
                    image,
                    tile_w,
                    tile_h,
                    tilecount,
                    columns,
                    spacing,
                    margin,
                    ..
                } => {
                    tilesets.push(TilesetRuntimeInfo {
                        first_gid: *first_gid,
                        tilecount: *tilecount,
                        cols: *columns,
                        image: image.clone(),
                        tile_w: *tile_w,
                        tile_h: *tile_h,
                        spacing: *spacing,
                        margin: *margin,
                    });

                    for gid in *first_gid..(*first_gid + *tilecount) {
                        gid_lut[gid as usize] = i as u16;
                    }
                }
            }
        }

        let mut index = GlobalIndex::new();
        let mut object_layers = Vec::new();
        let mut object_loc_by_handle = Vec::new();
        let mut object_handles_by_layer = Vec::new();
        let mut object_runtime_by_layer = Vec::new();
        let mut tile_layers: Vec<TileLayerDrawInfo> = Vec::new();
        let (draw_order, layer_kind_by_id) = build_draw_order_and_kind(&ir.layers);

        for (lz, layer) in ir.layers.iter().enumerate() {
            match &layer.kind {
                IrLayerKind::Objects { objects } => {
                    let bucket_layer = lz as LayerIdx;
                    let layer_idx = object_layers.len();
                    object_layers.push(ObjectLayer {
                        id: lz as LayerId,
                        name: layer.name.clone(),
                        visible: layer.visible,
                        opacity: layer.opacity,
                        offset: layer.offset,
                        properties: layer.properties.clone(),
                        objects: objects.clone(),
                        bucket_layer,
                    });
                    let mut handles_in_layer = Vec::with_capacity(objects.len());
                    let mut runtime_in_layer = Vec::with_capacity(objects.len());

                    for (object_idx, obj) in objects.iter().enumerate() {
                        let handle = index.alloc_object_handle();
                        let handle_idx = handle.0 as usize;
                        if handle_idx >= object_loc_by_handle.len() {
                            object_loc_by_handle.resize(handle_idx + 1, None);
                        }
                        object_loc_by_handle[handle_idx] = Some((layer_idx, object_idx));
                        handles_in_layer.push(Some(handle));
                        runtime_in_layer.push(Some(ObjectRuntimeState {
                            alive: true,
                            visible: obj.visible,
                            x: obj.x,
                            y: obj.y,
                            width: obj.width,
                            height: obj.height,
                        }));

                        let runtime =
                            runtime_in_layer[object_idx].expect("runtime must exist during build");
                        let world = vec2(runtime.x, runtime.y) + layer.offset;
                        let (chunk_min, chunk_max) =
                            object_chunk_span_runtime(obj, runtime, layer.offset);

                        for cy in chunk_min.y..=chunk_max.y {
                            for cx in chunk_min.x..=chunk_max.x {
                                let cc = crate::spatial::ChunkCoord { x: cx, y: cy };
                                let chunk_origin =
                                    vec2((cc.x * CHUNK_SIZE) as f32, (cc.y * CHUNK_SIZE) as f32);
                                index.insert_object(
                                    bucket_layer,
                                    cc,
                                    crate::spatial::ObjectRec {
                                        handle,
                                        rel_pos: world - chunk_origin,
                                    },
                                );
                            }
                        }
                    }
                    object_handles_by_layer.push(handles_in_layer);
                    object_runtime_by_layer.push(runtime_in_layer);
                    debug_assert!(matches!(
                        layer_kind_by_id.get(&(lz as LayerId)),
                        Some(LayerKindInfo::Objects(idx)) if *idx == layer_idx
                    ));
                }
                IrLayerKind::Tiles {
                    width,
                    height: _,
                    data,
                } => {
                    let lid = lz as LayerIdx;
                    let tile_layer_idx = tile_layers.len();

                    let tw = ir.tile_w as f32;
                    let th = ir.tile_h as f32;
                    for (idx, gid) in data.iter().enumerate() {
                        if *gid == 0 {
                            continue;
                        }
                        let col = idx % *width;
                        let row = idx / *width;
                        let mut world = vec2(col as f32 * tw, row as f32 * th);
                        world += layer.offset;
                        let tile_id = TileId(*gid);
                        let Some((ts, _)) = Self::ts_for_gid_from(tile_id, &gid_lut, &tilesets)
                        else {
                            continue;
                        };
                        let draw_origin = tile_draw_origin(world, ir.tile_h, ts.tile_h);
                        let oversized = ts.tile_w > ir.tile_w || ts.tile_h > ir.tile_h;

                        if oversized {
                            let handle = index.alloc_handle();
                            let (chunk_min, chunk_max) =
                                tile_chunk_span(draw_origin, ts.tile_w as f32, ts.tile_h as f32);
                            for cy in chunk_min.y..=chunk_max.y {
                                for cx in chunk_min.x..=chunk_max.x {
                                    let cc = crate::spatial::ChunkCoord { x: cx, y: cy };
                                    index.insert_tile_with_handle(
                                        handle,
                                        tile_id,
                                        lid,
                                        cc,
                                        draw_origin,
                                    );
                                }
                            }
                        } else {
                            index.add_tile(tile_id, lid, draw_origin);
                        }
                    }

                    tile_layers.push(TileLayerDrawInfo {
                        layer_id: lid,
                        visible: layer.visible,
                        opacity: layer.opacity.clamp(0.0, 1.0),
                    });
                    debug_assert!(matches!(
                        layer_kind_by_id.get(&(lz as LayerId)),
                        Some(LayerKindInfo::Tiles(idx)) if *idx == tile_layer_idx
                    ));
                }
                IrLayerKind::Unsupported => {}
            }
        }

        Ok(Self {
            source_ir: ir,
            index,
            tilesets,
            object_layers,
            object_loc_by_handle,
            object_handles_by_layer,
            object_runtime_by_layer,
            gid_lut,
            tile_layers,
            draw_order,
            layer_kind_by_id,
        })
    }
}

#[cfg(test)]
fn object_aabb_world(obj: &IrObject, layer_offset: Vec2) -> (Vec2, Vec2) {
    let origin = vec2(obj.x, obj.y) + layer_offset;

    match &obj.shape {
        IrObjectShape::Rectangle => {
            let x2 = origin.x + obj.width;
            let y2 = origin.y + obj.height;
            (
                vec2(origin.x.min(x2), origin.y.min(y2)),
                vec2(origin.x.max(x2), origin.y.max(y2)),
            )
        }
        IrObjectShape::Point => (origin - vec2(0.5, 0.5), origin + vec2(0.5, 0.5)),
        IrObjectShape::Polygon(points) | IrObjectShape::Polyline(points) => {
            if points.is_empty() {
                return (origin - vec2(0.5, 0.5), origin + vec2(0.5, 0.5));
            }

            let mut min_x = origin.x;
            let mut min_y = origin.y;
            let mut max_x = origin.x;
            let mut max_y = origin.y;

            for p in points {
                let wp = origin + *p;
                min_x = min_x.min(wp.x);
                min_y = min_y.min(wp.y);
                max_x = max_x.max(wp.x);
                max_y = max_y.max(wp.y);
            }

            (vec2(min_x, min_y), vec2(max_x, max_y))
        }
        IrObjectShape::Tile { .. } => {
            // Tile objects are drawn at (x, y - h), so AABB must match that.
            let w = if obj.width > 0.0 { obj.width } else { 1.0 };
            let h = if obj.height > 0.0 { obj.height } else { 1.0 };
            (vec2(origin.x, origin.y - h), vec2(origin.x + w, origin.y))
        }
    }
}

fn object_aabb_world_runtime(
    obj: &IrObject,
    runtime: ObjectRuntimeState,
    layer_offset: Vec2,
) -> (Vec2, Vec2) {
    let origin = vec2(runtime.x, runtime.y) + layer_offset;

    match &obj.shape {
        IrObjectShape::Rectangle => {
            let x2 = origin.x + runtime.width;
            let y2 = origin.y + runtime.height;
            (
                vec2(origin.x.min(x2), origin.y.min(y2)),
                vec2(origin.x.max(x2), origin.y.max(y2)),
            )
        }
        IrObjectShape::Point => (origin - vec2(0.5, 0.5), origin + vec2(0.5, 0.5)),
        IrObjectShape::Polygon(points) | IrObjectShape::Polyline(points) => {
            if points.is_empty() {
                return (origin - vec2(0.5, 0.5), origin + vec2(0.5, 0.5));
            }

            let mut min_x = origin.x;
            let mut min_y = origin.y;
            let mut max_x = origin.x;
            let mut max_y = origin.y;

            for p in points {
                let wp = origin + *p;
                min_x = min_x.min(wp.x);
                min_y = min_y.min(wp.y);
                max_x = max_x.max(wp.x);
                max_y = max_y.max(wp.y);
            }

            (vec2(min_x, min_y), vec2(max_x, max_y))
        }
        IrObjectShape::Tile { .. } => {
            let w = if runtime.width > 0.0 {
                runtime.width
            } else {
                1.0
            };
            let h = if runtime.height > 0.0 {
                runtime.height
            } else {
                1.0
            };
            (vec2(origin.x, origin.y - h), vec2(origin.x + w, origin.y))
        }
    }
}

#[cfg(test)]
pub(crate) fn object_chunk_span(
    obj: &IrObject,
    layer_offset: Vec2,
) -> (crate::spatial::ChunkCoord, crate::spatial::ChunkCoord) {
    let (min, max) = object_aabb_world(obj, layer_offset);
    (world_to_chunk(min), world_to_chunk(max))
}

pub(crate) fn object_chunk_span_runtime(
    obj: &IrObject,
    runtime: ObjectRuntimeState,
    layer_offset: Vec2,
) -> (crate::spatial::ChunkCoord, crate::spatial::ChunkCoord) {
    let (min, max) = object_aabb_world_runtime(obj, runtime, layer_offset);
    (world_to_chunk(min), world_to_chunk(max))
}

pub(crate) fn tile_chunk_span(
    world: Vec2,
    draw_w: f32,
    draw_h: f32,
) -> (crate::spatial::ChunkCoord, crate::spatial::ChunkCoord) {
    let max = vec2(
        world.x + draw_w.max(1.0) - f32::EPSILON,
        world.y + draw_h.max(1.0) - f32::EPSILON,
    );
    (world_to_chunk(world), world_to_chunk(max))
}

pub(crate) fn tile_draw_origin(world: Vec2, map_tile_h: u32, tile_h: u32) -> Vec2 {
    // For orthogonal tile layers, tiles are bottom-aligned to the map cell.
    // This keeps oversized tiles extending upward instead of downward.
    vec2(world.x, world.y + (map_tile_h as f32 - tile_h as f32))
}

fn object_has_tag(obj: &IrObject, tag: &str) -> bool {
    if let Some(v) = obj.properties.get_string("tag") {
        if v == tag {
            return true;
        }
    }
    if let Some(v) = obj.properties.get_string("tags") {
        return v.split(',').any(|t| t.trim() == tag);
    }
    false
}

fn properties_to_json_vec(props: &Properties) -> Vec<JsonValue> {
    let mut entries: Vec<_> = props.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));

    entries
        .into_iter()
        .map(|(name, value)| match value {
            PropertyValue::Bool(v) => json!({
                "name": name,
                "type": "bool",
                "value": v,
            }),
            PropertyValue::I64(v) => json!({
                "name": name,
                "type": "int",
                "value": v,
            }),
            PropertyValue::F32(v) => json!({
                "name": name,
                "type": "float",
                "value": v,
            }),
            PropertyValue::String(v) => json!({
                "name": name,
                "type": "string",
                "value": v,
            }),
        })
        .collect()
}
