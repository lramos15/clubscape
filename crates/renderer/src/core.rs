//! Platform-agnostic renderer state: loaded assets, the current scene, the camera, WorldView
//! entities placed as temporary scene objects, the per-frame triangle stream and picking.
//! The wasm/web layer and native tools drive this and hand the stream to a GPU rasterizer.

use std::collections::HashMap;

use serde::Deserialize;

use crate::chunk::Chunks;
use crate::error::RenderError;
use crate::model::{Bounds, Model, parse_model_pack};
use crate::model_draw::{ModelDrawer, ModelScratch};
use crate::palette::Palette;
use crate::raster::software::{Software, TextureSource};
use crate::raster::{DrawStats, Fill, RasterState, Tri};
use crate::scene::SceneData;
use crate::scene::block::{self, BLOCK_SIZE, Block};
use crate::scene::draw::{PickTarget, SceneDrawer, SceneView, TemporaryEntity};
use crate::texture::{Texture, TextureSet};

/// One original client cycle in milliseconds (animation frame lengths are in cycles).
pub const CLIENT_CYCLE_MS: f64 = 20.0;

/// Approved player base NPC (penguin 2063) used when the world view supplies no player model.
pub const PLAYER_BASE_NPC: i32 = 2063;

#[derive(Clone, Debug)]
pub struct Frame {
    pub lengths: Vec<i32>,
    pub xs: Vec<Vec<f32>>,
    pub ys: Vec<Vec<f32>>,
    pub zs: Vec<Vec<f32>>,
    pub bounds: Vec<Bounds>,
    pub sphere_radius: Vec<i32>,
}

/// Baked original animation frames for one NPC: lit base model plus per-sequence positions.
#[derive(Clone, Debug)]
pub struct NpcPack {
    pub base: Model,
    pub width_scale: i32,
    pub height_scale: i32,
    pub sequences: HashMap<i32, Frame>,
}

impl NpcPack {
    pub fn from_chunks(data: &[u8]) -> Result<Self, RenderError> {
        let base = Model::from_chunks(data)?;
        let chunks = Chunks::parse(data)?;
        let anim = chunks.ints("ANIM")?;
        let positions = chunks.shorts("FRMS")?;
        let bounds = chunks.ints("FBND")?;
        let scale = chunks.ints_opt("SCAL")?.unwrap_or_else(|| vec![128, 128]);
        let mut sequences = HashMap::new();
        let mut cursor = 0usize;
        let mut pos_cursor = 0usize;
        let mut bounds_cursor = 0usize;
        let vertex_count = base.vertex_count;
        while cursor + 2 <= anim.len() {
            let id = anim[cursor];
            let frames = anim[cursor + 1] as usize;
            cursor += 2;
            if cursor + frames > anim.len() {
                return Err(RenderError::Format("animation table truncated".into()));
            }
            let lengths = anim[cursor..cursor + frames].to_vec();
            cursor += frames;
            let mut frame = Frame {
                lengths,
                xs: Vec::new(),
                ys: Vec::new(),
                zs: Vec::new(),
                bounds: Vec::new(),
                sphere_radius: Vec::new(),
            };
            for _ in 0..frames {
                let end = pos_cursor + vertex_count * 3;
                if end > positions.len() || bounds_cursor + 6 > bounds.len() {
                    return Err(RenderError::Format("animation frame data truncated".into()));
                }
                let mut xs = Vec::with_capacity(vertex_count);
                let mut ys = Vec::with_capacity(vertex_count);
                let mut zs = Vec::with_capacity(vertex_count);
                for v in 0..vertex_count {
                    xs.push(f32::from(positions[pos_cursor + v * 3]));
                    ys.push(f32::from(positions[pos_cursor + v * 3 + 1]));
                    zs.push(f32::from(positions[pos_cursor + v * 3 + 2]));
                }
                pos_cursor = end;
                let b = &bounds[bounds_cursor..bounds_cursor + 6];
                frame.bounds.push(Bounds {
                    bucket_offset: b[0],
                    radius: b[1],
                    bottom: b[2],
                    height: b[3],
                    bucket_range: b[4],
                });
                frame.sphere_radius.push(b[5]);
                bounds_cursor += 6;
                frame.xs.push(xs);
                frame.ys.push(ys);
                frame.zs.push(zs);
            }
            sequences.insert(id, frame);
        }
        Ok(Self {
            base,
            width_scale: scale[0],
            height_scale: scale[1],
            sequences,
        })
    }

