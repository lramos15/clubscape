//! Persistent native camera consumer. Browser code transports events and source snapshots only.

use std::collections::VecDeque;

use clubscape_camera::{
    Focus, FrameEffects, InitialReference, InitializationProvenance, Input, NormalCamera,
    SOURCE_CYCLE_NS, Viewport, WheelInput, WheelRoute,
};
use clubscape_renderer::camera::{
    CameraContext, CameraDelivery, CameraObjectDefinition, CameraScene, CameraSourceSample,
    CameraTerrain, decimal,
};
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;

use crate::BridgeError;

pub const MAX_PENDING_INPUTS: usize = 4096;
pub const MAX_STEPS_PER_FRAME: u64 = 500;
const MAX_JSON_BYTES: usize = 16 * 1024 * 1024;

type Result<T> = std::result::Result<T, String>;

#[derive(Clone)]
struct TimedInput {
    at: u64,
    input: Input,
}

#[derive(Clone)]
struct Clock {
    rendered_at: u64,
    next_tick: u64,
    held: Input,
    pending: VecDeque<TimedInput>,
}

#[derive(Default)]
pub struct CameraConsumer {
    camera: Option<NormalCamera>,
    terrain: Option<CameraTerrain>,
    context: Option<CameraContext>,
    initialization: Option<InitializationProvenance>,
    clock: Option<Clock>,
    physical_mouse: [i32; 2],
}

fn sample_inputs(
    sample: &CameraSourceSample,
    scene: &CameraScene,
) -> Result<(Focus, FrameEffects)> {
    if !sample.missing.is_empty() {
        return Err(format!(
            "native camera source inputs unavailable: {}",
            sample.missing.join("; ")
        ));
    }
    let context = &sample.context;
    if context.actor_id.is_empty()
        || context.actor_id.len() > 256
        || context.region.is_empty()
        || context.region.len() > 256
        || !decimal(&context.revision)
        || !decimal(&context.tick)
        || context.scene_id != scene.id
        || context.scene_generation != scene.generation
    {
        return Err(
            "native camera source snapshot has a stale/malformed actor, revision or scene identity"
                .into(),
        );
    }
    let focus = sample.focus.ok_or("native camera requires actual focus identity, logical/rendered coordinates, footprint and plane")?;
    let effects = sample.effects.ok_or(
        "native camera requires explicit source effect state; absence does not mean inactive",
    )?;
    let rendered = sample
        .rendered_actor
        .as_ref()
        .ok_or("native camera requires the actual rendered actor placement")?;
    if focus.identity < 0
        || focus.plane > 3
        || focus.world_base != scene.base
        || !focus.rendered.iter().all(|v| v.is_finite())
        || rendered.actor_id != context.actor_id
        || rendered.plane != i32::from(focus.plane)
        || rendered.size_tiles <= 0
        || focus.rendered.map(|v| v as i32) != rendered.local
    {
        return Err("native camera focus and actual rendered actor/scene disagree".into());
    }
    Ok((focus, effects))
}

fn delivery(
    camera: &NormalCamera,
    context: CameraContext,
    initialization: InitializationProvenance,
) -> Result<CameraDelivery> {
    Ok(CameraDelivery {
        context,
        initialization,
        output: camera.output().map_err(|e| e.to_string())?,
        cycle: camera.state().cycle,
        preferences: camera.state().preferences,
    })
}

fn full_viewport(camera: &NormalCamera) -> Result<()> {
    if camera.state().projection.viewport != camera.state().viewport {
        return Err("native camera projection requires renderer subviewport/letterbox support; no zoom substitute was applied".into());
    }
    Ok(())
}

fn not_older(current: &CameraContext, previous: &CameraContext) -> Result<()> {
    for (current, previous) in [
        (&current.revision, &previous.revision),
        (&current.tick, &previous.tick),
        (&current.scene_generation, &previous.scene_generation),
    ] {
        if current.parse::<u64>().map_err(|e| e.to_string())?
            < previous.parse::<u64>().map_err(|e| e.to_string())?
        {
            return Err("native camera source revision/tick/scene generation regressed".into());
        }
    }
    Ok(())
}

