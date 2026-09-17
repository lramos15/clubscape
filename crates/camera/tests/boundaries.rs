use clubscape_camera::*;

#[derive(Clone)]
struct Grid {
    base: WorldBase,
    reported_dimensions: [u32; 2],
    heights: Vec<i32>,
    flags: Vec<u8>,
    missing_heights: bool,
    missing_flags: bool,
    missing_surface: bool,
}

impl Grid {
    fn flat() -> Self {
        Self {
            base: WorldBase { x: 3200, y: 3200 },
            reported_dimensions: [10, 10],
            heights: vec![-240; 4 * 11 * 11],
            flags: vec![0; 4 * 10 * 10],
            missing_heights: false,
            missing_flags: false,
            missing_surface: false,
        }
    }
    fn focus(&self) -> Focus {
        Focus {
            identity: 7,
            world_base: self.base,
            logical: [704, 704],
            rendered: [704.0, 704.0],
            plane: 0,
            footprint: 0,
            kind: FocusKind::Actor,
        }
    }
}

impl Terrain for Grid {
    fn world_base(&self) -> WorldBase {
        self.base
    }
    fn dimensions(&self) -> [u32; 2] {
        self.reported_dimensions
    }
    fn height_corner(&self, plane: u8, x: u32, y: u32) -> Result<i32> {
        if self.missing_heights || plane > 3 || x > 10 || y > 10 {
            return Err(CameraError::MissingHeight { plane, x, y });
        }
        Ok(self.heights[(usize::from(plane) * 11 + x as usize) * 11 + y as usize])
    }
    fn tile_settings(&self, plane: u8, x: u32, y: u32) -> Result<u8> {
        if self.missing_flags || plane > 3 || x >= 10 || y >= 10 {
            return Err(CameraError::MissingTileSettings { plane, x, y });
        }
        Ok(self.flags[(usize::from(plane) * 10 + x as usize) * 10 + y as usize])
    }
    fn surface_height(&self, plane: u8, x: i32, y: i32) -> Result<i32> {
        if self.missing_surface {
            return Err(CameraError::InvalidSurface);
        }
        integer_bilinear_height(self, x, y, plane)
    }
}

fn camera(grid: &Grid) -> NormalCamera {
    NormalCamera::initialize(
        InitialReference::TutorialStartingHouse,
        Viewport::new(0, 0, 1920, 1080).unwrap(),
        &grid.focus(),
        grid,
    )
    .unwrap()
    .0
}

#[test]
fn approved_initialization_uses_real_focus_not_reference_eye_or_zero() {
    let grid = Grid::flat();
    let (camera, provenance) = NormalCamera::initialize(
        InitialReference::TutorialStartingHouse,
        Viewport::new(0, 0, 1920, 1080).unwrap(),
        &grid.focus(),
        &grid,
    )
    .unwrap();
    let output = camera.output().unwrap();
    assert_eq!(output.focal_native, [704, -298, 704]);
    assert_eq!(output.pitch_native, 2048);
    assert_eq!(output.yaw_native, 0);
    assert_ne!(output.eye_native, [0; 3]);
    assert_ne!(output.eye_native, [5888, -2360, 4992]);
    assert_eq!(camera.state().preferences.follow_height, 50);
    assert!(!camera.state().preferences.middle_mouse_camera);
    assert_eq!(
        camera.state().preferences.zoom_bounds,
        ZoomBounds::PRESET_626
    );
    assert_eq!(camera.state().encoded_fov, [256, 205]);
    assert_eq!(output.projection.zoom, 662);
    assert_eq!(provenance.approval_sha256, APPROVAL_SHA256);
    assert_eq!(
        (provenance.pitch_native, provenance.pitch_legacy),
        (2048, 256)
    );
    assert!(!provenance.observed_osrs_account_defaults);
}

