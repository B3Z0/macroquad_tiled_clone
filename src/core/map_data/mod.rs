//! Canonical mutable map runtime model.
//!
//! This module owns canonical runtime map content and derived query/index sync helpers.

use crate::ir_map::*;
use crate::loader::json_loader::*;
use crate::spatial::{world_to_chunk, GlobalIndex, LayerIdx, ObjectHandle, TileId, CHUNK_SIZE};
use crate::MapError;
use macroquad::prelude::*;
use serde_json::{json, Value as JsonValue};
use std::collections::{HashMap, HashSet};
use std::path::Path;

mod load;
mod object;
mod persistence;
mod shared;
mod tile;

#[cfg(test)]
pub(crate) use shared::geometry::object_chunk_span;
#[cfg(test)]
pub(crate) use shared::geometry::object_chunk_span_runtime;
#[cfg(test)]
pub(crate) use tile::draw::tile_draw_origin;

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

pub(crate) use shared::layer_plan::{build_draw_order_and_kind, LayerKindInfo};

/// Runtime/query data for a loaded map, independent from render-frame state.
///
/// Canonical boundary:
/// - Canonical mutable runtime truth is stored in layer/object/tile metadata fields.
/// - `index` is derived query/cache state synchronized from canonical content.
pub struct MapData {
    pub(crate) source_ir: IrMap,
    pub(crate) derived_index: GlobalIndex,
    pub(crate) object_state: ObjectState,
    pub(crate) tile_state: TileState,
    pub(crate) layer_plan: LayerPlan,
}

pub(crate) struct ObjectState {
    pub(crate) object_layers: Vec<ObjectLayer>,
    pub(crate) object_location_by_handle: Vec<Option<(usize, usize)>>,
    pub(crate) object_handles_by_layer: Vec<Vec<Option<ObjectHandle>>>,
    pub(crate) object_runtime_by_layer: Vec<Vec<Option<ObjectRuntimeState>>>,
}

pub(crate) struct TileState {
    pub(crate) tileset_runtime_info: Vec<TilesetRuntimeInfo>,
    pub(crate) gid_lut: Vec<u16>, // lookup table for tile GIDs to tileset indices
    pub(crate) tile_layer_draw_info: Vec<TileLayerDrawInfo>,
}

pub(crate) struct LayerPlan {
    pub(crate) draw_order: Vec<LayerId>,
    pub(crate) layer_kind_by_id: HashMap<LayerId, LayerKindInfo>,
}
