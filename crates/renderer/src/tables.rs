//! Original trigonometric and reciprocal lookup tables.
//!
//! The pinned runtime binds RuneLite's `Perspective` tables: 2048 units per turn for
//! legacy model rotation (`UNIT = 0.0030679615`, a truncated pi/1024) and 16384 units
//! per turn for the scene camera (`UNIT14 = 3.834951969714103E-4`). Integer tables are
//! `(int)(65536.0 * f)` and float tables are `(float) f` of the same double value.

use std::sync::OnceLock;

pub const UNIT_2048: f64 = 0.0030679615;
pub const UNIT_16384: f64 = 3.834951969714103E-4;

pub struct Tables {
    pub sin2048: Vec<i32>,
    pub cos2048: Vec<i32>,
    pub sinf2048: Vec<f32>,
    pub cosf2048: Vec<f32>,
    pub sin16384: Vec<i32>,
    pub cos16384: Vec<i32>,
    pub sinf16384: Vec<f32>,
    pub cosf16384: Vec<f32>,
    /// `fh.ag[i] = 65536 / i` for `i >= 1` (index 0 is 0); used by near-plane clipping.
    pub reciprocal16: Vec<i32>,
    /// `fh.ab[i] = 32768 / i` for `i >= 1` (512 entries).
    pub reciprocal15: Vec<i32>,
}

impl Tables {
    fn build() -> Self {
        let mut t = Tables {
            sin2048: vec![0; 2048],
            cos2048: vec![0; 2048],
            sinf2048: vec![0.0; 2048],
            cosf2048: vec![0.0; 2048],
            sin16384: vec![0; 16384],
            cos16384: vec![0; 16384],
            sinf16384: vec![0.0; 16384],
            cosf16384: vec![0.0; 16384],
            reciprocal16: vec![0; 2048],
            reciprocal15: vec![0; 512],
        };
        for i in 0..2048usize {
            let s = ((i as f64) * UNIT_2048).sin();
            let c = ((i as f64) * UNIT_2048).cos();
            t.sinf2048[i] = s as f32;
            t.cosf2048[i] = c as f32;
            t.sin2048[i] = (65536.0 * s) as i32;
            t.cos2048[i] = (65536.0 * c) as i32;
        }
        for i in 0..16384usize {
            let s = ((i as f64) * UNIT_16384).sin();
            let c = ((i as f64) * UNIT_16384).cos();
            t.sinf16384[i] = s as f32;
            t.cosf16384[i] = c as f32;
            t.sin16384[i] = (65536.0 * s) as i32;
            t.cos16384[i] = (65536.0 * c) as i32;
        }
        for i in 1..2048usize {
            t.reciprocal16[i] = 65536 / i as i32;
        }
        for i in 1..512usize {
            t.reciprocal15[i] = 32768 / i as i32;
        }
        t
    }
}

static TABLES: OnceLock<Tables> = OnceLock::new();

pub fn tables() -> &'static Tables {
    TABLES.get_or_init(Tables::build)
}

/// Serializes the tables in the same `CSRC` layout as the Java exporter's `tables.bin`, so
/// tests can compare hashes against the manifest without shipping the dump at runtime.
pub fn tables_csrc_bytes() -> Vec<u8> {
    let t = tables();
    let mut out = Vec::new();
    out.extend_from_slice(b"CSRC");
    out.extend_from_slice(&1u32.to_le_bytes());
    fn chunk_i32(out: &mut Vec<u8>, tag: &[u8; 4], values: &[i32]) {
        out.extend_from_slice(tag);
        out.extend_from_slice(&((values.len() * 4) as u32).to_le_bytes());
        for v in values {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    fn chunk_f32(out: &mut Vec<u8>, tag: &[u8; 4], values: &[f32]) {
        out.extend_from_slice(tag);
        out.extend_from_slice(&((values.len() * 4) as u32).to_le_bytes());
        for v in values {
            out.extend_from_slice(&v.to_bits().to_le_bytes());
        }
    }
    chunk_i32(&mut out, b"SIN2", &t.sin2048);
    chunk_i32(&mut out, b"COS2", &t.cos2048);
    chunk_f32(&mut out, b"SNF2", &t.sinf2048);
    chunk_f32(&mut out, b"CSF2", &t.cosf2048);
    chunk_i32(&mut out, b"SN14", &t.sin16384);
    chunk_i32(&mut out, b"CS14", &t.cos16384);
    chunk_f32(&mut out, b"SF14", &t.sinf16384);
    chunk_f32(&mut out, b"CF14", &t.cosf16384);
    chunk_i32(&mut out, b"RCP1", &t.reciprocal16);
    out
}
