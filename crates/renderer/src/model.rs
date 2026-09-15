//! Lit source model as consumed by the original draw path (`fx`).
//!
//! Vertices are floats (the runtime stores animated/scaled vertices as `float`), face colors
//! are 16-bit HSL palette indices already lit by the original lighting code, and the bounds
//! are the exact integer cylinder/sphere values the original culling and depth bucketing use.

use crate::chunk::Chunks;
use crate::error::RenderError;

#[derive(Clone, Debug, Default)]
pub struct Model {
    pub vertex_count: usize,
    pub face_count: usize,
    pub xs: Vec<f32>,
    pub ys: Vec<f32>,
    pub zs: Vec<f32>,
    pub face_a: Vec<i32>,
    pub face_b: Vec<i32>,
    pub face_c: Vec<i32>,
    /// Face colors 1/2/3. `color_c == -1` means flat shaded; `color_c == -2` hides the face.
    pub color_a: Vec<i32>,
    pub color_b: Vec<i32>,
    pub color_c: Vec<i32>,
    pub textures: Option<Vec<i16>>,
    /// Texture coordinate group per face (`cp`), -1 when the face's own vertices form the plane.
    pub texture_coords: Option<Vec<i8>>,
    pub tex_p: Vec<i32>,
    pub tex_m: Vec<i32>,
    pub tex_n: Vec<i32>,
    pub priorities: Option<Vec<i8>>,
    pub alphas: Option<Vec<i8>>,
    /// `cl`: per-face depth bias (multiplied by 2 in the draw path).
    pub bias: Option<Vec<i8>>,
    /// `cm`: faces below this index use the color-override fill variants.
    pub override_faces: i32,
    /// `cd`: model-level transparency override byte.
    pub transparency: i32,
    /// `cc`: original "interactable/click-checked" flag (affects hover resolution only).
    pub clickable: bool,
    /// `ce.as`: render mode (0 default, 2 = always draw every face regardless of alpha pass).
    pub render_mode: i32,
    pub bounds: Bounds,
    pub vertex_groups: Option<Vec<Vec<i32>>>,
    pub face_groups: Option<Vec<Vec<i32>>>,
    pub vertex_groups_alt: Option<Vec<Vec<i32>>>,
    pub face_groups_alt: Option<Vec<Vec<i32>>>,
}

/// Original bounds computed by `fx.et` (cylinder, `cf == 1`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Bounds {
    /// `cg`: depth-bucket offset (`ceil(sqrt(radius^2 + height^2))`).
    pub bucket_offset: i32,
    /// `ch`: horizontal radius `ceil(sqrt(max(x^2 + z^2)))`.
    pub radius: i32,
    /// `cn`: `ceil(max(y))` (lowest point, +Y is down).
    pub bottom: i32,
    /// decoded `ed`: `ceil(max(-y))` (height above origin).
    pub height: i32,
    /// `cz`: total depth bucket range.
    pub bucket_range: i32,
}

impl Model {
    pub fn from_chunks(data: &[u8]) -> Result<Self, RenderError> {
        let chunks = Chunks::parse(data)?;
        let header = chunks.ints("MDHD")?;
        if header.len() < 8 {
            return Err(RenderError::Format("model header".into()));
        }
        let vertex_count = header[0] as usize;
        let face_count = header[1] as usize;
        let tex_count = header[2] as usize;
        let b = chunks.ints("BNDC")?;
        let bounds = Bounds { bucket_offset: b[0], radius: b[1], bottom: b[2], height: b[3], bucket_range: b[4] };
        let mut model = Model {
            vertex_count,
            face_count,
            xs: chunks.floats("VRTX")?,
            ys: chunks.floats("VRTY")?,
            zs: chunks.floats("VRTZ")?,
            face_a: chunks.ints("FIDA")?,
            face_b: chunks.ints("FIDB")?,
            face_c: chunks.ints("FIDC")?,
            color_a: chunks.ints("FCLA")?,
            color_b: chunks.ints("FCLB")?,
            color_c: chunks.ints("FCLC")?,
            textures: chunks.shorts_opt("FTEX")?,
            texture_coords: chunks.bytes_opt("FTXC")?,
            tex_p: chunks.ints_opt("TXPI")?.unwrap_or_default(),
            tex_m: chunks.ints_opt("TXMI")?.unwrap_or_default(),
            tex_n: chunks.ints_opt("TXNI")?.unwrap_or_default(),
            priorities: chunks.bytes_opt("FPRI")?,
            alphas: chunks.bytes_opt("FALP")?,
            bias: chunks.bytes_opt("FBIA")?,
            override_faces: header[3],
            transparency: header[4],
            clickable: header[5] != 0,
            render_mode: header[6],
            bounds,
            vertex_groups: chunks.jagged_opt("VGRP")?,
            face_groups: chunks.jagged_opt("FGRP")?,
            vertex_groups_alt: chunks.jagged_opt("VGR2")?,
            face_groups_alt: chunks.jagged_opt("FGR2")?,
        };
        model.validate(tex_count)?;
        Ok(model)
    }

