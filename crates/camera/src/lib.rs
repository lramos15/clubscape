//! Source-measured normal camera. Input ticks and render frames are separate operations.
mod effects;
mod math;
mod terrain;
mod viewport;

pub use terrain::{
    Focus, FocusKind, FocusProvider, SurfaceTriangle, Terrain, WorldBase, bilinear_height,
    footprint_height, integer_bilinear_height, integer_height,
};
pub use viewport::{
    AspectLimits, Projection, Viewport, ZoomBounds, decode_fov, encode_fov, projection,
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const SOURCE_CYCLE_NS: u64 = 20_000_000;
pub const ANGLE_UNITS_PER_TURN: i32 = 16384;
pub const MIN_PITCH: i32 = 1024;
pub const MAX_PITCH: i32 = 3064;
pub const APPROVAL_SHA256: &str =
    "f7f1ce92826fb1bafc1e17ecbbaace34e0851c4e5044cbcfafa86817ac3539e1";
pub const SOURCE_CONTRACT_SHA256: &str =
    "745dda42fcd53d3225b9f2714e2ebbad7dbf60a76c070da1438a7289e5e0a602";
pub type Result<T> = std::result::Result<T, CameraError>;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CameraError {
    #[error("actual source camera focus is unavailable")]
    MissingFocus,
    #[error("invalid/nonfinite source focus or plane")]
    InvalidFocus,
    #[error("camera focus or footprint lies outside supplied source terrain")]
    FocusOutsideTerrain,
    #[error("source terrain is empty")]
    EmptyTerrain,
    #[error("missing original height at plane {plane}, corner {x},{y}")]
    MissingHeight { plane: u8, x: u32, y: u32 },
    #[error("missing original tile settings at plane {plane}, tile {x},{y}")]
    MissingTileSettings { plane: u8, x: u32, y: u32 },
    #[error("focus, terrain and camera world bases disagree")]
    WorldBaseMismatch,
    #[error("invalid viewport geometry or source aspect limits")]
    InvalidViewport,
    #[error("invalid native FOV value")]
    InvalidFov,
    #[error("invalid source FOV bounds")]
    InvalidZoomBounds,
    #[error("invalid/degenerate original terrain surface geometry")]
    InvalidSurface,
    #[error("invalid normal-camera state or preference")]
    InvalidState,
    #[error("arithmetic overflow in camera input/state")]
    ArithmeticOverflow,
    #[error("invalid elapsed frame time")]
    InvalidFrameTime,
    #[error("invalid source camera render-effect input")]
    InvalidFrameEffects,
    #[error("missing original-compatible random draw for active shake channel {axis}")]
    MissingShakeSample { axis: u8 },
    #[error("normal camera cannot render an unimplemented server-locked camera")]
    LockedCamera,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrowKeys {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    #[default]
    Released,
    Primary,
    Secondary,
    Middle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WheelRoute {
    Camera,
    OtherWidget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WheelInput {
    pub rotation: i32,
    pub route: WheelRoute,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Input {
    pub arrows: ArrowKeys,
    pub mouse: [i32; 2],
    pub button: MouseButton,
    pub wheel: Option<WheelInput>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preferences {
    pub middle_mouse_camera: bool,
    pub wheel_disabled: bool,
    pub wheel_override_512: bool,
    pub follow_height: i32,
    pub distance_scale: [i32; 2],
    pub zoom_bounds: ZoomBounds,
}

impl Preferences {
    pub const SOURCE_CONSTRUCTOR_WITH_PRESET_626: Self = Self {
        middle_mouse_camera: false,
        wheel_disabled: false,
        wheel_override_512: false,
        follow_height: 50,
        distance_scale: [256, 320],
        zoom_bounds: ZoomBounds::PRESET_626,
    };

    fn validate(self) -> Result<()> {
        self.zoom_bounds.validate()?;
        if self.follow_height < 0
            || self
                .distance_scale
                .iter()
                .any(|value| !(1..=32767).contains(value))
        {
            return Err(CameraError::InvalidState);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InitialReference {
    TutorialStartingHouse,
    LumbridgeCastlePlaza,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitializationProvenance {
    pub approved_adaptation: String,
    pub approval_sha256: String,
    pub reference_path: String,
    pub reference_sha256: String,
    pub pitch_native: i32,
    pub yaw_native: i32,
    pub native_units_per_turn: i32,
    pub pitch_legacy: i32,
    pub yaw_legacy: i32,
    pub observed_osrs_account_defaults: bool,
}

impl InitialReference {
    pub fn provenance(self) -> InitializationProvenance {
        let (file, sha) = match self {
            Self::TutorialStartingHouse => (
                "tutorial-starting-house",
                "93f87ca82a91862db3d102f6910171d5c760810d53355eac6b6221a895bbe985",
            ),
            Self::LumbridgeCastlePlaza => (
                "lumbridge-castle-plaza",
                "edbfa14316ce35dc31b4c5ed8a5400ed951bc91eab9055284a506c71c8dfd9d9",
            ),
        };
        InitializationProvenance {
            approved_adaptation: "adaptation.m1.source_based_camera_initialization".into(),
            approval_sha256: APPROVAL_SHA256.into(),
            reference_path: format!("assets/reference/osrs240/scenes/{file}.png"),
            reference_sha256: sha.into(),
            pitch_native: 2048,
            yaw_native: 0,
            native_units_per_turn: ANGLE_UNITS_PER_TURN,
            pitch_legacy: 256,
            yaw_legacy: 0,
            observed_osrs_account_defaults: false,
        }
    }
}

/// Explicit serializable state. `restore` validates it but does not invent missing focus/terrain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub base: WorldBase,
    pub cycle: i32,
    pub focus_identity: Option<i32>,
    pub focus_plane: u8,
    pub logical_focus: [i32; 2],
    pub focal_native: [i32; 3],
    pub focal_float: [f32; 3],
    pub target_pitch: i32,
    pub target_yaw: i32,
    pub target_radians: [f32; 2],
    pub native_velocity: [i32; 2],
    pub legacy_velocity: [i32; 2],
    pub previous_mouse: [i32; 2],
    pub previous_legacy_mouse: [i32; 2],
    pub terrain_pitch_floor: i32,
    pub logical_focus_ground: i32,
    pub encoded_fov: [i32; 2],
    pub last_logical_fov: Option<[i32; 2]>,
    pub viewport: Viewport,
    pub projection: Projection,
    pub aspect_limits: AspectLimits,
    pub preferences: Preferences,
    pub locked: bool,
    pub output: Option<CameraOutput>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CameraOutput {
    pub world_base: WorldBase,
    pub eye_native: [i32; 3],
    pub eye_float: [f32; 3],
    pub pitch_native: i32,
    pub yaw_native: i32,
    pub pitch_radians: f32,
    pub yaw_radians: f32,
    pub focal_native: [i32; 3],
    pub focal_float: [f32; 3],
    pub radial_distance_native: i32,
    pub radial_distance_float_path: i32,
    pub projection: Projection,
}

#[derive(Clone, Debug)]
pub struct NormalCamera {
    state: State,
}

impl NormalCamera {
    /// Constructor mechanics with allocated viewport and approved preset626 bounds.
    /// Unbound state is not a world initialization; use `initialize` for normal entry.
    pub fn source_state(base: WorldBase, viewport: Viewport) -> Result<State> {
        let encoded_fov = [256, 205];
        let aspect_limits = AspectLimits::default();
        Ok(State {
            base,
            cycle: 0,
            focus_identity: None,
            focus_plane: 0,
            logical_focus: [0, 0],
            focal_native: [0; 3],
            focal_float: [0.0; 3],
            target_pitch: MIN_PITCH,
            target_yaw: 0,
            target_radians: [math::MIN_PITCH_RAD, 0.0],
            native_velocity: [0; 2],
            legacy_velocity: [0; 2],
            previous_mouse: [0; 2],
            previous_legacy_mouse: [0; 2],
            terrain_pitch_floor: 0,
            logical_focus_ground: 0,
            encoded_fov,
            last_logical_fov: None,
            viewport,
            projection: projection(viewport, encoded_fov, aspect_limits)?,
            aspect_limits,
            preferences: Preferences::SOURCE_CONSTRUCTOR_WITH_PRESET_626,
            locked: false,
            output: None,
        })
    }

    pub fn restore(state: State) -> Result<Self> {
        state.preferences.validate()?;
        if !(MIN_PITCH..=MAX_PITCH).contains(&state.target_pitch)
            || !(0..ANGLE_UNITS_PER_TURN).contains(&state.target_yaw)
            || state.focus_plane > 3
            || state.terrain_pitch_floor < 0
            || state.terrain_pitch_floor > MAX_PITCH * 256
            || !state
                .focal_float
                .iter()
                .chain(state.target_radians.iter())
                .all(|n| n.is_finite())
            || !(math::MIN_PITCH_RAD..=math::MAX_PITCH_RAD).contains(&state.target_radians[0])
        {
            return Err(CameraError::InvalidState);
        }
        let checked_projection =
            projection(state.viewport, state.encoded_fov, state.aspect_limits)?;
        if checked_projection != state.projection
            || state.focus_identity.is_some_and(|id| id < 0)
            || state.output.is_some_and(|output| {
                !output
                    .eye_float
                    .iter()
                    .chain(output.focal_float.iter())
                    .chain([output.pitch_radians, output.yaw_radians].iter())
                    .all(|v| v.is_finite())
                    || output.world_base != state.base
                    || state.focus_identity.is_none()
                    || !(MIN_PITCH..=MAX_PITCH).contains(&output.pitch_native)
                    || !(0..ANGLE_UNITS_PER_TURN).contains(&output.yaw_native)
                    || output.projection.zoom <= 0
                    || Viewport::new(
                        output.projection.viewport.x,
                        output.projection.viewport.y,
                        output.projection.viewport.width,
                        output.projection.viewport.height,
                    )
                    .is_err()
            })
        {
            return Err(CameraError::InvalidState);
        }
        Ok(Self { state })
    }

    /// Owner-approved ClubScape initialization, never an observed source account default.
    pub fn initialize(
        reference: InitialReference,
        viewport: Viewport,
        focus: &impl FocusProvider,
        terrain: &impl Terrain,
    ) -> Result<(Self, InitializationProvenance)> {
        let focus = focus.camera_focus()?;
        let mut camera = Self::restore(Self::source_state(terrain.world_base(), viewport)?)?;
        terrain::validate_focus(focus, terrain, camera.state.base)?;
        let provenance = reference.provenance();
        camera.set_target_angles(provenance.pitch_native, provenance.yaw_native)?;
        camera.state.focus_identity = Some(focus.identity);
        camera.state.focus_plane = focus.plane;
        camera.state.logical_focus = focus.logical;
        camera.state.focal_float[0] = focus.rendered[0];
        camera.state.focal_float[2] = focus.rendered[1];
        camera.render_frame(0, &focus, terrain)?;
        Ok((camera, provenance))
    }

    pub fn state(&self) -> &State {
        &self.state
    }
    pub fn snapshot(&self) -> State {
        self.state.clone()
    }
    pub fn output(&self) -> Result<CameraOutput> {
        self.state.output.ok_or(CameraError::MissingFocus)
    }

    pub fn set_middle_mouse_enabled(&mut self, enabled: bool) {
        self.state.preferences.middle_mouse_camera = enabled;
    }
    pub fn set_wheel_gates(&mut self, disabled: bool, override_512: bool) {
        self.state.preferences.wheel_disabled = disabled;
        self.state.preferences.wheel_override_512 = override_512;
    }
    pub fn set_zoom_bounds(&mut self, bounds: ZoomBounds) -> Result<()> {
        bounds.validate()?;
        self.state.preferences.zoom_bounds = bounds;
        Ok(())
    }
    pub fn set_distance_scale(&mut self, values: [i32; 2]) -> Result<()> {
        if values.iter().any(|v| !(1..=32767).contains(v)) {
            return Err(CameraError::InvalidState);
        }
        self.state.preferences.distance_scale = values;
        Ok(())
    }
    /// Opcode6201 casts to native shorts, then applies its source-defined 256/320 gates.
    pub fn apply_distance_scale_program(&mut self, values: [i32; 2]) {
        let encoded = values.map(|value| i32::from(value as i16));
        self.state.preferences.distance_scale = [
            if encoded[0] <= 0 { 256 } else { encoded[0] },
            if encoded[1] <= 0 { 320 } else { encoded[1] },
        ];
    }
    pub fn set_target_angles(&mut self, pitch: i32, yaw: i32) -> Result<()> {
        if !(MIN_PITCH..=MAX_PITCH).contains(&pitch) || !(0..ANGLE_UNITS_PER_TURN).contains(&yaw) {
            return Err(CameraError::InvalidState);
        }
        self.state.target_pitch = pitch;
        self.state.target_yaw = yaw;
        self.state.target_radians = [math::radians(pitch), math::radians(yaw)];
        Ok(())
    }
    pub fn resize(&mut self, viewport: Viewport) -> Result<Projection> {
        let projection = projection(viewport, self.state.encoded_fov, self.state.aspect_limits)?;
        self.state.viewport = viewport;
        self.state.projection = projection;
        Ok(projection)
    }

    /// Original script42 ordering: clamp logical values, encode, viewport, follow-height, persistence.
    pub fn apply_fov_program(&mut self, values: [i32; 2]) -> Result<()> {
        if self.state.preferences.wheel_disabled {
            return Ok(());
        }
        let bounds = self.state.preferences.zoom_bounds;
        bounds.validate()?;
        let logical = [
            values[0].clamp(bounds.min_height, bounds.max_height),
            values[1].clamp(bounds.min_width, bounds.max_width),
        ];
        let encoded = [encode_fov(logical[0]), encode_fov(logical[1])];
        let viewport = projection(self.state.viewport, encoded, self.state.aspect_limits)?;
        let mixed = viewport::factor(viewport.viewport.height, logical);
        let follow_height = (25 + mixed.wrapping_mul(25) / 256).max(0);
        self.state.encoded_fov = encoded;
        self.state.projection = viewport;
        self.state.preferences.follow_height = follow_height;
        self.state.last_logical_fov = Some(logical);
        Ok(())
    }

    pub fn wheel(&mut self, wheel: WheelInput) -> Result<()> {
        if wheel.route == WheelRoute::OtherWidget
            || wheel.rotation == 0
            || self.state.preferences.wheel_disabled
        {
            return Ok(());
        }
        let values = if self.state.preferences.wheel_override_512 {
            [512; 2]
        } else {
            let delta = wheel
                .rotation
                .checked_mul(25)
                .ok_or(CameraError::ArithmeticOverflow)?;
            [
                decode_fov(self.state.encoded_fov[0])?
                    .checked_sub(delta)
                    .ok_or(CameraError::ArithmeticOverflow)?,
                decode_fov(self.state.encoded_fov[1])?
                    .checked_sub(delta)
                    .ok_or(CameraError::ArithmeticOverflow)?,
            ]
        };
        self.apply_fov_program(values)
    }

    pub fn fixed_step(
        &mut self,
        input: Input,
        focus: &impl FocusProvider,
        terrain: &impl Terrain,
    ) -> Result<()> {
        let mut next = self.clone();
        next.fixed_step_inner(input, focus.camera_focus()?, terrain)?;
        *self = next;
        Ok(())
    }

    fn fixed_step_inner(
        &mut self,
        input: Input,
        focus: Focus,
        terrain: &impl Terrain,
    ) -> Result<()> {
        terrain::validate_focus(focus, terrain, self.state.base)?;
        if self.state.focus_identity.is_none() {
            return Err(CameraError::MissingFocus);
        }
        if self.state.locked {
            return Err(CameraError::LockedCamera);
        }
        if let Some(wheel) = input.wheel {
            self.wheel(wheel)?;
        }
        let s = &mut self.state;
        let target = terrain::terrain_pitch_target(
            terrain,
            [s.focal_native[0], s.focal_native[2]],
            focus.plane,
        )?;
        let divisor = if target > s.terrain_pitch_floor {
            24
        } else {
            80
        };
        s.terrain_pitch_floor += (target - s.terrain_pitch_floor) / divisor;
        s.logical_focus_ground =
            integer_height(terrain, focus.logical[0], focus.logical[1], focus.plane)?;
        s.logical_focus = focus.logical;
        s.focus_plane = focus.plane;
        s.focus_identity = Some(focus.identity);
        let middle = input.button == MouseButton::Middle && s.preferences.middle_mouse_camera;
        motor(
            &mut s.native_velocity,
            &mut s.previous_mouse,
            input,
            middle,
            8,
        )?;
        motor(
            &mut s.legacy_velocity,
            &mut s.previous_legacy_mouse,
            input,
            middle,
            1,
        )?;
        s.target_yaw = (s.target_yaw + s.native_velocity[0] / 2) & 16383;
        s.target_pitch = (s.target_pitch + s.native_velocity[1] / 2).clamp(MIN_PITCH, MAX_PITCH);
        s.cycle = s.cycle.wrapping_add(1);
        Ok(())
    }

    /// Normal frame with explicitly inactive source shake; otherwise use the effects variant.
    pub fn render_frame(
        &mut self,
        elapsed_ns: u64,
        focus: &impl FocusProvider,
        terrain: &impl Terrain,
    ) -> Result<CameraOutput> {
        self.render_frame_with_effects(elapsed_ns, focus, terrain, FrameEffects::NONE)
    }

    /// Active source effects require explicit phase and per-channel random inputs.
    pub fn render_frame_with_effects(
        &mut self,
        elapsed_ns: u64,
        focus: &impl FocusProvider,
        terrain: &impl Terrain,
        effects: FrameEffects,
    ) -> Result<CameraOutput> {
        let mut next = self.clone();
        let output =
            next.render_frame_inner(elapsed_ns, focus.camera_focus()?, terrain, effects)?;
        *self = next;
        Ok(output)
    }

    fn render_frame_inner(
        &mut self,
        elapsed_ns: u64,
        focus: Focus,
        terrain: &impl Terrain,
        effects: FrameEffects,
    ) -> Result<CameraOutput> {
        terrain::validate_focus(focus, terrain, self.state.base)?;
        if self.state.focus_identity.is_none() {
            return Err(CameraError::MissingFocus);
        }
        if self.state.locked {
            return Err(CameraError::LockedCamera);
        }
        if elapsed_ns > i64::MAX as u64 {
            return Err(CameraError::InvalidFrameTime);
        }
        let effect_pitch_floor = effects.pitch_floor()?;
        let s = &mut self.state;
        let time = elapsed_ns as f32 / SOURCE_CYCLE_NS as f32;
        s.target_radians[1] += ((s.legacy_velocity[0] as f32 * math::LEGACY_UNIT) * time) / 2.0;
        s.target_radians[0] = (s.target_radians[0]
            + ((s.legacy_velocity[1] as f32 * math::LEGACY_UNIT) * time) / 2.0)
            .clamp(math::MIN_PITCH_RAD, math::MAX_PITCH_RAD);
        s.target_yaw = math::angle(s.target_radians[1]);
        s.target_pitch = math::angle(s.target_radians[0]);
        let [x, y] = focus.rendered;
        if (s.focal_float[0] - x).abs() > 500.0 || (s.focal_float[2] - y).abs() > 500.0 {
            s.focal_float[0] = x;
            s.focal_float[2] = y;
        } else {
            let factor = elapsed_ns as f64 / 320_000_000.0;
            s.focal_float[0] =
                (f64::from(s.focal_float[0]) + f64::from(x - s.focal_float[0]) * factor) as f32;
            s.focal_float[2] =
                (f64::from(s.focal_float[2]) + f64::from(y - s.focal_float[2]) * factor) as f32;
        }
        let ground = if focus.kind == FocusKind::Actor {
            footprint_height(terrain, focus)? - 8.0
        } else {
            bilinear_height(terrain, x, y, focus.plane)?
        };
        s.focal_float[1] = ground - s.preferences.follow_height as f32;
        if !s
            .focal_float
            .iter()
            .chain(s.target_radians.iter())
            .all(|v| v.is_finite())
        {
            return Err(CameraError::InvalidFrameTime);
        }
        if s.focal_float
            .iter()
            .any(|v| f64::from(*v).abs() > f64::from(i32::MAX) - 1_000_000.0)
        {
            return Err(CameraError::ArithmeticOverflow);
        }
        s.focal_native = s.focal_float.map(|v| v as i32);
        s.focus_identity = Some(focus.identity);
        s.focus_plane = focus.plane;
        s.projection = projection(s.viewport, s.encoded_fov, s.aspect_limits)?;
        let pitch_floor = (s.terrain_pitch_floor / 256).max(effect_pitch_floor);
        let pitch = s.target_pitch.max(pitch_floor);
        let float_pitch = s.target_radians[0].max(math::radians(pitch_floor));
        let distance_factor =
            viewport::factor(s.projection.viewport.height, s.preferences.distance_scale);
        let distance = ((pitch >> 3) * 3 + 600) * distance_factor / 256;
        let float_distance = (math::legacy_angle(float_pitch) * 3 + 600) * distance_factor / 256;
        let mut output = CameraOutput {
            world_base: s.base,
            eye_native: math::native_orbit(s.focal_native, pitch, s.target_yaw, distance),
            eye_float: math::float_orbit(
                s.focal_float,
                float_pitch,
                s.target_radians[1],
                float_distance,
            ),
            pitch_native: pitch,
            yaw_native: s.target_yaw,
            pitch_radians: float_pitch,
            yaw_radians: s.target_radians[1],
            focal_native: s.focal_native,
            focal_float: s.focal_float,
            radial_distance_native: distance,
            radial_distance_float_path: float_distance,
            projection: s.projection,
        };
        effects.apply(&mut output)?;
        s.output = Some(output);
        Ok(output)
    }

    pub fn rebase(&mut self, new_base: WorldBase) -> Result<()> {
        let dx = new_base
            .x
            .checked_sub(self.state.base.x)
            .and_then(|v| v.checked_mul(128))
            .ok_or(CameraError::ArithmeticOverflow)?;
        let dy = new_base
            .y
            .checked_sub(self.state.base.y)
            .and_then(|v| v.checked_mul(128))
            .ok_or(CameraError::ArithmeticOverflow)?;
        let mut next = self.state.clone();
        for (axis, delta) in [(0, dx), (2, dy)] {
            next.focal_native[axis] = next.focal_native[axis]
                .checked_sub(delta)
                .ok_or(CameraError::ArithmeticOverflow)?;
            next.focal_float[axis] -= delta as f32;
            if let Some(output) = &mut next.output {
                output.eye_native[axis] = output.eye_native[axis]
                    .checked_sub(delta)
                    .ok_or(CameraError::ArithmeticOverflow)?;
                output.eye_float[axis] -= delta as f32;
                output.focal_native[axis] = next.focal_native[axis];
                output.focal_float[axis] = next.focal_float[axis];
                output.world_base = new_base;
            }
        }
        next.logical_focus[0] = next.logical_focus[0]
            .checked_sub(dx)
            .ok_or(CameraError::ArithmeticOverflow)?;
        next.logical_focus[1] = next.logical_focus[1]
            .checked_sub(dy)
            .ok_or(CameraError::ArithmeticOverflow)?;
        next.base = new_base;
        next.locked = false;
        self.state = next;
        Ok(())
    }
}

fn motor(
    velocity: &mut [i32; 2],
    previous: &mut [i32; 2],
    input: Input,
    middle: bool,
    scale: i32,
) -> Result<()> {
    if middle {
        let dx = previous[0]
            .checked_sub(input.mouse[0])
            .ok_or(CameraError::ArithmeticOverflow)?;
        let dy = input.mouse[1]
            .checked_sub(previous[1])
            .ok_or(CameraError::ArithmeticOverflow)?;
        velocity[0] = dx
            .checked_mul(2 * scale)
            .ok_or(CameraError::ArithmeticOverflow)?;
        velocity[1] = dy
            .checked_mul(2 * scale)
            .ok_or(CameraError::ArithmeticOverflow)?;
        for (axis, delta) in [(0, dx), (1, dy)] {
            previous[axis] = if delta == -1 || delta == 1 {
                input.mouse[axis]
            } else {
                ((i64::from(previous[axis]) + i64::from(input.mouse[axis])) / 2) as i32
            };
        }
    } else {
        let horizontal = if input.arrows.left {
            Some(-24 * scale)
        } else if input.arrows.right {
            Some(24 * scale)
        } else {
            None
        };
        let vertical = if input.arrows.up {
            Some(12 * scale)
        } else if input.arrows.down {
            Some(-12 * scale)
        } else {
            None
        };
        for (axis, target) in [horizontal, vertical].into_iter().enumerate() {
            velocity[axis] = if let Some(target) = target {
                let acceleration = target
                    .checked_sub(velocity[axis])
                    .ok_or(CameraError::ArithmeticOverflow)?
                    / 2;
                velocity[axis]
                    .checked_add(acceleration)
                    .ok_or(CameraError::ArithmeticOverflow)?
            } else {
                velocity[axis] / 2
            };
        }
        *previous = input.mouse;
    }
    Ok(())
}
pub use effects::{FrameEffects, ShakeChannel};
