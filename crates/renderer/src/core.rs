//! Platform-agnostic renderer state: loaded assets, the current scene, the camera, WorldView
//! entities placed as temporary scene objects, the per-frame triangle stream and picking.
//! The wasm/web layer and native tools drive this and hand the stream to a GPU rasterizer.

use std::collections::HashMap;

use serde::Deserialize;

use crate::actor::{
    ActivityContext, EquipModel, FitReport, NpcDefinition, NpcDefinitionRecord, PlayerBody,
    player_sequence_for,
};
use crate::anim::Sequence;
use crate::chunk::Chunks;
use crate::error::RenderError;
use crate::model::{Bounds, Model, parse_model_pack};
use crate::model_draw::{ModelDrawer, ModelScratch};
use crate::palette::Palette;
use crate::raster::software::{Software, TextureSource};
use crate::raster::{DrawStats, Fill, RasterState, Tri};
use crate::scene::SceneData;
use crate::scene::block::{self, BLOCK_SIZE, Block};
use crate::scene::draw::{PickTarget, RoofRemoval, SceneDrawer, SceneView, TemporaryEntity};
use crate::scene::minimap::{self, MapScenes, MinimapStats, WallColours};
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
    pub instance: Option<String>,
    pub name: Option<String>,
    pub activity: Option<String>,
    pub hitpoints: Option<i32>,
    pub max_hitpoints: Option<i32>,
    /// Shell extension (`PublicWorld`): `asset.source.osrs.cache2695.object.<id>` for scenery.
    pub asset_id: Option<String>,
    pub definition_id: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct WorldItem {
    pub id: String,
    pub source_id: Option<i32>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default)]
pub struct WorldEquipment {
    pub slot: String,
    pub item: Option<WorldItem>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct WorldPlayer {
    pub id: String,
    pub tile: WorldTile,
    pub animation: String,
    pub activity: String,
    pub hitpoints: Option<i32>,
    pub instance: Option<String>,
    pub equipment: Vec<WorldEquipment>,
    /// Shared-contract settings; `{ "setting": "run", "enabled": bool }` is the run toggle.
    pub settings: Vec<WorldSetting>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct WorldSetting {
    pub setting: String,
    pub enabled: Option<bool>,
}

/// Optional shell extension mirroring the protocol `Event` with `kind == "animation"`: the
/// server's source action animation for an actor (`animationAsset` =
/// `asset.source.osrs.cache2695.sequence.<id>` or a bare id). Each new `eventId` starts that
/// sequence on the actor; when it ends the actor returns to its movement/stand motion.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct WorldAnimationEvent {
    pub kind: String,
    pub event_id: String,
    pub actor_id: String,
    pub animation_asset: String,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct WorldGroundItem {
    pub id: String,
    pub tile: WorldTile,
    pub item: WorldGroundStack,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct WorldGroundStack {
    pub id: String,
    pub quantity: i64,
    pub source_id: Option<i32>,
}

/// Optional shell extension mirroring the protocol `DynamicObject` (door states, temporary
/// objects). Not part of the frozen shared contract; ignored when absent.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct WorldDynamicObject {
    pub id: String,
    pub object_id: Option<String>,
    pub source_id: Option<i32>,
    pub tile: WorldTile,
    pub instance: Option<String>,
    pub state: Option<String>,
    pub door_open: Option<bool>,
    pub quarter_turns: Option<i32>,
}

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct WorldViewInput {
    pub revision: String,
    pub tick: String,
    pub player: WorldPlayer,
    pub entities: Vec<WorldEntity>,
    pub ground_items: Vec<WorldGroundItem>,
    pub dynamic_objects: Vec<WorldDynamicObject>,
    /// Shell extension: server animation events (see [`WorldAnimationEvent`]).
    pub events: Vec<WorldAnimationEvent>,
}

/// Parses a source sequence identity: a bare id (`"879"`), `sequence.879` or the catalog form
/// `asset.source.osrs.cache2695.sequence.879`. Anything else (including other asset kinds) is
/// not a sequence.
pub fn parse_sequence_id(text: &str) -> Option<i32> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(id) = text.parse::<i32>() {
        return (id >= 0).then_some(id);
    }
    let (kind, id) = text.rsplit_once('.')?;
    if kind == "sequence" || kind.ends_with(".sequence") {
        id.parse::<i32>().ok().filter(|id| *id >= 0)
    } else {
        None
    }
}

/// One server animation event applied to an actor: the sequence and when it started.
#[derive(Clone, Debug, PartialEq)]
struct ActionMotion {
    event_id: String,
    sequence: i32,
    started_ms: f64,
}

/// A lit dynamic object variant (`models/dynamic/object-<id>-t<type>-r<rot>[-f<frame>].bin`).
#[derive(Clone, Debug)]
pub struct DynamicObjectModel {
    pub plain: Model,
    pub frames: Vec<Model>,
    pub frame_lengths: Vec<i32>,
}

#[derive(Clone, Debug)]
struct GroundItemState {
    id: String,
    tile: WorldTile,
    item: i32,
    quantity: i64,
}

#[derive(Clone, Debug)]
struct TemporaryObjectState {
    id: String,
    object: i32,
    tile: WorldTile,
    started_ms: f64,
}

#[derive(Clone, Debug)]
struct DoorState {
    object: i32,
    tile: WorldTile,
    open: bool,
    quarter_turns: i32,
}

/// State a cached minimap surface was drawn for.
#[derive(Clone, Debug, PartialEq, Eq)]
struct MinimapKey {
    scene: String,
    plane: i32,
    doors: Vec<(i32, i32, i32, i32)>,
}

/// A map-element icon position (`om.getMapIconId` of a floor decoration), as the original
/// collects them for the minimap widget (`bu.aa`): the sprite itself is UI data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MinimapIcon {
    pub x: i32,
    pub y: i32,
    pub plane: i32,
    pub element: i32,
}

