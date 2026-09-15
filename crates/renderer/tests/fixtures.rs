//! Pixel-exact fixture tests against the original-runtime captures in `assets/reference/osrs240`.
//!
//! These compare the CPU reference rasterizer + original draw path port against the source
//! software renderer output for the approved model fixtures. They are the foundation for the
//! GPU differential tests; they do not by themselves constitute presentation acceptance.

use std::path::{Path, PathBuf};

use clubscape_renderer::model::Model;
use clubscape_renderer::model_draw::{ModelDrawer, ModelScratch};
use clubscape_renderer::palette::Palette;
use clubscape_renderer::raster::software::Software;
use clubscape_renderer::texture::{Texture, TextureSet};
use clubscape_renderer::raster::{RasterState, Tri};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_png_rgb(path: &Path) -> (u32, u32, Vec<i32>) {
    let file = std::fs::File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let decoder = png::Decoder::new(std::io::BufReader::new(file));
    let mut reader = decoder.read_info().expect("png info");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("png frame");
    let bytes = &buf[..info.buffer_size()];
    let channels = match info.color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        other => panic!("unsupported color type {other:?}"),
    };
    let pixels = bytes
        .chunks_exact(channels)
        .map(|p| ((p[0] as i32) << 16) | ((p[1] as i32) << 8) | p[2] as i32)
        .collect();
    (info.width, info.height, pixels)
}

fn load_palette() -> Palette {
    let data = std::fs::read(repo_root().join("assets/compiled/render/palette.bin")).expect("palette.bin");
    Palette::from_chunks(&data).expect("palette")
}

fn load_textures() -> TextureSet {
    let mut set = TextureSet::default();
    let dir = repo_root().join("assets/compiled/render/textures");
    for entry in std::fs::read_dir(&dir).expect("textures dir") {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "bin") {
            set.insert(Texture::from_chunks(&std::fs::read(&path).unwrap()).expect("texture"));
        }
    }
    assert!(!set.is_empty());
    set
}

fn load_model(name: &str) -> Model {
    let data = std::fs::read(repo_root().join(format!("assets/compiled/render/models/{name}.bin"))).expect("model");
    Model::from_chunks(&data).expect("model parse")
}

struct Diff {
    differing: usize,
    max_channel: i32,
    first: Option<(usize, usize, i32, i32)>,
}

fn compare(width: usize, actual: &[i32], expected: &[i32]) -> Diff {
    let mut diff = Diff { differing: 0, max_channel: 0, first: None };
    for (i, (&a, &e)) in actual.iter().zip(expected).enumerate() {
        let a = a & 0xffffff;
        let e = e & 0xffffff;
        if a != e {
            diff.differing += 1;
            for shift in [16, 8, 0] {
                let d = ((a >> shift & 0xff) - (e >> shift & 0xff)).abs();
                diff.max_channel = diff.max_channel.max(d);
            }
            if diff.first.is_none() {
                diff.first = Some((i % width, i / width, a, e));
            }
        }
    }
    diff
}

fn render_legacy(model: &Model, yaw: i32, y_camera: i32, z_camera: i32, palette: &Palette, textures: &TextureSet) -> Vec<i32> {
    let state = RasterState::new(1920, 1080, 1024);
    let mut pixels = vec![0x303030; 1920 * 1080];
    let mut scratch = ModelScratch::default();
    let mut tris: Vec<Tri> = Vec::new();
    let mut drawer = ModelDrawer { state, palette: &palette.rgb, scratch: &mut scratch, alpha_pass: 2 };
    drawer.draw_legacy(model, 0, yaw, 0, 128, 0, y_camera, z_camera, 0, &mut tris).expect("draw");
    assert!(!tris.is_empty(), "no triangles emitted");
    let mut raster = Software::new(state, &mut pixels, &palette.rgb, textures);
    for tri in &tris {
        raster.draw(tri).expect("raster");
    }
    pixels
}

fn assert_exact(name: &str, actual: &[i32]) {
    let (w, h, expected) = read_png_rgb(&repo_root().join(format!("assets/reference/osrs240/models/{name}.png")));
    assert_eq!((w, h), (1920, 1080));
    let diff = compare(w as usize, actual, &expected);
    assert_eq!(
        diff.differing, 0,
        "{name}: {} pixels differ (max channel error {}), first at {:?}",
        diff.differing, diff.max_channel, diff.first
    );
}

#[test]
fn tree_1277_all_yaws_match_source_pixels() {
    let palette = load_palette();
    let textures = load_textures();
    let model = load_model("object-1277-model-1570-lit");
    assert_eq!((model.vertex_count, model.face_count), (90, 110));
    for yaw in [0, 256, 512, 1024] {
        let pixels = render_legacy(&model, yaw, 250, 750, &palette, &textures);
        assert_exact(&format!("tree-1277-yaw-{yaw}"), &pixels);
    }
}

#[test]
fn baked_goblin_frames_match_source_pixels() {
    let palette = load_palette();
    let textures = load_textures();
    for (seq, frames) in [(6181, 16), (6180, 16)] {
        for frame in 0..frames {
            let name = format!("npc-3028-seq-{seq}-frame-{frame}");
            let path = repo_root().join(format!("assets/compiled/render/models/baked/{name}.bin"));
            if !path.exists() {
                eprintln!("skipping {name}: baked frame not exported locally");
                continue;
            }
            let model = Model::from_chunks(&std::fs::read(path).unwrap()).unwrap();
            let pixels = render_legacy(&model, 256, 240, 650, &palette, &textures);
            assert_exact(&format!("npc-3028-sequence-{seq}-frame-{frame}"), &pixels);
        }
    }
}

#[test]
fn baked_penguin_frames_match_source_pixels() {
    let palette = load_palette();
    let textures = load_textures();
    for (seq, frames) in [(5668, 14), (5666, 8)] {
        for frame in 0..frames {
            let name = format!("npc-2063-seq-{seq}-frame-{frame}");
            let path = repo_root().join(format!("assets/compiled/render/models/baked/{name}.bin"));
            if !path.exists() {
                eprintln!("skipping {name}: baked frame not exported locally");
                continue;
            }
            let model = Model::from_chunks(&std::fs::read(path).unwrap()).unwrap();
            let pixels = render_legacy(&model, 256, 160, 400, &palette, &textures);
            assert_exact(&format!("npc-2063-sequence-{seq}-frame-{frame}"), &pixels);
        }
    }
}

#[test]
fn palette_build_matches_exported_palette() {
    let exported = load_palette();
    let built = Palette::build(0.8);
    let mismatches = exported.rgb.iter().zip(&built.rgb).filter(|(a, b)| a != b).count();
    assert_eq!(mismatches, 0, "palette computation differs from the original export in {mismatches} entries");
}

#[test]
fn trig_tables_match_exported_hash() {
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(repo_root().join("assets/compiled/render/manifest.json")).unwrap()).unwrap();
    let expected = manifest["files"]["tables.bin"]["sha256"].as_str().expect("tables hash recorded");
    let bytes = clubscape_renderer::tables::tables_csrc_bytes();
    let actual = sha256_hex(&bytes);
    assert_eq!(actual, expected, "computed trig tables differ from the original runtime's Perspective tables");
}

fn sha256_hex(bytes: &[u8]) -> String {
    use std::process::{Command, Stdio};
    use std::io::Write;
    let mut child = Command::new("sha256sum").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().expect("sha256sum");
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let out = child.wait_with_output().unwrap();
    String::from_utf8(out.stdout).unwrap().split_whitespace().next().unwrap().to_string()
}