#[test]
fn unbound_constructor_state_cannot_render_or_start_from_zero_focus() {
    let grid = Grid::flat();
    let mut c = NormalCamera::restore(
        NormalCamera::source_state(grid.base, Viewport::new(0, 0, 1920, 1080).unwrap()).unwrap(),
    )
    .unwrap();
    let old = c.snapshot();
    assert_eq!(c.output(), Err(CameraError::MissingFocus));
    assert_eq!(
        c.render_frame(SOURCE_CYCLE_NS, &grid.focus(), &grid),
        Err(CameraError::MissingFocus)
    );
    assert_eq!(
        c.fixed_step(Input::default(), &grid.focus(), &grid),
        Err(CameraError::MissingFocus)
    );
    assert_eq!(c.snapshot(), old);
}

#[test]
fn missing_focus_and_source_data_fail_explicitly() {
    struct Missing;
    impl FocusProvider for Missing {
        fn camera_focus(&self) -> Result<Focus> {
            Err(CameraError::MissingFocus)
        }
    }
    let grid = Grid::flat();
    assert_eq!(
        NormalCamera::initialize(
            InitialReference::LumbridgeCastlePlaza,
            Viewport::new(0, 0, 1920, 1080).unwrap(),
            &Missing,
            &grid
        )
        .unwrap_err(),
        CameraError::MissingFocus
    );
    for field in 0..2 {
        let mut absent = grid.clone();
        absent.missing_heights = field == 0;
        absent.missing_flags = field == 1;
        assert!(
            NormalCamera::initialize(
                InitialReference::LumbridgeCastlePlaza,
                Viewport::new(0, 0, 1920, 1080).unwrap(),
                &absent.focus(),
                &absent
            )
            .is_err()
        );
    }
    let mut empty = grid.clone();
    empty.reported_dimensions = [0, 0];
    assert_eq!(
        NormalCamera::initialize(
            InitialReference::TutorialStartingHouse,
            Viewport::new(0, 0, 1920, 1080).unwrap(),
            &empty.focus(),
            &empty,
        )
        .unwrap_err(),
        CameraError::EmptyTerrain
    );
}

#[test]
fn invalid_focus_and_mismatched_world_are_not_recentered_or_defaulted() {
    let grid = Grid::flat();
    let mut c = camera(&grid);
    let initial = c.snapshot();
    let mut wrong = grid.focus();
    wrong.rendered[0] = f32::NAN;
    assert!(c.render_frame(SOURCE_CYCLE_NS, &wrong, &grid).is_err());
    assert_eq!(c.snapshot(), initial);
    wrong = grid.focus();
    wrong.world_base.x += 1;
    assert_eq!(
        c.fixed_step(Input::default(), &wrong, &grid),
        Err(CameraError::WorldBaseMismatch)
    );
    wrong = grid.focus();
    wrong.rendered[1] = 1280.0;
    assert_eq!(
        c.render_frame(0, &wrong, &grid),
        Err(CameraError::FocusOutsideTerrain)
    );
    wrong = grid.focus();
    wrong.identity = -1;
    assert_eq!(
        c.render_frame(0, &wrong, &grid),
        Err(CameraError::MissingFocus)
    );
    wrong = grid.focus();
    wrong.plane = 4;
    assert_eq!(
        c.render_frame(0, &wrong, &grid),
        Err(CameraError::InvalidFocus)
    );
}

#[test]
fn missing_surface_during_tick_is_transactional() {
    let mut grid = Grid::flat();
    let mut c = camera(&grid);
    let old = c.snapshot();
    grid.missing_surface = true;
    assert_eq!(
        c.fixed_step(Input::default(), &grid.focus(), &grid),
        Err(CameraError::InvalidSurface)
    );
    assert_eq!(c.snapshot(), old);
}

#[test]
fn source_bridge_flag_selects_next_height_plane() {
    let mut grid = Grid::flat();
    grid.flags[(10 + 5) * 10 + 5] = 2;
    for x in 0..11 {
        for y in 0..11 {
            grid.heights[(11 + x) * 11 + y] = -480;
        }
    }
    assert_eq!(bilinear_height(&grid, 704.0, 704.0, 0).unwrap(), -480.0);
    let c = camera(&grid);
    assert_eq!(c.output().unwrap().focal_native[1], -538);
}