/// The source minimap raster of the current scene and plane: `client.bm(world, 512x512, 4.0,
/// plane, 0, 0, 48, 48)`, so tile (x, y) of the scene occupies raster x `48 + (x - base_x) * 4`
/// and y `512 - 48 - (y - base_y + 1) * 4` (rows run north to south).
#[derive(Clone, Debug)]
pub struct MinimapSurface {
    pub width: i32,
    pub height: i32,
    /// Raster pixels per tile.
    pub scale: i32,
    /// Raster offset of scene tile (0, 0): `(48, 48)` from the left and bottom edges.
    pub margin: (i32, i32),
    pub base_x: i32,
    pub base_y: i32,
    pub plane: i32,
    /// RGBA8, alpha 255 everywhere (as the native capture wrote it).
    pub rgba: Vec<u8>,
    /// 1 where the sweep drew map data, 0 where the original fill value survived.
    pub mask: Vec<u8>,
    /// Increments whenever the surface is redrawn (scene, plane or door state changed).
    pub revision: u64,
    pub stats: MinimapStats,
    /// False when a placement lacked its exported config/definition (see `stats.notes`).
    pub complete: bool,
    pub icons: Vec<MinimapIcon>,
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
    /// NPC footprint size in tiles (definition `size`).
    size: i32,
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

/// An original interface model draw (`gp` type-6 branch): the component's centre becomes the
/// projection centre inside the parent layer's clip rectangle and the model is drawn with the
/// legacy `fx.be(0, rotation_z, rotation_y, rotation_x, offset_x, sin[rotation_x] * model_zoom >> 16
/// + offset_y, cos[rotation_x] * model_zoom >> 16 + offset_y)` at the interface rasterizer zoom
/// (512). Defaults are the exported interface 679 component 73 fields (`manifest.model_widgets`).
///
/// Content type 328 (the character-design preview) is special-cased by the original right
/// before the draw (`gp.ab`): `rotation_x = 150`, `rotation_z = (int)(sin(cycle / 40.0) * 256)
/// & 2047` with the 20 ms client cycle, model type 5 = the local player's own animated model.
#[derive(Clone, Debug)]
pub struct PlayerPreview {
    /// Surface size: the parent layer (clip rectangle) the UI hands out as the preview bounds.
    pub width: i32,
    pub height: i32,
    /// Model component centre inside that surface.
    pub center_x: i32,
    pub center_y: i32,
    /// Original component content type; 328 applies the character-design overrides.
    pub content_type: i32,
    pub rasterizer_zoom: i32,
    pub model_zoom: i32,
    pub rotation_x: i32,
    pub rotation_y: i32,
    pub rotation_z: i32,
    pub offset_x: i32,
    pub offset_y: i32,
    /// Sequence to play; `None` selects the player's idle motion. `frame` overrides the clock.
    pub sequence: Option<i32>,
    pub frame: Option<usize>,
}

impl Default for PlayerPreview {
    fn default() -> Self {
        Self {
            width: 480,
            height: 315,
            center_x: 172 + 136 / 2,
            center_y: 91 + 192 / 2,
            content_type: 328,
            rasterizer_zoom: 512,
            model_zoom: 450,
            rotation_x: 0,
            rotation_y: 0,
            rotation_z: 0,
            offset_x: 0,
            offset_y: 175,
            sequence: None,
            frame: None,
        }
    }
}

impl PlayerPreview {
    /// Rotation X/Z actually used for the draw at `elapsed_ms` since the preview started
    /// (content type 328 sways the model with the 20 ms client cycle).
    pub fn draw_rotation(&self, elapsed_ms: f64) -> (i32, i32) {
        if self.content_type == 328 {
            let cycle = (elapsed_ms / CLIENT_CYCLE_MS).floor().max(0.0) as i32;
            let sway = ((f64::from(cycle) / 40.0).sin() * 256.0) as i32 & 2047;
            (150, sway)
        } else {
            (self.rotation_x, self.rotation_z)
        }
    }
}

/// A built interface preview: its own raster state (surface size, component centre, zoom 512);
/// the painter-ordered triangles are read through [`RendererCore::preview_triangles`].
#[derive(Clone, Debug)]
pub struct PreviewFrame {
    pub state: RasterState,
    pub sequence: i32,
    pub frame: usize,
    pub summary: FrameSummary,
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
    /// Original sequences (`anim/seq-<id>.bin`) for the skeletal animation port.
    sequences: HashMap<i32, Sequence>,
    /// NPC definitions (lit base model + definition sequences) for skeletal animation.
    npc_defs: HashMap<i32, NpcDefinition>,
    /// Animated+scaled frame cache keyed by (npc, sequence, frame).
    frame_cache: HashMap<(i32, i32, usize), Model>,
    /// The penguin player body and retargeting table (needs the human reference model).
    player_body: Option<PlayerBody>,
    /// Lit dynamic object models by (object id, type, orientation) with optional frames.
    dynamic_objects: HashMap<(i32, i32, i32), DynamicObjectModel>,
    /// Ground item models by item id: (min quantity, model) ascending.
    ground_item_models: HashMap<i32, Vec<(i64, Model)>>,
    /// Live layers from the last WorldView.
    ground_items: Vec<GroundItemState>,
    temporary_objects: Vec<TemporaryObjectState>,
    door_states: Vec<DoorState>,
    /// Roof removal mode bits (`ez.ny`; 0 = stock client, every roof drawn).
    roof_mode: i32,
    hovered_tile: Option<(i32, i32)>,
    destination_tile: Option<(i32, i32)>,
    /// Equipped-item models by item id.
    equip_models: HashMap<i32, EquipModel>,
    /// Assembled player model (body + gear) and the gear ids it was built for.
    player_assembled: Option<(Vec<i32>, Model, Vec<FitReport>)>,
    player_frame_cache: HashMap<(i32, usize), Model>,
    player_gear: Vec<(String, i32)>,
    /// Explicit `br` (the approved fixture captures used 0); `None` applies the stock rule.
    top_plane_override: Option<i32>,
    /// Instanced map flag (`cy.as`): the stock rule then always draws up to the player's plane.
    instanced_map: bool,
    /// Clock origin of the interface preview animation (first preview frame).
    preview_started_ms: Option<f64>,
    preview_tris: Vec<Tri>,
    player_activity: String,
    player_animation: String,
    player_dead: bool,
    /// Developer-only fallback: derive action motions from the activity string and adjacent
    /// scenery when the server supplies no animation. Off by default (not final M1 logic).
    motion_fallback: bool,
    /// Player movement tracking for the original run rule (two tiles per server tick).
    player_motion_tick: Option<(i64, WorldTile)>,
    player_running: bool,
    /// Latest server animation events per actor id (shell extension), with start times.
    action_motions: HashMap<String, ActionMotion>,
    /// Actors whose reported activity implies an action but whose motion is unknown this view.
    unknown_motions: Vec<String>,
    player_instance: Option<String>,
    /// Standalone lit models (e.g. the tree fixture) addressed by manifest id.
    models: HashMap<String, Model>,
    /// World blocks (64x64 map squares) keyed by square id, with their model packs.
    blocks: HashMap<i32, (Block, Vec<(String, Model)>)>,
    /// Original map-scene sprites and tile-shape masks (`minimap/mapscenes.bin`).
    map_scenes: Option<MapScenes>,
    /// Minimap sidecar bytes per square (`minimap/blocks/<square>.bin`), attached to blocks.
    minimap_sidecars: HashMap<i32, Vec<u8>>,
    /// Cached minimap surface and the state it was drawn for.
    minimap: Option<(MinimapKey, MinimapSurface)>,
    minimap_revision: u64,
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
            sequences: HashMap::new(),
            npc_defs: HashMap::new(),
            frame_cache: HashMap::new(),
            player_body: None,
            dynamic_objects: HashMap::new(),
            ground_item_models: HashMap::new(),
            ground_items: Vec::new(),
            temporary_objects: Vec::new(),
            door_states: Vec::new(),
            roof_mode: 0,
            hovered_tile: None,
            destination_tile: None,
            equip_models: HashMap::new(),
            player_assembled: None,
            player_frame_cache: HashMap::new(),
            player_gear: Vec::new(),
            top_plane_override: None,
            instanced_map: false,
            preview_started_ms: None,
            preview_tris: Vec::new(),
            player_activity: String::new(),
            player_animation: String::new(),
            player_dead: false,
            motion_fallback: false,
            player_motion_tick: None,
            player_running: false,
            action_motions: HashMap::new(),
            unknown_motions: Vec::new(),
            player_instance: None,
            models: HashMap::new(),
            blocks: HashMap::new(),
            map_scenes: None,
            minimap_sidecars: HashMap::new(),
            minimap: None,
            minimap_revision: 0,
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

    /// Loads an original sequence (frames + skeleton) for the skeletal animation port.
    pub fn load_sequence(&mut self, bytes: &[u8]) -> Result<i32, RenderError> {
        let sequence = Sequence::from_chunks(bytes)?;
        let id = sequence.id;
        self.sequences.insert(id, sequence);
        self.frame_cache.clear();
        self.player_frame_cache.clear();
        Ok(id)
    }

    pub fn has_sequence(&self, id: i32) -> bool {
        self.sequences.contains_key(&id)
    }

    /// Loads an NPC definition (manifest `npc_definitions` record as JSON) with its lit base.
    pub fn load_npc_definition(
        &mut self,
        record_json: &str,
        base_bytes: &[u8],
    ) -> Result<i32, RenderError> {
        let record: NpcDefinitionRecord = serde_json::from_str(record_json)
            .map_err(|e| RenderError::Scene(format!("npc definition json: {e}")))?;
        let base = Model::from_chunks(base_bytes)?;
        let id = record.npc_id;
        self.npc_defs.insert(id, NpcDefinition { record, base });
        self.frame_cache.retain(|(npc, _, _), _| *npc != id);
        Ok(id)
    }

    pub fn has_npc_definition(&self, npc_id: i32) -> bool {
        self.npc_defs.contains_key(&npc_id)
    }