    /// Frame index for `elapsed_ms` since the animation started, looping like the original
    /// per-cycle frame counter.
    pub fn frame_index(&self, sequence: i32, elapsed_ms: f64) -> Option<usize> {
        let seq = self.sequences.get(&sequence)?;
        let total: i32 = seq.lengths.iter().sum();
        if total <= 0 {
            return Some(0);
        }
        let cycles =
            ((elapsed_ms / CLIENT_CYCLE_MS).floor().max(0.0) as i64 % i64::from(total)) as i32;
        let mut acc = 0;
        for (i, &len) in seq.lengths.iter().enumerate() {
            acc += len;
            if cycles < acc {
                return Some(i);
            }
        }
        Some(seq.lengths.len() - 1)
    }

    /// Builds the original animated model for a frame (positions and float-derived bounds).
    pub fn frame_model(&self, sequence: i32, frame: usize) -> Option<Model> {
        let seq = self.sequences.get(&sequence)?;
        let mut model = self.base.clone();
        model.xs = seq.xs.get(frame)?.clone();
        model.ys = seq.ys[frame].clone();
        model.zs = seq.zs[frame].clone();
        model.bounds = seq.bounds[frame];
        Some(model)
    }
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct WorldTile {
    pub x: i32,
    pub y: i32,
    pub plane: i32,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct WorldEntity {
    pub id: String,
    pub kind: String,
    pub source_id: Option<i32>,
    pub tile: WorldTile,
    pub animation: String,
    pub available: Option<bool>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct WorldPlayer {
    pub id: String,
    pub tile: WorldTile,
    pub animation: String,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct WorldViewInput {
    pub revision: String,
    pub tick: String,
    pub player: WorldPlayer,
    pub entities: Vec<WorldEntity>,
}

#[derive(Clone, Debug)]
struct EntityState {
    id: String,
    npc: i32,
    tile: WorldTile,
    orientation: i32,
    sequence: i32,
    sequence_started_ms: f64,
    is_player: bool,
}

/// Camera in world units (tile * 128) matching `RenderCamera` from the shared contract.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub x: i32,
    pub height: i32,
    pub y: i32,
    pub pitch: i32,
    pub yaw: i32,
    pub zoom: i32,
    pub far: i32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            x: 0,
            height: -1000,
            y: 0,
            pitch: 2048,
            yaw: 0,
            zoom: 662,
            far: 3500,
        }
    }
}

/// A pick resolved to world terms (see [`RendererCore::pick_world`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorldPick {
    Tile {
        x: i32,
        y: i32,
        plane: i32,
    },
    /// A WorldView actor (player or NPC) by its WorldView id.
    Actor {
        id: String,
        x: i32,
        y: i32,
        plane: i32,
    },
    /// Original scenery: source object id, tag type (0-3), origin tile and tile span, plus the
    /// WorldView `object` entity standing on it when the shell listed one.
    Scenery {
        object_id: i32,
        kind: i32,
        x: i32,
        y: i32,
        plane: i32,
        span_x: i32,
        span_y: i32,
        entity: Option<String>,
    },
}

/// Parameters of an approved model capture (`drawFrustum(0, yaw, 0, 128, 0, camera_y, camera_z)`).
#[derive(Clone, Debug, Default)]
pub struct ModelFixture {
    pub model: String,
    pub npc: Option<(i32, i32, usize)>,
    pub yaw: i32,
    pub camera_y: i32,
    pub camera_z: i32,
}

impl Camera {
    /// The original viewport zoom for a viewport height (`client` resizable zoom curve):
    /// 662 at 1080 px, 883 at 1440 px, 471 at 768 px. Shells pass this as `zoom` unless they
    /// reproduce a different source-defined zoom state.
    pub fn source_zoom_for_height(height: i32) -> i32 {
        let n = height - 334;
        let d = if n < 0 {
            256
        } else if n >= 100 {
            205
        } else {
            (205 - 256) * n / 100 + 256
        };
        (f64::from(height) * f64::from(d) / 334.0) as i32
    }
}

