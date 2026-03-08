use super::*;

impl MapData {
    /// Loads runtime/query map data without binding render textures.
    pub fn load(path: &str) -> Result<Self, MapError> {
        let (ir, _) = decode_map_file_to_ir(path)?;
        Self::from_ir(ir)
    }

    pub(crate) fn from_ir(ir: IrMap) -> Result<Self, MapError> {
        let (draw_order, layer_kind_by_id) = build_draw_order_and_kind(&ir.layers);

        let mut index = GlobalIndex::new();
        let object_state =
            object::load::build_object_state_from_layers(&ir.layers, &layer_kind_by_id, &mut index);
        let tile_state = tile::load::build_tile_state_from_ir(&ir, &layer_kind_by_id, &mut index);

        Ok(Self {
            source_ir: ir,
            index,
            object_state,
            tile_state,
            layer_plan: LayerPlan {
                draw_order,
                layer_kind_by_id,
            },
        })
    }
}
