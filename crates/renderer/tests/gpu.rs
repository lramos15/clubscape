//! Differential tests: the wgpu compute rasterizer must reproduce the CPU reference (and
//! therefore the original captures) pixel for pixel on a real GPU device.

#![cfg(feature = "gpu")]

mod common;

use std::path::Path;

use clubscape_renderer::gpu::{GpuFrame, GpuRasterizer, GpuTextures, pack_frame};
use clubscape_renderer::model::Model;
use clubscape_renderer::model_draw::{ModelDrawer, ModelScratch};
use clubscape_renderer::palette::Palette;
use clubscape_renderer::raster::software::Software;
use clubscape_renderer::raster::{RasterState, Tri};
use clubscape_renderer::scene::SceneData;
use clubscape_renderer::scene::draw::{SceneDrawer, SceneView};
use clubscape_renderer::texture::{Texture, TextureSet};

use common::{read_asset, repo_root};

fn load_palette() -> Palette {
    Palette::from_chunks(
        &std::fs::read(repo_root().join("assets/compiled/render/palette.bin")).unwrap(),
    )
    .unwrap()
}

fn load_textures() -> TextureSet {
    let mut set = TextureSet::default();
    for entry in std::fs::read_dir(repo_root().join("assets/compiled/render/textures")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "bin") {
            set.insert(Texture::from_chunks(&std::fs::read(&path).unwrap()).unwrap());
        }
    }
    set
}

fn read_png_rgb(path: &Path) -> Vec<i32> {
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut buf = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buf).unwrap();
    let channels = if info.color_type == png::ColorType::Rgba {
        4
    } else {
        3
    };
    buf[..info.buffer_size()]
        .chunks_exact(channels)
        .map(|p| ((p[0] as i32) << 16) | ((p[1] as i32) << 8) | p[2] as i32)
        .collect()
}

struct Gpu {
    raster: GpuRasterizer,
    palette: Palette,
    textures: TextureSet,
    adapter_name: String,
}

/// An explicit `--features gpu` run is a GPU fidelity run: no hardware adapter is a failure,
/// never a pass with the cases skipped. Runs without a GPU simply do not enable the feature.
fn gpu(width: u32, height: u32) -> Gpu {
    let (adapter, device, queue) =
        match pollster::block_on(clubscape_renderer::gpu::device::request_native_device()) {
            Ok(v) => v,
            Err(e) => panic!(
                "GPU fidelity tests need a hardware wgpu adapter (Vulkan/Metal): {e}. \
                 Run without `--features gpu` on machines without one; a missing GPU is not a pass."
            ),
        };
    let palette = load_palette();
    let textures = load_textures();
    let gpu_textures = GpuTextures::from_set(&textures);
    let raster =
        GpuRasterizer::new(device, queue, &palette.rgb, &gpu_textures, width, height).unwrap();
    Gpu {
        raster,
        palette,
        textures,
        adapter_name: adapter.get_info().name,
    }
}

/// Human-readable GPU pass duration of a completed frame: the timestamp-query span when the
/// adapter supports it and the readback has been mapped, otherwise explicitly unavailable.
fn gpu_time_label(g: &Gpu, frame: &GpuFrame) -> String {
    if !g.raster.timestamps_supported() {
        return "gpu time unavailable (adapter has no timestamp queries)".into();
    }
    let mut polls = 0;
    while frame.timestamps_pending() && polls < 1000 {
        g.raster.poll_once().unwrap();
        polls += 1;
    }
    match frame.gpu_duration_ns() {
        Some(ns) => format!("gpu pass {:.3} ms (timestamp query)", ns as f64 / 1e6),
        None => "gpu time unavailable (timestamp readback not mapped)".into(),
    }
}

fn cpu_render(
    state: &RasterState,
    tris: &[Tri],
    palette: &Palette,
    textures: &TextureSet,
    clear: i32,
) -> Vec<i32> {
    let mut pixels = vec![clear; (state.width * state.height) as usize];
    let mut raster = Software::new(*state, &mut pixels, &palette.rgb, textures);
    for tri in tris {
        let _ = raster.draw(tri);
    }
    pixels
}

fn diff_count(a: &[i32], b: &[i32]) -> (usize, i32, Option<usize>) {
    let mut n = 0;
    let mut max = 0;
    let mut first = None;
    for (i, (&x, &y)) in a.iter().zip(b).enumerate() {
        let (x, y) = (x & 0xffffff, y & 0xffffff);
        if x != y {
            n += 1;
            first.get_or_insert(i);
            for s in [16, 8, 0] {
                max = max.max(((x >> s & 255) - (y >> s & 255)).abs());
            }
        }
    }
    (n, max, first)
}