#[derive(Clone, Debug, Default)]
pub struct FrameSummary {
    pub triangles: usize,
    pub stats: DrawStats,
    pub entities_drawn: usize,
    pub entities_skipped: Vec<String>,
    pub missing_models: usize,
    pub cpu_build_ms: f64,
}

pub struct RendererCore {
    pub palette: Palette,
    pub textures: TextureSet,
    pub state: RasterState,
    pub camera: Camera,
    scene: Option<SceneData>,
    scene_models: Vec<Option<Model>>,
    scene_id: Option<String>,
    drawer: Option<SceneDrawer>,
    npc_packs: HashMap<i32, NpcPack>,
    /// Standalone lit models (e.g. the tree fixture) addressed by manifest id.
    models: HashMap<String, Model>,
    /// World blocks (64x64 map squares) keyed by square id, with their model packs.
    blocks: HashMap<i32, (Block, Vec<(String, Model)>)>,
    /// WorldView scenery entities (`object`/`temporary_object`): id, source object id, tile.
    scenery_entities: Vec<(String, i32, WorldTile)>,
    /// Animation clock origin (ms) of the current scene.
    scene_started_ms: f64,
    entities: Vec<EntityState>,
    plane: i32,
    tris: Vec<Tri>,
    picks: Vec<PickTarget>,
    pick_buffer: Option<Vec<i32>>,
    pub last_summary: FrameSummary,
    pub loaded_asset_hashes: Vec<(String, String)>,
}

impl RendererCore {
    pub fn new(palette: Palette, width: i32, height: i32) -> Self {
        Self {
            palette,
            textures: TextureSet::default(),
            state: RasterState::new(width, height, 662),
            camera: Camera::default(),
            scene: None,
            scene_models: Vec::new(),
            scene_id: None,
            drawer: None,
            npc_packs: HashMap::new(),
            models: HashMap::new(),
            blocks: HashMap::new(),
            scenery_entities: Vec::new(),
            scene_started_ms: 0.0,
            entities: Vec::new(),
            plane: 0,
            tris: Vec::new(),
            picks: Vec::new(),
            pick_buffer: None,
            last_summary: FrameSummary::default(),
            loaded_asset_hashes: Vec::new(),
        }
    }

    pub fn add_texture(&mut self, bytes: &[u8]) -> Result<i32, RenderError> {
        let texture = Texture::from_chunks(bytes)?;
        let id = texture.id;
        self.textures.insert(texture);
        Ok(id)
    }

    /// Registers an NPC pack under its NPC id (the manifest maps ids to pack files).
    pub fn load_npc_pack_as(&mut self, npc_id: i32, bytes: &[u8]) -> Result<(), RenderError> {
        let pack = NpcPack::from_chunks(bytes)?;
        self.npc_packs.insert(npc_id, pack);
        Ok(())
    }

    pub fn has_npc_pack(&self, npc_id: i32) -> bool {
        self.npc_packs.contains_key(&npc_id)
    }

    pub fn load_scene(
        &mut self,
        id: &str,
        scene_bytes: &[u8],
        pack_bytes: &[u8],
    ) -> Result<(), RenderError> {
        let scene = SceneData::from_chunks(scene_bytes)?;
        let entries = parse_model_pack(pack_bytes)?;
        if entries.len() != scene.model_keys.len() {
            return Err(RenderError::InvalidAsset(format!(
                "scene {id} references {} models but the pack holds {}",
                scene.model_keys.len(),
                entries.len()
            )));
        }
        let mut models = Vec::with_capacity(entries.len());
        for ((key, model), expected) in entries.into_iter().zip(&scene.model_keys) {
            if &key != expected {
                return Err(RenderError::InvalidAsset(format!(
                    "scene {id} model pack order mismatch at {expected}"
                )));
            }
            models.push(Some(model));
        }
        self.install_scene(id, scene, models)
    }