impl CameraConsumer {
    #[allow(clippy::too_many_arguments)]
    pub fn bind(
        &mut self,
        scene: CameraScene,
        sample: CameraSourceSample,
        definitions: Vec<CameraObjectDefinition>,
        reference: InitialReference,
        viewport: Viewport,
        now_ns: u64,
    ) -> Result<CameraDelivery> {
        let terrain = CameraTerrain::new(scene, definitions)?;
        let (focus, effects) = sample_inputs(&sample, terrain.scene())?;
        terrain.require_surface(focus.plane, focus.logical)?;
        terrain.require_surface(focus.plane, focus.rendered.map(|v| v as i32))?;
        let same_actor = self
            .context
            .as_ref()
            .is_some_and(|c| c.actor_id == sample.context.actor_id);
        let (mut camera, initialization) = if same_actor {
            not_older(
                &sample.context,
                self.context
                    .as_ref()
                    .ok_or("native camera context is unbound")?,
            )?;
            let mut camera = self
                .camera
                .as_ref()
                .ok_or("native camera state is unbound")?
                .clone();
            if camera.state().base != terrain.scene().base {
                camera
                    .rebase(terrain.scene().base)
                    .map_err(|e| e.to_string())?;
            }
            camera.resize(viewport).map_err(|e| e.to_string())?;
            (
                camera,
                self.initialization
                    .clone()
                    .ok_or("native camera initialization provenance is missing")?,
            )
        } else {
            NormalCamera::initialize(reference, viewport, &focus, &terrain)
                .map_err(|e| e.to_string())?
        };
        camera
            .render_frame_with_effects(0, &focus, &terrain, effects)
            .map_err(|e| e.to_string())?;
        full_viewport(&camera)?;
        let mouse = if same_actor {
            self.physical_mouse
        } else {
            [0, 0]
        };
        let clock = Clock {
            rendered_at: now_ns,
            next_tick: now_ns
                .checked_add(SOURCE_CYCLE_NS)
                .ok_or("native camera clock overflow")?,
            held: Input {
                mouse,
                ..Input::default()
            },
            pending: VecDeque::new(),
        };
        let result = delivery(&camera, sample.context.clone(), initialization.clone())?;
        self.camera = Some(camera);
        self.terrain = Some(terrain);
        self.context = Some(sample.context);
        self.initialization = Some(initialization);
        self.clock = Some(clock);
        self.physical_mouse = mouse;
        Ok(result)
    }

    pub fn input(&mut self, at_ns: u64, input: Input) -> Result<()> {
        let clock = self
            .clock
            .as_mut()
            .ok_or("native camera input is suspended/unbound")?;
        if at_ns < clock.rendered_at || clock.pending.back().is_some_and(|p| p.at > at_ns) {
            return Err("native camera input timestamp regressed".into());
        }
        if clock.pending.len() >= MAX_PENDING_INPUTS {
            return Err("native camera input queue exceeds its bounded capacity".into());
        }
        clock.pending.push_back(TimedInput { at: at_ns, input });
        self.physical_mouse = input.mouse;
        Ok(())
    }

    pub fn frame(&mut self, now_ns: u64, sample: CameraSourceSample) -> Result<CameraDelivery> {
        let terrain = self
            .terrain
            .as_ref()
            .ok_or("native camera terrain is unbound")?;
        let (focus, effects) = sample_inputs(&sample, terrain.scene())?;
        let previous = self
            .context
            .as_ref()
            .ok_or("native camera context is unbound")?;
        let current = &sample.context;
        not_older(current, previous)?;
        if current.actor_id != previous.actor_id
            || current.region != previous.region
            || current.instance != previous.instance
        {
            return Err(
                "native camera frame has a stale actor/world; bind the coherent source scene first"
                    .into(),
            );
        }
        let mut clock = self
            .clock
            .clone()
            .ok_or("native camera frame is suspended/unbound")?;
        let elapsed = now_ns
            .checked_sub(clock.rendered_at)
            .ok_or("native camera frame timestamp regressed")?;
        let steps = if now_ns < clock.next_tick {
            0
        } else {
            (now_ns - clock.next_tick) / SOURCE_CYCLE_NS + 1
        };
        if steps > MAX_STEPS_PER_FRAME {
            return Err("native camera logical backlog exceeds 500 ticks; explicitly suspend/rebind instead of dropping time".into());
        }
        let mut camera = self
            .camera
            .as_ref()
            .ok_or("native camera state is unbound")?
            .clone();
        for _ in 0..steps {
            let mut rotation = 0_i32;
            while clock
                .pending
                .front()
                .is_some_and(|p| p.at <= clock.next_tick)
            {
                let event = clock
                    .pending
                    .pop_front()
                    .ok_or("native camera input queue changed")?;
                if let Some(wheel) = event.input.wheel
                    && wheel.route == WheelRoute::Camera
                {
                    rotation = rotation
                        .checked_add(wheel.rotation)
                        .ok_or("native camera wheel accumulator overflow")?;
                }
                clock.held = Input {
                    wheel: None,
                    ..event.input
                };
            }
            let input = Input {
                wheel: (rotation != 0).then_some(WheelInput {
                    rotation,
                    route: WheelRoute::Camera,
                }),
                ..clock.held
            };
            camera
                .fixed_step(input, &focus, terrain)
                .map_err(|e| e.to_string())?;
            clock.next_tick = clock
                .next_tick
                .checked_add(SOURCE_CYCLE_NS)
                .ok_or("native camera clock overflow")?;
        }
        camera
            .render_frame_with_effects(elapsed, &focus, terrain, effects)
            .map_err(|e| e.to_string())?;
        full_viewport(&camera)?;
        clock.rendered_at = now_ns;
        let result = delivery(
            &camera,
            current.clone(),
            self.initialization
                .clone()
                .ok_or("native camera provenance is missing")?,
        )?;
        self.camera = Some(camera);
        self.context = Some(sample.context);
        self.clock = Some(clock);
        Ok(result)
    }