/// Renders on the GPU and returns the pixels with the completed frame (its completion callback
/// must have fired before the readback returned; the caller reads timing from the frame).
fn gpu_render(g: &mut Gpu, state: &RasterState, tris: &[Tri], clear: u32) -> (Vec<i32>, GpuFrame) {
    let packed = pack_frame(state, tris, &g.textures);
    let frame = g.raster.render(state, &packed, clear).unwrap();
    let pixels = g.raster.read_back().unwrap();
    assert!(
        frame.is_complete(),
        "queue completion callback did not fire before readback"
    );
    (pixels, frame)
}

/// All 58 approved model captures (4 tree yaws + 54 NPC frames from the published packs) must
/// match both the CPU port and the source PNG on the GPU; no case may be skipped.
#[test]
fn gpu_matches_cpu_and_source_on_all_58_model_captures() {
    let mut g = gpu(1920, 1080);
    eprintln!("adapter: {}", g.adapter_name);
    let state = RasterState::new(1920, 1080, 1024);
    let mut cases: Vec<(String, Model, i32, i32, i32)> = Vec::new();
    let tree = Model::from_chunks(&read_asset("models/object-1277-model-1570-lit.bin")).unwrap();
    for yaw in [0, 256, 512, 1024] {
        cases.push((format!("tree-1277-yaw-{yaw}"), tree.clone(), yaw, 250, 750));
    }
    for (capture, model, y, z) in common::npc_capture_models() {
        cases.push((capture, model, 256, y, z));
    }
    assert_eq!(cases.len(), 58, "4 tree yaws + 54 NPC frames");
    let mut scratch = ModelScratch::default();
    let mut gpu_ns: Vec<u64> = Vec::new();
    for (capture, model, yaw, y, z) in &cases {
        let mut tris = Vec::new();
        let mut drawer = ModelDrawer {
            state,
            palette: &g.palette.rgb,
            scratch: &mut scratch,
            alpha_pass: 2,
        };
        drawer
            .draw_legacy(model, 0, *yaw, 0, 128, 0, *y, *z, 0, &mut tris)
            .unwrap();
        let cpu = cpu_render(&state, &tris, &g.palette, &g.textures, 0x303030);
        let (gpu, frame) = gpu_render(&mut g, &state, &tris, 0x303030);
        let (n, max, first) = diff_count(&gpu, &cpu);
        assert_eq!(
            n, 0,
            "{capture}: GPU differs from CPU in {n} pixels (max channel {max}), first index {first:?}"
        );
        let expected = read_png_rgb(
            &repo_root().join(format!("assets/reference/osrs240/models/{capture}.png")),
        );
        let (n, max, _) = diff_count(&gpu, &expected);
        assert_eq!(
            n, 0,
            "{capture}: GPU differs from source capture in {n} pixels (max channel {max})"
        );
        let label = gpu_time_label(&g, &frame);
        if let Some(ns) = frame.gpu_duration_ns() {
            gpu_ns.push(ns);
        }
        eprintln!(
            "{capture}: {} tris, identical to CPU and source; {label}",
            tris.len()
        );
    }
    eprintln!(
        "58/58 model captures identical on {} ({} with timestamp spans, max {:.3} ms)",
        g.adapter_name,
        gpu_ns.len(),
        gpu_ns.iter().copied().max().unwrap_or(0) as f64 / 1e6
    );
}

struct SceneCase {
    name: &'static str,
    camera: [i32; 3],
    pitch: i32,
    yaw: i32,
    focal: [i32; 2],
    base: [i32; 2],
}

const SCENES: &[SceneCase] = &[
    SceneCase {
        name: "tutorial-starting-house",
        camera: [5888, -2360, 4992],
        pitch: 2048,
        yaw: 0,
        focal: [3094, 3103],
        base: [3048, 3056],
    },
    SceneCase {
        name: "tutorial-survival-coast",
        camera: [0, 0, 0],
        pitch: 2048,
        yaw: 1024,
        focal: [3101, 3085],
        base: [3048, 3032],
    },
    SceneCase {
        name: "lumbridge-castle-plaza",
        camera: [0, 0, 0],
        pitch: 2048,
        yaw: 0,
        focal: [3222, 3218],
        base: [3168, 3168],
    },
    SceneCase {
        name: "lumbridge-river-bridge",
        camera: [0, 0, 0],
        pitch: 2048,
        yaw: 2048,
        focal: [3223, 3217],
        base: [3168, 3168],
    },
    SceneCase {
        name: "lumbridge-windmill-route",
        camera: [0, 0, 0],
        pitch: 2048,
        yaw: 1536,
        focal: [3166, 3306],
        base: [3120, 3240],
    },
];