    fn install_scene(
        &mut self,
        id: &str,
        scene: SceneData,
        models: Vec<Option<Model>>,
    ) -> Result<(), RenderError> {
        let mut missing_textures = Vec::new();
        for model in models.iter().flatten() {
            if let Some(t) = &model.textures {
                for &tex in &t[..model.face_count] {
                    if tex != -1
                        && self.textures.get(i32::from(tex)).is_none()
                        && !missing_textures.contains(&tex)
                    {
                        missing_textures.push(tex);
                    }
                }
            }
        }
        for paint in scene.paints.values() {
            if paint.texture != -1
                && self.textures.get(paint.texture).is_none()
                && !missing_textures.contains(&(paint.texture as i16))
            {
                missing_textures.push(paint.texture as i16);
            }
        }
        if !missing_textures.is_empty() {
            return Err(RenderError::MissingAsset(format!(
                "scene {id} needs textures {missing_textures:?} that are not loaded"
            )));
        }
        self.drawer = Some(SceneDrawer::new(
            &scene,
            self.state,
            &self.palette.rgb,
            self.camera.far.max(50),
        ));
        self.scene = Some(scene);
        self.scene_models = models;
        self.scene_id = Some(id.to_string());
        self.pick_buffer = None;
        Ok(())
    }

    /// Loads a world block (64x64 map square) and its model pack for later assembly.
    pub fn load_block(
        &mut self,
        square: i32,
        block_bytes: &[u8],
        pack_bytes: &[u8],
    ) -> Result<(), RenderError> {
        let block = Block::from_chunks(block_bytes)?;
        if block.square != square {
            return Err(RenderError::InvalidAsset(format!(
                "block file is square {} but was registered as {square}",
                block.square
            )));
        }
        let models = parse_model_pack(pack_bytes)?;
        if models.len() != block.model_keys.len() {
            return Err(RenderError::InvalidAsset(format!(
                "block {square} pack holds {} models for {} keys",
                models.len(),
                block.model_keys.len()
            )));
        }
        self.blocks.insert(square, (block, models));
        Ok(())
    }

    pub fn has_block(&self, square: i32) -> bool {
        self.blocks.contains_key(&square)
    }

    pub fn unload_block(&mut self, square: i32) {
        self.blocks.remove(&square);
    }

    /// Map squares the original loader would need for a scene at `base` (the extended grid,
    /// `base - 40 .. base + 144`), in `x << 8 | y` form.
    pub fn squares_for_base(base_x: i32, base_y: i32) -> Vec<i32> {
        let mut out = Vec::new();
        for x in (base_x - 40) >> 6..=(base_x + 143) >> 6 {
            for y in (base_y - 40) >> 6..=(base_y + 143) >> 6 {
                out.push((x << 8) | y);
            }
        }
        out
    }

    /// The original scene base for a player tile: the 104x104 scene is centred on the player's
    /// chunk (`(tile >> 3) - 6) * 8`).
    pub fn base_for_tile(x: i32, y: i32) -> (i32, i32) {
        (((x >> 3) - 6) * 8, ((y >> 3) - 6) * 8)
    }

    /// Whether a tile sits within `margin` tiles of the current scene's main-area edge (the
    /// original rebuilds its scene when the player comes within 16 tiles of the edge).
    pub fn needs_recenter(&self, x: i32, y: i32, margin: i32) -> bool {
        match &self.scene {
            Some(scene) => {
                let lx = x - scene.base_x;
                let ly = y - scene.base_y;
                lx < margin || ly < margin || lx >= 104 - margin || ly >= 104 - margin
            }
            None => true,
        }
    }

    /// Assembles a scene around `base` from the loaded blocks (missing squares stay empty, as
    /// unloaded map squares do in the original). `randomize_phases` gives animated scenery the
    /// original random start frames; tests pass `false` for reproducibility.
    pub fn assemble_scene(
        &mut self,
        base_x: i32,
        base_y: i32,
        randomize_phases: bool,
        now_ms: f64,
    ) -> Result<Vec<i32>, RenderError> {
        if base_x % 8 != 0 || base_y % 8 != 0 {
            return Err(RenderError::Scene(format!(
                "scene base {base_x},{base_y} is not on the original 8-tile chunk lattice"
            )));
        }
        let wanted = Self::squares_for_base(base_x, base_y);
        let mut present: Vec<(&Block, &[(String, Model)])> = Vec::new();
        let mut missing = Vec::new();
        for square in &wanted {
            match self.blocks.get(square) {
                Some((block, models)) => present.push((block, models.as_slice())),
                None => missing.push(*square),
            }
        }
        if present.is_empty() {
            return Err(RenderError::MissingAsset(format!(
                "no blocks loaded for scene base {base_x},{base_y} (needs {wanted:?})"
            )));
        }
        let (scene, models) = block::assemble(base_x, base_y, &present, randomize_phases)?;
        let id = format!("blocks@{base_x},{base_y}");
        self.install_scene(&id, scene, models)?;
        self.scene_started_ms = now_ms;
        Ok(missing)
    }

