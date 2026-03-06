#[cfg(test)]
mod tests {
    use super::*;
    use crate::spatial::TileId;

    #[test]
    fn query_visible_rect_returns_chunks_in_stable_order() {
        let mut index = GlobalIndex::new();
        index.add_tile(TileId(1), 0, vec2(520.0, 520.0)); // (2,2)
        index.add_tile(TileId(1), 0, vec2(0.0, 0.0)); // (0,0)
        index.add_tile(TileId(1), 0, vec2(260.0, 0.0)); // (1,0)
        index.add_tile(TileId(1), 0, vec2(0.0, 260.0)); // (0,1)

        let view = query_visible_rect(&index, vec2(0.0, 0.0), vec2(800.0, 800.0));
        let coords: Vec<ChunkCoord> = view.chunks.iter().map(|c| c.coord).collect();

        assert!(coords
            .windows(2)
            .all(|w| (w[0].y, w[0].x) <= (w[1].y, w[1].x)));
    }
}