#[test]
fn footprint_minimum_samples_corners_and_internal_vertices() {
    let mut grid = Grid::flat();
    grid.heights[5 * 11 + 5] = -800;
    let mut focus = grid.focus();
    focus.logical = [640, 640];
    focus.rendered = [640.0, 640.0];
    focus.footprint = 256;
    let sampled = footprint_height(&grid, focus).unwrap();
    assert_eq!(sampled, -800.0);
    let c = NormalCamera::initialize(
        InitialReference::LumbridgeCastlePlaza,
        Viewport::new(0, 0, 1920, 1080).unwrap(),
        &focus,
        &grid,
    )
    .unwrap()
    .0;
    assert_eq!(c.output().unwrap().focal_native[1], -858);
}

#[test]
fn footprint_grid_steps_preserve_fractional_source_offsets() {
    let mut grid = Grid::flat();
    grid.heights[5 * 11 + 5] = -800;
    let mut focus = grid.focus();
    focus.footprint = 256;
    assert_eq!(footprint_height(&grid, focus).unwrap(), -380.0);
}

#[test]
fn footprint_outside_available_source_fails_without_height_fallback() {
    let grid = Grid::flat();
    let mut focus = grid.focus();
    focus.rendered = [4.0, 4.0];
    focus.footprint = 100;
    assert_eq!(
        footprint_height(&grid, focus),
        Err(CameraError::FocusOutsideTerrain)
    );
}

#[test]
fn source_triangle_height_is_not_bilinear_and_invalid_faces_fail() {
    let triangle = SurfaceTriangle {
        horizontal: [[128, 128], [0, 128], [128, 0]],
        heights: [-232, -248, -232],
    };
    assert_eq!(triangle.height_at([64, 64]).unwrap(), Some(-240));
    assert_eq!(triangle.height_at([0, 0]).unwrap(), None);
    let invalid = SurfaceTriangle {
        horizontal: [[0, 0], [1, 1], [2, 2]],
        heights: [0; 3],
    };
    assert_eq!(invalid.height_at([1, 1]), Err(CameraError::InvalidSurface));
    let large = SurfaceTriangle {
        horizontal: [
            [i32::MIN, i32::MIN],
            [i32::MAX, i32::MIN],
            [i32::MIN, i32::MAX],
        ],
        heights: [-240; 3],
    };
    assert_eq!(large.height_at([i32::MIN, i32::MIN]).unwrap(), Some(-240));
}

#[test]
fn follow_equality_at_500_smooths_but_501_snaps() {
    let grid = Grid::flat();
    for (delta, expected) in [(500, 735.25), (501, 1205.0), (-500, 672.75), (-501, 203.0)] {
        let mut c = camera(&grid);
        let mut focus = grid.focus();
        focus.rendered[0] += delta as f32;
        focus.logical[0] += delta;
        c.render_frame(SOURCE_CYCLE_NS, &focus, &grid).unwrap();
        assert_eq!(c.state().focal_float[0], expected);
    }
}

#[test]
fn source_integer_motor_release_and_opposed_key_precedence() {
    let grid = Grid::flat();
    let mut c = camera(&grid);
    let left = Input {
        arrows: ArrowKeys {
            left: true,
            right: true,
            up: true,
            down: true,
        },
        ..Input::default()
    };
    c.fixed_step(left, &grid.focus(), &grid).unwrap();
    assert_eq!(c.state().native_velocity, [-96, 48]);
    c.fixed_step(Input::default(), &grid.focus(), &grid)
        .unwrap();
    assert_eq!(c.state().native_velocity, [-48, 24]);
}

#[test]
fn fixed_cycles_preserve_float_frame_state_and_signed_odd_release() {
    let grid = Grid::flat();
    let mut c = camera(&grid);
    let old = c.output().unwrap();
    let radians = c.state().target_radians;
    for _ in 0..12 {
        c.fixed_step(
            Input {
                arrows: ArrowKeys {
                    left: true,
                    ..ArrowKeys::default()
                },
                ..Input::default()
            },
            &grid.focus(),
            &grid,
        )
        .unwrap();
    }
    assert_eq!(c.state().native_velocity[0], -191);
    assert_eq!(c.state().legacy_velocity[0], -23);
    assert_eq!(c.state().target_radians, radians);
    assert_eq!(c.output().unwrap(), old);
    c.fixed_step(Input::default(), &grid.focus(), &grid)
        .unwrap();
    assert_eq!(c.state().native_velocity[0], -95);
    assert_eq!(c.state().legacy_velocity[0], -11);
    assert_eq!(c.state().cycle, 13);
    let frame = c
        .render_frame(SOURCE_CYCLE_NS, &grid.focus(), &grid)
        .unwrap();
    assert_eq!(c.state().cycle, 13);
    assert_ne!(frame.eye_native, old.eye_native);
}