    /// Installs the player body: the approved penguin base (NPC 2063) with its native
    /// sequences and the human reference used only to derive the label retargeting table.
    pub fn load_player_body(
        &mut self,
        penguin_base: &[u8],
        width_scale: i32,
        height_scale: i32,
        native_sequences: Vec<i32>,
        human_reference: &[u8],
    ) -> Result<(), RenderError> {
        let base = Model::from_chunks(penguin_base)?;
        let human = Model::from_chunks(human_reference)?;
        if base.vertex_groups.is_none() || human.vertex_groups.is_none() {
            return Err(RenderError::InvalidAsset(
                "player body models need vertex labels".into(),
            ));
        }
        self.player_body = Some(PlayerBody::new(
            base,
            width_scale,
            height_scale,
            native_sequences,
            &human,
        ));
        self.player_assembled = None;
        self.player_frame_cache.clear();
        Ok(())
    }

    pub fn load_equip_model(&mut self, item_id: i32, bytes: &[u8]) -> Result<(), RenderError> {
        let model = Model::from_chunks(bytes)?;
        self.equip_models
            .insert(item_id, EquipModel { item_id, model });
        self.player_assembled = None;
        self.player_frame_cache.clear();
        Ok(())
    }

    /// Loads a lit dynamic object variant (door state, fire, morph variant) with its frames.
    pub fn load_dynamic_object(
        &mut self,
        object_id: i32,
        kind: i32,
        orientation: i32,
        plain: &[u8],
        frames: &[Vec<u8>],
        frame_lengths: Vec<i32>,
    ) -> Result<(), RenderError> {
        let plain = Model::from_chunks(plain)?;
        let mut frame_models = Vec::with_capacity(frames.len());
        for bytes in frames {
            frame_models.push(Model::from_chunks(bytes)?);
        }
        if frame_models.len() != frame_lengths.len() {
            return Err(RenderError::InvalidAsset(format!(
                "object {object_id} has {} frames but {} lengths",
                frame_models.len(),
                frame_lengths.len()
            )));
        }
        self.dynamic_objects.insert(
            (object_id, kind, orientation),
            DynamicObjectModel {
                plain,
                frames: frame_models,
                frame_lengths,
            },
        );
        Ok(())
    }

    pub fn has_dynamic_object(&self, object_id: i32, kind: i32, orientation: i32) -> bool {
        self.dynamic_objects
            .contains_key(&(object_id, kind, orientation))
    }

    /// Loads a ground item model for quantities `>= min_quantity`.
    pub fn load_ground_item(
        &mut self,
        item_id: i32,
        min_quantity: i64,
        bytes: &[u8],
    ) -> Result<(), RenderError> {
        let model = Model::from_chunks(bytes)?;
        let entry = self.ground_item_models.entry(item_id).or_default();
        entry.retain(|(q, _)| *q != min_quantity);
        entry.push((min_quantity, model));
        entry.sort_by_key(|(q, _)| *q);
        Ok(())
    }

    fn ground_item_model(&self, item_id: i32, quantity: i64) -> Option<&Model> {
        let variants = self.ground_item_models.get(&item_id)?;
        variants
            .iter()
            .rev()
            .find(|(q, _)| quantity >= *q)
            .map(|(_, m)| m)
            .or_else(|| variants.first().map(|(_, m)| m))
    }

