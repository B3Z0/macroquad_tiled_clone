# macroquad_tiled_clone

Minimal Tiled JSON loader & renderer for Macroquad.

## Roadmap

- **Phase 1 (MVP):**  
  - Parse basic Tiled JSON (map metadata + single tile layer)  
  - Load one Texture2D, compute sprite rects  
  - Double‐loop draw with `draw_texture_ex`

- **Phase 2:**  
  - Named layers, batching, image/object layers

- **Phase 3+:**  
  - Animations, properties, infinite maps, isometric, extensions…

## Getting Started

1. Add to your project:  
   ```toml
   macroquad_tiled_clone = { git = "https://github.com/yourusername/macroquad_tiled_clone" }
