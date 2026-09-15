//! wasm-bindgen ABI consumed by `web/renderer/src/index.ts`.
//!
//! Exact exported names (documented for the shell integrator):
//! * `WasmRenderer::new(canvas, width, height, palette_bytes)` — requests a real WebGPU adapter and
//!   device through wgpu, configures the canvas surface; rejects with a descriptive error when
//!   WebGPU is unavailable or the adapter is a software fallback. No WebGL fallback exists.
//! * `add_texture(bytes)`, `load_npc_pack(npc_id, bytes)`, `load_scene(id, scene_bytes, pack_bytes)`
//! * `resize(width, height)`, `set_camera(x, height, y, pitch, yaw, zoom, far)`
//! * `update_world(json)` — the shared `WorldView` serialized as JSON
//! * `frame(now_ms)` → `Promise<FrameRecordJs>` resolved after the GPU queue reports completion
//! * `pick(x, y)` → JSON `ScenePick` or `null`
//! * `device_epoch()`, `timestamps_supported()`, `asset_hashes()`

#![allow(clippy::new_ret_no_self, clippy::too_many_arguments)]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::future_to_promise;

use crate::core::{Camera, RendererCore};
use crate::error::RenderError;
use crate::gpu::{GpuRasterizer, GpuTextures, pack_frame};
use crate::palette::Palette;
use crate::scene::draw::PickTarget;

fn js_err(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
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
        let shared = self.inner.clone();
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
                inner.core.build_frame(now_ms).map_err(js_err)?;
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
                    let gpu_frame = raster.render(&state, &packed, 0).map_err(js_err)?;
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
            // Wait for the queue's submitted-work-done signal (genuine GPU completion).
            let mut record = record;
            let mut waited = 0u32;
            while !done.is_complete() {
                wasm_bindgen_futures::JsFuture::from(js_sys::Promise::new(&mut |resolve, _| {
                    let _ = web_sys::window()
                        .expect("window")
                        .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 0);
                }))
                .await
                .map_err(|e| js_err(format!("timer: {e:?}")))?;
                waited += 1;
                if waited > 20_000 {
                    return Err(js_err("GPU work never reported completion"));
                }
                if let Some(reason) = shared.borrow().device_lost.lock().expect("lock").clone() {
                    return Err(js_err(format!("WebGPU device lost during frame: {reason}")));
                }
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
        let (base_x, base_y) = inner.core.scene_base()?;
        let target = inner.core.pick(x, y)?;
        Some(match target {
            PickTarget::Tile { plane, x, y } => format!(
                r#"{{"kind":"tile","tile":{{"x":{},"y":{},"plane":{}}}}}"#,
                x + base_x,
                y + base_y,
                plane
            ),
            PickTarget::Object { hash, plane, x, y } => {
                format!(
                    r#"{{"kind":"entity","id":"{}","tile":{{"x":{},"y":{},"plane":{}}}}}"#,
                    hash,
                    x + base_x,
                    y + base_y,
                    plane
                )
            }
        })
    }

    pub fn last_frame_triangles(&self) -> u32 {
        self.inner.borrow().core.last_summary.triangles as u32
    }
}

impl From<RenderError> for JsValue {
    fn from(error: RenderError) -> Self {
        js_err(error)
    }
}
