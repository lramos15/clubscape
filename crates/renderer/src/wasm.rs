//! wasm-bindgen ABI consumed by `web/renderer/src/index.ts`.
//!
//! Exact exported names (documented for the shell integrator):
//! * `WasmRenderer::new(canvas, width, height, palette_bytes)` — requests a real WebGPU adapter and
//!   device through wgpu, configures the canvas surface; rejects with a descriptive error when
//!   WebGPU is unavailable or the adapter is a software fallback. No WebGL fallback exists.
//! * `add_texture(bytes)`, `load_npc_pack(npc_id, bytes)`, `load_model(id, bytes)`,
//!   `load_scene(id, scene_bytes, pack_bytes)`
//! * `resize(width, height)`, `set_camera(x, height, y, pitch, yaw, zoom, far)`
//! * `update_world(json, now_ms)` — the shared `WorldView` serialized as JSON
//! * `frame(now_ms)` → `Promise<FrameRecordJs>` resolved after the GPU queue reports completion
//! * `frame_model_fixture(model, npc, sequence, frame, yaw, camera_y, camera_z)` — developer
//!   replay of an approved model capture
//! * `pick(x, y)` → JSON `ScenePick` or `undefined`
//! * `adapter_info()`, `device_epoch()`, `timestamps_supported()`, `device_lost_reason()`,
//!   `scene_id()`, `last_frame_triangles()`
//!
//! See `crates/renderer/README.md` for the full ABI table.

#![allow(clippy::new_ret_no_self, clippy::too_many_arguments)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::core::{Camera, ModelFixture, PlayerPreview, RendererCore, WorldPick};
use crate::error::RenderError;
use crate::gpu::{GpuRasterizer, GpuTextures, pack_frame};
use crate::palette::Palette;

fn js_err(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

fn json_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".into())
}

enum FrameKind {
    Scene { now_ms: f64 },
    ModelFixture(ModelFixture),
}

struct Inner {
    core: RendererCore,
    raster: Option<GpuRasterizer>,
    surface: wgpu::Surface<'static>,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    sequence: u64,
    device_epoch: u32,
    device_lost: Arc<Mutex<Option<String>>>,
    /// Uncaptured WebGPU validation/OOM/internal errors since the last frame; any entry fails
    /// the frame explicitly instead of letting an invalid pipeline present a stale canvas.
    gpu_errors: Arc<Mutex<Vec<String>>>,
    textures_dirty: bool,
    /// Separate small rasterizer for interface model previews (surface-sized, coverage alpha).
    preview_raster: Option<GpuRasterizer>,
    preview_dirty: bool,
}

#[wasm_bindgen]
pub struct WasmRenderer {
    inner: Rc<RefCell<Inner>>,
}

#[wasm_bindgen]
pub struct FrameRecordJs {
    pub sequence: f64,
    pub submitted_at_ms: f64,
    pub completed_at_ms: f64,
    pub draw_calls: u32,
    pub primitives: u32,
    pub cpu_encode_ms: f64,
    pub gpu_duration_ms: f64,
    pub gpu_duration_known: bool,
    pub entities_drawn: u32,
    pub texture_fallbacks: u32,
    skipped: String,
}

#[wasm_bindgen]
impl FrameRecordJs {
    #[wasm_bindgen(getter)]
    pub fn skipped(&self) -> String {
        self.skipped.clone()
    }
}