    /// Client cycles elapsed on the scene animation clock.
    pub fn animation_cycles(&self, now_ms: f64) -> i64 {
        ((now_ms - self.scene_started_ms) / CLIENT_CYCLE_MS)
            .floor()
            .max(0.0) as i64
    }

    /// World tile bounds of a map square (`x << 8 | y`).
    pub fn square_origin(square: i32) -> (i32, i32) {
        ((square >> 8) * BLOCK_SIZE, (square & 0xFF) * BLOCK_SIZE)
    }

    pub fn scene_id(&self) -> Option<&str> {
        self.scene_id.as_deref()
    }

    pub fn scene_base(&self) -> Option<(i32, i32)> {
        self.scene.as_ref().map(|s| (s.base_x, s.base_y))
    }

    pub fn resize(&mut self, width: i32, height: i32) {
        let zoom = self.state.zoom;
        self.state = RasterState::new(width, height, zoom);
        if let Some(scene) = &self.scene {
            self.drawer = Some(SceneDrawer::new(
                scene,
                self.state,
                &self.palette.rgb,
                self.camera.far.max(50),
            ));
        }
        self.pick_buffer = None;
    }

    pub fn set_camera(&mut self, camera: Camera) -> Result<(), RenderError> {
        if camera.zoom <= 0 {
            return Err(RenderError::Scene("camera zoom must be positive".into()));
        }
        if camera.far < 50 {
            return Err(RenderError::Scene(
                "camera far clip must be at least the source near plane (50)".into(),
            ));
        }
        let far_changed = camera.far != self.camera.far;
        self.camera = camera;
        self.state.zoom = camera.zoom;
        if far_changed && let Some(scene) = &self.scene {
            self.drawer = Some(SceneDrawer::new(
                scene,
                self.state,
                &self.palette.rgb,
                camera.far,
            ));
        }
        Ok(())
    }

    /// Applies an authoritative world view: entity placement and animation identity only.
    pub fn update_world(&mut self, json: &str, now_ms: f64) -> Result<(), RenderError> {
        let view: WorldViewInput = serde_json::from_str(json)
            .map_err(|e| RenderError::Scene(format!("world view json: {e}")))?;
        let mut next: Vec<EntityState> = Vec::new();
        let apply = |id: &str,
                     npc: i32,
                     tile: &WorldTile,
                     animation: &str,
                     is_player: bool,
                     previous: &[EntityState]| {
            let sequence = animation.trim().parse::<i32>().unwrap_or(-1);
            let old = previous.iter().find(|e| e.id == id);
            let mut orientation = old.map(|o| o.orientation).unwrap_or(0);
            if let Some(o) = old
                && (o.tile.x != tile.x || o.tile.y != tile.y)
            {
                orientation = facing(o.tile.x, o.tile.y, tile.x, tile.y);
            }
            let started = match old {
                Some(o) if o.sequence == sequence => o.sequence_started_ms,
                _ => now_ms,
            };
            EntityState {
                id: id.to_string(),
                npc,
                tile: tile.clone(),
                orientation,
                sequence,
                sequence_started_ms: started,
                is_player,
            }
        };
        self.scenery_entities = view
            .entities
            .iter()
            .filter(|e| e.kind == "object" || e.kind == "temporary_object")
            .filter_map(|e| e.source_id.map(|id| (e.id.clone(), id, e.tile.clone())))
            .collect();
        let previous = std::mem::take(&mut self.entities);
        next.push(apply(
            &view.player.id,
            PLAYER_BASE_NPC,
            &view.player.tile,
            &view.player.animation,
            true,
            &previous,
        ));
        for entity in &view.entities {
            if entity.kind != "npc" && entity.kind != "player" {
                continue;
            }
            let Some(npc) = entity.source_id else {
                continue;
            };
            next.push(apply(
                &entity.id,
                npc,
                &entity.tile,
                &entity.animation,
                entity.kind == "player",
                &previous,
            ));
        }
        self.plane = view.player.tile.plane;
        self.entities = next;
        Ok(())
    }