#[test]
fn terrain_pitch_uses_nine_by_nine_scan_and_distinct_rise_fall_divisors() {
    let mut grid = Grid::flat();
    let mut c = camera(&grid);
    c.fixed_step(Input::default(), &grid.focus(), &grid)
        .unwrap();
    assert_eq!(c.state().terrain_pitch_floor, 10922);

    let mut state = c.snapshot();
    state.terrain_pitch_floor = MIN_PITCH * 256;
    c = NormalCamera::restore(state).unwrap();
    grid.heights[11 + 1] = -1240;
    c.fixed_step(Input::default(), &grid.focus(), &grid)
        .unwrap();
    assert_eq!(c.state().terrain_pitch_floor, 283904);

    grid.heights[11 + 1] = -240;
    let mut state = c.snapshot();
    state.terrain_pitch_floor = MAX_PITCH * 256;
    c = NormalCamera::restore(state).unwrap();
    c.set_target_angles(MIN_PITCH, 0).unwrap();
    c.fixed_step(Input::default(), &grid.focus(), &grid)
        .unwrap();
    assert_eq!(c.state().terrain_pitch_floor, 777856);
    let frame = c.render_frame(0, &grid.focus(), &grid).unwrap();
    assert_eq!(frame.pitch_native, 3038);
    assert_eq!(c.state().target_pitch, MIN_PITCH);
}

#[test]
fn invalid_mouse_arithmetic_fails_without_changing_state() {
    let grid = Grid::flat();
    let mut c = camera(&grid);
    c.set_middle_mouse_enabled(true);
    let old = c.snapshot();
    assert_eq!(
        c.fixed_step(
            Input {
                mouse: [i32::MIN, 0],
                button: MouseButton::Middle,
                ..Input::default()
            },
            &grid.focus(),
            &grid
        ),
        Err(CameraError::ArithmeticOverflow)
    );
    assert_eq!(c.snapshot(), old);
}

#[test]
fn restored_extreme_motor_arithmetic_returns_an_error_not_a_panic() {
    let grid = Grid::flat();
    let mut state = camera(&grid).snapshot();
    state.native_velocity[0] = i32::MIN;
    let mut c = NormalCamera::restore(state).unwrap();
    let old = c.snapshot();
    assert_eq!(
        c.fixed_step(
            Input {
                arrows: ArrowKeys {
                    right: true,
                    ..ArrowKeys::default()
                },
                ..Input::default()
            },
            &grid.focus(),
            &grid
        ),
        Err(CameraError::ArithmeticOverflow)
    );
    assert_eq!(c.snapshot(), old);
}

#[test]
fn wheel_gates_bounds_and_native_roundtrip_order_are_preserved() {
    let grid = Grid::flat();
    let mut c = camera(&grid);
    c.apply_fov_program([512; 2]).unwrap();
    c.wheel(WheelInput {
        rotation: 1,
        route: WheelRoute::Camera,
    })
    .unwrap();
    assert_eq!(c.state().last_logical_fov, Some([487; 2]));
    c.wheel(WheelInput {
        rotation: 1,
        route: WheelRoute::Camera,
    })
    .unwrap();
    assert_eq!(c.state().last_logical_fov, Some([461; 2]));
    let old = c.snapshot();
    c.wheel(WheelInput {
        rotation: 10,
        route: WheelRoute::OtherWidget,
    })
    .unwrap();
    assert_eq!(c.snapshot(), old);
    c.set_wheel_gates(true, false);
    let old = c.snapshot();
    c.wheel(WheelInput {
        rotation: 1,
        route: WheelRoute::Camera,
    })
    .unwrap();
    c.apply_fov_program([896; 2]).unwrap();
    assert_eq!(c.snapshot(), old);
    c.set_wheel_gates(false, true);
    c.wheel(WheelInput {
        rotation: 40,
        route: WheelRoute::Camera,
    })
    .unwrap();
    assert_eq!(c.state().last_logical_fov, Some([512; 2]));
    c.set_wheel_gates(false, false);
    c.wheel(WheelInput {
        rotation: i32::MAX,
        route: WheelRoute::Camera,
    })
    .unwrap_err();
}

