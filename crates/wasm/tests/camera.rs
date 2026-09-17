use clubscape_camera::{
    ArrowKeys, Focus, FocusKind, FrameEffects, InitialReference, Input, MouseButton,
    SOURCE_CYCLE_NS, SurfaceTriangle, Viewport, WheelInput, WheelRoute, WorldBase,
};
use clubscape_renderer::camera::{
    CameraContext, CameraScene, CameraSourceSample, CameraSurface, RenderedActorPlacement,
    renderer_camera,
};
use clubscape_wasm::camera::{CameraConsumer, MAX_PENDING_INPUTS, MAX_STEPS_PER_FRAME};

// Controlled consumer inputs, not a production actor, original readback or visual acceptance.
fn fixture(base: WorldBase, generation: u64, actor: &str) -> (CameraScene, CameraSourceSample) {
    let mut surfaces = Vec::new();
    for _ in 0..4 {
        for x in 0..104 {
            for y in 0..104 {
                let x = x * 128;
                let y = y * 128;
                surfaces.push(Some(CameraSurface {
                    decoration: None,
                    triangles: vec![
                        SurfaceTriangle {
                            horizontal: [[x + 128, y + 128], [x, y + 128], [x + 128, y]],
                            heights: [-64; 3],
                        },
                        SurfaceTriangle {
                            horizontal: [[x, y], [x + 128, y], [x, y + 128]],
                            heights: [-64; 3],
                        },
                    ],
                }));
            }
        }
    }
    let scene = CameraScene {
        version: 1,
        id: format!("controlled@{},{}", base.x, base.y),
        generation: generation.to_string(),
        base,
        dimensions: [104, 104],
        heights: vec![Some(-64); 4 * 105 * 105],
        settings: vec![Some(0); 4 * 104 * 104],
        surfaces,
    };
    let position = [(3215 - base.x) * 128 + 64, (3218 - base.y) * 128 + 64];
    let sample = CameraSourceSample {
        context: CameraContext {
            actor_id: actor.into(),
            region: "controlled-region".into(),
            instance: None,
            revision: "7".into(),
            tick: "9".into(),
            scene_id: scene.id.clone(),
            scene_generation: scene.generation.clone(),
        },
        rendered_actor: Some(RenderedActorPlacement {
            actor_id: actor.into(),
            local: position,
            plane: 0,
            size_tiles: 1,
        }),
        focus: Some(Focus {
            identity: 42,
            world_base: base,
            logical: position,
            rendered: position.map(|v| v as f32),
            plane: 0,
            footprint: 0,
            kind: FocusKind::Actor,
        }),
        effects: Some(FrameEffects::NONE),
        missing: vec![],
    };
    (scene, sample)
}

fn bound(now: u64) -> (CameraConsumer, CameraSourceSample) {
    let (scene, sample) = fixture(WorldBase { x: 3168, y: 3168 }, 1, "actor.controlled");
    let mut consumer = CameraConsumer::default();
    consumer
        .bind(
            scene,
            sample.clone(),
            vec![],
            InitialReference::TutorialStartingHouse,
            Viewport::new(0, 0, 1920, 1080).unwrap(),
            now,
        )
        .unwrap();
    (consumer, sample)
}

#[test]
fn approved_constructor_and_integer_renderer_abi_are_not_full_hud_fixture_projection() {
    let (mut consumer, sample) = bound(0);
    let output = consumer.frame(0, sample).unwrap();
    assert_eq!(output.output.projection.zoom, 662);
    assert_eq!(output.output.pitch_native, 2048);
    assert_eq!(output.output.yaw_native, 0);
    assert_eq!(output.cycle, 0);
    assert!(!output.initialization.observed_osrs_account_defaults);
    let camera = renderer_camera(
        output.output,
        Viewport::new(0, 0, 1920, 1080).unwrap(),
        3500,
    )
    .unwrap();
    assert_eq!(camera.x, 3168 * 128 + output.output.eye_native[0]);
    assert_eq!(camera.height, output.output.eye_native[1]);
    assert_eq!(camera.y, 3168 * 128 + output.output.eye_native[2]);
    assert_eq!(camera.far, 3500);
    let prior = consumer.state().unwrap().clone();
    assert!(
        consumer
            .resize(Viewport::new(0, 0, 1, 32767).unwrap())
            .is_err()
    );
    assert_eq!(consumer.state().unwrap(), &prior);
    consumer
        .resize(Viewport::new(0, 0, 1024, 768).unwrap())
        .unwrap();
    assert_eq!(consumer.state().unwrap().projection.zoom, 471);
    assert_eq!(consumer.state().unwrap().focal_native, prior.focal_native);
}