    /// Builds this frame's painter-ordered triangle stream. Returns a summary; the stream is
    /// available through [`Self::triangles`].
    pub fn build_frame(&mut self, now_ms: f64) -> Result<&FrameSummary, RenderError> {
        let start = now();
        let animation_cycles = self.animation_cycles(now_ms);
        let scene = self
            .scene
            .as_ref()
            .ok_or_else(|| RenderError::Scene("no scene loaded".into()))?;
        let drawer = self.drawer.as_mut().expect("drawer follows scene");
        let base_x = scene.base_x;
        let base_y = scene.base_y;
        drawer.begin_frame(scene);
        let mut temp_models: Vec<Model> = Vec::new();
        let mut skipped = Vec::new();
        let mut drawn = 0usize;
        for entity in &self.entities {
            let Some(pack) = self.npc_packs.get(&entity.npc) else {
                skipped.push(format!(
                    "{}: no animation pack for npc {}",
                    entity.id, entity.npc
                ));
                continue;
            };
            let sequence = if pack.sequences.contains_key(&entity.sequence) {
                entity.sequence
            } else {
                // Fall back to the pack's first sequence (idle) when the world view names an
                // animation this pack does not carry; report it rather than hide the actor.
                let mut ids: Vec<i32> = pack.sequences.keys().copied().collect();
                ids.sort_unstable();
                let Some(&first) = ids.first() else {
                    skipped.push(format!("{}: pack has no sequences", entity.id));
                    continue;
                };
                if entity.sequence >= 0 {
                    skipped.push(format!(
                        "{}: sequence {} not baked, using {}",
                        entity.id, entity.sequence, first
                    ));
                }
                first
            };
            let elapsed = now_ms - entity.sequence_started_ms;
            let Some(frame) = pack.frame_index(sequence, elapsed) else {
                continue;
            };
            let Some(model) = pack.frame_model(sequence, frame) else {
                continue;
            };
            let local_x = entity.tile.x - base_x;
            let local_y = entity.tile.y - base_y;
            if local_x < 0 || local_y < 0 || local_x >= scene.max_x || local_y >= scene.max_y {
                skipped.push(format!("{}: tile outside loaded scene", entity.id));
                continue;
            }
            let plane = entity.tile.plane.clamp(0, scene.planes - 1);
            let x = local_x * 128 + 64;
            let z = local_y * 128 + 64;
            let height = tile_height(scene, plane, x, z);
            let model_index = temp_models.len();
            temp_models.push(model);
            let ok = drawer.add_temporary(
                scene,
                &TemporaryEntity {
                    plane,
                    tile_x: local_x,
                    tile_y: local_y,
                    size_x: 1,
                    size_y: 1,
                    x,
                    height,
                    z,
                    orientation: entity.orientation,
                    hash: entity_hash(&entity.id, entity.is_player),
                    model: model_index,
                },
            );
            if ok {
                drawn += 1;
            } else {
                skipped.push(format!("{}: tile slots full", entity.id));
            }
        }
        let view = SceneView {
            camera_x: self.camera.x - base_x * 128,
            camera_height: self.camera.height,
            camera_z: self.camera.y - base_y * 128,
            pitch: self.camera.pitch,
            yaw: self.camera.yaw,
            plane: self.plane,
            focal_x: self.camera.x - base_x * 128,
            focal_z: self.camera.y - base_y * 128,
            center_on_camera: true,
            far_clip: self.camera.far,
            animation_cycles,
        };
        if self.camera.zoom > 0 {
            self.state.zoom = self.camera.zoom;
        }
        drawer.state = self.state;
        self.tris.clear();
        drawer.draw(
            scene,
            &self.scene_models,
            &temp_models,
            &view,
            &mut self.tris,
        );
        self.picks = drawer.picks.clone();
        let mut stats = DrawStats::default();
        for tri in &self.tris {
            stats.count(tri);
        }
        self.pick_buffer = None;
        self.last_summary = FrameSummary {
            triangles: self.tris.len(),
            stats,
            entities_drawn: drawn,
            entities_skipped: skipped,
            missing_models: drawer.missing_models.len(),
            cpu_build_ms: now() - start,
        };
        Ok(&self.last_summary)
    }

    pub fn triangles(&self) -> &[Tri] {
        &self.tris
    }