#[test]
fn configurable_source_bounds_and_invalid_order() {
    let grid = Grid::flat();
    let mut c = camera(&grid);
    c.set_zoom_bounds(ZoomBounds {
        min_height: 200,
        max_height: 700,
        min_width: 250,
        max_width: 800,
    })
    .unwrap();
    c.apply_fov_program([-1, 9999]).unwrap();
    assert_eq!(c.state().last_logical_fov, Some([200, 800]));
    let old = c.snapshot();
    assert_eq!(
        c.set_zoom_bounds(ZoomBounds {
            min_height: 701,
            max_height: 700,
            min_width: 0,
            max_width: 1
        }),
        Err(CameraError::InvalidZoomBounds)
    );
    assert_eq!(c.snapshot(), old);
}

#[test]
fn native_distance_opcode_short_gates_and_integer_orbit_overflow_are_preserved() {
    let grid = Grid::flat();
    let mut c = camera(&grid);
    let fov = c.state().encoded_fov;
    c.apply_distance_scale_program([0, 32768]);
    assert_eq!(c.state().preferences.distance_scale, [256, 320]);
    c.apply_distance_scale_program([65537, 65538]);
    assert_eq!(c.state().preferences.distance_scale, [1, 2]);
    c.apply_distance_scale_program([32767; 2]);
    let output = c.render_frame(0, &grid.focus(), &grid).unwrap();
    assert_eq!(output.radial_distance_native, 175098);
    assert_eq!(output.eye_native, [704, 6964, 7966]);
    assert!(output.eye_float[1] < -100_000.0);
    assert_eq!(c.state().encoded_fov, fov);
}

#[test]
fn logical_bounds_preserve_native_short_conversion_and_follow_height_clamp() {
    let grid = Grid::flat();
    let mut c = camera(&grid);
    c.set_zoom_bounds(ZoomBounds {
        min_height: -512,
        max_height: 2048,
        min_width: -512,
        max_width: 2048,
    })
    .unwrap();
    c.apply_fov_program([-512; 2]).unwrap();
    assert_eq!(c.state().encoded_fov, [32; 2]);
    assert_eq!(c.state().last_logical_fov, Some([-512; 2]));
    assert_eq!(c.state().preferences.follow_height, 0);

    c.apply_fov_program([2048; 2]).unwrap();
    assert_eq!(c.state().encoded_fov, [256; 2]);
    assert_eq!(c.state().last_logical_fov, Some([2048; 2]));
    assert_eq!(c.state().preferences.follow_height, 225);
    assert_eq!(
        NormalCamera::restore(c.snapshot()).unwrap().snapshot(),
        c.snapshot()
    );
}

#[test]
fn mouse_option_gate_and_source_pitch_bounds_remain_explicit() {
    let grid = Grid::flat();
    let mut c = camera(&grid);
    c.fixed_step(
        Input {
            mouse: [110, 106],
            button: MouseButton::Middle,
            ..Input::default()
        },
        &grid.focus(),
        &grid,
    )
    .unwrap();
    assert_eq!(c.state().native_velocity, [0; 2]);
    c.set_middle_mouse_enabled(true);
    c.fixed_step(
        Input {
            mouse: [120, 112],
            button: MouseButton::Middle,
            ..Input::default()
        },
        &grid.focus(),
        &grid,
    )
    .unwrap();
    assert_eq!(c.state().native_velocity, [-160, 96]);
    assert_eq!(c.state().previous_mouse, [115, 109]);
    let old = c.snapshot();
    for angles in [(MIN_PITCH - 1, 0), (MAX_PITCH + 1, 0), (MIN_PITCH, -1)] {
        assert_eq!(
            c.set_target_angles(angles.0, angles.1),
            Err(CameraError::InvalidState)
        );
        assert_eq!(c.snapshot(), old);
    }
}

