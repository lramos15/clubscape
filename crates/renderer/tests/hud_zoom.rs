//! The full-HUD zoom port against the original client's own values (`hud/zoom-table.json`,
//! `export.py --profile zoom`: `client.fk` after the Resizable-Classic layout for each canvas).
mod common;

use clubscape_renderer::core::{
    FULL_HUD_ZOOM_PARAMETERS, VIEWPORT_ONLY_ZOOM_PARAMETERS, hud_viewport,
};
use common::read_asset;

#[test]
fn full_hud_zoom_port_matches_the_native_probe_table() {
    let manifest: serde_json::Value = serde_json::from_slice(&read_asset("manifest.json")).unwrap();
    let file = manifest["hud_zoom"]["file"]
        .as_str()
        .expect("manifest hud_zoom (export.py --profile zoom)");
    let table: serde_json::Value = serde_json::from_slice(&read_asset(file)).unwrap();
    let params = &table["parameters_after_layout"];
    assert_eq!(params["fy"], 127);
    assert_eq!(params["fg"], 127);
    assert_eq!(params["fu"], 1);
    assert_eq!(params["fz"], 32767);
    assert_eq!(params["fh"], 1);
    assert_eq!(params["fq"], 32767);
    let defaults = &table["defaults_before_layout"];
    assert_eq!(defaults["fy"], 256);
    assert_eq!(defaults["fg"], 205);
    let samples = table["samples"].as_array().unwrap();
    assert!(samples.len() >= 12, "{} native samples", samples.len());
    let mut covered = [false; 4];
    for sample in samples {
        let canvas = sample["canvas"].as_array().unwrap();
        let (w, h) = (
            canvas[0].as_i64().unwrap() as i32,
            canvas[1].as_i64().unwrap() as i32,
        );
        let viewport = sample["viewport"].as_array().unwrap();
        assert_eq!(
            viewport[0].as_i64().unwrap() as i32,
            w,
            "viewport follows the canvas"
        );
        assert_eq!(viewport[1].as_i64().unwrap() as i32, h);
        let native = sample["zoom"].as_i64().unwrap() as i32;
        let ported = hud_viewport(w, h, FULL_HUD_ZOOM_PARAMETERS);
        assert_eq!(
            ported.zoom, native,
            "{w}x{h}: native zoom {native}, port {}",
            ported.zoom
        );
        assert_eq!(
            (ported.x, ported.y, ported.width, ported.height),
            (0, 0, w, h)
        );
        match (w, h) {
            (1024, 768) => covered[0] = true,
            (1920, 1080) => covered[1] = true,
            (2560, 1440) => covered[2] = true,
            (1280, 720) => covered[3] = true,
            _ => {}
        }
    }
    assert!(
        covered.iter().all(|c| *c),
        "approved range corners sampled: {covered:?}"
    );
    assert_eq!(hud_viewport(1920, 1080, FULL_HUD_ZOOM_PARAMETERS).zoom, 410);
    // The viewport-only fixtures keep the stock parameters: 662 at 1080 px, 471 at 768 px.
    assert_eq!(
        hud_viewport(1920, 1080, VIEWPORT_ONLY_ZOOM_PARAMETERS).zoom,
        662
    );
    assert_eq!(
        hud_viewport(1024, 768, VIEWPORT_ONLY_ZOOM_PARAMETERS).zoom,
        471
    );
    assert_eq!(
        hud_viewport(2560, 1440, VIEWPORT_ONLY_ZOOM_PARAMETERS).zoom,
        883
    );
    // Interpolation band (334..434 px) and the below-334 hop are exercised too.
    assert_eq!(
        hud_viewport(600, 300, VIEWPORT_ONLY_ZOOM_PARAMETERS).zoom,
        300 * 256 / 334
    );
    assert_eq!(
        hud_viewport(600, 384, VIEWPORT_ONLY_ZOOM_PARAMETERS).zoom,
        (384.0 * f64::from((205 - 256) * 50 / 100 + 256) / 334.0) as i32
    );
}