#[test]
fn timestamped_input_obeys_separate_logical_and_frame_clocks_without_wheel_replay() {
    let origin = (1_u64 << 60) + 123;
    let (mut consumer, sample) = bound(origin);
    let right = Input {
        arrows: ArrowKeys {
            right: true,
            ..ArrowKeys::default()
        },
        ..Input::default()
    };
    consumer.input(origin + 11_000_000, right).unwrap();
    assert_eq!(
        consumer
            .frame(origin + 10_000_000, sample.clone())
            .unwrap()
            .cycle,
        0
    );
    assert_eq!(
        consumer
            .frame(origin + SOURCE_CYCLE_NS, sample.clone())
            .unwrap()
            .cycle,
        1
    );
    assert_eq!(consumer.state().unwrap().native_velocity, [96, 0]);
    assert_eq!(consumer.state().unwrap().legacy_velocity, [12, 0]);
    let yaw = consumer.state().unwrap().target_yaw;
    assert_eq!(
        consumer
            .frame(origin + 30_000_000, sample.clone())
            .unwrap()
            .cycle,
        1
    );
    assert_ne!(
        consumer.state().unwrap().target_yaw,
        yaw,
        "frame integration is not a fixed input step"
    );
    consumer
        .input(
            origin + 31_000_000,
            Input {
                wheel: Some(WheelInput {
                    rotation: 1,
                    route: WheelRoute::Camera,
                }),
                ..right
            },
        )
        .unwrap();
    consumer.frame(origin + 80_000_000, sample.clone()).unwrap();
    let fov = consumer.state().unwrap().encoded_fov;
    assert_eq!(consumer.state().unwrap().cycle, 4);
    assert!(consumer.state().unwrap().last_logical_fov.is_some());
    consumer.frame(origin + 100_000_000, sample).unwrap();
    assert_eq!(consumer.state().unwrap().encoded_fov, fov);
}

#[test]
fn release_and_middle_mouse_gate_are_owned_by_native_state() {
    let (mut consumer, sample) = bound(0);
    let middle = Input {
        mouse: [20, 30],
        button: MouseButton::Middle,
        ..Input::default()
    };
    consumer.input(1, middle).unwrap();
    consumer.frame(20_000_000, sample.clone()).unwrap();
    assert_eq!(consumer.state().unwrap().native_velocity, [0, 0]);
    consumer.set_middle_mouse_enabled(true).unwrap();
    consumer
        .input(
            21_000_000,
            Input {
                mouse: [24, 35],
                ..middle
            },
        )
        .unwrap();
    consumer.frame(40_000_000, sample.clone()).unwrap();
    let velocity = consumer.state().unwrap().native_velocity;
    assert_eq!(velocity, [-64, 80]);
    consumer.input(41_000_000, Input::default()).unwrap();
    consumer.frame(60_000_000, sample).unwrap();
    assert_eq!(
        consumer.state().unwrap().native_velocity,
        velocity.map(|v| v / 2)
    );
}

#[test]
fn rebase_and_reconnect_preserve_motors_but_not_physical_input_backlog() {
    let (mut consumer, sample) = bound(0);
    consumer
        .input(
            1,
            Input {
                arrows: ArrowKeys {
                    left: true,
                    ..ArrowKeys::default()
                },
                ..Input::default()
            },
        )
        .unwrap();
    consumer.frame(20_000_000, sample).unwrap();
    let prior = consumer.state().unwrap().clone();
    consumer
        .input(
            21_000_000,
            Input {
                wheel: Some(WheelInput {
                    rotation: 3,
                    route: WheelRoute::Camera,
                }),
                ..Input::default()
            },
        )
        .unwrap();
    consumer.suspend();
    assert!(consumer.input(22_000_000, Input::default()).is_err());
    let (scene, sample) = fixture(WorldBase { x: 3176, y: 3176 }, 2, "actor.controlled");
    let result = consumer
        .bind(
            scene,
            sample.clone(),
            vec![],
            InitialReference::LumbridgeCastlePlaza,
            prior.viewport,
            5_000_000_000,
        )
        .unwrap();
    assert_eq!(result.output.world_base, WorldBase { x: 3176, y: 3176 });
    assert_eq!(
        consumer.state().unwrap().native_velocity,
        prior.native_velocity
    );
    assert_eq!(
        consumer.state().unwrap().legacy_velocity,
        prior.legacy_velocity
    );
    assert_eq!(consumer.state().unwrap().target_yaw, prior.target_yaw);
    assert_eq!(consumer.state().unwrap().cycle, prior.cycle);
    assert_eq!(
        consumer.state().unwrap().focal_native[0] + 3176 * 128,
        prior.focal_native[0] + 3168 * 128
    );
    consumer.frame(5_020_000_000, sample).unwrap();
    assert_eq!(
        consumer.state().unwrap().native_velocity[0],
        prior.native_velocity[0] / 2
    );
    assert_eq!(consumer.state().unwrap().encoded_fov, prior.encoded_fov);
    let (scene, sample) = fixture(WorldBase { x: 3168, y: 3168 }, 3, "actor.other");
    consumer
        .bind(
            scene,
            sample,
            vec![],
            InitialReference::TutorialStartingHouse,
            prior.viewport,
            6_000_000_000,
        )
        .unwrap();
    assert_eq!(consumer.state().unwrap().target_yaw, 0);
    assert_eq!(consumer.state().unwrap().cycle, 0);
}

