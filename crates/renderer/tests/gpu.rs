//! Differential tests: the wgpu compute rasterizer must reproduce the CPU reference (and
//! therefore the original captures) pixel for pixel on a real GPU device.

#![cfg(feature = "gpu")]

use std::path::{Path, PathBuf};

use clubscape_renderer::gpu::{GpuRasterizer, GpuTextures, pack_frame};
use clubscape_renderer::model::Model;
use clubscape_renderer::model_draw::{ModelDrawer, ModelScratch};
use clubscape_renderer::palette::Palette;
use clubscape_renderer::raster::software::Software;
use clubscape_renderer::raster::{RasterState, Tri};
use clubscape_renderer::scene::SceneData;
use clubscape_renderer::scene::draw::{SceneDrawer, SceneView};
use clubscape_renderer::texture::{Texture, TextureSet};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

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

fn gpu(width: u32, height: u32) -> Option<Gpu> {
    let (adapter, device, queue) =
        match pollster::block_on(clubscape_renderer::gpu::device::request_native_device()) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("skipping GPU test: {e}");
                return None;
            }
        };
    let palette = load_palette();
    let textures = load_textures();
    let gpu_textures = GpuTextures::from_set(&textures);
    let raster =
        GpuRasterizer::new(device, queue, &palette.rgb, &gpu_textures, width, height).unwrap();
    Some(Gpu {
        raster,
        palette,
        textures,
        adapter_name: adapter.get_info().name,
    })
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

fn gpu_render(g: &mut Gpu, state: &RasterState, tris: &[Tri], clear: u32) -> Vec<i32> {
    let packed = pack_frame(state, tris, &g.textures);
    let frame = g.raster.render(state, &packed, clear).unwrap();
    let pixels = g.raster.read_back().unwrap();
    assert!(
        frame.is_complete(),
        "queue completion callback did not fire before readback"
    );
    pixels
}

#[test]
fn gpu_matches_cpu_on_model_fixtures() {
    let Some(mut g) = gpu(1920, 1080) else { return };
    eprintln!("adapter: {}", g.adapter_name);
    let state = RasterState::new(1920, 1080, 1024);
    let mut cases: Vec<(String, Model, i32, i32, i32, Option<String>)> = Vec::new();
    let tree = Model::from_chunks(
        &std::fs::read(
            repo_root().join("assets/compiled/render/models/object-1277-model-1570-lit.bin"),
        )
        .unwrap(),
    )
    .unwrap();
    for yaw in [0, 256, 512, 1024] {
        cases.push((
            format!("tree yaw {yaw}"),
            tree.clone(),
            yaw,
            250,
            750,
            Some(format!("tree-1277-yaw-{yaw}")),
        ));
    }
    for (npc, seq, frames, y, z) in [
        (3028, 6181, 16, 240, 650),
        (3028, 6180, 16, 240, 650),
        (2063, 5668, 14, 160, 400),
        (2063, 5666, 8, 160, 400),
    ] {
        for frame in 0..frames {
            let path = repo_root().join(format!(
                "assets/compiled/render/models/baked/npc-{npc}-seq-{seq}-frame-{frame}.bin"
            ));
            if let Ok(bytes) = std::fs::read(&path) {
                cases.push((
                    format!("npc {npc} seq {seq} frame {frame}"),
                    Model::from_chunks(&bytes).unwrap(),
                    256,
                    y,
                    z,
                    Some(format!("npc-{npc}-sequence-{seq}-frame-{frame}")),
                ));
            }
        }
    }
    let mut scratch = ModelScratch::default();
    for (name, model, yaw, y, z, capture) in cases {
        let mut tris = Vec::new();
        let mut drawer = ModelDrawer {
            state,
            palette: &g.palette.rgb,
            scratch: &mut scratch,
            alpha_pass: 2,
        };
        drawer
            .draw_legacy(&model, 0, yaw, 0, 128, 0, y, z, 0, &mut tris)
            .unwrap();
        let cpu = cpu_render(&state, &tris, &g.palette, &g.textures, 0x303030);
        let gpu = gpu_render(&mut g, &state, &tris, 0x303030);
        let (n, max, first) = diff_count(&gpu, &cpu);
        assert_eq!(
            n, 0,
            "{name}: GPU differs from CPU in {n} pixels (max channel {max}), first index {first:?}"
        );
        if let Some(capture) = capture {
            let expected = read_png_rgb(
                &repo_root().join(format!("assets/reference/osrs240/models/{capture}.png")),
            );
            let (n, max, _) = diff_count(&gpu, &expected);
            assert_eq!(
                n, 0,
                "{name}: GPU differs from source capture in {n} pixels (max channel {max})"
            );
        }
    }
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
    let data =
        std::fs::read(repo_root().join(format!("assets/compiled/render/scenes/{}.bin", case.name)))
            .unwrap();
    let scene = SceneData::from_chunks(&data).unwrap();
    let pack = std::fs::read(repo_root().join(format!(
        "assets/compiled/render/scenes/{}.models.bin",
        case.name
    )))
    .unwrap();
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
        focal_x: (case.focal[0] - case.base[0]) * 128,
        focal_z: (case.focal[1] - case.base[1]) * 128,
        center_on_camera: true,
        far_clip: 32768,
    };
    let mut tris = Vec::new();
    drawer.begin_frame(&scene);
    drawer.draw(&scene, &models, &[], &view, &mut tris);
    let _ = case.camera;
    tris
}

#[test]
fn gpu_matches_cpu_and_source_on_scene_fixtures() {
    let Some(mut g) = gpu(1920, 1080) else { return };
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
        let gpu = gpu_render(&mut g, &state, &tris, 0);
        let elapsed = start.elapsed();
        let (n, max, first) = diff_count(&gpu, &cpu);
        let expected = read_png_rgb(
            &repo_root().join(format!("assets/reference/osrs240/scenes/{}.png", case.name)),
        );
        let (ns, maxs, _) = diff_count(&gpu, &expected);
        eprintln!(
            "{}: {} tris, gpu vs cpu {n} px (max {max}), gpu vs source {ns} px (max {maxs}), {:?} incl. readback, gpu ns {:?}",
            case.name,
            tris.len(),
            elapsed,
            g.raster.last_record.triangles
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
