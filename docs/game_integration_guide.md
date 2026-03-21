# Game Integration Guide

This guide shows how to use `macroquad_tiled_clone` in a real game-shaped flow.

The short version is:

- Use `Map` when you want loading, queries, runtime mutation, and rendering in one object.
- Use `MapData` when you want loading, queries, runtime mutation, and save/export without textures or draw calls.

## Mental Model

Internally, `Map` is composed of three parts:

- `MapData`
  - The canonical runtime map state.
  - Owns parsed layers, objects, tile runtime state, query/index data, and save/export logic.
- `MacroquadRenderAssets`
  - The loaded tileset textures and atlas metadata needed for rendering.
- `RenderState`
  - Frame-local draw settings and bookkeeping such as cull padding, debug draw flags, and dedupe stamps.

From a game perspective:

- `MapData` is "what exists in the world right now".
- `MacroquadRenderAssets` is "what the renderer needs to draw it".
- `RenderState` is "how this frame is being drawn".
- `Map` is "the thing your game usually stores and uses".

## When To Use `Map`

Use `Map` if your game is built with Macroquad and you want to:

- load a map and its textures
- draw visible tiles and tile-objects
- query visible objects/tiles for gameplay
- mutate object or tile runtime state
- save the current runtime map state back to JSON

Typical load path:

```rust
use macroquad_tiled_clone::Map;

let mut map = Map::load("assets2/map.json").await?;
```

This is the normal choice for an actual game loop.

## When To Use `MapData`

Use `MapData` if you want the map as runtime/query data only:

- no texture loading
- no draw calls
- useful for tools, tests, simulation, editors, or headless systems

Typical load path:

```rust
use macroquad_tiled_clone::MapData;

let data = MapData::load("assets2/map.json")?;
```

`MapData` still supports visible queries, runtime mutation, and saving. It just does not render.

## A Real Game Flow

This is the intended shape for a Macroquad game:

1. Load the map once at startup or when changing levels.
2. Each frame, compute the camera's visible world rectangle.
3. Draw the visible map.
4. Query visible objects or tiles for gameplay logic.
5. Mutate runtime object/tile state through handles.
6. Save when needed.

## End-To-End Example

```rust
use macroquad::prelude::*;
use macroquad_tiled_clone::{
    Map, ObjectHandle, ObjectQueryFilter, PropertyValue, TileQueryFilter,
};

struct GameState {
    map: Map,
    player_pos: Vec2,
    player_size: Vec2,
    enemies_layer: usize,
    collision_layer: usize,
}

impl GameState {
    async fn load() -> Result<Self, macroquad_tiled_clone::MapError> {
        let mut map = Map::load("assets2/map.json").await?;
        map.set_cull_padding(256.0);

        Ok(Self {
            map,
            player_pos: vec2(160.0, 160.0),
            player_size: vec2(32.0, 32.0),
            enemies_layer: 0,
            collision_layer: 0,
        })
    }

    fn camera_rect(&self) -> Rect {
        let size = vec2(screen_width(), screen_height());
        Rect::new(
            self.player_pos.x - size.x * 0.5,
            self.player_pos.y - size.y * 0.5,
            size.x,
            size.y,
        )
    }

    fn update(&mut self) {
        let speed = 2.0;
        if is_key_down(KeyCode::A) {
            self.player_pos.x -= speed;
        }
        if is_key_down(KeyCode::D) {
            self.player_pos.x += speed;
        }
        if is_key_down(KeyCode::W) {
            self.player_pos.y -= speed;
        }
        if is_key_down(KeyCode::S) {
            self.player_pos.y += speed;
        }

        let view = self.camera_rect();
        let view_min = view.point();
        let view_max = view.point() + view.size();

        // Query visible enemies by object class/tag.
        let enemy_handles = self.map.query_visible_object_handles(
            self.enemies_layer,
            view_min,
            view_max,
            ObjectQueryFilter {
                kind: Some("enemy"),
                tag: Some("active"),
            },
        );

        for handle in enemy_handles {
            self.update_enemy(handle);
        }

        // Example tile query: find visible solid tiles with a specific gid.
        let solid_tiles = self.map.query_visible_tile_handles(
            self.collision_layer,
            view_min,
            view_max,
            TileQueryFilter { gid: Some(17) },
        );

        if is_key_pressed(KeyCode::Space) {
            // Disable those visible tiles at runtime.
            let _changed = self.map.set_tiles_alive_by_handle(&solid_tiles, false);
        }

        if is_key_pressed(KeyCode::F5) {
            self.map
                .save_to_json("out/runtime_map.json")
                .expect("failed to save runtime map");
        }
    }

    fn update_enemy(&mut self, handle: ObjectHandle) {
        let Some(runtime) = self.map.object_runtime_by_handle(handle).copied() else {
            return;
        };

        let dir = (self.player_pos - vec2(runtime.x, runtime.y)).normalize_or_zero();
        let new_x = runtime.x + dir.x;
        let new_y = runtime.y + dir.y;

        let _ = self.map.update_object_bounds_position_by_handle(
            handle,
            new_x,
            new_y,
            runtime.width,
            runtime.height,
        );
    }

    fn draw(&mut self) {
        clear_background(BLACK);

        let view = self.camera_rect();
        let cam = Camera2D {
            target: self.player_pos,
            zoom: vec2(2.0 / view.w, 2.0 / view.h),
            ..Default::default()
        };

        set_camera(&cam);
        self.map.draw(view.point(), view.point() + view.size());

        draw_rectangle(
            self.player_pos.x - self.player_size.x * 0.5,
            self.player_pos.y - self.player_size.y * 0.5,
            self.player_size.x,
            self.player_size.y,
            BLUE,
        );

        set_default_camera();
        draw_text("WASD move | SPACE disable visible gid 17 tiles | F5 save", 20.0, 30.0, 24.0, WHITE);
    }
}

#[macroquad::main("Runtime Map Example")]
async fn main() {
    let mut game = GameState::load().await.expect("failed to load game state");

    loop {
        game.update();
        game.draw();
        next_frame().await;
    }
}
```