    pub fn resize(&mut self, viewport: Viewport) -> Result<()> {
        let mut camera = self
            .camera
            .as_ref()
            .ok_or("native camera is unbound")?
            .clone();
        camera.resize(viewport).map_err(|e| e.to_string())?;
        full_viewport(&camera)?;
        self.camera = Some(camera);
        Ok(())
    }

    pub fn face_yaw(&mut self, yaw: i32) -> Result<()> {
        if self.clock.is_none() {
            return Err("native camera is suspended/unbound".into());
        }
        let camera = self.camera.as_mut().ok_or("native camera is unbound")?;
        camera
            .set_target_angles(camera.state().target_pitch, yaw)
            .map_err(|e| e.to_string())
    }

    pub fn set_middle_mouse_enabled(&mut self, enabled: bool) -> Result<()> {
        self.camera
            .as_mut()
            .ok_or("native camera is unbound")?
            .set_middle_mouse_enabled(enabled);
        Ok(())
    }

    pub fn set_wheel_gates(&mut self, disabled: bool, override_512: bool) -> Result<()> {
        self.camera
            .as_mut()
            .ok_or("native camera is unbound")?
            .set_wheel_gates(disabled, override_512);
        Ok(())
    }

    /// Discard undelivered physical inputs and wall-clock backlog, not yaw, motors or preferences.
    pub fn suspend(&mut self) {
        self.clock = None;
    }

    pub fn state(&self) -> Result<&clubscape_camera::State> {
        self.camera
            .as_ref()
            .map(NormalCamera::state)
            .ok_or_else(|| "native camera is unbound".into())
    }
}

fn decode<T: DeserializeOwned>(json: &str) -> Result<T> {
    if json.len() > MAX_JSON_BYTES {
        return Err("native camera JSON exceeds its byte budget".into());
    }
    serde_json::from_str(json).map_err(|e| format!("native camera input JSON: {e}"))
}