    fn validate(&mut self, tex_count: usize) -> Result<(), RenderError> {
        let v = self.vertex_count;
        let f = self.face_count;
        if v == 0 || f == 0 {
            return Err(RenderError::InvalidAsset("empty model".into()));
        }
        if self.xs.len() < v || self.ys.len() < v || self.zs.len() < v {
            return Err(RenderError::InvalidAsset("vertex arrays shorter than vertex count".into()));
        }
        for arr in [&self.face_a, &self.face_b, &self.face_c] {
            if arr.len() < f {
                return Err(RenderError::InvalidAsset("face index arrays shorter than face count".into()));
            }
            if arr[..f].iter().any(|&i| i < 0 || i as usize >= v) {
                return Err(RenderError::InvalidAsset("face index out of range".into()));
            }
        }
        for arr in [&self.color_a, &self.color_b, &self.color_c] {
            if arr.len() < f {
                return Err(RenderError::InvalidAsset("face color arrays shorter than face count".into()));
            }
        }
        for arr in [&self.tex_p, &self.tex_m, &self.tex_n] {
            if arr.len() < tex_count {
                return Err(RenderError::InvalidAsset("texture index arrays shorter than texture count".into()));
            }
            if arr[..tex_count].iter().any(|&i| i < 0 || i as usize >= v) {
                return Err(RenderError::InvalidAsset("texture vertex index out of range".into()));
            }
        }
        if let Some(coords) = &self.texture_coords {
            if coords.len() < f {
                return Err(RenderError::InvalidAsset("texture coord array short".into()));
            }
            for &c in &coords[..f] {
                if c != -1 && (c as u8) as usize >= tex_count {
                    return Err(RenderError::InvalidAsset("texture coord group out of range".into()));
                }
            }
        }
        for arr in [&self.textures.as_deref().map(|_| ()), &None] {
            let _ = arr;
        }
        if let Some(t) = &self.textures {
            if t.len() < f {
                return Err(RenderError::InvalidAsset("texture array short".into()));
            }
        }
        for opt in [&self.priorities, &self.alphas, &self.bias] {
            if let Some(a) = opt {
                if a.len() < f {
                    return Err(RenderError::InvalidAsset("per-face byte array short".into()));
                }
            }
        }
        if let Some(groups) = &self.vertex_groups {
            for g in groups {
                if g.iter().any(|&i| i < 0 || i as usize >= v) {
                    return Err(RenderError::InvalidAsset("vertex group index out of range".into()));
                }
            }
        }
        if let Some(groups) = &self.face_groups {
            for g in groups {
                if g.iter().any(|&i| i < 0 || i as usize >= f) {
                    return Err(RenderError::InvalidAsset("face group index out of range".into()));
                }
            }
        }
        Ok(())
    }

    /// Exact port of `fx.et`: cylinder bounds used by the scene draw path.
    pub fn compute_cylinder_bounds(&mut self) {
        let mut top = 0.0f32;
        let mut bottom = 0.0f32;
        let mut radius_sq = 0.0f32;
        for i in 0..self.vertex_count {
            let x = self.xs[i];
            let y = self.ys[i];
            let z = self.zs[i];
            if -y > top {
                top = -y;
            }
            if y > bottom {
                bottom = y;
            }
            let r = x * x + z * z;
            if r > radius_sq {
                radius_sq = r;
            }
        }
        let bottom_i = f64::from(bottom).ceil() as i32;
        let height_i = f64::from(top).ceil() as i32;
        let radius_i = f64::from(radius_sq).sqrt().ceil() as i32;
        let bucket_offset = f64::from(radius_i * radius_i + height_i * height_i).sqrt().ceil() as i32;
        let bucket_range = bucket_offset + f64::from(radius_i * radius_i + bottom_i * bottom_i).sqrt().ceil() as i32;
        self.bounds = Bounds { bucket_offset, radius: radius_i, bottom: bottom_i, height: height_i, bucket_range };
    }

    /// Exact port of `fx.bd`: sphere bounds used by the legacy `drawFrustum` path (`cf == 2`).
    pub fn sphere_bounds(&self) -> (i32, i32) {
        let mut max = 0.0f32;
        for i in 0..self.vertex_count {
            let x = self.xs[i];
            let z = self.zs[i];
            let y = self.ys[i];
            let d = x * x + z * z + y * y;
            if d > max {
                max = d;
            }
        }
        let radius = f64::from(max).sqrt().ceil() as i32;
        (radius, radius + radius)
    }

    /// Exact port of `fx.jp`/`er.qw`: `v = scale * v / 128.0f` per axis in float.
    pub fn scale(&mut self, sx: i32, sy: i32, sz: i32) {
        for i in 0..self.vertex_count {
            self.xs[i] = sx as f32 * self.xs[i] / 128.0;
            self.ys[i] = sy as f32 * self.ys[i] / 128.0;
            self.zs[i] = sz as f32 * self.zs[i] / 128.0;
        }
        self.compute_cylinder_bounds();
    }
}