#[test]
fn viewport_validation_letterboxing_and_resizing() {
    assert_eq!(
        Viewport::new(0, 0, 0, 1080),
        Err(CameraError::InvalidViewport)
    );
    assert_eq!(
        Viewport::new(0, 0, 1920, 0),
        Err(CameraError::InvalidViewport)
    );
    assert_eq!(
        Viewport::new(-1, 0, 1920, 1080),
        Err(CameraError::InvalidViewport)
    );
    let boxed = projection(
        Viewport::new(0, 0, 2000, 500).unwrap(),
        [128; 2],
        AspectLimits {
            min_ratio: 200,
            max_ratio: 1000,
            min_scale: 128,
            max_scale: 128,
        },
    )
    .unwrap();
    assert!(boxed.viewport.width < 2000);
    assert!(boxed.viewport.x > 0);
    let grid = Grid::flat();
    let mut c = camera(&grid);
    let before = c.output().unwrap();
    c.resize(Viewport::new(0, 0, 1024, 768).unwrap()).unwrap();
    let after = c.render_frame(0, &grid.focus(), &grid).unwrap();
    assert_eq!(before.focal_float, after.focal_float);
    assert_ne!(before.projection.zoom, after.projection.zoom);
}

#[test]
fn region_rebase_retains_world_eye_and_does_not_reinitialize_to_fixture() {
    let grid = Grid::flat();
    let mut c = camera(&grid);
    c.set_target_angles(1536, 4096).unwrap();
    c.render_frame(0, &grid.focus(), &grid).unwrap();
    let before = c.output().unwrap();
    let source = c.snapshot();
    let mut locked = source.clone();
    locked.locked = true;
    c = NormalCamera::restore(locked).unwrap();
    c.rebase(WorldBase { x: 3208, y: 3208 }).unwrap();
    let after = c.output().unwrap();
    assert_eq!(
        before.eye_native[0] + 3200 * 128,
        after.eye_native[0] + 3208 * 128
    );
    assert_eq!(
        before.eye_native[2] + 3200 * 128,
        after.eye_native[2] + 3208 * 128
    );
    assert_eq!(c.state().target_radians, source.target_radians);
    assert_eq!(c.state().native_velocity, source.native_velocity);
    assert!(!c.state().locked);
    c.rebase(grid.base).unwrap();
    assert_eq!(c.output().unwrap(), before);
}

#[test]
fn serialized_invalid_state_does_not_receive_success_shaped_defaults() {
    let grid = Grid::flat();
    let mut state = camera(&grid).snapshot();
    state.focal_float[0] = f32::NAN;
    assert_eq!(
        NormalCamera::restore(state).unwrap_err(),
        CameraError::InvalidState
    );
    let mut state = camera(&grid).snapshot();
    state.projection.zoom = 0;
    assert_eq!(
        NormalCamera::restore(state).unwrap_err(),
        CameraError::InvalidState
    );
    let mut state = camera(&grid).snapshot();
    state.focus_identity = None;
    assert_eq!(
        NormalCamera::restore(state).unwrap_err(),
        CameraError::InvalidState
    );
}

#[test]
fn serializing_and_restoring_retains_the_next_native_camera_step() {
    let grid = Grid::flat();
    let mut c = camera(&grid);
    let input = Input {
        arrows: ArrowKeys {
            right: true,
            up: true,
            ..ArrowKeys::default()
        },
        ..Input::default()
    };
    c.fixed_step(input, &grid.focus(), &grid).unwrap();
    c.render_frame(10_000_000, &grid.focus(), &grid).unwrap();
    let bytes = serde_json::to_vec(&c.snapshot()).unwrap();
    let mut restored = NormalCamera::restore(serde_json::from_slice(&bytes).unwrap()).unwrap();
    c.fixed_step(input, &grid.focus(), &grid).unwrap();
    restored.fixed_step(input, &grid.focus(), &grid).unwrap();
    assert_eq!(
        c.render_frame(SOURCE_CYCLE_NS, &grid.focus(), &grid)
            .unwrap(),
        restored
            .render_frame(SOURCE_CYCLE_NS, &grid.focus(), &grid)
            .unwrap()
    );
    assert_eq!(c.snapshot(), restored.snapshot());
}

