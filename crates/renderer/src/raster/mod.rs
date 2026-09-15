//! Triangle command stream and the exact software rasterizer that defines its meaning.
//!
//! The CPU side of the renderer (model/scene traversal) emits [`Tri`] commands in the
//! original painter's order. [`software::Software`] executes them with the original integer
//! scanline algorithms and is the reference for the GPU compute rasterizer.

pub mod software;

/// Rasterizer state (`fd`): target size, projection center, zoom, clip rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RasterState {
    /// `ak`/`av`: target width/height in pixels.
    pub width: i32,
    pub height: i32,
    /// `ar`/`aw`: projection center.
    pub center_x: i32,
    pub center_y: i32,
    /// `au`: projection zoom (512 default, 1024 model fixtures, viewport-derived for scenes).
    pub zoom: i32,
    /// Row addressing: `bn[y] = origin + y * stride`.
    pub stride: i32,
    pub origin: i32,
    /// `ad`: 4-pixel Gouraud color banding (the stock client default is `true`).
    pub low_detail: bool,
}

impl RasterState {
    pub fn new(width: i32, height: i32, zoom: i32) -> Self {
        Self {
            width,
            height,
            center_x: width / 2,
            center_y: height / 2,
            zoom,
            stride: width,
            origin: 0,
            low_detail: true,
        }
    }

    /// `fh.ai`: viewport rectangle inside a larger target.
    pub fn with_viewport(target_stride: i32, x: i32, y: i32, width: i32, height: i32, zoom: i32) -> Self {
        Self {
            width,
            height,
            center_x: width / 2,
            center_y: height / 2,
            zoom,
            stride: target_stride,
            origin: y * target_stride + x,
            low_detail: true,
        }
    }

    /// `at`: negative clip x (`-center_x`).
    pub fn clip_neg_x(&self) -> i32 {
        -self.center_x
    }
    /// `an`: positive clip x (`width - center_x`).
    pub fn clip_pos_x(&self) -> i32 {
        self.width - self.center_x
    }
    /// `am`.
    pub fn clip_neg_y(&self) -> i32 {
        -self.center_y
    }
    /// `ah`.
    pub fn clip_pos_y(&self) -> i32 {
        self.height - self.center_y
    }
    #[inline]
    pub fn row_offset(&self, y: i32) -> i32 {
        self.origin.wrapping_add(y.wrapping_mul(self.stride))
    }
}

/// Per-triangle fill kind with the exact integer inputs of the original routines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fill {
    /// `ft.ao`: HSL palette indices interpolated per scanline.
    Gouraud { colors: [i32; 3] },
    /// `ft.al`: single RGB.
    Flat { rgb: i32 },
    /// `ft.aj` (model faces) / `ft.ay` (tile paints): perspective texture plane from camera-space
    /// P/M/N vertices plus HSL shade per vertex.
    Textured {
        colors: [i32; 3],
        px: [i32; 3],
        py: [i32; 3],
        pz: [i32; 3],
        texture: i32,
        /// `true` for the model routine (`aj`/`bq`), `false` for the tile routine (`ay`/`bf`).
        model_variant: bool,
    },
}

/// One original triangle draw: screen coordinates already truncated to int (`(int)float`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tri {
    pub y: [i32; 3],
    pub x: [i32; 3],
    pub fill: Fill,
    /// `fd.ap`: 0 opaque, otherwise source alpha (destination weight).
    pub alpha: i32,
    /// `fd.aq`: clamp spans horizontally to `[0, width)`.
    pub clip_x: bool,
    /// Opaque identity for picking (entity/tile hash index); 0 when none.
    pub pick: u32,
}

/// Aggregate counts for frame records.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DrawStats {
    pub triangles: usize,
    pub gouraud: usize,
    pub flat: usize,
    pub textured: usize,
    pub alpha_blended: usize,
}

impl DrawStats {
    pub fn count(&mut self, tri: &Tri) {
        self.triangles += 1;
        match tri.fill {
            Fill::Gouraud { .. } => self.gouraud += 1,
            Fill::Flat { .. } => self.flat += 1,
            Fill::Textured { .. } => self.textured += 1,
        }
        if tri.alpha != 0 {
            self.alpha_blended += 1;
        }
    }
}
