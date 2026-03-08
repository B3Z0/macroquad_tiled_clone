use crate::core::map_data::ObjectRuntimeState;
use crate::ir_map::{IrObject, IrObjectShape};
use crate::spatial::world_to_chunk;
use macroquad::prelude::{vec2, Vec2};

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