    pub fn load_model(&mut self, id: &str, bytes: &[u8]) -> Result<(), RenderError> {
        let model = Model::from_chunks(bytes)?;
        self.models.insert(id.to_string(), model);
        Ok(())
    }

    /// Developer fixture replay: draws one model through the original legacy draw
    /// (`fx.be`, 2048 units per turn, zoom 1024) exactly as the approved model captures did.
    /// `npc` selects a baked animation frame from a loaded NPC pack; otherwise `model` names a
    /// standalone lit model. The frame background is the capture's 0x303030.
    pub fn build_model_fixture_frame(
        &mut self,
        fixture: &ModelFixture,
    ) -> Result<&FrameSummary, RenderError> {
        let start = now();
        let model = match fixture.npc {
            Some((npc, sequence, frame)) => self
                .npc_packs
                .get(&npc)
                .ok_or_else(|| RenderError::Scene(format!("npc pack {npc} not loaded")))?
                .frame_model(sequence, frame)
                .ok_or_else(|| {
                    RenderError::Scene(format!(
                        "npc {npc} has no sequence {sequence} frame {frame}"
                    ))
                })?,
            None => self
                .models
                .get(&fixture.model)
                .ok_or_else(|| RenderError::Scene(format!("model {} not loaded", fixture.model)))?
                .clone(),
        };
        let mut state = RasterState::new(self.state.width, self.state.height, 1024);
        state.zoom = 1024;
        self.state = state;
        let mut scratch = ModelScratch::default();
        self.tris.clear();
        {
            let mut drawer = ModelDrawer {
                state,
                palette: &self.palette.rgb,
                scratch: &mut scratch,
                alpha_pass: 2,
            };
            drawer
                .draw_legacy(
                    &model,
                    0,
                    fixture.yaw,
                    0,
                    128,
                    0,
                    fixture.camera_y,
                    fixture.camera_z,
                    0,
                    &mut self.tris,
                )
                .map_err(|_| RenderError::Scene("model fixture draw aborted".into()))?;
        }
        self.picks.clear();
        self.pick_buffer = None;
        self.entities.clear();
        let mut stats = DrawStats::default();
        for tri in &self.tris {
            stats.count(tri);
        }
        self.last_summary = FrameSummary {
            triangles: self.tris.len(),
            stats,
            entities_drawn: 1,
            entities_skipped: Vec::new(),
            missing_models: 0,
            cpu_build_ms: now() - start,
        };
        Ok(&self.last_summary)
    }

    pub fn pick_targets(&self) -> &[PickTarget] {
        &self.picks
    }

    /// Picks in world terms: actors resolve to their WorldView ids, scenery to the original
    /// object id/type and its tile (matched to a WorldView `object` entity when one sits there).
    pub fn pick_world(&mut self, x: i32, y: i32) -> Option<WorldPick> {
        let target = self.pick(x, y)?;
        let scene = self.scene.as_ref()?;
        let (base_x, base_y) = (scene.base_x, scene.base_y);
        Some(match target {
            PickTarget::Tile { plane, x, y } => WorldPick::Tile {
                x: x + base_x,
                y: y + base_y,
                plane,
            },
            PickTarget::Object { hash, plane, x, y } => {
                if let Some(entity) = self
                    .entities
                    .iter()
                    .find(|e| entity_hash(&e.id, e.is_player) == hash)
                {
                    return Some(WorldPick::Actor {
                        id: entity.id.clone(),
                        x: entity.tile.x,
                        y: entity.tile.y,
                        plane: entity.tile.plane,
                    });
                }
                // Original object tag layout: x | y << 7 | type << 14 | plane << 16 | id << 20.
                let object_id = ((hash >> 20) & 0xFFFF_FFFF) as i32;
                let kind = ((hash >> 14) & 3) as i32;
                let (mut tx, mut ty, mut span) = (x + base_x, y + base_y, (1, 1));
                if let Some(object) = scene.game_objects.iter().find(|o| o.hash == hash) {
                    tx = object.min_x + base_x;
                    ty = object.min_y + base_y;
                    span = (
                        object.max_x - object.min_x + 1,
                        object.max_y - object.min_y + 1,
                    );
                }
                let entity = self
                    .scenery_entities
                    .iter()
                    .find(|(_, source_id, tile)| {
                        *source_id == object_id
                            && tile.plane == plane
                            && tile.x >= tx
                            && tile.x < tx + span.0
                            && tile.y >= ty
                            && tile.y < ty + span.1
                    })
                    .map(|(id, _, _)| id.clone());
                WorldPick::Scenery {
                    object_id,
                    kind,
                    x: tx,
                    y: ty,
                    plane,
                    span_x: span.0,
                    span_y: span.1,
                    entity,
                }
            }
        })
    }