#[wasm_bindgen]
impl WasmRenderer {
    /// Creates the renderer on a real WebGPU device. Fails explicitly when unavailable.
    #[wasm_bindgen(constructor)]
    pub fn new(
        canvas: web_sys::HtmlCanvasElement,
        width: u32,
        height: u32,
        palette_bytes: Vec<u8>,
    ) -> js_sys::Promise {
        future_to_promise(async move {
            let palette = Palette::from_chunks(&palette_bytes).map_err(js_err)?;
            let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
            descriptor.backends = wgpu::Backends::BROWSER_WEBGPU;
            let instance = wgpu::Instance::new(descriptor);
            let surface = instance
                .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
                .map_err(|e| js_err(format!("canvas surface: {e}")))?;
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    force_fallback_adapter: false,
                    compatible_surface: Some(&surface),
                    apply_limit_buckets: false,
                })
                .await
                .map_err(|e| js_err(format!("WebGPU adapter unavailable: {e}")))?;
            let info = adapter.get_info();
            if matches!(info.device_type, wgpu::DeviceType::Cpu) {
                return Err(js_err(format!(
                    "WebGPU adapter is a software fallback ({}); refusing to start",
                    info.name
                )));
            }
            let mut features = wgpu::Features::empty();
            if adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
                features |= wgpu::Features::TIMESTAMP_QUERY;
            }
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("clubscape-renderer"),
                    required_features: features,
                    required_limits: wgpu::Limits::downlevel_defaults()
                        .using_resolution(adapter.limits()),
                    experimental_features: wgpu::ExperimentalFeatures::disabled(),
                    memory_hints: wgpu::MemoryHints::Performance,
                    trace: wgpu::Trace::Off,
                })
                .await
                .map_err(|e| js_err(format!("WebGPU device request failed: {e}")))?;
            let device_lost: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
            {
                let slot = device_lost.clone();
                device.set_device_lost_callback(move |reason, message| {
                    *slot.lock().expect("lock") = Some(format!("{reason:?}: {message}"));
                });
            }
            let gpu_errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
            {
                let slot = gpu_errors.clone();
                device.on_uncaptured_error(Arc::new(move |error: wgpu::Error| {
                    slot.lock().expect("lock").push(error.to_string());
                }));
            }
            let caps = surface.get_capabilities(&adapter);
            let format = caps
                .formats
                .first()
                .copied()
                .ok_or_else(|| js_err("surface has no supported formats"))?;
            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format,
                color_space: wgpu::SurfaceColorSpace::Auto,
                width: width.max(1),
                height: height.max(1),
                present_mode: wgpu::PresentMode::Fifo,
                alpha_mode: caps
                    .alpha_modes
                    .first()
                    .copied()
                    .unwrap_or(wgpu::CompositeAlphaMode::Opaque),
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);
            let core = RendererCore::new(palette, width as i32, height as i32);
            let inner = Inner {
                core,
                raster: None,
                surface,
                adapter,
                device,
                queue,
                format,
                width,
                height,
                sequence: 0,
                device_epoch: 1,
                device_lost,
                gpu_errors,
                textures_dirty: true,
                preview_raster: None,
                preview_dirty: true,
            };
            Ok(WasmRenderer {
                inner: Rc::new(RefCell::new(inner)),
            }
            .into())
        })
    }

    pub fn adapter_info(&self) -> String {
        let inner = self.inner.borrow();
        let info = inner.adapter.get_info();
        format!(
            "{} ({:?}, backend {:?})",
            info.name, info.device_type, info.backend
        )
    }

    pub fn timestamps_supported(&self) -> bool {
        self.inner
            .borrow()
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY)
    }

    pub fn device_epoch(&self) -> u32 {
        self.inner.borrow().device_epoch
    }

    /// Non-null once the WebGPU device reported loss; frames then fail explicitly.
    pub fn device_lost_reason(&self) -> Option<String> {
        self.inner
            .borrow()
            .device_lost
            .lock()
            .expect("lock")
            .clone()
    }

    pub fn add_texture(&self, bytes: Vec<u8>) -> Result<i32, JsValue> {
        let mut inner = self.inner.borrow_mut();
        let id = inner.core.add_texture(&bytes).map_err(js_err)?;
        inner.textures_dirty = true;
        inner.preview_dirty = true;
        Ok(id)
    }

    pub fn load_npc_pack(&self, npc_id: i32, bytes: Vec<u8>) -> Result<(), JsValue> {
        self.inner
            .borrow_mut()
            .core
            .load_npc_pack_as(npc_id, &bytes)
            .map_err(js_err)
    }

    pub fn load_scene(
        &self,
        id: String,
        scene_bytes: Vec<u8>,
        pack_bytes: Vec<u8>,
    ) -> Result<(), JsValue> {
        self.inner
            .borrow_mut()
            .core
            .load_scene(&id, &scene_bytes, &pack_bytes)
            .map_err(js_err)
    }

    pub fn scene_id(&self) -> Option<String> {
        self.inner.borrow().core.scene_id().map(|s| s.to_string())
    }

    pub fn resize(&self, width: u32, height: u32) {
        let mut inner = self.inner.borrow_mut();
        if width == 0 || height == 0 {
            return;
        }
        inner.width = width;
        inner.height = height;
        let caps = inner.surface.get_capabilities(&inner.adapter);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: inner.format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps
                .alpha_modes
                .first()
                .copied()
                .unwrap_or(wgpu::CompositeAlphaMode::Opaque),
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        inner.surface.configure(&inner.device, &config);
        inner.core.resize(width as i32, height as i32);
        if let Some(raster) = inner.raster.as_mut() {
            raster.resize(width, height);
        }
    }

    /// Camera in world units (tile * 128), 16384 units per turn, height negative-up.
    pub fn set_camera(
        &self,
        x: i32,
        height: i32,
        y: i32,
        pitch: i32,
        yaw: i32,
        zoom: i32,
        far: i32,
    ) -> Result<(), JsValue> {
        self.inner
            .borrow_mut()
            .core
            .set_camera(Camera {
                x,
                height,
                y,
                pitch,
                yaw,
                zoom,
                far,
            })
            .map_err(js_err)
    }

    pub fn update_world(&self, json: String, now_ms: f64) -> Result<(), JsValue> {
        self.inner
            .borrow_mut()
            .core
            .update_world(&json, now_ms)
            .map_err(js_err)
    }

    /// Builds and submits one frame; resolves when the GPU queue reports the work complete.
    pub fn frame(&self, now_ms: f64) -> js_sys::Promise {
        Self::run_frame(self.inner.clone(), FrameKind::Scene { now_ms })
    }

    /// Developer fixture replay of an approved model capture (legacy draw, zoom 1024,
    /// background 0x303030). `npc`/`sequence`/`frame` select a baked NPC frame, otherwise
    /// `model` names a model loaded with `load_model`.
    pub fn frame_model_fixture(
        &self,
        model: String,
        npc: i32,
        sequence: i32,
        frame: u32,
        yaw: i32,
        camera_y: i32,
        camera_z: i32,
    ) -> js_sys::Promise {
        let fixture = ModelFixture {
            model,
            npc: (npc >= 0).then_some((npc, sequence, frame as usize)),
            yaw,
            camera_y,
            camera_z,
        };
        Self::run_frame(self.inner.clone(), FrameKind::ModelFixture(fixture))
    }

    pub fn load_model(&self, id: String, bytes: Vec<u8>) -> Result<(), JsValue> {
        self.inner
            .borrow_mut()
            .core
            .load_model(&id, &bytes)
            .map_err(js_err)
    }

    fn run_frame(shared: Rc<RefCell<Inner>>, kind: FrameKind) -> js_sys::Promise {
        future_to_promise(async move {
            // (Re)create the GPU rasterizer when textures changed, validating pipeline creation
            // through an error scope so shader/pipeline failures reject instead of presenting
            // a stale canvas.
            let scope = {
                let mut inner = shared.borrow_mut();
                if let Some(reason) = inner.device_lost.lock().expect("lock").clone() {
                    return Err(js_err(format!("WebGPU device lost: {reason}")));
                }
                if inner.raster.is_none() || inner.textures_dirty {
                    let scope = inner.device.push_error_scope(wgpu::ErrorFilter::Validation);
                    let textures = GpuTextures::from_set(&inner.core.textures);
                    let (w, h) = (inner.width, inner.height);
                    let raster = GpuRasterizer::new(
                        inner.device.clone(),
                        inner.queue.clone(),
                        &inner.core.palette.rgb,
                        &textures,
                        w,
                        h,
                    )
                    .map_err(js_err)?;
                    inner.raster = Some(raster);
                    inner.textures_dirty = false;
                    Some(scope)
                } else {
                    None
                }
            };
            if let Some(scope) = scope
                && let Some(error) = scope.pop().await
            {
                shared.borrow_mut().raster = None;
                return Err(js_err(format!("WebGPU pipeline creation failed: {error}")));
            }
            let (done, record) = {
                let mut inner = shared.borrow_mut();
                let cpu_start = js_sys::Date::now();
                let clear = match &kind {
                    FrameKind::Scene { now_ms } => {
                        inner.core.build_frame(*now_ms).map_err(js_err)?;
                        0
                    }
                    FrameKind::ModelFixture(fixture) => {
                        inner
                            .core
                            .build_model_fixture_frame(fixture)
                            .map_err(js_err)?;
                        0x30_3030
                    }
                };
                let state = inner.core.state;
                let packed = pack_frame(&state, inner.core.triangles(), &inner.core.textures);
                let (w, h) = (inner.width, inner.height);
                let (gpu_frame, presented, texture_fallbacks) = {
                    let Inner {
                        raster,
                        surface,
                        queue,
                        format,
                        ..
                    } = &mut *inner;
                    let raster = raster.as_mut().expect("raster created above");
                    raster.resize(w, h);
                    let gpu_frame = raster.render(&state, &packed, clear).map_err(js_err)?;
                    let surface_texture = match surface.get_current_texture() {
                        wgpu::CurrentSurfaceTexture::Success(t)
                        | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
                        other => {
                            return Err(js_err(format!(
                                "canvas surface texture unavailable: {other:?}"
                            )));
                        }
                    };
                    let view = surface_texture
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    raster.blit(&view, *format);
                    queue.present(surface_texture);
                    (
                        gpu_frame,
                        raster.completion_signal(),
                        raster.last_record.texture_fallbacks,
                    )
                };
                let cpu_encode_ms = js_sys::Date::now() - cpu_start;
                inner.sequence += 1;
                let summary = inner.core.last_summary.clone();
                let record = FrameRecordJs {
                    sequence: inner.sequence as f64,
                    submitted_at_ms: js_sys::Date::now(),
                    completed_at_ms: 0.0,
                    draw_calls: 2,
                    primitives: summary.triangles as u32,
                    cpu_encode_ms,
                    gpu_duration_ms: 0.0,
                    gpu_duration_known: false,
                    entities_drawn: summary.entities_drawn as u32,
                    texture_fallbacks: texture_fallbacks as u32,
                    skipped: summary.entities_skipped.join("; "),
                };
                ((gpu_frame, presented), record)
            };
            let (gpu_frame, done) = done;
            // Wait for the queue's submitted-work-done signal (genuine GPU completion). The
            // callback wakes this future directly; the watchdog only guards a silent device.
            let mut record = record;
            await_signal(
                &shared,
                done.completed(),
                "GPU work never reported completion",
            )
            .await?;
            // Timestamp readbacks map slightly after completion; wait so the record carries the
            // measured span of this frame rather than none.
            if gpu_frame.timestamps_pending() {
                await_signal(
                    &shared,
                    gpu_frame.timestamps_ready(),
                    "timestamp readback never mapped",
                )
                .await?;
            }
            {
                let inner = shared.borrow();
                let mut errors = inner.gpu_errors.lock().expect("lock");
                if !errors.is_empty() {
                    let joined = errors.join("\n");
                    errors.clear();
                    return Err(js_err(format!(
                        "WebGPU reported errors during frame {}: {joined}",
                        record.sequence
                    )));
                }
            }
            record.completed_at_ms = js_sys::Date::now();
            if let Some(ns) = gpu_frame.gpu_duration_ns() {
                record.gpu_duration_ms = ns as f64 / 1_000_000.0;
                record.gpu_duration_known = true;
            }
            Ok(record.into())
        })
    }

    /// JSON `{"kind":"tile","tile":{...}}` / `{"kind":"entity","id":..,"tile":{...}}` or null.
    pub fn pick(&self, x: i32, y: i32) -> Option<String> {
        let mut inner = self.inner.borrow_mut();
        let pick = inner.core.pick_world(x, y)?;
        let tile = |x: i32, y: i32, plane: i32| format!(r#"{{"x":{x},"y":{y},"plane":{plane}}}"#);
        Some(match pick {
            WorldPick::Tile { x, y, plane } => {
                format!(r#"{{"kind":"tile","tile":{}}}"#, tile(x, y, plane))
            }
            WorldPick::Actor { id, x, y, plane } => {
                format!(
                    r#"{{"kind":"entity","id":{},"tile":{}}}"#,
                    json_string(&id),
                    tile(x, y, plane)
                )
            }
            WorldPick::Scenery {
                object_id,
                kind,
                x,
                y,
                plane,
                span_x,
                span_y,
                entity,
            } => {
                let detail = format!(
                    r#""scenery":{{"objectId":{object_id},"type":{kind},"spanX":{span_x},"spanY":{span_y}}}"#
                );
                match entity {
                    // Scenery the WorldView lists as an interactable object entity.
                    Some(id) => format!(
                        r#"{{"kind":"entity","id":{},"tile":{},{detail}}}"#,
                        json_string(&id),
                        tile(x, y, plane)
                    ),
                    // Plain scenery: a tile pick for the contract, with the source object noted.
                    None => format!(r#"{{"kind":"tile","tile":{},{detail}}}"#, tile(x, y, plane)),
                }
            }
        })
    }

    /// Loads a world block (64x64 map square) for scene assembly.
    pub fn load_block(
        &self,
        square: i32,
        block_bytes: Vec<u8>,
        pack_bytes: Vec<u8>,
    ) -> Result<(), JsValue> {
        self.inner
            .borrow_mut()
            .core
            .load_block(square, &block_bytes, &pack_bytes)
            .map_err(js_err)
    }

    pub fn has_block(&self, square: i32) -> bool {
        self.inner.borrow().core.has_block(square)
    }

    pub fn unload_block(&self, square: i32) {
        self.inner.borrow_mut().core.unload_block(square);
    }

    /// Map squares (`x << 8 | y`) a scene at `base` needs, as the original loader requests them.
    pub fn squares_for_base(base_x: i32, base_y: i32) -> Vec<i32> {
        RendererCore::squares_for_base(base_x, base_y)
    }

    /// Original scene base for a player tile (`((tile >> 3) - 6) * 8`).
    pub fn base_for_tile(x: i32, y: i32) -> Vec<i32> {
        let (bx, by) = RendererCore::base_for_tile(x, y);
        vec![bx, by]
    }

    /// Whether the tile is within `margin` tiles of the current scene edge (or no scene exists).
    pub fn needs_recenter(&self, x: i32, y: i32, margin: i32) -> bool {
        self.inner.borrow().core.needs_recenter(x, y, margin)
    }

    /// Assembles the scene around `base` from loaded blocks; returns the missing squares.
    pub fn assemble_scene(
        &self,
        base_x: i32,
        base_y: i32,
        now_ms: f64,
    ) -> Result<Vec<i32>, JsValue> {
        self.inner
            .borrow_mut()
            .core
            .assemble_scene(base_x, base_y, true, now_ms)
            .map_err(js_err)
    }

    pub fn last_frame_triangles(&self) -> u32 {
        self.inner.borrow().core.last_summary.triangles as u32
    }

    // ----- Skeletal animation, definitions, player body and gear -----

    /// Loads an exported sequence (`anim/seq-<id>.bin`, chunks SEQH/SEQL/SEQF/SEQI/SKEL/FRMT).
    /// Returns the sequence id.
    pub fn load_sequence(&self, bytes: Vec<u8>) -> Result<i32, JsValue> {
        self.inner
            .borrow_mut()
            .core
            .load_sequence(&bytes)
            .map_err(js_err)
    }

    /// Loads an NPC definition (`manifest.npc_definitions[i]` as JSON) with its lit base model.
    /// Returns the NPC id.
    pub fn load_npc_definition(
        &self,
        record_json: String,
        base_bytes: Vec<u8>,
    ) -> Result<i32, JsValue> {
        self.inner
            .borrow_mut()
            .core
            .load_npc_definition(&record_json, &base_bytes)
            .map_err(js_err)
    }

    /// Installs the approved player body: the penguin base model (NPC 2063, model 21547) at its
    /// definition scales, the sequences that drive it natively (stand/walk), and the human
    /// reference body used to retarget player-appearance sequences onto penguin labels.
    pub fn load_player_body(
        &self,
        penguin_base: Vec<u8>,
        width_scale: i32,
        height_scale: i32,
        native_sequences: Vec<i32>,
        human_reference: Vec<u8>,
    ) -> Result<(), JsValue> {
        self.inner
            .borrow_mut()
            .core
            .load_player_body(
                &penguin_base,
                width_scale,
                height_scale,
                native_sequences,
                &human_reference,
            )
            .map_err(js_err)
    }

    /// Loads an equippable item's worn model (`models/item-<id>-equip.bin`).
    pub fn load_equip_model(&self, item_id: i32, bytes: Vec<u8>) -> Result<(), JsValue> {
        self.inner
            .borrow_mut()
            .core
            .load_equip_model(item_id, &bytes)
            .map_err(js_err)
    }

    /// Loads one dynamic object variant (`manifest.dynamic_objects[i].variants[j]`): the plain
    /// model plus optional baked animation frames (`Array<Uint8Array>`) with their lengths.
    pub fn load_dynamic_object(
        &self,
        object_id: i32,
        kind: i32,
        orientation: i32,
        plain: Vec<u8>,
        frames: js_sys::Array,
        frame_lengths: Vec<i32>,
    ) -> Result<(), JsValue> {
        let frames: Vec<Vec<u8>> = frames
            .iter()
            .map(|value| {
                value
                    .dyn_into::<js_sys::Uint8Array>()
                    .map(|array| array.to_vec())
                    .map_err(|_| js_err("dynamic object frames must be Uint8Array values"))
            })
            .collect::<Result<_, _>>()?;
        self.inner
            .borrow_mut()
            .core
            .load_dynamic_object(object_id, kind, orientation, &plain, &frames, frame_lengths)
            .map_err(js_err)
    }

    /// Loads a ground-item stack model for quantities `>= min_quantity`.
    pub fn load_ground_item(
        &self,
        item_id: i32,
        min_quantity: f64,
        bytes: Vec<u8>,
    ) -> Result<(), JsValue> {
        self.inner
            .borrow_mut()
            .core
            .load_ground_item(item_id, min_quantity as i64, &bytes)
            .map_err(js_err)
    }

    /// Top drawn plane override (`br`, the original `dh` plane argument). The approved fixture
    /// captures pinned it to 0 (ground plane only); `undefined` restores the stock live rule
    /// (`cz.ch`: all planes unless a roof-flagged tile of the player's plane lies on the
    /// camera→player line at pitch < 2480, then the player's plane).
    pub fn set_top_plane_override(&self, limit: Option<i32>) {
        self.inner.borrow_mut().core.set_top_plane_override(limit);
    }

    /// Instanced map flag (`cy.as`): the stock rule then always draws up to the player's plane.
    pub fn set_instanced_map(&self, instanced: bool) {
        self.inner.borrow_mut().core.set_instanced_map(instanced);
    }

    /// Developer-only: derive action motions from the activity string and adjacent scenery when
    /// the world view supplies no source animation. Off by default; not final M1 logic.
    pub fn set_motion_fallback(&self, enabled: bool) {
        self.inner.borrow_mut().core.set_motion_fallback(enabled);
    }

    /// Whether the player is running (original two-tiles-per-server-tick rule, or the `run`
    /// setting when ticks are unavailable).
    pub fn player_running(&self) -> bool {
        self.inner.borrow().core.player_running()
    }

    /// Actors whose reported state implies an action but whose source motion was not supplied
    /// in the last world view (JSON array of strings). Empty when every motion is explicit.
    pub fn unknown_motions(&self) -> String {
        let inner = self.inner.borrow();
        let items: Vec<String> = inner
            .core
            .unknown_motions()
            .iter()
            .map(|m| json_string(m))
            .collect();
        format!("[{}]", items.join(","))
    }

    /// Original roof-removal mode bits (1 player tile, 2 hovered tile, 4 walk destination,
    /// 8 camera line); 0 draws every roof like the stock client the fixtures were captured with.
    pub fn set_roof_mode(&self, mode: i32) {
        self.inner.borrow_mut().core.set_roof_mode(mode);
    }

    /// Hovered world tile and walk destination consulted by roof modes 2 and 4 (`undefined`
    /// clears either).
    pub fn set_roof_context(
        &self,
        hovered_x: Option<i32>,
        hovered_y: Option<i32>,
        destination_x: Option<i32>,
        destination_y: Option<i32>,
    ) {
        let pair = |x: Option<i32>, y: Option<i32>| x.zip(y);
        self.inner.borrow_mut().core.set_roof_context(
            pair(hovered_x, hovered_y),
            pair(destination_x, destination_y),
        );
    }

    /// JSON report of the current gear fit on the penguin body: per item the bound human label,
    /// the penguin label chosen, `penetration` (deepest body vertex inside the item's box, target
    /// ≤ 1), `gap` (item↔body clearance, target ≤ 2), `anchorShift` (contact-solve translation
    /// from the retargeted design position), `designPenetration` (the same box measure on the
    /// human body the item was designed for) in source units, and the retarget scale. Empty
    /// array before a body/gear is assembled.
    pub fn player_fit_report(&self) -> String {
        let inner = self.inner.borrow();
        let Some((_, fits)) = inner.core.player_fit_report() else {
            return "[]".into();
        };
        let items: Vec<String> = fits
            .iter()
            .map(|f| {
                format!(
                    r#"{{"itemId":{},"slot":{},"humanLabel":{},"penguinLabel":{},"penetration":{:.3},"gap":{:.3},"anchorShift":{:.3},"shiftDirection":[{:.3},{:.3},{:.3}],"pcaBoxPenetration":{:.3},"designPenetration":{:.3},"scale":{:.4}}}"#,
                    f.item_id,
                    json_string(&f.slot),
                    f.human_label,
                    f.penguin_label,
                    f.penetration,
                    f.gap,
                    f.anchor_shift,
                    f.shift_direction[0],
                    f.shift_direction[1],
                    f.shift_direction[2],
                    f.pca_box_penetration,
                    f.design_penetration,
                    f.scale
                )
            })
            .collect();
        format!("[{}]", items.join(","))
    }

    /// Current scene placement for HUD helpers (minimap): JSON
    /// `{"baseX","baseY","sizeTiles":104,"blocks":bool}` or `undefined` without a scene.
    pub fn scene_placement(&self) -> Option<String> {
        let inner = self.inner.borrow();
        let (base_x, base_y) = inner.core.scene_base()?;
        Some(format!(
            r#"{{"baseX":{base_x},"baseY":{base_y},"sizeTiles":104,"blocks":{}}}"#,
            inner.core.scene_id().is_none()
        ))
    }

    /// Renders the model-only player preview an interface model component shows and resolves
    /// with tightly packed RGBA8 pixels (`Uint8ClampedArray`, `width * height * 4`; alpha 255
    /// only where the model covered the pixel) after the GPU completed the work and the
    /// readback mapped. `options_json` fields (all optional, defaults = the exported interface
    /// 679 component 73 draw): `width`, `height`, `centerX`, `centerY`, `contentType` (328 =
    /// character-design sway/pitch overrides), `rasterizerZoom`, `modelZoom`, `rotationX`,
    /// `rotationY`, `rotationZ`, `offsetX`, `offsetY`, `sequence`, `frame`. Resolves
    /// `undefined` when no player body is loaded.
    pub fn frame_player_preview(&self, options_json: String, now_ms: f64) -> js_sys::Promise {
        let shared = self.inner.clone();
        future_to_promise(async move {
            let preview = parse_preview(&options_json)?;
            let (w, h) = (preview.width as u32, preview.height as u32);
            let scope = {
                let mut inner = shared.borrow_mut();
                if let Some(reason) = inner.device_lost.lock().expect("lock").clone() {
                    return Err(js_err(format!("WebGPU device lost: {reason}")));
                }
                let size_changed = inner
                    .preview_raster
                    .as_ref()
                    .is_some_and(|r| r.size() != (w, h));
                if inner.preview_raster.is_none() || inner.preview_dirty || size_changed {
                    let scope = inner.device.push_error_scope(wgpu::ErrorFilter::Validation);
                    let textures = GpuTextures::from_set(&inner.core.textures);
                    let raster = GpuRasterizer::new(
                        inner.device.clone(),
                        inner.queue.clone(),
                        &inner.core.palette.rgb,
                        &textures,
                        w,
                        h,
                    )
                    .map_err(js_err)?;
                    inner.preview_raster = Some(raster);
                    inner.preview_dirty = false;
                    Some(scope)
                } else {
                    None
                }
            };
            if let Some(scope) = scope
                && let Some(error) = scope.pop().await
            {
                shared.borrow_mut().preview_raster = None;
                return Err(js_err(format!(
                    "WebGPU preview pipeline creation failed: {error}"
                )));
            }
            let (readback, done) = {
                let mut inner = shared.borrow_mut();
                let Inner {
                    core,
                    preview_raster,
                    ..
                } = &mut *inner;
                let Some(frame) = core
                    .build_player_preview_frame(&preview, now_ms)
                    .map_err(js_err)?
                else {
                    return Ok(JsValue::UNDEFINED);
                };
                let packed = pack_frame(&frame.state, core.preview_triangles(), &core.textures);
                let raster = preview_raster
                    .as_mut()
                    .expect("preview raster created above");
                raster
                    .render_with_coverage(&frame.state, &packed, 0, true)
                    .map_err(js_err)?;
                let readback = raster.begin_read_back();
                (readback, raster.completion_signal())
            };
            await_signal(
                &shared,
                done.completed(),
                "preview GPU work never completed",
            )
            .await?;
            await_signal(&shared, readback.mapped(), "preview readback never mapped").await?;
            if let Some(Err(e)) = readback.is_ready() {
                return Err(js_err(format!("preview readback: {e}")));
            }
            {
                let inner = shared.borrow();
                let mut errors = inner.gpu_errors.lock().expect("lock");
                if !errors.is_empty() {
                    let joined = errors.join("\n");
                    errors.clear();
                    return Err(js_err(format!(
                        "WebGPU reported errors during the preview: {joined}"
                    )));
                }
            }
            let rgba = readback.take().map_err(js_err)?;
            let array = js_sys::Uint8ClampedArray::new_with_length(rgba.len() as u32);
            array.copy_from(&rgba);
            Ok(array.into())
        })
    }
}

/// Awaits a GPU callback signal, failing when the device is reported lost or nothing answers
/// within ~20 s (a 1 s watchdog timer runs alongside so a silent device cannot hang a frame).
async fn await_signal(
    shared: &Rc<RefCell<Inner>>,
    signal: impl Future<Output = ()>,
    timeout_message: &str,
) -> Result<(), JsValue> {
    let mut signal = std::pin::pin!(signal);
    let mut ticks = 0u32;
    loop {
        let watchdog =
            wasm_bindgen_futures::JsFuture::from(js_sys::Promise::new(&mut |resolve, _| {
                let _ = web_sys::window()
                    .expect("window")
                    .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 1000);
            }));
        let mut watchdog = std::pin::pin!(watchdog);
        let fired = std::future::poll_fn(|cx| {
            if signal.as_mut().poll(cx).is_ready() {
                return std::task::Poll::Ready(true);
            }
            match watchdog.as_mut().poll(cx) {
                std::task::Poll::Ready(_) => std::task::Poll::Ready(false),
                std::task::Poll::Pending => std::task::Poll::Pending,
            }
        })
        .await;
        if fired {
            return Ok(());
        }
        if let Some(reason) = shared.borrow().device_lost.lock().expect("lock").clone() {
            return Err(js_err(format!("WebGPU device lost: {reason}")));
        }
        ticks += 1;
        if ticks >= 20 {
            return Err(js_err(timeout_message));
        }
    }
}

fn parse_preview(options_json: &str) -> Result<PlayerPreview, JsValue> {
    let mut preview = PlayerPreview::default();
    if options_json.trim().is_empty() {
        return Ok(preview);
    }
    let value: serde_json::Value = serde_json::from_str(options_json)
        .map_err(|e| js_err(format!("preview options json: {e}")))?;
    let int = |name: &str, current: i32| -> Result<i32, JsValue> {
        match value.get(name) {
            None | Some(serde_json::Value::Null) => Ok(current),
            Some(v) => v
                .as_f64()
                .filter(|f| f.is_finite())
                .map(|f| f.trunc() as i32)
                .ok_or_else(|| js_err(format!("preview option {name} must be a finite number"))),
        }
    };
    preview.width = int("width", preview.width)?;
    preview.height = int("height", preview.height)?;
    preview.center_x = int("centerX", preview.center_x)?;
    preview.center_y = int("centerY", preview.center_y)?;
    preview.content_type = int("contentType", preview.content_type)?;
    preview.rasterizer_zoom = int("rasterizerZoom", preview.rasterizer_zoom)?;
    preview.model_zoom = int("modelZoom", preview.model_zoom)?;
    preview.rotation_x = int("rotationX", preview.rotation_x)?;
    preview.rotation_y = int("rotationY", preview.rotation_y)?;
    preview.rotation_z = int("rotationZ", preview.rotation_z)?;
    preview.offset_x = int("offsetX", preview.offset_x)?;
    preview.offset_y = int("offsetY", preview.offset_y)?;
    preview.sequence = match value.get("sequence") {
        None | Some(serde_json::Value::Null) => None,
        Some(_) => Some(int("sequence", 0)?),
    };
    preview.frame = match value.get("frame") {
        None | Some(serde_json::Value::Null) => None,
        Some(_) => Some(int("frame", 0)?.max(0) as usize),
    };
    if preview.width <= 0 || preview.height <= 0 || preview.width > 4096 || preview.height > 4096 {
        return Err(js_err(format!(
            "preview surface {}x{} is out of range",
            preview.width, preview.height
        )));
    }
    Ok(preview)
}

impl From<RenderError> for JsValue {
    fn from(error: RenderError) -> Self {
        js_err(error)
    }
}
