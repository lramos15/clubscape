use std::{collections::BTreeMap, fs, io::Read, path::PathBuf};

use clubscape_camera::*;
use flate2::read::GzDecoder;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const TRACE_MANIFEST_SHA256: &str =
    "47bd231c8b316f053d311f5c1ca2c37db402465def34ec2637f1a42ebe570df2";
const PUBLICATION_PATH: &str = "assets/manifests/osrs/cache2695-published.json";
const PUBLICATION_SHA256: &str = "84f48642594a6805f5f89c341d178345b7b0ca33f056242f4cb624517cafdb87";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_checked(relative: &str, sha256: &str) -> Vec<u8> {
    let path = root().join(relative);
    let bytes = fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        sha256,
        "{relative}"
    );
    bytes
}

fn decode(relative: &str, bytes: &[u8]) -> Value {
    if relative.ends_with(".gz") {
        let mut decoded = Vec::new();
        GzDecoder::new(bytes).read_to_end(&mut decoded).unwrap();
        serde_json::from_slice(&decoded).unwrap()
    } else {
        serde_json::from_slice(bytes).unwrap()
    }
}

fn load_checked(relative: &str, sha256: &str) -> Value {
    decode(relative, &read_checked(relative, sha256))
}

fn publication_record<'a>(publication: &'a Value, relative: &str) -> &'a Value {
    let records: Vec<_> = publication["published_files"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|record| record["path"] == relative)
        .collect();
    assert_eq!(
        records.len(),
        1,
        "missing/duplicate source input {relative}"
    );
    records[0]
}

fn load_published(publication: &Value, relative: &str) -> Value {
    let record = publication_record(publication, relative);
    let bytes = read_checked(relative, record["sha256"].as_str().unwrap());
    assert_eq!(bytes.len() as u64, record["size_bytes"].as_u64().unwrap());
    decode(relative, &bytes)
}

struct OriginalTerrain {
    base: WorldBase,
    scene_origin: WorldBase,
    regions: BTreeMap<(i32, i32), Value>,
    decoration_raise: BTreeMap<(u8, i32, i32), i32>,
}

impl OriginalTerrain {
    fn new(publication: &Value) -> Self {
        let mut regions = BTreeMap::new();
        let definitions = load_published(
            publication,
            "assets/source/osrs/cache2695/collections/object.json.gz",
        );
        let mut decoration_raise = BTreeMap::new();
        for x in 49..=51 {
            for y in 49..=51 {
                let region = load_published(
                    publication,
                    &format!(
                        "assets/source/osrs/cache2695/world/{}.json.gz",
                        (x << 8) | y
                    ),
                );
                for placement in region["placements"].as_array().unwrap() {
                    if i(&placement[4]) == 22 {
                        let definition = &definitions
                            [format!("asset.source.osrs.cache2695.object.{}", i(&placement[0]))];
                        decoration_raise.insert(
                            (i(&placement[3]) as u8, i(&placement[1]), i(&placement[2])),
                            i(&definition["raise"]),
                        );
                    }
                }
                regions.insert((x, y), region);
            }
        }
        Self {
            base: WorldBase { x: 3168, y: 3168 },
            scene_origin: WorldBase { x: 3168, y: 3168 },
            regions,
            decoration_raise,
        }
    }

    fn value(&self, field: &str, plane: u8, x: u32, y: u32) -> Result<i32> {
        // Native yk rebases coordinates; this probe does not load a replacement scene.
        let wx = self.scene_origin.x + x as i32;
        let wy = self.scene_origin.y + y as i32;
        let region = self
            .regions
            .get(&(wx >> 6, wy >> 6))
            .ok_or(CameraError::MissingHeight { plane, x, y })?;
        let offset = (usize::from(plane) * 64 + (wx & 63) as usize) * 64 + (wy & 63) as usize;
        region[field][offset]
            .as_i64()
            .map(|v| v as i32)
            .ok_or(CameraError::MissingHeight { plane, x, y })
    }
}