    /// Retargeting table and last equipment fit report (proposal data for owner review).
    pub fn player_fit_report(&self) -> Option<(&crate::anim::LabelMap, &[FitReport])> {
        let body = self.player_body.as_ref()?;
        let fits = self
            .player_assembled
            .as_ref()
            .map(|(_, _, f)| f.as_slice())
            .unwrap_or(&[]);
        Some((&body.label_map, fits))
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
        let mut block = Block::from_chunks(block_bytes)?;
        if block.square != square {
            return Err(RenderError::InvalidAsset(format!(
                "block file is square {} but was registered as {square}",
                block.square
            )));
        }
        if let Some(sidecar) = self.minimap_sidecars.get(&square) {
            block.attach_minimap(sidecar)?;
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

    /// Loads the original map-scene sprites and tile-shape masks the minimap needs.
    pub fn load_map_scenes(&mut self, bytes: &[u8]) -> Result<(), RenderError> {
        self.map_scenes = Some(MapScenes::from_chunks(bytes)?);
        self.minimap = None;
        Ok(())
    }

    pub fn has_map_scenes(&self) -> bool {
        self.map_scenes.is_some()
    }

    /// Loads a square's minimap sidecar (wall placement configs and object-definition map
    /// fields). Attaches to the block immediately when it is loaded, and to any later load.
    pub fn load_minimap_block(&mut self, square: i32, bytes: &[u8]) -> Result<(), RenderError> {
        if let Some((block, _)) = self.blocks.get_mut(&square) {
            block.attach_minimap(bytes)?;
        } else {
            // Validate the header now so a wrong file fails at load time, not at assembly.
            let chunks = Chunks::parse(bytes)?;
            let h = chunks.ints("MBHD")?;
            if h.first() != Some(&square) {
                return Err(RenderError::InvalidAsset(format!(
                    "minimap sidecar is square {:?} but was registered as {square}",
                    h.first()
                )));
            }
        }
        self.minimap_sidecars.insert(square, bytes.to_vec());
        self.minimap = None;
        Ok(())
    }

    pub fn has_minimap_block(&self, square: i32) -> bool {
        self.blocks
            .get(&square)
            .is_some_and(|(block, _)| block.minimap_ready)
    }

    /// Door-state wall replacements by tile index, for the minimap (`fe.getConfig` type 0 at
    /// the door's rotation; the original tag keeps its interactive bit).
    fn minimap_wall_overrides(&self, scene: &SceneData) -> HashMap<usize, crate::scene::Wall> {
        let mut out = HashMap::new();
        for door in &self.door_states {
            let rotation = door.quarter_turns.rem_euclid(4);
            let ex = door.tile.x - scene.base_x + scene.offset;
            let ey = door.tile.y - scene.base_y + scene.offset;
            if ex < 0 || ey < 0 || ex >= scene.width || ey >= scene.height {
                continue;
            }
            let plane = door.tile.plane.clamp(0, scene.planes - 1);
            let index = scene.tile_index(plane, ex, ey);
            if let Some(existing) = scene.walls.get(&index) {
                let mut wall = existing.clone();
                wall.orientation_a = 1 << rotation;
                wall.orientation_b = 0;
                wall.model_b = -1;
                wall.config = rotation << 6;
                out.insert(index, wall);
            }
        }
        out
    }

    /// The source minimap of the current scene on the player's plane (see [`MinimapSurface`]),
    /// redrawn only when the scene, plane or door states change. Requires the map-scene assets
    /// and a scene whose blocks carry their minimap sidecars; missing inputs are errors, never
    /// a blank or approximate map.
    pub fn minimap_surface(&mut self) -> Result<&MinimapSurface, RenderError> {
        let scene = self
            .scene
            .as_ref()
            .ok_or_else(|| RenderError::Scene("no scene loaded for the minimap".into()))?;
        let assets = self.map_scenes.as_ref().ok_or_else(|| {
            RenderError::MissingAsset("minimap/mapscenes.bin is not loaded".into())
        })?;
        let plane = self.plane.clamp(0, scene.planes - 1);
        let mut doors: Vec<(i32, i32, i32, i32)> = self
            .door_states
            .iter()
            .map(|d| {
                (
                    d.tile.plane,
                    d.tile.x,
                    d.tile.y,
                    d.quarter_turns.rem_euclid(4),
                )
            })
            .collect();
        doors.sort_unstable();
        let key = MinimapKey {
            scene: self.scene_id.clone().unwrap_or_default(),
            plane,
            doors,
        };
        if self.minimap.as_ref().is_some_and(|(k, _)| *k == key) {
            return Ok(&self.minimap.as_ref().expect("checked").1);
        }
        let overrides = self.minimap_wall_overrides(scene);
        let mut raster = minimap::Raster::new(512, 512);
        let stats = minimap::render(
            scene,
            plane,
            assets,
            WallColours::REFERENCE,
            &overrides,
            4.0,
            (0, 0),
            (48, 48),
            &mut raster,
        )?;
        let (rgba, mask) = minimap::to_rgba(&raster);
        // bu.aa: floor decorations with a map element on the drawn plane, main area only.
        let mut icons = Vec::new();
        for x in 0..scene.max_x {
            for y in 0..scene.max_y {
                let index = scene.tile_index(plane, x + scene.offset, y + scene.offset);
                if scene.flags.get(index).is_none_or(|f| f & 1 == 0) {
                    continue;
                }
                let Some(decor) = scene.floor_decorations.get(&index) else {
                    continue;
                };
                let Some(def) = scene
                    .object_defs
                    .get(&crate::scene::tag_object_id(decor.hash))
                else {
                    continue;
                };
                if def.map_icon >= 0 {
                    icons.push(MinimapIcon {
                        x: scene.base_x + x,
                        y: scene.base_y + y,
                        plane,
                        element: def.map_icon,
                    });
                }
            }
        }
        self.minimap_revision += 1;
        let surface = MinimapSurface {
            width: raster.width,
            height: raster.height,
            scale: 4,
            margin: (48, 48),
            base_x: scene.base_x,
            base_y: scene.base_y,
            plane,
            rgba,
            mask,
            revision: self.minimap_revision,
            complete: stats.unresolved == 0,
            stats,
            icons,
        };
        self.minimap = Some((key, surface));
        Ok(&self.minimap.as_ref().expect("just set").1)
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

    /// Applies an authoritative world view: entity placement, animation identity, equipment
    /// and instance visibility only. Nothing here decides game rules.
    pub fn update_world(&mut self, json: &str, now_ms: f64) -> Result<(), RenderError> {
        let view: WorldViewInput = serde_json::from_str(json)
            .map_err(|e| RenderError::Scene(format!("world view json: {e}")))?;
        let mut next: Vec<EntityState> = Vec::new();
        let previous = std::mem::take(&mut self.entities);
        let player_tile = view.player.tile.clone();
        self.player_instance = view.player.instance.clone();
        self.player_activity = view.player.activity.clone();
        self.player_animation = view.player.animation.clone();
        self.player_dead = view.player.hitpoints.is_some_and(|hp| hp <= 0);
        let mut gear: Vec<(String, i32)> = view
            .player
            .equipment
            .iter()
            .filter_map(|e| {
                e.item
                    .as_ref()
                    .and_then(|i| i.source_id)
                    .map(|id| (e.slot.clone(), id))
            })
            .collect();
        gear.sort();
        if gear != self.player_gear {
            self.player_gear = gear;
            self.player_assembled = None;
            self.player_frame_cache.clear();
        }
        // Server animation events (shell extension): a new event id starts that sequence on
        // its actor at this update; stale actors are dropped with their events.
        self.unknown_motions.clear();
        let mut live_actors: std::collections::HashSet<String> =
            view.entities.iter().map(|e| e.id.clone()).collect();
        live_actors.insert(view.player.id.clone());
        self.action_motions
            .retain(|actor, _| live_actors.contains(actor));
        for event in view.events.iter().filter(|e| e.kind == "animation") {
            let Some(sequence) = parse_sequence_id(&event.animation_asset) else {
                self.unknown_motions.push(format!(
                    "{}: animation event {} names no source sequence ({:?})",
                    event.actor_id, event.event_id, event.animation_asset
                ));
                continue;
            };
            let fresh = self
                .action_motions
                .get(&event.actor_id)
                .is_none_or(|m| m.event_id != event.event_id);
            if fresh {
                self.action_motions.insert(
                    event.actor_id.clone(),
                    ActionMotion {
                        event_id: event.event_id.clone(),
                        sequence,
                        started_ms: now_ms,
                    },
                );
            }
        }
        // Movement: the original plays the run sequence when the player covers two tiles in
        // one server tick, the walk sequence for one. Ticks come from the view; the run toggle
        // setting disambiguates when several ticks elapsed between views.
        let moving = previous
            .iter()
            .find(|e| e.is_player && e.id == view.player.id)
            .is_some_and(|o| o.tile.x != player_tile.x || o.tile.y != player_tile.y);
        let run_setting = view
            .player
            .settings
            .iter()
            .find(|s| s.setting == "run")
            .and_then(|s| s.enabled);
        let tick = view.tick.trim().parse::<i64>().ok();
        if let (Some(tick), Some((last_tick, last_tile))) = (tick, self.player_motion_tick.as_ref())
            && tick > *last_tick
        {
            let ticks = tick - last_tick;
            let steps = i64::from(
                (player_tile.x - last_tile.x)
                    .abs()
                    .max((player_tile.y - last_tile.y).abs()),
            );
            self.player_running = if steps >= 2 * ticks {
                true
            } else if steps <= ticks {
                false
            } else {
                run_setting.unwrap_or(self.player_running)
            };
        } else if tick.is_none() {
            // No tick information: only the run toggle can say (explicit data, never a default).
            self.player_running = run_setting.unwrap_or(false);
        }
        if let Some(tick) = tick
            && self
                .player_motion_tick
                .as_ref()
                .is_none_or(|(t, _)| tick > *t)
        {
            self.player_motion_tick = Some((tick, player_tile.clone()));
        }
        // The player's sequence, from explicit data only: the server-bound `animation` (bare or
        // catalog sequence id), else the latest animation event for the player, else the
        // movement stance (walk/run/stand). Activities without a supplied source animation are
        // reported as unknown motion rather than guessed — unless the developer fallback
        // (activity + adjacent scenery, not final M1 logic) is switched on.
        let stance = if moving {
            if self.player_running {
                crate::actor::PLAYER_RUN
            } else {
                crate::actor::PLAYER_WALK
            }
        } else {
            crate::actor::PLAYER_IDLE
        };
        // An event motion ends with its sequence, with a newer event, when the actor moves, or
        // when the server reports the activity back at rest.
        let at_rest = matches!(view.player.activity.as_str(), "" | "idle" | "walking");
        let expired = self.action_motions.get(&view.player.id).is_some_and(|m| {
            moving
                || at_rest
                || self
                    .sequence_frame(m.sequence, now_ms - m.started_ms)
                    .is_some_and(|(_, ended)| ended)
        });
        if expired {
            self.action_motions.remove(&view.player.id);
        }
        let player_sequence = match parse_sequence_id(&view.player.animation) {
            Some(id) => id,
            None => {
                let event_motion = self.action_motions.get(&view.player.id).map(|m| m.sequence);
                match event_motion {
                    Some(id) if !moving => id,
                    _ => {
                        let action_reported = self.player_dead
                            || !matches!(view.player.activity.as_str(), "" | "idle" | "walking");
                        if action_reported && self.motion_fallback {
                            let context = ActivityContext {
                                weapon_item: self
                                    .player_gear
                                    .iter()
                                    .find(|(slot, _)| slot == "weapon")
                                    .map(|(_, id)| *id),
                                moving,
                                running: self.player_running,
                                dead: self.player_dead,
                                ..self.adjacent_context(&player_tile)
                            };
                            player_sequence_for(&view.player.activity, &context)
                        } else {
                            if action_reported {
                                self.unknown_motions.push(format!(
                                    "{}: activity {:?}{} without a source animation (player.animation empty, no animation event); playing the {} stance",
                                    view.player.id,
                                    view.player.activity,
                                    if self.player_dead { " (hitpoints 0)" } else { "" },
                                    if moving { "movement" } else { "stand" }
                                ));
                            }
                            stance
                        }
                    }
                }
            }
        };
        let event_started: HashMap<String, f64> = self
            .action_motions
            .iter()
            .map(|(actor, m)| (actor.clone(), m.started_ms))
            .collect();
        let apply = |id: &str,
                     npc: i32,
                     tile: &WorldTile,
                     sequence: i32,
                     is_player: bool,
                     size: i32,
                     previous: &[EntityState]| {
            let old = previous.iter().find(|e| e.id == id);
            let mut orientation = old.map(|o| o.orientation).unwrap_or(0);
            if let Some(o) = old
                && (o.tile.x != tile.x || o.tile.y != tile.y)
            {
                orientation = facing(o.tile.x, o.tile.y, tile.x, tile.y);
            }
            let started = match old {
                Some(o) if o.sequence == sequence => o.sequence_started_ms,
                _ => event_started.get(id).copied().unwrap_or(now_ms),
            };
            EntityState {
                id: id.to_string(),
                npc,
                tile: tile.clone(),
                orientation,
                sequence,
                sequence_started_ms: started,
                is_player,
                size,
            }
        };
        let object_source = |e: &WorldEntity| -> Option<i32> {
            e.source_id.or_else(|| {
                let asset = e.asset_id.as_deref().or(e.definition_id.as_deref())?;
                asset.rsplit('.').next()?.parse().ok()
            })
        };
        self.scenery_entities = view
            .entities
            .iter()
            .filter(|e| e.kind == "object" || e.kind == "temporary_object")
            .filter_map(|e| object_source(e).map(|id| (e.id.clone(), id, e.tile.clone())))
            .collect();
        // Temporary objects (fires): animated from the moment they first appear.
        let previous_temporary = std::mem::take(&mut self.temporary_objects);
        self.temporary_objects = view
            .entities
            .iter()
            .filter(|e| e.kind == "temporary_object" && e.instance == view.player.instance)
            .filter_map(|e| {
                let object = object_source(e)?;
                let started_ms = previous_temporary
                    .iter()
                    .find(|p| p.id == e.id)
                    .map(|p| p.started_ms)
                    .unwrap_or(now_ms);
                Some(TemporaryObjectState {
                    id: e.id.clone(),
                    object,
                    tile: e.tile.clone(),
                    started_ms,
                })
            })
            .collect();
        // Ground items: the original item layer shows the tile's top three stacks.
        self.ground_items = view
            .ground_items
            .iter()
            .filter_map(|g| {
                g.item.source_id.map(|item| GroundItemState {
                    id: g.id.clone(),
                    tile: g.tile.clone(),
                    item,
                    quantity: g.item.quantity.max(1),
                })
            })
            .collect();
        // Door states (shell extension mirroring the protocol DynamicObject).
        self.door_states = view
            .dynamic_objects
            .iter()
            .filter(|d| d.instance == view.player.instance && d.door_open.is_some())
            .filter_map(|d| {
                let object = d
                    .source_id
                    .or_else(|| d.object_id.as_deref()?.rsplit('.').next()?.parse().ok())?;
                Some(DoorState {
                    object,
                    tile: d.tile.clone(),
                    open: d.door_open.unwrap_or(false),
                    quarter_turns: d.quarter_turns.unwrap_or(0),
                })
            })
            .collect();
        next.push(apply(
            &view.player.id,
            PLAYER_BASE_NPC,
            &player_tile,
            player_sequence,
            true,
            1,
            &previous,
        ));
        for entity in &view.entities {
            if entity.kind != "npc" && entity.kind != "player" {
                continue;
            }
            // Instanced entities are visible only inside the player's instance.
            if entity.instance != view.player.instance {
                continue;
            }
            let Some(npc) = entity.source_id else {
                continue;
            };
            let def = self.npc_defs.get(&npc);
            let old = previous.iter().find(|e| e.id == entity.id);
            let moved = old.is_some_and(|o| o.tile.x != entity.tile.x || o.tile.y != entity.tile.y);
            let event_expired = self.action_motions.get(&entity.id).is_some_and(|m| {
                moved
                    || self
                        .sequence_frame(m.sequence, now_ms - m.started_ms)
                        .is_some_and(|(_, ended)| ended)
            });
            if event_expired {
                self.action_motions.remove(&entity.id);
            }
            let named = parse_sequence_id(&entity.animation)
                .or_else(|| self.action_motions.get(&entity.id).map(|m| m.sequence));
            let dead = entity.hitpoints.is_some_and(|hp| hp <= 0)
                && entity.max_hitpoints.is_some_and(|m| m > 0);
            // The original client plays the definition's own stand/walk sequences locally;
            // deaths and actions are server-sent animations.
            let sequence = match (named, def) {
                (Some(id), _) => id,
                (None, Some(def)) => {
                    if dead
                        && self.motion_fallback
                        && let Some(&death) = def.record.combat_sequences.last()
                    {
                        death
                    } else {
                        if dead {
                            self.unknown_motions.push(format!(
                                "{}: hitpoints 0 without a source animation; playing the definition stance",
                                entity.id
                            ));
                        }
                        if moved && def.walk() >= 0 {
                            def.walk()
                        } else {
                            def.stand()
                        }
                    }
                }
                (None, None) => -1,
            };
            let size = def.map(|d| d.record.size.max(1)).unwrap_or(1);
            next.push(apply(
                &entity.id,
                npc,
                &entity.tile,
                sequence,
                entity.kind == "player",
                size,
                &previous,
            ));
        }
        self.plane = view.player.tile.plane;
        self.entities = next;
        Ok(())
    }

    /// What stands on the tiles around the player (scene object ids and WorldView scenery /
    /// NPC entities), used only to pick the original tool motion for an activity.
    fn adjacent_context(&self, tile: &WorldTile) -> ActivityContext {
        let mut context = ActivityContext::default();
        let Some(scene) = &self.scene else {
            return context;
        };
        let classify = |object_id: i32, context: &mut ActivityContext| match object_id {
            // Tutorial Island / Lumbridge source object ids: trees, rocks, fishing, fire, range,
            // furnace, anvil (content pack spawn definitions).
            1276..=1282 | 1283..=1290 | 9730..=9732 | 1315..=1319 | 1330 | 1331 | 1332 | 9734 => {
                context.adjacent_tree = true
            }
            10943 | 11161 | 11360 | 11361 | 11364 | 11365 | 10079 | 10080 | 10081 | 10082
            | 10083 => {
                if matches!(object_id, 10082 | 10083 | 24009) {
                    context.adjacent_furnace = true;
                } else {
                    context.adjacent_rock = true;
                }
            }
            24009 | 3994 | 16469 => context.adjacent_furnace = true,
            26185..=26195 => context.adjacent_fire = true,
            9682 | 9736 | 114 | 12269 => context.adjacent_range = true,
            2097 | 2031 | 24006 => context.adjacent_anvil = true,
            _ => {}
        };
        for dx in -1..=1 {
            for dy in -1..=1 {
                let ex = tile.x - scene.base_x + scene.offset + dx;
                let ey = tile.y - scene.base_y + scene.offset + dy;
                if ex < 0 || ey < 0 || ex >= scene.width || ey >= scene.height {
                    continue;
                }
                let plane = tile.plane.clamp(0, scene.planes - 1);
                let index = scene.tile_index(plane, ex, ey);
                for slot in 0..5 {
                    if let Some(&id) = scene.slots.get(&(index * 5 + slot)) {
                        let object_id = ((scene.game_objects[id].hash >> 20) & 0xFFFF_FFFF) as i32;
                        classify(object_id, &mut context);
                    }
                }
                if let Some(wall) = scene.walls.get(&index) {
                    classify(((wall.hash >> 20) & 0xFFFF_FFFF) as i32, &mut context);
                }
                if let Some(floor) = scene.floor_decorations.get(&index) {
                    classify(((floor.hash >> 20) & 0xFFFF_FFFF) as i32, &mut context);
                }
            }
        }
        for (_, object_id, t) in &self.scenery_entities {
            if (t.x - tile.x).abs() <= 1 && (t.y - tile.y).abs() <= 1 && t.plane == tile.plane {
                classify(*object_id, &mut context);
            }
        }
        for entity in &self.entities {
            if entity.npc == 3317
                && (entity.tile.x - tile.x).abs() <= 1
                && (entity.tile.y - tile.y).abs() <= 1
            {
                context.adjacent_fishing_spot = true;
            }
        }
        context
    }

    /// Animated, scaled model for an actor frame through the skeletal port (cached).
    fn npc_frame_model(&mut self, npc: i32, sequence_id: i32, frame: usize) -> Option<Model> {
        if let Some(model) = self.frame_cache.get(&(npc, sequence_id, frame)) {
            return Some(model.clone());
        }
        let def = self.npc_defs.get(&npc)?;
        let sequence = self.sequences.get(&sequence_id)?;
        let mut model = def.base.clone();
        crate::anim::apply_frame(&mut model, sequence, frame, None).ok()?;
        crate::anim::scale_float(&mut model, def.record.width_scale, def.record.height_scale);
        model.compute_cylinder_bounds();
        self.frame_cache
            .insert((npc, sequence_id, frame), model.clone());
        Some(model)
    }

    /// The player's animated model for a sequence frame: penguin body + attached gear.
    fn player_frame_model(
        &mut self,
        sequence_id: i32,
        frame: usize,
    ) -> Result<Option<Model>, RenderError> {
        if let Some(model) = self.player_frame_cache.get(&(sequence_id, frame)) {
            return Ok(Some(model.clone()));
        }
        let Some(body) = self.player_body.as_ref() else {
            return Ok(None);
        };
        let gear_ids: Vec<i32> = self.player_gear.iter().map(|(_, id)| *id).collect();
        if self
            .player_assembled
            .as_ref()
            .is_none_or(|(ids, _, _)| ids != &gear_ids)
        {
            let gear: Vec<(String, &EquipModel)> = self
                .player_gear
                .iter()
                .filter_map(|(slot, id)| self.equip_models.get(id).map(|m| (slot.clone(), m)))
                .collect();
            let (assembled, fits) = body.assemble(&gear);
            self.player_assembled = Some((gear_ids.clone(), assembled, fits));
        }
        let Some(sequence) = self.sequences.get(&sequence_id) else {
            return Ok(None);
        };
        let assembled = &self.player_assembled.as_ref().expect("assembled above").1;
        let model = body.frame(assembled, sequence, frame)?;
        self.player_frame_cache
            .insert((sequence_id, frame), model.clone());
        Ok(Some(model))
    }

    /// Overrides the top drawn plane (`br`). The approved fixture captures were taken with the
    /// original `dh` plane argument 0 (ground plane only); live rendering uses the stock rule.
    pub fn set_top_plane_override(&mut self, limit: Option<i32>) {
        self.top_plane_override = limit.map(|l| l.clamp(0, 3));
    }

    /// Marks the loaded map as instanced (`cy.as`): the stock top-plane rule then always draws up
    /// to the player's plane.
    pub fn set_instanced_map(&mut self, instanced: bool) {
        self.instanced_map = instanced;
    }

    /// Port of `cz.ch`: the top plane the live client draws. 3 (everything) unless the camera
    /// pitch is below 310 legacy units (2480) and the player's plane carries the roof setting
    /// (bit 4) on the camera tile or any tile of the Bresenham line from the camera tile to the
    /// focal (player) tile, in which case the player's plane. Outside the main area, or in an
    /// instanced map, the player's plane.
    fn stock_top_plane(&self, base_x: i32, base_y: i32) -> i32 {
        let Some(scene) = self.scene.as_ref() else {
            return self.plane;
        };
        let plane = self.plane;
        if self.instanced_map {
            return plane;
        }
        let mut top = 3;
        let focal = self
            .entities
            .iter()
            .find(|e| e.is_player)
            .map(|e| (e.tile.x - base_x, e.tile.y - base_y))
            .unwrap_or((
                (self.camera.x - base_x * 128) >> 7,
                (self.camera.y - base_y * 128) >> 7,
            ));
        if self.camera.pitch < 310 << 3 {
            let (mut cx, mut cy) = (
                (self.camera.x - base_x * 128) >> 7,
                (self.camera.y - base_y * 128) >> 7,
            );
            let (fx, fy) = focal;
            // `dz.as/ax` for the main worldview: the 104x104 main area (`ok..pe`, `ne..un`).
            let inside = |x: i32, y: i32| {
                x >= scene.min_x && y >= scene.min_y && x < scene.max_x && y < scene.max_y
            };
            if !inside(cx, cy) || !inside(fx, fy) {
                return plane;
            }
            let oy = scene.offset;
            let roof = |x: i32, y: i32| scene.setting(plane, x + oy, y + oy) & 4 != 0;
            if roof(cx, cy) {
                top = plane;
            }
            let dx = (fx - cx).abs();
            let dy = (fy - cy).abs();
            if dx > dy {
                let step = dy * 65536 / dx;
                let mut acc = 32768;
                while cx != fx {
                    if cx < fx {
                        cx += 1;
                    } else if cx > fx {
                        cx -= 1;
                    }
                    if roof(cx, cy) {
                        top = plane;
                    }
                    acc += step;
                    if acc >= 65536 {
                        acc -= 65536;
                        if cy < fy {
                            cy += 1;
                        } else if cy > fy {
                            cy -= 1;
                        }
                        if roof(cx, cy) {
                            top = plane;
                        }
                    }
                }
            } else if dy > 0 {
                let step = dx * 65536 / dy;
                let mut acc = 32768;
                while cy != fy {
                    if cy < fy {
                        cy += 1;
                    } else if cy > fy {
                        cy -= 1;
                    }
                    if roof(cx, cy) {
                        top = plane;
                    }
                    acc += step;
                    if acc >= 65536 {
                        acc -= 65536;
                        if cx < fx {
                            cx += 1;
                        } else if cx > fx {
                            cx -= 1;
                        }
                        if roof(cx, cy) {
                            top = plane;
                        }
                    }
                }
            }
        }
        top
    }

    /// Developer-only: derive action motions from the activity string and adjacent scenery when
    /// the server supplies no animation. Not final M1 logic; off by default.
    pub fn set_motion_fallback(&mut self, enabled: bool) {
        self.motion_fallback = enabled;
    }

    /// Actors whose reported state implies an action but whose source motion was not supplied
    /// in the last world view (explicit interop gaps, never guessed).
    pub fn unknown_motions(&self) -> &[String] {
        &self.unknown_motions
    }

    /// Whether the player is currently running (original two-tiles-per-tick rule / run toggle).
    pub fn player_running(&self) -> bool {
        self.player_running
    }

    /// Sets the original roof-removal mode bits (1 player tile, 2 hovered tile, 4 destination,
    /// 8 camera line). 0 keeps every roof, as the stock client and the approved captures do.
    pub fn set_roof_mode(&mut self, mode: i32) {
        self.roof_mode = mode & 15;
    }

    /// Hovered world tile and walk destination for roof removal modes 2 and 4.
    pub fn set_roof_context(
        &mut self,
        hovered: Option<(i32, i32)>,
        destination: Option<(i32, i32)>,
    ) {
        self.hovered_tile = hovered;
        self.destination_tile = destination;
    }

    fn roof_removal(&self) -> RoofRemoval {
        let Some(scene) = &self.scene else {
            return RoofRemoval::default();
        };
        let local = |t: (i32, i32)| (t.0 - scene.base_x, t.1 - scene.base_y);
        let player = self
            .entities
            .iter()
            .find(|e| e.is_player)
            .map(|e| local((e.tile.x, e.tile.y)));
        RoofRemoval {
            mode: self.roof_mode,
            player_tile: player,
            hovered_tile: self.hovered_tile.map(local),
            destination_tile: self.destination_tile.map(local),
            camera_tile: Some((
                (self.camera.x >> 7) - scene.base_x,
                (self.camera.y >> 7) - scene.base_y,
            )),
        }
    }

    /// Developer preview: the assembled player (body + the given gear) at a sequence frame,
    /// animated and scaled exactly like the in-scene actor.
    pub fn player_model_for_preview(
        &mut self,
        sequence_id: i32,
        frame: usize,
        gear: &[(&str, i32)],
    ) -> Result<Option<Model>, RenderError> {
        let Some(body) = self.player_body.as_ref() else {
            return Ok(None);
        };
        let Some(sequence) = self.sequences.get(&sequence_id) else {
            return Ok(None);
        };
        let gear: Vec<(String, &EquipModel)> = gear
            .iter()
            .filter_map(|(slot, id)| self.equip_models.get(id).map(|m| (slot.to_string(), m)))
            .collect();
        let (assembled, _) = body.assemble(&gear);
        let frame = frame.min(sequence.frame_count().saturating_sub(1));
        body.frame(&assembled, sequence, frame).map(Some)
    }

    /// Frame index for a sequence through the skeletal port's timing (`rd.az` semantics).
    fn sequence_frame(&self, sequence_id: i32, elapsed_ms: f64) -> Option<(usize, bool)> {
        let sequence = self.sequences.get(&sequence_id)?;
        let cycles = (elapsed_ms / CLIENT_CYCLE_MS).floor().max(0.0) as i64;
        match sequence.frame_at(cycles) {
            Some(frame) => Some((frame, false)),
            None => Some((sequence.frame_count().saturating_sub(1), true)),
        }
    }

    /// Builds this frame's painter-ordered triangle stream. Returns a summary; the stream is
    /// available through [`Self::triangles`].
    pub fn build_frame(&mut self, now_ms: f64) -> Result<&FrameSummary, RenderError> {
        let start = now();
        let animation_cycles = self.animation_cycles(now_ms);
        let (base_x, base_y) = {
            let scene = self
                .scene
                .as_ref()
                .ok_or_else(|| RenderError::Scene("no scene loaded".into()))?;
            let drawer = self.drawer.as_mut().expect("drawer follows scene");
            drawer.begin_frame(scene);
            (scene.base_x, scene.base_y)
        };
        let mut temp_models: Vec<Model> = Vec::new();
        // Unknown motions are interop gaps the shell must see (never silently idle).
        let mut skipped: Vec<String> = self
            .unknown_motions
            .iter()
            .map(|m| format!("motion unknown: {m}"))
            .collect();
        let mut drawn = 0usize;
        // Resolve every actor's model first (the skeletal path may need &mut self for caches).
        let entities = self.entities.clone();
        let mut resolved: Vec<(EntityState, Model)> = Vec::with_capacity(entities.len());
        for entity in &entities {
            let elapsed = now_ms - entity.sequence_started_ms;
            let model = if entity.is_player && self.player_body.is_some() {
                let sequence_id = if self.sequences.contains_key(&entity.sequence) {
                    entity.sequence
                } else {
                    skipped.push(format!(
                        "{}: sequence {} not loaded, standing",
                        entity.id, entity.sequence
                    ));
                    crate::actor::PLAYER_IDLE
                };
                if !self.sequences.contains_key(&sequence_id) {
                    skipped.push(format!(
                        "{}: idle sequence {} not loaded",
                        entity.id, sequence_id
                    ));
                    continue;
                }
                let Some((frame, ended)) = self.sequence_frame(sequence_id, elapsed) else {
                    continue;
                };
                if ended && sequence_id != crate::actor::PLAYER_DEATH {
                    // One-shot actions return to the stance once finished.
                    match self.sequence_frame(crate::actor::PLAYER_IDLE, elapsed) {
                        Some((idle_frame, _)) => {
                            self.player_frame_model(crate::actor::PLAYER_IDLE, idle_frame)?
                        }
                        None => None,
                    }
                } else {
                    self.player_frame_model(sequence_id, frame)?
                }
            } else if let Some(def) = self.npc_defs.get(&entity.npc) {
                let mut sequence_id = entity.sequence;
                if !self.sequences.contains_key(&sequence_id) {
                    let fallback = if def.stand() >= 0 { def.stand() } else { -1 };
                    if sequence_id >= 0 {
                        skipped.push(format!(
                            "{}: sequence {} not loaded, using {}",
                            entity.id, sequence_id, fallback
                        ));
                    }
                    sequence_id = fallback;
                }
                if sequence_id < 0 || !self.sequences.contains_key(&sequence_id) {
                    // Definition without a stand sequence (or unloaded): draw the static base.
                    let mut model = def.base.clone();
                    crate::anim::scale_float(
                        &mut model,
                        def.record.width_scale,
                        def.record.height_scale,
                    );
                    model.compute_cylinder_bounds();
                    Some(model)
                } else {
                    let Some((frame, _)) = self.sequence_frame(sequence_id, elapsed) else {
                        continue;
                    };
                    self.npc_frame_model(entity.npc, sequence_id, frame)
                }
            } else if let Some(pack) = self.npc_packs.get(&entity.npc) {
                // Baked-frame pack (validation fixtures / packs without a definition).
                let sequence = if pack.sequences.contains_key(&entity.sequence) {
                    entity.sequence
                } else {
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
                let Some(frame) = pack.frame_index(sequence, elapsed) else {
                    continue;
                };
                pack.frame_model(sequence, frame)
            } else {
                skipped.push(format!(
                    "{}: no animation pack for npc {}",
                    entity.id, entity.npc
                ));
                continue;
            };
            let Some(model) = model else { continue };
            resolved.push((entity.clone(), model));
        }
        // Live layers: door states, temporary objects (fires) and ground items.
        let mut door_walls: Vec<(i32, i32, i32, crate::scene::Wall)> = Vec::new();
        for door in &self.door_states {
            let rotation = door.quarter_turns.rem_euclid(4);
            let Some(scene) = self.scene.as_ref() else {
                break;
            };
            let lx = door.tile.x - base_x;
            let ly = door.tile.y - base_y;
            let ex = lx + scene.offset;
            let ey = ly + scene.offset;
            if ex < 0 || ey < 0 || ex >= scene.width || ey >= scene.height {
                continue;
            }
            let plane = door.tile.plane.clamp(0, scene.planes - 1);
            let index = scene.tile_index(plane, ex, ey);
            let Some(existing) = scene.walls.get(&index) else {
                skipped.push(format!(
                    "door {} at {},{}: no wall on that tile",
                    door.object, door.tile.x, door.tile.y
                ));
                continue;
            };
            let Some(variant) = self.dynamic_objects.get(&(door.object, 0, rotation)) else {
                skipped.push(format!(
                    "door {} rotation {rotation}: model not loaded",
                    door.object
                ));
                continue;
            };
            let model_index = temp_models.len();
            temp_models.push(variant.plain.clone());
            let wall = crate::scene::Wall {
                model_a: crate::scene::draw::TEMP_MODEL_BASE + model_index as i32,
                model_b: -1,
                orientation_a: 1 << rotation,
                orientation_b: 0,
                x: existing.x,
                height: existing.height,
                z: existing.z,
                hash: existing.hash,
                // A door variant is always a straight wall (type 0) at the given rotation.
                config: rotation << 6,
            };
            let _ = door.open;
            door_walls.push((plane, lx, ly, wall));
        }
        let mut temporaries: Vec<(TemporaryEntity, Model)> = Vec::new();
        for temporary in &self.temporary_objects {
            let Some(scene) = self.scene.as_ref() else {
                break;
            };
            let Some(variant) = self.dynamic_objects.get(&(temporary.object, 10, 0)) else {
                skipped.push(format!(
                    "{}: temporary object {} model not loaded",
                    temporary.id, temporary.object
                ));
                continue;
            };
            let model = if variant.frames.is_empty() {
                variant.plain.clone()
            } else {
                let total: i64 = variant
                    .frame_lengths
                    .iter()
                    .map(|&l| i64::from(l.max(1)))
                    .sum();
                let cycles = ((now_ms - temporary.started_ms) / CLIENT_CYCLE_MS)
                    .floor()
                    .max(0.0) as i64
                    % total.max(1);
                let mut acc = 0i64;
                let mut frame = 0usize;
                for (i, &len) in variant.frame_lengths.iter().enumerate() {
                    acc += i64::from(len.max(1));
                    if cycles < acc {
                        frame = i;
                        break;
                    }
                }
                variant.frames[frame].clone()
            };
            let lx = temporary.tile.x - base_x;
            let ly = temporary.tile.y - base_y;
            if lx < 0 || ly < 0 || lx >= scene.max_x || ly >= scene.max_y {
                continue;
            }
            let plane = temporary.tile.plane.clamp(0, scene.planes - 1);
            let x = lx * 128 + 64;
            let z = ly * 128 + 64;
            let height = tile_height(scene, plane, x, z);
            // Original object tag: x | y<<7 | type<<14 | plane<<16 | id<<20.
            let hash = i64::from(lx & 127)
                | (i64::from(ly & 127) << 7)
                | (2i64 << 14)
                | (i64::from(plane) << 16)
                | (i64::from(temporary.object) << 20);
            temporaries.push((
                TemporaryEntity {
                    plane,
                    tile_x: lx,
                    tile_y: ly,
                    size_x: 1,
                    size_y: 1,
                    x,
                    height,
                    z,
                    orientation: 0,
                    hash,
                    model: 0,
                },
                model,
            ));
        }
        let mut item_layers: Vec<(i32, i32, i32, crate::scene::draw::ItemLayer)> = Vec::new();
        {
            let mut per_tile: HashMap<(i32, i32, i32), Vec<&GroundItemState>> = HashMap::new();
            for item in &self.ground_items {
                per_tile
                    .entry((item.tile.plane, item.tile.x, item.tile.y))
                    .or_default()
                    .push(item);
            }
            for ((plane, tx, ty), stack) in per_tile {
                let Some(scene) = self.scene.as_ref() else {
                    break;
                };
                let lx = tx - base_x;
                let ly = ty - base_y;
                if lx < 0 || ly < 0 || lx >= scene.max_x || ly >= scene.max_y {
                    continue;
                }
                let plane = plane.clamp(0, scene.planes - 1);
                let x = lx * 128 + 64;
                let z = ly * 128 + 64;
                let height = tile_height(scene, plane, x, z);
                // The original shows the three most recently dropped stacks (list head first).
                let mut models = Vec::new();
                for item in stack.iter().take(3) {
                    match self.ground_item_model(item.item, item.quantity) {
                        Some(model) => {
                            models.push(temp_models.len());
                            temp_models.push(model.clone());
                        }
                        None => skipped.push(format!(
                            "{}: ground item {} model not loaded",
                            item.id, item.item
                        )),
                    }
                }
                if models.is_empty() {
                    continue;
                }
                let hash = i64::from(lx & 127)
                    | (i64::from(ly & 127) << 7)
                    | (3i64 << 14)
                    | (i64::from(plane) << 16)
                    | (i64::from(stack[0].item) << 20);
                item_layers.push((
                    plane,
                    lx,
                    ly,
                    crate::scene::draw::ItemLayer {
                        models,
                        x,
                        height,
                        z,
                        offset: 0,
                        deferred: false,
                        hash,
                    },
                ));
            }
        }
        let roof = self.roof_removal();
        let top_plane = match self.top_plane_override {
            Some(limit) => limit,
            None => self.stock_top_plane(base_x, base_y),
        };
        let scene = self.scene.as_ref().expect("scene checked above");
        let drawer = self.drawer.as_mut().expect("drawer follows scene");
        for (plane, lx, ly, wall) in door_walls {
            if !drawer.override_wall(scene, plane, lx, ly, wall) {
                skipped.push(format!(
                    "door at {},{}: override refused",
                    lx + base_x,
                    ly + base_y
                ));
            }
        }
        for (mut entity, model) in temporaries {
            entity.model = temp_models.len();
            temp_models.push(model);
            if drawer.add_temporary(scene, &entity) {
                drawn += 1;
            } else {
                skipped.push(format!(
                    "temporary object at {},{}: tile slots full",
                    entity.tile_x + base_x,
                    entity.tile_y + base_y
                ));
            }
        }
        for (plane, lx, ly, layer) in item_layers {
            if !drawer.add_item_layer(scene, plane, lx, ly, layer) {
                skipped.push(format!(
                    "ground items at {},{}: outside scene",
                    lx + base_x,
                    ly + base_y
                ));
            }
        }
        for (entity, model) in resolved {
            let local_x = entity.tile.x - base_x;
            let local_y = entity.tile.y - base_y;
            if local_x < 0 || local_y < 0 || local_x >= scene.max_x || local_y >= scene.max_y {
                skipped.push(format!("{}: tile outside loaded scene", entity.id));
                continue;
            }
            let plane = entity.tile.plane.clamp(0, scene.planes - 1);
            let size = entity.size.max(1);
            // Multi-tile NPCs stand on the centre of their footprint (original actor placement).
            let x = local_x * 128 + size * 64;
            let z = local_y * 128 + size * 64;
            let height = tile_height(scene, plane, x, z);
            let model_index = temp_models.len();
            temp_models.push(model);
            let ok = drawer.add_temporary(
                scene,
                &TemporaryEntity {
                    plane,
                    tile_x: local_x,
                    tile_y: local_y,
                    size_x: size,
                    size_y: size,
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
            top_plane,
            focal_x: self.camera.x - base_x * 128,
            focal_z: self.camera.y - base_y * 128,
            center_on_camera: true,
            far_clip: self.camera.far,
            animation_cycles,
            roof,
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

    /// Static-placement projection cache statistics of the last frame: (replayed, projected).
    pub fn model_cache_stats(&self) -> (usize, usize) {
        self.drawer
            .as_ref()
            .map(|d| d.model_cache_stats())
            .unwrap_or((0, 0))
    }

    pub fn load_model(&mut self, id: &str, bytes: &[u8]) -> Result<(), RenderError> {
        let model = Model::from_chunks(bytes)?;
        self.models.insert(id.to_string(), model);
        Ok(())
    }

    pub fn load_model_value(&mut self, id: &str, model: Model) {
        self.models.insert(id.to_string(), model);
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

    /// Builds the model-only player preview an interface model component shows (character
    /// creator): the current body and gear, animated by the player's idle motion unless a
    /// sequence/frame is given. The triangle stream targets a `width`×`height` surface whose
    /// uncovered pixels must stay transparent; the scene frame, its picks and raster state are
    /// left untouched. Returns `Ok(None)` when no player body is loaded.
    pub fn build_player_preview_frame(
        &mut self,
        preview: &PlayerPreview,
        now_ms: f64,
    ) -> Result<Option<PreviewFrame>, RenderError> {
        let start = now();
        if preview.width <= 0 || preview.height <= 0 {
            return Err(RenderError::Scene(format!(
                "preview surface {}x{} is empty",
                preview.width, preview.height
            )));
        }
        let sequence_id = preview.sequence.unwrap_or(crate::actor::PLAYER_IDLE);
        let started = *self.preview_started_ms.get_or_insert(now_ms);
        let frame = match preview.frame {
            Some(frame) => frame,
            None => {
                let Some((frame, _)) = self.sequence_frame(sequence_id, now_ms - started) else {
                    return Err(RenderError::Scene(format!(
                        "preview sequence {sequence_id} is not loaded"
                    )));
                };
                frame
            }
        };
        let Some(mut model) = self.player_frame_model(sequence_id, frame)? else {
            return Ok(None);
        };
        model.compute_cylinder_bounds();
        let (rotation_x, rotation_z) = preview.draw_rotation(now_ms - started);
        let t = crate::tables::tables();
        let sin_x =
            (t.sin2048[(rotation_x & 2047) as usize].wrapping_mul(preview.model_zoom)) >> 16;
        let cos_x =
            (t.cos2048[(rotation_x & 2047) as usize].wrapping_mul(preview.model_zoom)) >> 16;
        let mut state = RasterState::new(preview.width, preview.height, preview.rasterizer_zoom);
        state.center_x = preview.center_x;
        state.center_y = preview.center_y;
        let mut scratch = ModelScratch::default();
        self.preview_tris.clear();
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
                    rotation_z,
                    preview.rotation_y,
                    rotation_x,
                    preview.offset_x,
                    sin_x + preview.offset_y,
                    cos_x + preview.offset_y,
                    0,
                    &mut self.preview_tris,
                )
                .map_err(|_| RenderError::Scene("player preview draw aborted".into()))?;
        }
        let mut stats = DrawStats::default();
        for tri in &self.preview_tris {
            stats.count(tri);
        }
        let summary = FrameSummary {
            triangles: self.preview_tris.len(),
            stats,
            entities_drawn: 1,
            entities_skipped: Vec::new(),
            missing_models: 0,
            cpu_build_ms: now() - start,
        };
        Ok(Some(PreviewFrame {
            state,
            sequence: sequence_id,
            frame,
            summary,
        }))
    }

    /// Triangles of the last [`Self::build_player_preview_frame`].
    pub fn preview_triangles(&self) -> &[Tri] {
        &self.preview_tris
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

    /// The plane frames and the minimap are drawn for (normally the player's tile plane from
    /// the WorldView; developer fixtures without a player set it directly).
    pub fn set_plane(&mut self, plane: i32) {
        self.plane = plane;
    }

    pub fn plane(&self) -> i32 {
        self.plane
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
