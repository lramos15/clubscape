mod common;

use clubscape_renderer::RenderError;
use clubscape_renderer::chunk::Chunks;
use clubscape_renderer::core::RendererCore;
use clubscape_renderer::palette::Palette;
use clubscape_renderer::scene::terrain::{FloorDefs, RawTerrain};

const UNDERLAY: [i32; 5] = [0, 0, 64, 128, 64];
const OVERLAY: [i32; 10] = [0, -1, 0x808080, 0, 0, 128, -1, 0, 0, 0];
const SQUARE: i32 = 12850;
const TILES: usize = 4 * 64 * 64;

fn ints(values: &[i32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn container(chunks: Vec<(&str, Vec<u8>)>) -> Vec<u8> {
    let mut bytes = b"CSRC".to_vec();
    bytes.extend_from_slice(&1u32.to_le_bytes());
    for (tag, payload) in chunks {
        assert_eq!(tag.len(), 4);
        bytes.extend_from_slice(tag.as_bytes());
        bytes.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&payload);
    }
    bytes
}

fn floors(underlays: &[i32], overlays: &[i32]) -> Vec<u8> {
    container(vec![("FUND", ints(underlays)), ("FOVL", ints(overlays))])
}

fn raw_fields(underlay: i32, overlay: i32) -> Vec<i32> {
    let mut fields = vec![0; TILES * 6];
    fields[0] = underlay;
    fields[1] = overlay;
    fields
}

fn parse_raw(
    terrain: Option<&[i32]>,
    shadows: Option<&[i32]>,
) -> Result<Option<RawTerrain>, RenderError> {
    let mut chunks = Vec::new();
    if let Some(values) = terrain {
        chunks.push(("BTER", ints(values)));
    }
    if let Some(values) = shadows {
        chunks.push(("BSHD", ints(values)));
    }
    let bytes = container(chunks);
    RawTerrain::from_block_chunks(&Chunks::parse(&bytes)?)
}

fn block(terrain: Option<&[i32]>) -> Vec<u8> {
    let mut chunks = vec![
        (
            "BLHD",
            ints(&[SQUARE, 50, 50, 3200, 3200, 64, 4, 0, 0, 0, 0, 0, 0]),
        ),
        ("BFLG", ints(&vec![0; TILES])),
        ("BLNK", vec![0xff; TILES]),
        ("BOBC", vec![0; TILES]),
        ("BOBF", vec![0; TILES * 5]),
        ("BHGT", ints(&vec![0; 4 * 65 * 65])),
        ("BROF", ints(&vec![0; TILES])),
        ("BSET", vec![0; TILES]),
        ("MODL", Vec::new()),
        ("BPNT", Vec::new()),
        ("BTMD", Vec::new()),
        ("BWAL", Vec::new()),
        ("BWDC", Vec::new()),
        ("BFDC", Vec::new()),
        ("BOBJ", Vec::new()),
        ("BDYN", Vec::new()),
    ];
    if let Some(terrain) = terrain {
        chunks.push(("BTER", ints(terrain)));
        chunks.push(("BSHD", Vec::new()));
    }
    container(chunks)
}

fn core_with_previous_scene(terrain: Option<&[i32]>) -> RendererCore {
    let mut core = RendererCore::new(
        Palette::from_chunks(&common::read_asset("palette.bin")).unwrap(),
        1920,
        1080,
    );
    for bytes in common::texture_bytes() {
        core.add_texture(&bytes).unwrap();
    }
    core.load_scene(
        "previous-source-scene",
        &common::read_asset("scenes/tutorial-starting-house.bin"),
        &common::read_asset("scenes/tutorial-starting-house.models.bin"),
    )
    .unwrap();
    core.load_block(SQUARE, &block(terrain), b"CSMP\0\0\0\0")
        .unwrap();
    core
}

fn assert_assembly_rejected(core: &mut RendererCore, expected: &str) {
    let id = core.scene_id().map(str::to_owned);
    let paints = core.scene().unwrap().paints.clone();
    let flags = core.scene().unwrap().flags.clone();
    let diagnostics = core.unknown_motions().to_vec();
    match core.assemble_scene(3168, 3168, false, 0.0) {
        Err(error) => assert!(error.to_string().contains(expected), "{error}"),
        Ok(_) => panic!("live block assembly accepted missing {expected}"),
    }
    assert_eq!(core.scene_id(), id.as_deref());
    assert_eq!(core.scene().unwrap().paints, paints);
    assert_eq!(core.scene().unwrap().flags, flags);
    assert_eq!(core.unknown_motions(), diagnostics);
}

#[test]
fn published_floor_definitions_retain_source_counts() {
    let defs = FloorDefs::from_chunks(&common::read_asset("terrain/floors.bin")).unwrap();
    assert_eq!((defs.underlays.len(), defs.overlays.len()), (251, 643));
}

#[test]
fn floor_record_remainders_and_duplicate_ids_are_rejected() {
    let mut underlay_remainder = UNDERLAY.to_vec();
    underlay_remainder.push(17);
    let mut overlay_remainder = OVERLAY.to_vec();
    overlay_remainder.push(17);
    for (name, underlays, overlays) in [
        ("FUND remainder", underlay_remainder, OVERLAY.to_vec()),
        ("FOVL remainder", UNDERLAY.to_vec(), overlay_remainder),
        ("duplicate FUND", UNDERLAY.repeat(2), OVERLAY.to_vec()),
        ("duplicate FOVL", UNDERLAY.to_vec(), OVERLAY.repeat(2)),
    ] {
        assert!(
            FloorDefs::from_chunks(&floors(&underlays, &overlays)).is_err(),
            "accepted {name}"
        );
    }
}

#[test]
fn invalid_floor_values_are_rejected() {
    for (index, value) in [(0, -1), (1, i32::MAX), (2, 256), (3, -1), (4, 0)] {
        let mut underlay = UNDERLAY;
        underlay[index] = value;
        assert!(
            FloorDefs::from_chunks(&floors(&underlay, &OVERLAY)).is_err(),
            "accepted FUND[{index}]={value}"
        );
    }
    for (index, value) in [
        (0, -1),
        (1, -2),
        (2, 0x1000000),
        (3, i32::MAX),
        (4, 256),
        (5, -1),
        (6, -2),
        (7, i32::MIN),
        (8, 256),
        (9, -1),
    ] {
        let mut overlay = OVERLAY;
        overlay[index] = value;
        assert!(
            FloorDefs::from_chunks(&floors(&UNDERLAY, &overlay)).is_err(),
            "accepted FOVL[{index}]={value}"
        );
    }
}

#[test]
fn invalid_floor_reload_preserves_previous_definitions() {
    let mut core = core_with_previous_scene(Some(&raw_fields(1, 0)));
    core.load_floor_defs(&floors(&UNDERLAY, &OVERLAY)).unwrap();
    let mut truncated = UNDERLAY.to_vec();
    truncated.push(0);
    assert!(core.load_floor_defs(&floors(&truncated, &OVERLAY)).is_err());
    let defs = core.floor_defs().unwrap();
    assert_eq!(defs.underlays.len(), 1);
    assert_eq!(defs.underlays[&0].hue_multiplier, 64);
    assert_eq!(core.scene_id(), Some("previous-source-scene"));
}

#[test]
fn raw_terrain_requires_both_new_chunks() {
    let fields = raw_fields(1, 0);
    assert!(parse_raw(None, None).unwrap().is_none());
    assert!(parse_raw(Some(&fields), Some(&[])).unwrap().is_some());
    assert!(
        parse_raw(Some(&fields), None).is_err(),
        "BTER without BSHD silently removed all scenery shadows"
    );
    assert!(
        parse_raw(None, Some(&[])).is_err(),
        "BSHD without BTER was ignored"
    );
}

#[test]
fn raw_terrain_values_are_checked_before_narrowing_or_indexing() {
    for (index, value) in [
        (0, 65537),
        (1, i32::MIN),
        (2, 12),
        (2, -1),
        (3, 4),
        (3, -1),
        (4, 256),
        (5, i32::MAX),
    ] {
        let mut fields = raw_fields(1, 0);
        fields[index] = value;
        assert!(
            parse_raw(Some(&fields), Some(&[])).is_err(),
            "accepted BTER[{index}]={value}"
        );
    }
}

#[test]
fn invalid_shadow_records_are_rejected() {
    let fields = raw_fields(1, 0);
    for shadows in [
        vec![0],
        vec![-1, 0, 0, 0, 0, 1],
        vec![4, 0, 0, 0, 0, 1],
        vec![0, 64, 0, 0, 0, 1],
        vec![0, 0, -1, 0, 0, 1],
        vec![0, 0, 0, i32::MAX, 0, 1],
        vec![0, 0, 0, 0, i32::MIN, 1],
        vec![0, 0, 0, 0, 0, 256],
    ] {
        assert!(
            parse_raw(Some(&fields), Some(&shadows)).is_err(),
            "accepted BSHD {shadows:?}"
        );
    }
}

#[test]
fn live_assembly_requires_floor_definitions() {
    let mut core = core_with_previous_scene(Some(&raw_fields(1, 0)));
    assert_assembly_rejected(&mut core, "floor");
}

#[test]
fn live_assembly_requires_raw_terrain() {
    let mut core = core_with_previous_scene(None);
    core.load_floor_defs(&floors(&UNDERLAY, &OVERLAY)).unwrap();
    assert_assembly_rejected(&mut core, "BTER");
}

#[test]
fn live_assembly_requires_every_referenced_floor() {
    let mut underlay = UNDERLAY;
    underlay[0] = 9;
    let mut core = core_with_previous_scene(Some(&raw_fields(1, 0)));
    core.load_floor_defs(&floors(&underlay, &OVERLAY)).unwrap();
    assert_assembly_rejected(&mut core, "underlay");

    let mut overlay = OVERLAY;
    overlay[0] = 9;
    let mut core = core_with_previous_scene(Some(&raw_fields(1, 1)));
    core.load_floor_defs(&floors(&UNDERLAY, &overlay)).unwrap();
    assert_assembly_rejected(&mut core, "overlay");
}

#[test]
fn complete_terrain_inputs_build_a_live_scene() {
    let mut core = core_with_previous_scene(Some(&raw_fields(1, 0)));
    core.load_floor_defs(&floors(&UNDERLAY, &OVERLAY)).unwrap();
    core.assemble_scene(3168, 3168, false, 0.0).unwrap();
    let stats = core.terrain_rebuilt().unwrap();
    assert_eq!((stats.paints, stats.tile_models), (1, 0));
    assert_eq!((stats.missing_underlays, stats.missing_overlays), (0, 0));
    assert_eq!(core.scene_id(), Some("blocks@3168,3168"));
}