fn decoded_scene(bytes: &[u8], generation: u64) -> Result<CameraScene> {
    if bytes.len() > MAX_JSON_BYTES {
        return Err("native camera scene exceeds its byte budget".into());
    }
    let chunks = clubscape_renderer::chunk::Chunks::parse(bytes).map_err(|e| e.to_string())?;
    let h = chunks.ints("SCHD").map_err(|e| e.to_string())?;
    if h.len() < 19
        || !(1..=256).contains(&h[2])
        || !(1..=256).contains(&h[3])
        || h[4] != 4
        || !(0..=256).contains(&h[6])
        || h[7] != 0
        || h[9] != 0
        || !(1..=104).contains(&h[8])
        || !(1..=104).contains(&h[10])
        || h[6] + h[8] > h[2]
        || h[6] + h[10] > h[3]
        || !(0..=8).contains(&h[13])
        || !(h[13]..=16).contains(&h[14])
        || h[15] != 1 << h[14]
        || h[16] != 1 << h[13]
        || h[2] > 1 << (h[14] - h[13])
        || h[3] > h[16]
        || h[17] != 4 * h[15]
    {
        return Err("native camera scene header is out of bounds".into());
    }
    let models = chunks.ints("TMOD").map_err(|e| e.to_string())?;
    let mut cursor = 0;
    while cursor < models.len() {
        let header = models
            .get(cursor..cursor + 9)
            .ok_or("native camera tile model header is truncated")?;
        if !(0..=32).contains(&header[6]) || !(0..=16).contains(&header[7]) {
            return Err("native camera tile model counts are out of bounds".into());
        }
        cursor +=
            9 + header[6] as usize * 3 + header[7] as usize * if header[8] != 0 { 7 } else { 6 };
        if cursor > models.len() {
            return Err("native camera tile model arrays are truncated".into());
        }
    }
    let scene =
        clubscape_renderer::scene::SceneData::from_chunks(bytes).map_err(|e| e.to_string())?;
    CameraScene::from_scene(&scene, &scene.name, generation).map_err(|e| e.to_string())
}

fn js_error(error: impl std::fmt::Display) -> JsValue {
    BridgeError::new("camera_unavailable", error.to_string()).js()
}

fn json(value: &impl serde::Serialize) -> std::result::Result<String, JsValue> {
    serde_json::to_string(value).map_err(js_error)
}

#[wasm_bindgen]
#[derive(Default)]
pub struct NativeCamera {
    consumer: CameraConsumer,
}

#[wasm_bindgen]
impl NativeCamera {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// CPU-only decoder of the same original scene data exposed by the live renderer getter.
    pub fn decode_scene(bytes: &[u8], generation: u64) -> std::result::Result<String, JsValue> {
        json(&decoded_scene(bytes, generation).map_err(js_error)?)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn bind(
        &mut self,
        scene: &str,
        sample: &str,
        definitions: &str,
        reference: &str,
        width: u32,
        height: u32,
        now_ns: u64,
    ) -> std::result::Result<String, JsValue> {
        let reference = match reference {
            "TutorialStartingHouse" => InitialReference::TutorialStartingHouse,
            "LumbridgeCastlePlaza" => InitialReference::LumbridgeCastlePlaza,
            _ => {
                return Err(js_error(
                    "unknown approved native camera initialization reference",
                ));
            }
        };
        let viewport = Viewport::new(0, 0, width, height).map_err(js_error)?;
        let delivery = self
            .consumer
            .bind(
                decode(scene).map_err(js_error)?,
                decode(sample).map_err(js_error)?,
                decode(definitions).map_err(js_error)?,
                reference,
                viewport,
                now_ns,
            )
            .map_err(js_error)?;
        json(&delivery)
    }

    pub fn input(&mut self, at_ns: u64, input: &str) -> std::result::Result<(), JsValue> {
        self.consumer
            .input(at_ns, decode(input).map_err(js_error)?)
            .map_err(js_error)
    }

    pub fn frame(&mut self, now_ns: u64, sample: &str) -> std::result::Result<String, JsValue> {
        let delivery = self
            .consumer
            .frame(now_ns, decode(sample).map_err(js_error)?)
            .map_err(js_error)?;
        json(&delivery)
    }

    pub fn resize(&mut self, width: u32, height: u32) -> std::result::Result<(), JsValue> {
        self.consumer
            .resize(Viewport::new(0, 0, width, height).map_err(js_error)?)
            .map_err(js_error)
    }

    pub fn face_yaw(&mut self, yaw: i32) -> std::result::Result<(), JsValue> {
        self.consumer.face_yaw(yaw).map_err(js_error)
    }

    pub fn set_middle_mouse_enabled(&mut self, enabled: bool) -> std::result::Result<(), JsValue> {
        self.consumer
            .set_middle_mouse_enabled(enabled)
            .map_err(js_error)
    }

    pub fn set_wheel_gates(
        &mut self,
        disabled: bool,
        override_512: bool,
    ) -> std::result::Result<(), JsValue> {
        self.consumer
            .set_wheel_gates(disabled, override_512)
            .map_err(js_error)
    }

    pub fn suspend(&mut self) {
        self.consumer.suspend();
    }

    pub fn state(&self) -> std::result::Result<String, JsValue> {
        json(self.consumer.state().map_err(js_error)?)
    }
}