impl Terrain for OriginalTerrain {
    fn world_base(&self) -> WorldBase {
        self.base
    }
    fn dimensions(&self) -> [u32; 2] {
        [104, 104]
    }
    fn height_corner(&self, plane: u8, x: u32, y: u32) -> Result<i32> {
        self.value("heights", plane, x, y)
    }
    fn tile_settings(&self, plane: u8, x: u32, y: u32) -> Result<u8> {
        self.value("tile_settings", plane, x, y).map(|v| v as u8)
    }
    fn surface_height(&self, plane: u8, x: i32, y: i32) -> Result<i32> {
        let tx = (x >> 7) as u32;
        let ty = (y >> 7) as u32;
        let p = if plane < 3 && self.tile_settings(1, tx, ty)? & 2 != 0 {
            plane + 1
        } else {
            plane
        };
        let x0 = (tx * 128) as i32;
        let y0 = (ty * 128) as i32;
        let sw = self.height_corner(p, tx, ty)?;
        let se = self.height_corner(p, tx + 1, ty)?;
        let ne = self.height_corner(p, tx + 1, ty + 1)?;
        let nw = self.height_corner(p, tx, ty + 1)?;
        for triangle in [
            SurfaceTriangle {
                horizontal: [[x0 + 128, y0 + 128], [x0, y0 + 128], [x0 + 128, y0]],
                heights: [ne, nw, se],
            },
            SurfaceTriangle {
                horizontal: [[x0, y0], [x0 + 128, y0], [x0, y0 + 128]],
                heights: [sw, se, nw],
            },
        ] {
            if let Some(height) = triangle.height_at([x, y])? {
                let raised = self
                    .decoration_raise
                    .get(&(
                        plane,
                        self.scene_origin.x + tx as i32,
                        self.scene_origin.y + ty as i32,
                    ))
                    .copied()
                    .unwrap_or(0);
                return Ok(height - raised);
            }
        }
        Err(CameraError::InvalidSurface)
    }
}

fn i(value: &Value) -> i32 {
    value.as_i64().unwrap() as i32
}
fn f(value: &Value) -> f32 {
    value.as_f64().unwrap() as f32
}
fn pair(value: &Value) -> [i32; 2] {
    [i(&value[0]), i(&value[1])]
}
fn triple(value: &Value) -> [i32; 3] {
    [i(&value[0]), i(&value[1]), i(&value[2])]
}

#[derive(Default)]
struct Comparison {
    fields: BTreeMap<String, usize>,
    float_fields: BTreeMap<String, usize>,
    failures: Vec<Value>,
    stages: usize,
    float_checks: usize,
    float_bit_mismatches: usize,
    max_float_abs_error: f64,
}