## How Gameplay Should Think About Objects

Objects are split into two kinds of data:

- Authored object metadata (`IrObject`)
  - id
  - class name
  - shape
  - custom properties
- Runtime object state (`ObjectRuntimeState`)
  - alive
  - visible
  - x, y
  - width, height

That means:

- authored object data tells you what the object is
- runtime state tells you what the object is doing right now

Typical pattern:

```rust
let handles = map.query_visible_object_handles(layer_idx, view_min, view_max, filter);

for handle in handles {
    let Some(obj) = map.object_by_handle(handle) else { continue; };
    let Some(runtime) = map.object_runtime_by_handle(handle) else { continue; };

    println!(
        "object id={} class={} at ({}, {})",
        obj.id,
        obj.class_name,
        runtime.x,
        runtime.y
    );
}
```

Use the handle as the stable runtime identity.

## How Gameplay Should Think About Tiles

Tiles also have stable runtime handles.

Typical patterns:

- query visible tiles in a layer
- filter by gid
- disable tiles
- replace gids in a region

Examples:

```rust
let handles = map.query_visible_tile_handles(
    layer_idx,
    view_min,
    view_max,
    TileQueryFilter { gid: Some(5) },
);

let disabled = map.set_tiles_alive_by_handle(&handles, false);
println!("disabled {disabled} tiles");
```

```rust
let changed = map.replace_visible_tiles_gid_in_rect(
    layer_idx,
    view_min,
    view_max,
    TileQueryFilter { gid: Some(3) },
    9,
);

println!("changed {} tiles", changed.len());
```

## Rendering Modes

There are two main rendering styles.

### 1. Normal Game Rendering

Use this most of the time:

```rust
map.draw(view_min, view_max);
```

This:

- computes visible chunks
- draws tile layers
- draws tile-objects in object layers
- respects layer order
- optionally draws debug overlays if `set_debug_draw(true)` was enabled

### 2. Manual Multi-Pass Rendering

Use this when you want explicit control over passes:

```rust
let stamp = map.next_frame_stamp();
map.draw_visible_rect_with_stamp(view_min, view_max, stamp);
map.draw_objects_tiles_with_stamp(view_min, view_max, stamp);
map.draw_objects_debug_with_stamp(view_min, view_max, stamp);
```

Important rule:

- if you manually compose object passes in one frame, reuse the same `stamp`

That keeps dedupe behavior correct for multi-chunk objects.

## Save/Export Behavior

Saving is runtime-aware:

```rust
map.save_to_json("out/runtime_map.json")?;
```

What gets saved:

- canonical map/layer/object content
- runtime-mutated object position/size/visibility
- live object content

What does not get saved:

- render state
- textures
- derived index/cache data

Despawned objects (`alive = false`) are omitted from saved object arrays.

## A Practical Architecture For A Game

One reasonable structure is:

- `Map` owns the world/map runtime plus rendering
- your game stores separate entities like player, camera, UI, combat state
- game systems query the map each frame using visible rectangles
- systems mutate map objects/tiles through handles when world state changes

Example ownership split:

- `GameState`
  - `map: Map`
  - `player`
  - `camera`
  - `enemies`
  - `ui`

Map responsibilities:

- map loading
- visible queries
- tile/object runtime mutation
- rendering
- persistence

Game responsibilities:

- player movement
- combat
- inventory
- quest state
- AI decisions
- which map handles correspond to current gameplay targets

## Recommended Usage Patterns

- Load one `Map` per level.
- Keep layer indices or resolve them once during setup.
- Query by visible rectangle instead of scanning everything.
- Store `ObjectHandle` and `TileHandle` when you need stable runtime references.
- Use authored object properties to configure behavior.
- Use runtime state for movement, visibility, enabling/disabling, and save/export.

## Common Mistakes

- Treating `IrObject` as the full runtime object state.
  - Runtime position/visibility/alive state is separate.
- Mutating your own cached copy of object data.
  - Use handle-based map APIs so the index stays synchronized.
- Mixing multiple frame stamps in one manual render composition.
  - Use one shared stamp per frame.
- Using `Map` for tools that do not need textures.
  - Prefer `MapData` for headless/data-only use cases.