fn fixture_camera(name: &str) -> [i32; 3] {
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repo_root().join("research/reference-pack/v1/manifest.json"))
            .unwrap(),
    )
    .unwrap();
    let record = manifest["original_inputs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"].as_str() == Some(&format!("original.scenes.{name}")))
        .unwrap();
    let cam = record["settings"]["camera_local_units"].as_array().unwrap();
    [
        cam[0].as_i64().unwrap() as i32,
        cam[1].as_i64().unwrap() as i32,
        cam[2].as_i64().unwrap() as i32,
    ]
}

fn scene_triangles(case: &SceneCase, palette: &Palette, state: RasterState) -> Vec<Tri> {
    let data = read_asset(&format!("scenes/{}.bin", case.name));
    let scene = SceneData::from_chunks(&data).unwrap();
    let pack = read_asset(&format!("scenes/{}.models.bin", case.name));
    let models: Vec<Option<Model>> = clubscape_renderer::model::parse_model_pack(&pack)
        .unwrap()
        .into_iter()
        .map(|(_, m)| Some(m))
        .collect();
    let camera = fixture_camera(case.name);
    let mut drawer = SceneDrawer::new(&scene, state, &palette.rgb, 32768);
    let view = SceneView {
        camera_x: camera[0],
        camera_height: camera[1],
        camera_z: camera[2],
        pitch: case.pitch,
        yaw: case.yaw,
        plane: 0,
        top_plane: 0,
        focal_x: (case.focal[0] - case.base[0]) * 128,
        focal_z: (case.focal[1] - case.base[1]) * 128,
        center_on_camera: true,
        far_clip: 32768,
        animation_cycles: 0,
        roof: Default::default(),
    };
    let mut tris = Vec::new();
    drawer.begin_frame(&scene);
    drawer.draw(&scene, &models, &[], &view, &mut tris);
    let _ = case.camera;
    tris
}

#[test]
fn gpu_matches_cpu_and_source_on_scene_fixtures() {
    let mut g = gpu(1920, 1080);
    eprintln!(
        "adapter: {} timestamps: {}",
        g.adapter_name,
        g.raster.timestamps_supported()
    );
    let state = RasterState::new(1920, 1080, 662);
    for case in SCENES {
        let tris = scene_triangles(case, &g.palette, state);
        let cpu = cpu_render(&state, &tris, &g.palette, &g.textures, 0);
        let start = std::time::Instant::now();
        let (gpu, frame) = gpu_render(&mut g, &state, &tris, 0);
        let elapsed = start.elapsed();
        let (n, max, first) = diff_count(&gpu, &cpu);
        let expected = read_png_rgb(
            &repo_root().join(format!("assets/reference/osrs240/scenes/{}.png", case.name)),
        );
        let (ns, maxs, _) = diff_count(&gpu, &expected);
        assert_eq!(
            g.raster.last_record.triangles,
            tris.len(),
            "frame record triangle count"
        );
        eprintln!(
            "{}: {} tris, gpu vs cpu {n} px (max {max}), gpu vs source {ns} px (max {maxs}), \
             {:?} wall time incl. submit+readback, {}",
            case.name,
            tris.len(),
            elapsed,
            gpu_time_label(&g, &frame)
        );
        assert_eq!(
            n, 0,
            "{}: GPU differs from CPU in {n} pixels (max channel {max}), first {first:?}",
            case.name
        );
        assert_eq!(
            ns, 0,
            "{}: GPU differs from source capture in {ns} pixels (max channel {maxs})",
            case.name
        );
    }
}