#[test]
fn missing_or_stale_source_inputs_fail_transactionally_and_never_become_ready() {
    let (mut consumer, sample) = bound(0);
    let prior = consumer.state().unwrap().clone();
    for variant in 0..5 {
        let mut invalid = sample.clone();
        match variant {
            0 => invalid.focus = None,
            1 => invalid.effects = None,
            2 => invalid.context.scene_generation = "2".into(),
            3 => invalid.context.actor_id = "actor.other".into(),
            _ => invalid.context.revision = "6".into(),
        }
        assert!(consumer.frame(20_000_000, invalid).is_err());
        assert_eq!(consumer.state().unwrap(), &prior);
    }
    let mut invalid = sample.clone();
    invalid.effects.as_mut().unwrap().shake[0] = Some(clubscape_camera::ShakeChannel {
        random_radius: 1,
        sine_amplitude: 2,
        sine_frequency: 3,
        phase: 4,
        random_sample: None,
    });
    assert!(
        consumer
            .frame(20_000_000, invalid)
            .unwrap_err()
            .contains("random draw")
    );
    assert_eq!(consumer.state().unwrap(), &prior);
    assert!(
        consumer
            .frame((MAX_STEPS_PER_FRAME + 1) * SOURCE_CYCLE_NS, sample)
            .is_err()
    );
    assert_eq!(consumer.state().unwrap(), &prior);
}

#[test]
fn event_queue_and_clock_arithmetic_are_bounded_without_silent_drops() {
    let (mut consumer, sample) = bound(0);
    for i in 0..MAX_PENDING_INPUTS {
        consumer.input(i as u64, Input::default()).unwrap();
    }

    assert!(
        consumer
            .input(MAX_PENDING_INPUTS as u64, Input::default())
            .is_err()
    );
    consumer.frame(SOURCE_CYCLE_NS, sample.clone()).unwrap();
    assert!(consumer.input(0, Input::default()).is_err());
    let prior = consumer.state().unwrap().clone();
    assert!(consumer.frame(1, sample).is_err());
    assert_eq!(consumer.state().unwrap(), &prior);
    let (scene, sample) = fixture(WorldBase { x: 3168, y: 3168 }, 2, "actor.controlled");
    assert!(
        consumer
            .bind(
                scene,
                sample,
                vec![],
                InitialReference::TutorialStartingHouse,
                prior.viewport,
                u64::MAX
            )
            .is_err()
    );
    assert_eq!(consumer.state().unwrap(), &prior);
}

#[test]
fn reconnect_keeps_the_actual_mouse_sample_and_rejects_old_source_revisions() {
    let (mut consumer, sample) = bound(0);
    consumer
        .input(
            1,
            Input {
                mouse: [100, 200],
                ..Input::default()
            },
        )
        .unwrap();
    consumer.frame(SOURCE_CYCLE_NS, sample.clone()).unwrap();
    consumer
        .input(
            SOURCE_CYCLE_NS + 1,
            Input {
                mouse: [107, 204],
                wheel: Some(WheelInput {
                    rotation: 2,
                    route: WheelRoute::Camera,
                }),
                ..Input::default()
            },
        )
        .unwrap();
    consumer.suspend();
    let (scene, mut current) = fixture(WorldBase { x: 3168, y: 3168 }, 2, "actor.controlled");
    current.context.revision = "8".into();
    current.context.tick = "10".into();
    consumer
        .bind(
            scene,
            current.clone(),
            vec![],
            InitialReference::TutorialStartingHouse,
            Viewport::new(0, 0, 1920, 1080).unwrap(),
            1_000_000_000,
        )
        .unwrap();
    consumer.frame(1_020_000_000, current).unwrap();
    assert_eq!(consumer.state().unwrap().previous_mouse, [107, 204]);
    assert_eq!(consumer.state().unwrap().previous_legacy_mouse, [107, 204]);
    assert!(
        consumer.state().unwrap().last_logical_fov.is_none(),
        "the queued wheel was not replayed"
    );
    let prior = consumer.state().unwrap().clone();
    let (scene, old) = fixture(WorldBase { x: 3176, y: 3176 }, 3, "actor.controlled");
    assert!(
        consumer
            .bind(
                scene,
                old,
                vec![],
                InitialReference::TutorialStartingHouse,
                prior.viewport,
                2_000_000_000
            )
            .unwrap_err()
            .contains("regressed")
    );
    assert_eq!(consumer.state().unwrap(), &prior);
}