#[test]
fn invalid_frame_time_and_rebase_overflow_are_transactional() {
    let grid = Grid::flat();
    let mut c = camera(&grid);
    let old = c.snapshot();
    assert_eq!(
        c.render_frame(u64::MAX, &grid.focus(), &grid),
        Err(CameraError::InvalidFrameTime)
    );
    assert_eq!(c.snapshot(), old);
    assert_eq!(
        c.rebase(WorldBase {
            x: i32::MIN,
            y: i32::MAX
        }),
        Err(CameraError::ArithmeticOverflow)
    );
    assert_eq!(c.snapshot(), old);
}

#[test]
fn bytecode_derived_shake_channels_preserve_order_and_angle_conversion() {
    let grid = Grid::flat();
    let mut c = camera(&grid);
    c.set_target_angles(MIN_PITCH, 0).unwrap();
    let baseline = c.render_frame(0, &grid.focus(), &grid).unwrap();
    let channel = ShakeChannel {
        random_radius: 2,
        sine_amplitude: 0,
        sine_frequency: 0,
        phase: 0,
        random_sample: Some(0.0),
    };
    let output = c
        .render_frame_with_effects(
            0,
            &grid.focus(),
            &grid,
            FrameEffects {
                shake: [Some(channel); 5],
                suppress_jitter: false,
            },
        )
        .unwrap();
    assert_eq!(output.eye_native, baseline.eye_native.map(|v| v - 2));
    assert_eq!(output.eye_float, baseline.eye_float.map(|v| v - 2.0));
    assert_eq!(output.yaw_native, 16382);
    assert_eq!(output.pitch_native, MIN_PITCH);
    assert_eq!(c.state().target_yaw, 0);
    assert_eq!(c.state().target_pitch, MIN_PITCH);
    assert_eq!(output.focal_float, baseline.focal_float);
}

#[test]
fn source_suppression_keeps_shake_pitch_floor_without_consuming_a_random_draw() {
    let grid = Grid::flat();
    let mut c = camera(&grid);
    c.set_target_angles(MIN_PITCH, 0).unwrap();
    let mut effects = FrameEffects::NONE;
    effects.suppress_jitter = true;
    effects.shake[4] = Some(ShakeChannel {
        random_radius: 8,
        sine_amplitude: 512,
        sine_frequency: 50,
        phase: 123,
        random_sample: None,
    });
    let output = c
        .render_frame_with_effects(0, &grid.focus(), &grid, effects)
        .unwrap();
    assert_eq!(output.pitch_native, 1536);
    assert_eq!(output.radial_distance_native, 1470);
    assert_eq!(c.state().target_pitch, MIN_PITCH);
}

#[test]
fn active_effect_inputs_cannot_silently_use_a_zero_phase_or_random_fallback() {
    let grid = Grid::flat();
    let mut c = camera(&grid);
    let old = c.snapshot();
    let mut effects = FrameEffects::NONE;
    effects.shake[0] = Some(ShakeChannel {
        random_radius: 2,
        sine_amplitude: 20,
        sine_frequency: 100,
        phase: 1,
        random_sample: None,
    });
    assert_eq!(
        c.render_frame_with_effects(0, &grid.focus(), &grid, effects),
        Err(CameraError::MissingShakeSample { axis: 0 })
    );
    assert_eq!(c.snapshot(), old);
    effects.shake[0].as_mut().unwrap().random_sample = Some(1.0);
    assert_eq!(
        c.render_frame_with_effects(0, &grid.focus(), &grid, effects),
        Err(CameraError::InvalidFrameEffects)
    );
    assert_eq!(c.snapshot(), old);
    effects.shake[0].as_mut().unwrap().random_sample = Some(0.5);
    let output = c
        .render_frame_with_effects(0, &grid.focus(), &grid, effects)
        .unwrap();
    assert_eq!(output.eye_native[0], old.output.unwrap().eye_native[0] + 17);
}