impl Comparison {
    fn ints(&mut self, scenario: &str, stage: &str, field: &str, actual: &[i32], expected: &[i32]) {
        *self.fields.entry(field.into()).or_default() += actual.len();
        if actual != expected {
            self.failures.push(json!({"scenario":scenario,"stage":stage,"field":field,"actual":actual,"expected":expected}));
        }
    }
    fn floats(
        &mut self,
        scenario: &str,
        stage: &str,
        field: &str,
        actual: &[f32],
        expected: &[f32],
    ) {
        assert_eq!(actual.len(), expected.len());
        *self.float_fields.entry(field.into()).or_default() += actual.len();
        for (a, e) in actual.iter().zip(expected) {
            self.float_checks += 1;
            let error = (f64::from(*a) - f64::from(*e)).abs();
            self.max_float_abs_error = self.max_float_abs_error.max(error);
            if a.to_bits() != e.to_bits() {
                self.float_bit_mismatches += 1;
                self.failures.push(
                    json!({"scenario":scenario,"stage":stage,"field":field,"actual":a,"expected":e,
                    "actual_bits":a.to_bits(),"expected_bits":e.to_bits(),"absolute_error":error}),
                );
            }
        }
    }
    fn state(&mut self, scenario: &str, stage: &str, camera: &NormalCamera, expected: &Value) {
        self.stages += 1;
        let s = camera.state();
        self.ints(
            scenario,
            stage,
            "normal_mode_and_button_mask",
            &[0, 0],
            &[
                i(&expected["camera_mode"]),
                i(&expected["runelite_mouse_button_mask"]),
            ],
        );
        self.ints(
            scenario,
            stage,
            "pitch_relaxer_disabled",
            &[0],
            &[i32::from(
                expected["native_pitch_relaxer"].as_bool().unwrap(),
            )],
        );
        self.floats(
            scenario,
            stage,
            "source_camera_speed",
            &[1.0],
            &[f(&expected["runelite_camera_speed"])],
        );
        if let Some(base) = expected.get("world_base") {
            self.ints(
                scenario,
                stage,
                "world_base",
                &[s.base.x, s.base.y],
                &pair(base),
            );
            self.ints(
                scenario,
                stage,
                "world_plane",
                &[i32::from(s.focus_plane)],
                &[i(&expected["plane"])],
            );
            self.ints(
                scenario,
                stage,
                "wheel_varbits",
                &[
                    i32::from(s.preferences.wheel_disabled),
                    i32::from(s.preferences.wheel_override_512),
                ],
                &pair(&expected["wheel_varbits"]),
            );
        }
        self.ints(
            scenario,
            stage,
            "target_angles",
            &[s.target_pitch, s.target_yaw],
            &[i(&expected["target_pitch"]), i(&expected["target_yaw"])],
        );
        self.floats(
            scenario,
            stage,
            "target_float_pitch_yaw",
            &s.target_radians,
            &[
                f(&expected["target_float_pitch_yaw"][0]),
                f(&expected["target_float_pitch_yaw"][1]),
            ],
        );
        self.ints(
            scenario,
            stage,
            "source_cycle",
            &[s.cycle],
            &[i(&expected["source_cycle"])],
        );
        self.ints(
            scenario,
            stage,
            "focal_native",
            &s.focal_native,
            &[
                i(&expected["focal_x_y"][0]),
                i(&expected["focal_height"]),
                i(&expected["focal_x_y"][1]),
            ],
        );
        self.floats(
            scenario,
            stage,
            "focal_float",
            &s.focal_float,
            &[
                f(&expected["focal_float_x_height_y"][0]),
                f(&expected["focal_float_x_height_y"][1]),
                f(&expected["focal_float_x_height_y"][2]),
            ],
        );
        self.ints(
            scenario,
            stage,
            "legacy_velocity",
            &s.legacy_velocity,
            &[
                i(&expected["runelite_yaw_velocity"]),
                i(&expected["runelite_pitch_velocity"]),
            ],
        );
        self.ints(
            scenario,
            stage,
            "projection",
            &[
                s.projection.zoom,
                s.projection.viewport.width as i32,
                s.projection.viewport.height as i32,
            ],
            &[
                i(&expected["viewport_zoom"]),
                i(&expected["viewport"][0]),
                i(&expected["viewport"][1]),
            ],
        );
        self.ints(
            scenario,
            stage,
            "fov",
            &s.encoded_fov,
            &pair(&expected["viewport_projection_shorts"]),
        );
        self.ints(
            scenario,
            stage,
            "distance_scale",
            &s.preferences.distance_scale,
            &pair(&expected["viewport_distance_scale_shorts"]),
        );
        self.ints(
            scenario,
            stage,
            "mouse_preference",
            &[i32::from(s.preferences.middle_mouse_camera)],
            &[i32::from(
                expected["mouse_camera_enabled"].as_bool().unwrap(),
            )],
        );
        self.ints(
            scenario,
            stage,
            "lock",
            &[i32::from(s.locked)],
            &[i32::from(expected["locked"].as_bool().unwrap())],
        );
        let values = [
            ("dn", 0, 82733095),
            ("ek", s.focus_identity.unwrap_or(-1), -1922841141),
            ("ng", i32::from(s.focus_plane), -686553453),
            ("np", s.logical_focus[0], 1126376453),
            ("nq", s.logical_focus[1], 320618265),
            ("md", s.terrain_pitch_floor, -649517237),
            ("nr", s.logical_focus_ground, 1533582705),
            ("do", s.preferences.follow_height, 643105531),
            ("jm", s.native_velocity[0], 249810553),
            ("jh", s.native_velocity[1], -745135755),
            ("jp", s.previous_mouse[1], -1074208083),
            ("jw", s.previous_mouse[0], -561637219),
            ("kx", 0, 1092546751),
        ];
        for (field, value, writer) in values {
            let raw = value.wrapping_mul(writer);
            self.ints(
                scenario,
                stage,
                &format!("native.{field}.raw"),
                &[raw],
                &[i(&expected["native_state"][field]["raw"])],
            );
            let decoded = if field == "nr" {
                raw.wrapping_mul(1423746095)
            } else {
                value
            };
            self.ints(
                scenario,
                stage,
                &format!("native.{field}.decoded"),
                &[decoded],
                &[i(&expected["native_state"][field]["decoded"])],
            );
        }
        let output = s.output;
        self.ints(
            scenario,
            stage,
            "eye_native",
            &output.map_or([0; 3], |v| v.eye_native),
            &triple(&expected["camera_x_height_y"]),
        );
        self.floats(
            scenario,
            stage,
            "eye_float",
            &output.map_or([0.0; 3], |v| v.eye_float),
            &[
                f(&expected["camera_float_x_height_y"][0]),
                f(&expected["camera_float_x_height_y"][1]),
                f(&expected["camera_float_x_height_y"][2]),
            ],
        );
        self.ints(
            scenario,
            stage,
            "camera_angles",
            &output.map_or([0; 2], |v| [v.pitch_native, v.yaw_native]),
            &[i(&expected["camera_pitch"]), i(&expected["camera_yaw"])],
        );
        if expected.get("camera_varc_ints").is_some() {
            let zoom = s.last_logical_fov.unwrap_or([-1; 2]);
            let b = s.preferences.zoom_bounds;
            for (key, value) in [
                (73, zoom[0]),
                (74, zoom[1]),
                (1338, b.min_height),
                (1339, b.max_height),
                (1340, b.min_width),
                (1341, b.max_width),
            ] {
                self.ints(
                    scenario,
                    stage,
                    &format!("varc.{key}"),
                    &[value],
                    &[i(&expected["camera_varc_ints"][key.to_string()])],
                );
            }
        }
    }
}