/// Interface preview surfaces: coverage alpha marks exactly the pixels a triangle wrote, the
/// colours match the CPU reference, and the asynchronous readback path delivers the same bytes.
#[test]
fn gpu_preview_surface_has_exact_coverage_alpha() {
    let mut g = gpu(480, 315);
    let tree = Model::from_chunks(&read_asset("models/object-1277-model-1570-lit.bin")).unwrap();
    // The interface projection: component centre, zoom 512, pitch 150 (content type 328).
    let mut state = RasterState::new(480, 315, 512);
    state.center_x = 240;
    state.center_y = 187;
    let t = clubscape_renderer::tables::tables();
    let zoom = 450;
    let (sin_x, cos_x) = ((t.sin2048[150] * zoom) >> 16, (t.cos2048[150] * zoom) >> 16);
    let mut tris = Vec::new();
    let mut scratch = ModelScratch::default();
    {
        let mut drawer = ModelDrawer {
            state,
            palette: &g.palette.rgb,
            scratch: &mut scratch,
            alpha_pass: 2,
        };
        drawer
            .draw_legacy(
                &tree,
                0,
                0,
                0,
                150,
                0,
                sin_x + 175,
                cos_x + 175,
                0,
                &mut tris,
            )
            .unwrap();
    }
    assert!(tris.len() > 50);
    // A pixel is covered when some triangle wrote it: opaque fills are clear-independent,
    // translucent fills differ from both clear colours.
    let dark = cpu_render(&state, &tris, &g.palette, &g.textures, 0x010203);
    let light = cpu_render(&state, &tris, &g.palette, &g.textures, 0xFEFDFC);
    let black = cpu_render(&state, &tris, &g.palette, &g.textures, 0);
    let covered: Vec<bool> = dark
        .iter()
        .zip(&light)
        .map(|(a, b)| !((a & 0xFFFFFF) == 0x010203 && (b & 0xFFFFFF) == 0xFEFDFC))
        .collect();
    let packed = pack_frame(&state, &tris, &g.textures);
    let frame = g
        .raster
        .render_with_coverage(&state, &packed, 0, true)
        .unwrap();
    let pending = g.raster.begin_read_back();
    // Poll like the browser does until the map callback fired.
    for _ in 0..10_000 {
        if pending.is_ready().is_some() {
            break;
        }
        g.raster.poll_once().unwrap();
    }
    assert!(frame.is_complete(), "queue completion did not fire");
    let rgba = pending.take().unwrap();
    assert_eq!(rgba.len(), 480 * 315 * 4);
    let mut alpha_mismatch = 0;
    let mut color_mismatch = 0;
    let mut covered_count = 0;
    for (i, c) in covered.iter().enumerate() {
        let p = &rgba[i * 4..i * 4 + 4];
        let alpha = p[3];
        if *c {
            covered_count += 1;
            if alpha != 255 {
                alpha_mismatch += 1;
            }
            let rgb = ((p[0] as i32) << 16) | ((p[1] as i32) << 8) | p[2] as i32;
            if rgb != (black[i] & 0xFFFFFF) {
                color_mismatch += 1;
            }
        } else if alpha != 0 {
            alpha_mismatch += 1;
        }
    }
    let mut gpu_only = 0;
    let mut cpu_only = 0;
    let mut sample = Vec::new();
    for (i, c) in covered.iter().enumerate() {
        let alpha = rgba[i * 4 + 3];
        if *c && alpha != 255 {
            cpu_only += 1;
        }
        if !*c && alpha != 0 {
            gpu_only += 1;
            if sample.len() < 5 {
                let p = &rgba[i * 4..i * 4 + 4];
                sample.push((
                    i % 480,
                    i / 480,
                    p[0],
                    p[1],
                    p[2],
                    dark[i] & 0xFFFFFF,
                    light[i] & 0xFFFFFF,
                ));
            }
        }
    }
    eprintln!(
        "preview coverage {covered_count} px on {} (gpu-only {gpu_only}, cpu-only {cpu_only}) sample {sample:?}",
        g.adapter_name
    );
    assert!(
        covered_count > 500,
        "tree preview covered {covered_count} px"
    );
    assert_eq!(
        alpha_mismatch, 0,
        "coverage alpha differs from the CPU coverage"
    );
    assert_eq!(
        color_mismatch, 0,
        "covered colours differ from the CPU reference"
    );
    // The opaque path is unchanged: every alpha is 255.
    g.raster.render(&state, &packed, 0x303030).unwrap();
    let opaque = g.raster.read_back().unwrap();
    assert_eq!(opaque.len(), 480 * 315);
    let (n, max, _) = diff_count(
        &opaque,
        &cpu_render(&state, &tris, &g.palette, &g.textures, 0x303030),
    );
    assert_eq!((n, max), (0, 0), "opaque preview differs from CPU");
}