    /// Resolves the topmost drawn triangle under a pixel using the exact fill coverage.
    pub fn pick(&mut self, x: i32, y: i32) -> Option<PickTarget> {
        if x < 0 || y < 0 || x >= self.state.width || y >= self.state.height {
            return None;
        }
        if self.pick_buffer.is_none() {
            let mut buffer = vec![0i32; (self.state.width * self.state.height) as usize];
            {
                let mut raster =
                    Software::new(self.state, &mut buffer, &self.palette.rgb, &NoTexels);
                for tri in &self.tris {
                    if tri.pick == 0 {
                        continue;
                    }
                    let id_tri = Tri {
                        fill: Fill::Flat {
                            rgb: tri.pick as i32,
                        },
                        alpha: 0,
                        ..*tri
                    };
                    let _ = raster.draw(&id_tri);
                }
            }
            self.pick_buffer = Some(buffer);
        }
        let id = self.pick_buffer.as_ref()?[(y * self.state.width + x) as usize];
        if id <= 0 {
            return None;
        }
        self.picks.get(id as usize - 1).copied()
    }

    pub fn scene(&self) -> Option<&SceneData> {
        self.scene.as_ref()
    }
}

struct NoTexels;
impl TextureSource for NoTexels {
    fn texels(&self, _id: i32) -> Option<&[i32]> {
        None
    }
    fn opaque(&self, _id: i32) -> bool {
        false
    }
    fn average(&self, _id: i32) -> i32 {
        0
    }
}

/// Original actor orientation from a tile step (2048 units per turn, 0 = south).
fn facing(from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> i32 {
    let dx = (to_x - from_x).signum();
    let dy = (to_y - from_y).signum();
    match (dx, dy) {
        (0, -1) => 0,
        (-1, -1) => 256,
        (-1, 0) => 512,
        (-1, 1) => 768,
        (0, 1) => 1024,
        (1, 1) => 1280,
        (1, 0) => 1536,
        (1, -1) => 1792,
        _ => 0,
    }
}

/// `Perspective.getTileHeight`: bilinear height at a local position; bridge tiles use the
/// plane above like the original (`tileSettings[1] & 2`, exported as the bridge flag).
fn tile_height(scene: &SceneData, plane: i32, x: i32, z: i32) -> i32 {
    let tx = x >> 7;
    let tz = z >> 7;
    let ex = tx + scene.offset;
    let ez = tz + scene.offset;
    if ex < 0 || ez < 0 || ex + 1 > scene.width || ez + 1 > scene.height {
        return 0;
    }
    let mut plane = plane;
    if plane < 3 && scene.flags[scene.tile_index(0, ex, ez)] & crate::scene::flag::BRIDGE_BELOW != 0
    {
        plane += 1;
    }
    let fx = x & 127;
    let fz = z & 127;
    let h00 = scene.height(plane, ex, ez);
    let h10 = scene.height(plane, ex + 1, ez);
    let h01 = scene.height(plane, ex, ez + 1);
    let h11 = scene.height(plane, ex + 1, ez + 1);
    let a = (h00 * (128 - fx) + h10 * fx) >> 7;
    let b = (h01 * (128 - fx) + h11 * fx) >> 7;
    (a * (128 - fz) + b * fz) >> 7
}

fn entity_hash(id: &str, is_player: bool) -> i64 {
    let mut h: i64 = if is_player {
        0x1000_0000_0000
    } else {
        0x2000_0000_0000
    };
    for b in id.bytes() {
        h = h.wrapping_mul(31).wrapping_add(i64::from(b));
    }
    h
}

#[cfg(not(target_arch = "wasm32"))]
fn now() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

#[cfg(all(target_arch = "wasm32", feature = "web"))]
fn now() -> f64 {
    js_sys::Date::now()
}

#[cfg(all(target_arch = "wasm32", not(feature = "web")))]
fn now() -> f64 {
    0.0
}