#[test]
fn all_original_scenario_readbacks() {
    read_checked(
        "research/camera-source/contract.json",
        SOURCE_CONTRACT_SHA256,
    );
    read_checked(
        "research/camera-source/native-methods.json.gz",
        "d94d1778f0fe4773c713daadf84156f1398010d6f4bf437be8831660bb60fb48",
    );
    let manifest = load_checked(
        "research/camera-source/traces/manifest.json",
        TRACE_MANIFEST_SHA256,
    );
    let publication = load_checked(PUBLICATION_PATH, PUBLICATION_SHA256);
    let mut report = Vec::new();
    let mut all_failures = Vec::new();
    let mut total_states = 0;
    for entry in manifest["traces"].as_array().unwrap() {
        let scenario = entry["scenario"].as_str().unwrap();
        let path = entry["trace"]["path"].as_str().unwrap();
        let trace = load_checked(path, entry["trace"]["sha256"].as_str().unwrap());
        let rows = trace["traces"].as_array().unwrap();
        assert_eq!(rows.len() as u64, entry["states"].as_u64().unwrap());
        assert_eq!(trace["native_logic_cycle_period_ns"], SOURCE_CYCLE_NS);
        let mut terrain = OriginalTerrain::new(&publication);
        let mut initial =
            NormalCamera::source_state(terrain.base, Viewport::new(0, 0, 1920, 1080).unwrap())
                .unwrap();
        initial.preferences.follow_height = 25;
        initial.preferences.middle_mouse_camera = true;
        initial.preferences.zoom_bounds = ZoomBounds {
            min_height: 0,
            max_height: 0,
            min_width: 0,
            max_width: 0,
        };
        initial.encoded_fov = [127, 127];
        initial.projection =
            projection(initial.viewport, initial.encoded_fov, initial.aspect_limits).unwrap();
        initial.focus_identity = Some(0);
        initial.logical_focus = [6976, 6464];
        let mut camera = NormalCamera::restore(initial).unwrap();
        let mut comparisons = Comparison::default();
        for (index, row) in rows.iter().enumerate() {
            let label = row["label"].as_str().unwrap();
            let stage = format!("{index}:{label}");
            let mut focus = Focus {
                identity: 0,
                world_base: terrain.base,
                logical: [i(&row["player_local"]["x"]), i(&row["player_local"]["y"])],
                rendered: [
                    f(&row["source_actor_render_coordinates"][0]),
                    f(&row["source_actor_render_coordinates"][1]),
                ],
                plane: i(&row["plane"]) as u8,
                footprint: i(&row["source_actor_footprint_size"]) as u16,
                kind: FocusKind::Actor,
            };
            if index == 0 {
                // This row is the unchanged native helper setup, before its first tick.
            } else if label == "before-original-region-rebase" {
                let mut state = camera.snapshot();
                state.locked = true;
                camera = NormalCamera::restore(state).unwrap();
            } else if label.starts_with("after-original-region-rebase") {
                let base = pair(&row["world_base"]);
                camera
                    .rebase(WorldBase {
                        x: base[0],
                        y: base[1],
                    })
                    .unwrap();
                terrain.base = WorldBase {
                    x: base[0],
                    y: base[1],
                };
            } else if label == "original-source-626-bounds-initialized" {
                camera.set_zoom_bounds(ZoomBounds::PRESET_626).unwrap();
            } else {
                if label == "middle-disabled-source-option" {
                    camera.set_middle_mouse_enabled(false);
                }
                if label == "enabled-neutral" {
                    camera.set_middle_mouse_enabled(true);
                }
                if let Some(angles) = label.strip_prefix("controlled-orbit-") {
                    let values: Vec<i32> = angles.split('-').map(|s| s.parse().unwrap()).collect();
                    camera.set_target_angles(values[0], values[1]).unwrap();
                }
                if label.starts_with("follow-threshold-") || label.starts_with("render-frame-") {
                    let mut state = camera.snapshot();
                    state.focal_float[0] = 6976.0;
                    state.focal_float[2] = 6464.0;
                    state.focal_native[0] = 6976;
                    state.focal_native[2] = 6464;
                    if label.starts_with("render-frame-") {
                        state.native_velocity = [0; 2];
                        state.legacy_velocity = [0; 2];
                    }
                    camera = NormalCamera::restore(state).unwrap();
                    if label.starts_with("render-frame-") {
                        camera.set_target_angles(1536, 0).unwrap();
                    }
                }
                if let Some(size) = label.strip_prefix("native-resize-") {
                    let parts: Vec<u32> = size.split('x').map(|s| s.parse().unwrap()).collect();
                    camera
                        .resize(Viewport::new(0, 0, parts[0], parts[1]).unwrap())
                        .unwrap();
                }
                if let Some(values) = label.strip_prefix("native-distance-scale-") {
                    let parts: Vec<i32> = values.split('-').map(|s| s.parse().unwrap()).collect();
                    camera.apply_distance_scale_program([parts[0], parts[1]]);
                }
                if label == "controlled-source-zoom-512" {
                    camera.apply_fov_program([512, 512]).unwrap();
                }
                if label == "original-wheel-event-delivery" || label == "original-script39-direct" {
                    camera
                        .wheel(WheelInput {
                            rotation: 1,
                            route: WheelRoute::Camera,
                        })
                        .unwrap();
                }
                if label == "source-wheel-disabled-varbit" {
                    camera.set_wheel_gates(true, false);
                }
                let source = &row["input"];
                let keys = source["keys"].as_array().unwrap();
                let held = |code: i32| keys.iter().any(|v| i(v) == code);
                let wheel = i(&source["wheel_rotation"]);
                let input = Input {
                    arrows: ArrowKeys {
                        left: held(96),
                        right: held(97),
                        up: held(98),
                        down: held(99),
                    },
                    mouse: pair(&source["mouse"]),
                    button: if i(&source["native_mouse_button"]) == 4 {
                        MouseButton::Middle
                    } else {
                        MouseButton::Released
                    },
                    wheel: (wheel != 0).then_some(WheelInput {
                        rotation: wheel,
                        route: if label == "sidebar-wheel" {
                            WheelRoute::OtherWidget
                        } else {
                            WheelRoute::Camera
                        },
                    }),
                };
                focus.world_base = terrain.base;
                camera.fixed_step(input, &focus, &terrain).unwrap();
                comparisons.state(
                    scenario,
                    &format!("{stage}:fixed"),
                    &camera,
                    &source["after_original_fixed_tick"],
                );
                camera
                    .render_frame(
                        source["controlled_render_frame_nanoseconds"]
                            .as_u64()
                            .unwrap(),
                        &focus,
                        &terrain,
                    )
                    .unwrap();
            }
            comparisons.ints(
                scenario,
                &stage,
                "source_player_ground_height",
                &[
                    integer_height(&terrain, focus.logical[0], focus.logical[1], focus.plane)
                        .unwrap(),
                ],
                &[i(&row["source_player_ground_height"])],
            );
            comparisons.state(scenario, &stage, &camera, row);
        }
        total_states += rows.len();
        report.push(json!({"scenario":scenario,"source_states":rows.len(),"comparison_stages":comparisons.stages,
            "integer_values_checked":comparisons.fields.values().sum::<usize>(), "integer_fields":comparisons.fields,
            "float_fields":comparisons.float_fields,
            "float_values_checked":comparisons.float_checks,"float_bit_mismatches":comparisons.float_bit_mismatches,
            "maximum_float_absolute_error":comparisons.max_float_abs_error,"mismatch_count":comparisons.failures.len()}));
        all_failures.extend(comparisons.failures);
    }
    let mut source_inputs = vec![
        publication_record(
            &publication,
            "assets/source/osrs/cache2695/collections/object.json.gz",
        )
        .clone(),
    ];
    for x in 49..=51 {
        for y in 49..=51 {
            source_inputs.push(
                publication_record(
                    &publication,
                    &format!(
                        "assets/source/osrs/cache2695/world/{}.json.gz",
                        (x << 8) | y
                    ),
                )
                .clone(),
            );
        }
    }
    let implementation_inputs: Vec<_> = [
        "Cargo.lock",
        "crates/camera/Cargo.toml",
        "crates/camera/src/lib.rs",
        "crates/camera/src/math.rs",
        "crates/camera/src/effects.rs",
        "crates/camera/src/terrain.rs",
        "crates/camera/src/viewport.rs",
        "crates/camera/tests/boundaries.rs",
        "crates/camera/tests/conformance.rs",
        "crates/camera/initialization.json",
        "crates/camera/source-readbacks.json",
        "crates/camera/README.md",
    ]
    .into_iter()
    .map(|path| {
        let bytes = fs::read(root().join(path)).unwrap();
        json!({"path":path,"size_bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(&bytes))})
    })
    .collect();
    let totals = json!({
        "comparison_stages": report.iter().map(|r| r["comparison_stages"].as_u64().unwrap()).sum::<u64>(),
        "integer_values_checked": report.iter().map(|r| r["integer_values_checked"].as_u64().unwrap()).sum::<u64>(),
        "float_values_checked": report.iter().map(|r| r["float_values_checked"].as_u64().unwrap()).sum::<u64>(),
    });
    let result = json!({"schema_version":1,"source_states":total_states,"scenarios":report,"mismatches":all_failures,
        "totals":totals,"implementation_inputs":implementation_inputs,
        "source_contract_sha256":SOURCE_CONTRACT_SHA256,"source_trace_manifest_sha256":TRACE_MANIFEST_SHA256,
        "source_publication":{"path":PUBLICATION_PATH,"sha256":PUBLICATION_SHA256},
        "source_inputs":source_inputs,"integer_equality_required":true,"float_bit_equality_required":true,
        "source_qualification":"Controlled offline original-client traces; no authenticated session initialization observed.",
        "comparison_qualification":"Rust host controller and exact native readbacks, not browser/renderer or M1 acceptance."});
    let path = root().join("target/camera-conformance.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let bytes = serde_json::to_vec_pretty(&result).unwrap();
    fs::write(&path, &bytes).unwrap();
    assert_eq!(total_states, 354);
    assert!(
        all_failures.is_empty(),
        "{} native readback mismatches; first errors: {}\nComplete report: {}",
        all_failures.len(),
        serde_json::to_string_pretty(&all_failures.iter().take(20).collect::<Vec<_>>()).unwrap(),
        path.display()
    );
    if let Some(publish) = std::env::var_os("CAMERA_CONFORMANCE_REPORT") {
        fs::write(root().join(publish), bytes).unwrap();
    }
}

#[test]
fn constructor_preferences_and_approved_initialization_are_hash_bound() {
    let original = load_checked(
        "research/camera-source/traces/initial.json",
        "3005124064d8bcaba6c7733e374409c904f80ebc3b170d2b57f39d3bc79ee314",
    );
    let constructor = &original["constructor_defaults"];
    let state = NormalCamera::source_state(
        WorldBase { x: 3168, y: 3168 },
        Viewport::new(0, 0, 1920, 1080).unwrap(),
    )
    .unwrap();
    assert_eq!(
        [state.target_pitch, state.target_yaw],
        [
            i(&constructor["target_pitch"]),
            i(&constructor["target_yaw"])
        ]
    );
    assert_eq!(
        state.target_radians.map(f32::to_bits),
        [
            f(&constructor["target_float_pitch_yaw"][0]).to_bits(),
            f(&constructor["target_float_pitch_yaw"][1]).to_bits(),
        ]
    );
    assert_eq!(
        state.encoded_fov,
        pair(&constructor["viewport_projection_shorts"])
    );
    assert_eq!(
        state.preferences.distance_scale,
        pair(&constructor["viewport_distance_scale_shorts"])
    );
    assert_eq!(
        state.preferences.follow_height,
        i(&constructor["native_state"]["do"]["decoded"])
    );
    assert_eq!(
        state.preferences.middle_mouse_camera,
        constructor["mouse_camera_enabled"].as_bool().unwrap()
    );
    assert_eq!(
        state.focus_identity.unwrap_or(-1),
        i(&constructor["native_state"]["ek"]["decoded"])
    );
    assert!(state.output.is_none());

    let approval = load_checked(
        "milestones/approvals/m1-camera-initialization-v1.json",
        APPROVAL_SHA256,
    );
    assert_eq!(approval["decision"], "approved");
    assert_eq!(
        approval["owner_record"],
        "approve_source_based_initialization"
    );
    assert_eq!(
        approval["original_camera_contract"]["source_session_initialization_observed"],
        false
    );
    let captures = load_checked(
        "assets/reference/osrs240/captures.json",
        "1ea61e13890747c29000ae778642337e101850472f12861c3ad9e49d71f8818e",
    );
    for reference in [
        InitialReference::TutorialStartingHouse,
        InitialReference::LumbridgeCastlePlaza,
    ] {
        let provenance = reference.provenance();
        let source = captures["captures"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| {
                format!(
                    "assets/reference/osrs240/{}",
                    entry["path"].as_str().unwrap()
                ) == provenance.reference_path
            })
            .unwrap();
        read_checked(&provenance.reference_path, &provenance.reference_sha256);
        assert_eq!(source["sha256"], provenance.reference_sha256);
        assert_eq!(source["settings"]["pitch_input"], provenance.pitch_native);
        assert_eq!(source["settings"]["yaw_input"], provenance.yaw_native);
        assert_eq!(
            source["settings"]["angle_units_per_turn"],
            provenance.native_units_per_turn
        );
        assert_eq!(provenance.pitch_native, provenance.pitch_legacy * 8);
        assert_eq!(provenance.yaw_native, provenance.yaw_legacy * 8);
        assert!(!provenance.observed_osrs_account_defaults);
    }
}
