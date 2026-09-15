//! Exact CPU port of the original software rasterizer fills (`ft`): Gouraud (`ao`/`jm`),
//! flat (`al`/`px`), model-textured (`aj`/`bq`) and tile-textured (`ay`/`bf`).
//!
//! The Java code is transliterated with wrapping 32-bit integer semantics. Variable names
//! follow the decompiled originals where the structure matters for exactness. Out-of-range
//! palette/texture/pixel accesses abort the current draw (`Err(Abort)`), mirroring the
//! original `ArrayIndexOutOfBoundsException` swallowed by the model draw loop.

#![allow(clippy::too_many_arguments, clippy::many_single_char_names, clippy::cognitive_complexity)]

use std::num::Wrapping;

use super::{Fill, RasterState, Tri};

type W = Wrapping<i32>;

#[inline]
fn w(v: i32) -> W {
    Wrapping(v)
}

/// The draw aborted like the original swallowed exception would.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Abort;

/// Provides texel arrays for textured fills (`fg`/`ec`).
pub trait TextureSource {
    /// `ec.ae(id)`: 128x128 texel array, or `None` if the texture is unavailable.
    fn texels(&self, id: i32) -> Option<&[i32]>;
    /// `ec.ag(id)`: `true` writes every texel; `false` skips texel value 0.
    fn opaque(&self, id: i32) -> bool;
    /// `ec.ab(id)`: average HSL-ish color used when the texture is unavailable.
    fn average(&self, id: i32) -> i32;
}

/// A texture source with no textures (every textured face falls back to flat Gouraud).
pub struct NoTextures;

impl TextureSource for NoTextures {
    fn texels(&self, _id: i32) -> Option<&[i32]> {
        None
    }
    fn opaque(&self, _id: i32) -> bool {
        false
    }
    fn average(&self, _id: i32) -> i32 {
        0
    }
}

pub struct Software<'a, T: TextureSource> {
    pub state: RasterState,
    pub pixels: &'a mut [i32],
    pub palette: &'a [i32],
    pub textures: &'a T,
    clip_x: bool,
    alpha: i32,
    tex_opaque: bool,
}

impl<'a, T: TextureSource> Software<'a, T> {
    pub fn new(state: RasterState, pixels: &'a mut [i32], palette: &'a [i32], textures: &'a T) -> Self {
        Self { state, pixels, palette, textures, clip_x: false, alpha: 0, tex_opaque: false }
    }

    /// Executes one triangle command exactly as the original would.
    pub fn draw(&mut self, tri: &Tri) -> Result<(), Abort> {
        self.clip_x = tri.clip_x;
        self.alpha = tri.alpha;
        let [y1, y2, y3] = tri.y;
        let [x1, x2, x3] = tri.x;
        match tri.fill {
            Fill::Gouraud { colors: [c1, c2, c3] } => self.gouraud(y1, y2, y3, x1, x2, x3, c1, c2, c3),
            Fill::Flat { rgb } => self.flat(y1, y2, y3, x1, x2, x3, rgb),
            Fill::Textured { colors: [c1, c2, c3], px, py, pz, texture, model_variant } => {
                if model_variant {
                    self.textured_model(
                        y1, y2, y3, x1, x2, x3, c1, c2, c3, px[0], px[1], px[2], py[0], py[1], py[2], pz[0], pz[1], pz[2],
                        texture,
                    )
                } else {
                    self.textured_tile(
                        y1, y2, y3, x1, x2, x3, c1, c2, c3, px[0], px[1], px[2], py[0], py[1], py[2], pz[0], pz[1], pz[2],
                        texture,
                    )
                }
            }
        }
    }

    #[inline]
    fn put(&mut self, index: i32, value: i32) -> Result<(), Abort> {
        match self.pixels.get_mut(index as usize) {
            Some(slot) if index >= 0 => {
                *slot = value;
                Ok(())
            }
            _ => Err(Abort),
        }
    }

    #[inline]
    fn get(&self, index: i32) -> Result<i32, Abort> {
        if index < 0 {
            return Err(Abort);
        }
        self.pixels.get(index as usize).copied().ok_or(Abort)
    }

    #[inline]
    fn pal(&self, index: i32) -> Result<i32, Abort> {
        if index < 0 {
            return Err(Abort);
        }
        self.palette.get(index as usize).copied().ok_or(Abort)
    }

    // ---------------------------------------------------------------- Gouraud (ft.ao / jm)

    /// `ft.ao`: arguments are (y1,y2,y3,x1,x2,x3,c1,c2,c3) already truncated to int.
    pub fn gouraud(&mut self, y1: i32, y2: i32, y3: i32, x1: i32, x2: i32, x3: i32, c1: i32, c2: i32, c3: i32) -> Result<(), Abort> {
        let (var13, var14, var15) = (w(x1), w(x2), w(x3));
        let (mut var16, mut var17, mut var18) = (w(y1), w(y2), w(y3));
        let (mut var10, mut var11, mut var12) = (w(c1), w(c2), w(c3));
        let var19 = var14 - var13;
        let var20 = var17 - var16;
        let var21 = var15 - var13;
        let var22 = var18 - var16;
        let var23 = var11 - var10;
        let var24 = var12 - var10;
        let var25 = if var18 != var17 { ((var15 - var14) << 14) / (var18 - var17) } else { w(0) };
        let var26 = if var17 != var16 { (var19 << 14) / var20 } else { w(0) };
        let var27 = if var18 != var16 { (var21 << 14) / var22 } else { w(0) };
        let var28 = var19 * var22 - var21 * var20;
        if var28 == w(0) {
            return Ok(());
        }
        let var29 = ((var23 * var22 - var24 * var20) << 8) / var28;
        let var30 = ((var24 * var19 - var23 * var21) << 8) / var28;
        let var32 = w(self.state.height);
        let stride = w(self.state.stride);
        let mut var13 = var13;
        let mut var14 = var14;
        let mut var15 = var15;
        if var16 <= var17 && var16 <= var18 {
            if var16 < var32 {
                if var17 > var32 {
                    var17 = var32;
                }
                if var18 > var32 {
                    var18 = var32;
                }
                var10 = (var10 << 8) - var29 * var13 + var29;
                if var17 < var18 {
                    let mut var41 = var13 << 14;
                    var15 = var41;
                    if var16 < w(0) {
                        var15 -= var27 * var16;
                        var41 -= var26 * var16;
                        var10 -= var30 * var16;
                        var16 = w(0);
                    }
                    var14 <<= 14;
                    if var17 < w(0) {
                        var14 -= var25 * var17;
                        var17 = w(0);
                    }
                    if (var16 == var17 || var27 >= var26) && (var16 != var17 || var27 <= var25) {
                        var18 -= var17;
                        var17 -= var16;
                        let mut off = w(self.state.row_offset(var16.0));
                        while {
                            var17 -= 1;
                            var17 >= w(0)
                        } {
                            self.gouraud_scanline(off.0, (var41 >> 14).0, (var15 >> 14).0, var10.0, var29.0)?;
                            var15 += var27;
                            var41 += var26;
                            var10 += var30;
                            off += stride;
                        }
                        while {
                            var18 -= 1;
                            var18 >= w(0)
                        } {
                            self.gouraud_scanline(off.0, (var14 >> 14).0, (var15 >> 14).0, var10.0, var29.0)?;
                            var15 += var27;
                            var14 += var25;
                            var10 += var30;
                            off += stride;
                        }
                    } else {
                        var18 -= var17;
                        var17 -= var16;
                        let mut off = w(self.state.row_offset(var16.0));
                        while {
                            var17 -= 1;
                            var17 >= w(0)
                        } {
                            self.gouraud_scanline(off.0, (var15 >> 14).0, (var41 >> 14).0, var10.0, var29.0)?;
                            var15 += var27;
                            var41 += var26;
                            var10 += var30;
                            off += stride;
                        }
                        while {
                            var18 -= 1;
                            var18 >= w(0)
                        } {
                            self.gouraud_scanline(off.0, (var15 >> 14).0, (var14 >> 14).0, var10.0, var29.0)?;
                            var15 += var27;
                            var14 += var25;
                            var10 += var30;
                            off += stride;
                        }
                    }
                } else {
                    let mut var40 = var13 << 14;
                    var14 = var40;
                    if var16 < w(0) {
                        var14 -= var27 * var16;
                        var40 -= var26 * var16;
                        var10 -= var30 * var16;
                        var16 = w(0);
                    }
                    var15 <<= 14;
                    if var18 < w(0) {
                        var15 -= var25 * var18;
                        var18 = w(0);
                    }
                    if (var16 == var18 || var27 >= var26) && (var16 != var18 || var25 <= var26) {
                        var17 -= var18;
                        var18 -= var16;
                        let mut off = w(self.state.row_offset(var16.0));
                        while {
                            var18 -= 1;
                            var18 >= w(0)
                        } {
                            self.gouraud_scanline(off.0, (var40 >> 14).0, (var14 >> 14).0, var10.0, var29.0)?;
                            var14 += var27;
                            var40 += var26;
                            var10 += var30;
                            off += stride;
                        }
                        while {
                            var17 -= 1;
                            var17 >= w(0)
                        } {
                            self.gouraud_scanline(off.0, (var40 >> 14).0, (var15 >> 14).0, var10.0, var29.0)?;
                            var15 += var25;
                            var40 += var26;
                            var10 += var30;
                            off += stride;
                        }
                    } else {
                        var17 -= var18;
                        var18 -= var16;
                        let mut off = w(self.state.row_offset(var16.0));
                        while {
                            var18 -= 1;
                            var18 >= w(0)
                        } {
                            self.gouraud_scanline(off.0, (var14 >> 14).0, (var40 >> 14).0, var10.0, var29.0)?;
                            var14 += var27;
                            var40 += var26;
                            var10 += var30;
                            off += stride;
                        }
                        while {
                            var17 -= 1;
                            var17 >= w(0)
                        } {
                            self.gouraud_scanline(off.0, (var15 >> 14).0, (var40 >> 14).0, var10.0, var29.0)?;
                            var15 += var25;
                            var40 += var26;
                            var10 += var30;
                            off += stride;
                        }
                    }
                }
            }
        } else if var17 <= var18 {
            if var17 < var32 {
                if var18 > var32 {
                    var18 = var32;
                }
                if var16 > var32 {
                    var16 = var32;
                }
                var11 = (var11 << 8) - var29 * var14 + var29;
                if var18 < var16 {
                    let mut var45 = var14 << 14;
                    var13 = var45;
                    if var17 < w(0) {
                        var13 -= var26 * var17;
                        var45 -= var25 * var17;
                        var11 -= var30 * var17;
                        var17 = w(0);
                    }
                    var15 <<= 14;
                    if var18 < w(0) {
                        var15 -= var27 * var18;
                        var18 = w(0);
                    }
                    if (var17 == var18 || var26 >= var25) && (var17 != var18 || var26 <= var27) {
                        var16 -= var18;
                        var18 -= var17;
                        let mut off = w(self.state.row_offset(var17.0));
                        while {
                            var18 -= 1;
                            var18 >= w(0)
                        } {
                            self.gouraud_scanline(off.0, (var45 >> 14).0, (var13 >> 14).0, var11.0, var29.0)?;
                            var13 += var26;
                            var45 += var25;
                            var11 += var30;
                            off += stride;
                        }
                        while {
                            var16 -= 1;
                            var16 >= w(0)
                        } {
                            self.gouraud_scanline(off.0, (var15 >> 14).0, (var13 >> 14).0, var11.0, var29.0)?;
                            var13 += var26;
                            var15 += var27;
                            var11 += var30;
                            off += stride;
                        }
                    } else {
                        var16 -= var18;
                        var18 -= var17;
                        let mut off = w(self.state.row_offset(var17.0));
                        while {
                            var18 -= 1;
                            var18 >= w(0)
                        } {
                            self.gouraud_scanline(off.0, (var13 >> 14).0, (var45 >> 14).0, var11.0, var29.0)?;
                            var13 += var26;
                            var45 += var25;
                            var11 += var30;
                            off += stride;
                        }
                        while {
                            var16 -= 1;
                            var16 >= w(0)
                        } {
                            self.gouraud_scanline(off.0, (var13 >> 14).0, (var15 >> 14).0, var11.0, var29.0)?;
                            var13 += var26;
                            var15 += var27;
                            var11 += var30;
                            off += stride;
                        }
                    }
                } else {
                    let mut var44 = var14 << 14;
                    var15 = var44;
                    if var17 < w(0) {
                        var15 -= var26 * var17;
                        var44 -= var25 * var17;
                        var11 -= var30 * var17;
                        var17 = w(0);
                    }
                    var13 <<= 14;
                    if var16 < w(0) {
                        var13 -= var27 * var16;
                        var16 = w(0);
                    }
                    if var26 < var25 {
                        var18 -= var16;
                        var16 -= var17;
                        let mut off = w(self.state.row_offset(var17.0));
                        while {
                            var16 -= 1;
                            var16 >= w(0)
                        } {
                            self.gouraud_scanline(off.0, (var15 >> 14).0, (var44 >> 14).0, var11.0, var29.0)?;
                            var15 += var26;
                            var44 += var25;
                            var11 += var30;
                            off += stride;
                        }
                        while {
                            var18 -= 1;
                            var18 >= w(0)
                        } {
                            self.gouraud_scanline(off.0, (var13 >> 14).0, (var44 >> 14).0, var11.0, var29.0)?;
                            var13 += var27;
                            var44 += var25;
                            var11 += var30;
                            off += stride;
                        }
                    } else {
                        var18 -= var16;
                        var16 -= var17;
                        let mut off = w(self.state.row_offset(var17.0));
                        while {
                            var16 -= 1;
                            var16 >= w(0)
                        } {
                            self.gouraud_scanline(off.0, (var44 >> 14).0, (var15 >> 14).0, var11.0, var29.0)?;
                            var15 += var26;
                            var44 += var25;
                            var11 += var30;
                            off += stride;
                        }
                        while {
                            var18 -= 1;
                            var18 >= w(0)
                        } {
                            self.gouraud_scanline(off.0, (var44 >> 14).0, (var13 >> 14).0, var11.0, var29.0)?;
                            var13 += var27;
                            var44 += var25;
                            var11 += var30;
                            off += stride;
                        }
                    }
                }
            }
        } else if var18 < var32 {
            if var16 > var32 {
                var16 = var32;
            }
            if var17 > var32 {
                var17 = var32;
            }
            var12 = (var12 << 8) - var29 * var15 + var29;
            if var16 < var17 {
                let mut var49 = var15 << 14;
                var14 = var49;
                if var18 < w(0) {
                    var14 -= var25 * var18;
                    var49 -= var27 * var18;
                    var12 -= var30 * var18;
                    var18 = w(0);
                }
                var13 <<= 14;
                if var16 < w(0) {
                    var13 -= var26 * var16;
                    var16 = w(0);
                }
                if var25 < var27 {
                    var17 -= var16;
                    var16 -= var18;
                    let mut off = w(self.state.row_offset(var18.0));
                    while {
                        var16 -= 1;
                        var16 >= w(0)
                    } {
                        self.gouraud_scanline(off.0, (var14 >> 14).0, (var49 >> 14).0, var12.0, var29.0)?;
                        var14 += var25;
                        var49 += var27;
                        var12 += var30;
                        off += stride;
                    }
                    while {
                        var17 -= 1;
                        var17 >= w(0)
                    } {
                        self.gouraud_scanline(off.0, (var14 >> 14).0, (var13 >> 14).0, var12.0, var29.0)?;
                        var14 += var25;
                        var13 += var26;
                        var12 += var30;
                        off += stride;
                    }
                } else {
                    var17 -= var16;
                    var16 -= var18;
                    let mut off = w(self.state.row_offset(var18.0));
                    while {
                        var16 -= 1;
                        var16 >= w(0)
                    } {
                        self.gouraud_scanline(off.0, (var49 >> 14).0, (var14 >> 14).0, var12.0, var29.0)?;
                        var14 += var25;
                        var49 += var27;
                        var12 += var30;
                        off += stride;
                    }
                    while {
                        var17 -= 1;
                        var17 >= w(0)
                    } {
                        self.gouraud_scanline(off.0, (var13 >> 14).0, (var14 >> 14).0, var12.0, var29.0)?;
                        var14 += var25;
                        var13 += var26;
                        var12 += var30;
                        off += stride;
                    }
                }
            } else {
                let mut var48 = var15 << 14;
                var13 = var48;
                if var18 < w(0) {
                    var13 -= var25 * var18;
                    var48 -= var27 * var18;
                    var12 -= var30 * var18;
                    var18 = w(0);
                }
                var14 <<= 14;
                if var17 < w(0) {
                    var14 -= var26 * var17;
                    var17 = w(0);
                }
                if var25 < var27 {
                    var16 -= var17;
                    var17 -= var18;
                    let mut off = w(self.state.row_offset(var18.0));
                    while {
                        var17 -= 1;
                        var17 >= w(0)
                    } {
                        self.gouraud_scanline(off.0, (var13 >> 14).0, (var48 >> 14).0, var12.0, var29.0)?;
                        var13 += var25;
                        var48 += var27;
                        var12 += var30;
                        off += stride;
                    }
                    while {
                        var16 -= 1;
                        var16 >= w(0)
                    } {
                        self.gouraud_scanline(off.0, (var14 >> 14).0, (var48 >> 14).0, var12.0, var29.0)?;
                        var14 += var26;
                        var48 += var27;
                        var12 += var30;
                        off += stride;
                    }
                } else {
                    var16 -= var17;
                    var17 -= var18;
                    let mut off = w(self.state.row_offset(var18.0));
                    while {
                        var17 -= 1;
                        var17 >= w(0)
                    } {
                        self.gouraud_scanline(off.0, (var48 >> 14).0, (var13 >> 14).0, var12.0, var29.0)?;
                        var13 += var25;
                        var48 += var27;
                        var12 += var30;
                        off += stride;
                    }
                    while {
                        var16 -= 1;
                        var16 >= w(0)
                    } {
                        self.gouraud_scanline(off.0, (var48 >> 14).0, (var14 >> 14).0, var12.0, var29.0)?;
                        var14 += var26;
                        var48 += var27;
                        var12 += var30;
                        off += stride;
                    }
                }
            }
        }
        Ok(())
    }

    /// `ft.jm`: Gouraud scanline with palette lookup, 4-pixel banding and alpha blend.
    fn gouraud_scanline(&mut self, offset: i32, x_start: i32, x_end: i32, color: i32, step: i32) -> Result<(), Abort> {
        let mut var5 = x_start;
        let mut var6 = x_end;
        if self.clip_x {
            if var6 > self.state.width {
                var6 = self.state.width;
            }
            if var5 < 0 {
                var5 = 0;
            }
        }
        if var5 >= var6 {
            return Ok(());
        }
        let mut var2 = w(offset) + w(var5);
        let mut var7 = w(color) + w(step) * w(var5);
        let mut var8 = w(step);
        let alpha = self.alpha;
        if self.state.low_detail {
            let mut var4 = (var6 - var5) >> 2;
            var8 <<= 2;
            if alpha == 0 {
                if var4 > 0 {
                    loop {
                        let idx = ((var7.0 & !(var7.0 >> 31)) >> 8) as i32;
                        let rgb = self.pal(idx)?;
                        var7 += var8;
                        for _ in 0..4 {
                            self.put(var2.0, rgb)?;
                            var2 += 1;
                        }
                        var4 -= 1;
                        if var4 <= 0 {
                            break;
                        }
                    }
                }
                var4 = (var6 - var5) & 3;
                if var4 > 0 {
                    let idx = (var7.0 & !(var7.0 >> 31)) >> 8;
                    let rgb = self.pal(idx)?;
                    loop {
                        self.put(var2.0, rgb)?;
                        var2 += 1;
                        var4 -= 1;
                        if var4 <= 0 {
                            break;
                        }
                    }
                }
            } else {
                let var37 = w(alpha);
                let var38 = w(256 - alpha);
                if var4 > 0 {
                    loop {
                        let idx = (var7.0 & !(var7.0 >> 31)) >> 8;
                        let mut var3 = w(self.pal(idx)?);
                        var7 += var8;
                        var3 = (((var3 & w(16711935)) * var38 >> 8) & w(16711935)) + (((var3 & w(0xFF00)) * var38 >> 8) & w(0xFF00));
                        for _ in 0..4 {
                            let var41 = w(self.get(var2.0)?);
                            let out = var3 + (((var41 & w(16711935)) * var37 >> 8) & w(16711935)) + (((var41 & w(0xFF00)) * var37 >> 8) & w(0xFF00));
                            self.put(var2.0, out.0)?;
                            var2 += 1;
                        }
                        var4 -= 1;
                        if var4 <= 0 {
                            break;
                        }
                    }
                }
                var4 = (var6 - var5) & 3;
                if var4 > 0 {
                    let idx = (var7.0 & !(var7.0 >> 31)) >> 8;
                    let mut var3 = w(self.pal(idx)?);
                    var3 = (((var3 & w(16711935)) * var38 >> 8) & w(16711935)) + (((var3 & w(0xFF00)) * var38 >> 8) & w(0xFF00));
                    loop {
                        let var45 = w(self.get(var2.0)?);
                        let out = var3 + (((var45 & w(16711935)) * var37 >> 8) & w(16711935)) + (((var45 & w(0xFF00)) * var37 >> 8) & w(0xFF00));
                        self.put(var2.0, out.0)?;
                        var2 += 1;
                        var4 -= 1;
                        if var4 <= 0 {
                            break;
                        }
                    }
                }
            }
        } else {
            let mut var4 = var6 - var5;
            if alpha == 0 {
                loop {
                    let idx = (var7.0 & !(var7.0 >> 31)) >> 8;
                    let rgb = self.pal(idx)?;
                    self.put(var2.0, rgb)?;
                    var2 += 1;
                    var7 += var8;
                    var4 -= 1;
                    if var4 <= 0 {
                        break;
                    }
                }
            } else {
                let var34 = w(alpha);
                let var10 = w(256 - alpha);
                loop {
                    let idx = (var7.0 & !(var7.0 >> 31)) >> 8;
                    let mut var3 = w(self.pal(idx)?);
                    var7 += var8;
                    var3 = (((var3 & w(16711935)) * var10 >> 8) & w(16711935)) + (((var3 & w(0xFF00)) * var10 >> 8) & w(0xFF00));
                    let var12 = w(self.get(var2.0)?);
                    let out = var3 + (((var12 & w(16711935)) * var34 >> 8) & w(16711935)) + (((var12 & w(0xFF00)) * var34 >> 8) & w(0xFF00));
                    self.put(var2.0, out.0)?;
                    var2 += 1;
                    var4 -= 1;
                    if var4 <= 0 {
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------- Flat (ft.al / px)

    /// `ft.al`: flat RGB triangle.
    pub fn flat(&mut self, y1: i32, y2: i32, y3: i32, x1: i32, x2: i32, x3: i32, rgb: i32) -> Result<(), Abort> {
        let (mut var11, mut var12, mut var13) = (w(x1), w(x2), w(x3));
        let (mut var14, mut var15, mut var16) = (w(y1), w(y2), w(y3));
        let var17 = if var15 != var14 { ((var12 - var11) << 14) / (var15 - var14) } else { w(0) };
        let var18 = if var16 != var15 { ((var13 - var12) << 14) / (var16 - var15) } else { w(0) };
        let var19 = if var16 != var14 { ((var11 - var13) << 14) / (var14 - var16) } else { w(0) };
        let var21 = w(self.state.height);
        let stride = w(self.state.stride);
        if var14 <= var15 && var14 <= var16 {
            if var14 < var21 {
                if var15 > var21 {
                    var15 = var21;
                }
                if var16 > var21 {
                    var16 = var21;
                }
                if var15 < var16 {
                    let mut var27 = var11 << 14;
                    var13 = var27;
                    if var14 < w(0) {
                        var13 -= var19 * var14;
                        var27 -= var17 * var14;
                        var14 = w(0);
                    }
                    var12 <<= 14;
                    if var15 < w(0) {
                        var12 -= var18 * var15;
                        var15 = w(0);
                    }
                    if (var14 == var15 || var19 >= var17) && (var14 != var15 || var19 <= var18) {
                        var16 -= var15;
                        var15 -= var14;
                        let mut off = w(self.state.row_offset(var14.0));
                        while {
                            var15 -= 1;
                            var15 >= w(0)
                        } {
                            self.flat_scanline(off.0, rgb, (var27 >> 14).0, (var13 >> 14).0)?;
                            var13 += var19;
                            var27 += var17;
                            off += stride;
                        }
                        while {
                            var16 -= 1;
                            var16 >= w(0)
                        } {
                            self.flat_scanline(off.0, rgb, (var12 >> 14).0, (var13 >> 14).0)?;
                            var13 += var19;
                            var12 += var18;
                            off += stride;
                        }
                    } else {
                        var16 -= var15;
                        var15 -= var14;
                        let mut off = w(self.state.row_offset(var14.0));
                        while {
                            var15 -= 1;
                            var15 >= w(0)
                        } {
                            self.flat_scanline(off.0, rgb, (var13 >> 14).0, (var27 >> 14).0)?;
                            var13 += var19;
                            var27 += var17;
                            off += stride;
                        }
                        while {
                            var16 -= 1;
                            var16 >= w(0)
                        } {
                            self.flat_scanline(off.0, rgb, (var13 >> 14).0, (var12 >> 14).0)?;
                            var13 += var19;
                            var12 += var18;
                            off += stride;
                        }
                    }
                } else {
                    let mut var26 = var11 << 14;
                    var12 = var26;
                    if var14 < w(0) {
                        var12 -= var19 * var14;
                        var26 -= var17 * var14;
                        var14 = w(0);
                    }
                    var13 <<= 14;
                    if var16 < w(0) {
                        var13 -= var18 * var16;
                        var16 = w(0);
                    }
                    if (var14 == var16 || var19 >= var17) && (var14 != var16 || var18 <= var17) {
                        var15 -= var16;
                        var16 -= var14;
                        let mut off = w(self.state.row_offset(var14.0));
                        while {
                            var16 -= 1;
                            var16 >= w(0)
                        } {
                            self.flat_scanline(off.0, rgb, (var26 >> 14).0, (var12 >> 14).0)?;
                            var12 += var19;
                            var26 += var17;
                            off += stride;
                        }
                        while {
                            var15 -= 1;
                            var15 >= w(0)
                        } {
                            self.flat_scanline(off.0, rgb, (var26 >> 14).0, (var13 >> 14).0)?;
                            var13 += var18;
                            var26 += var17;
                            off += stride;
                        }
                    } else {
                        var15 -= var16;
                        var16 -= var14;
                        let mut off = w(self.state.row_offset(var14.0));
                        while {
                            var16 -= 1;
                            var16 >= w(0)
                        } {
                            self.flat_scanline(off.0, rgb, (var12 >> 14).0, (var26 >> 14).0)?;
                            var12 += var19;
                            var26 += var17;
                            off += stride;
                        }
                        while {
                            var15 -= 1;
                            var15 >= w(0)
                        } {
                            self.flat_scanline(off.0, rgb, (var13 >> 14).0, (var26 >> 14).0)?;
                            var13 += var18;
                            var26 += var17;
                            off += stride;
                        }
                    }
                }
            }
        } else if var15 <= var16 {
            if var15 < var21 {
                if var16 > var21 {
                    var16 = var21;
                }
                if var14 > var21 {
                    var14 = var21;
                }
                if var16 < var14 {
                    let mut var31 = var12 << 14;
                    var11 = var31;
                    if var15 < w(0) {
                        var11 -= var17 * var15;
                        var31 -= var18 * var15;
                        var15 = w(0);
                    }
                    var13 <<= 14;
                    if var16 < w(0) {
                        var13 -= var19 * var16;
                        var16 = w(0);
                    }
                    if (var15 == var16 || var17 >= var18) && (var15 != var16 || var17 <= var19) {
                        var14 -= var16;
                        var16 -= var15;
                        let mut off = w(self.state.row_offset(var15.0));
                        while {
                            var16 -= 1;
                            var16 >= w(0)
                        } {
                            self.flat_scanline(off.0, rgb, (var31 >> 14).0, (var11 >> 14).0)?;
                            var11 += var17;
                            var31 += var18;
                            off += stride;
                        }
                        while {
                            var14 -= 1;
                            var14 >= w(0)
                        } {
                            self.flat_scanline(off.0, rgb, (var13 >> 14).0, (var11 >> 14).0)?;
                            var11 += var17;
                            var13 += var19;
                            off += stride;
                        }
                    } else {
                        var14 -= var16;
                        var16 -= var15;
                        let mut off = w(self.state.row_offset(var15.0));
                        while {
                            var16 -= 1;
                            var16 >= w(0)
                        } {
                            self.flat_scanline(off.0, rgb, (var11 >> 14).0, (var31 >> 14).0)?;
                            var11 += var17;
                            var31 += var18;
                            off += stride;
                        }
                        while {
                            var14 -= 1;
                            var14 >= w(0)
                        } {
                            self.flat_scanline(off.0, rgb, (var11 >> 14).0, (var13 >> 14).0)?;
                            var11 += var17;
                            var13 += var19;
                            off += stride;
                        }
                    }
                } else {
                    let mut var30 = var12 << 14;
                    var13 = var30;
                    if var15 < w(0) {
                        var13 -= var17 * var15;
                        var30 -= var18 * var15;
                        var15 = w(0);
                    }
                    var11 <<= 14;
                    if var14 < w(0) {
                        var11 -= var19 * var14;
                        var14 = w(0);
                    }
                    if var17 < var18 {
                        var16 -= var14;
                        var14 -= var15;
                        let mut off = w(self.state.row_offset(var15.0));
                        while {
                            var14 -= 1;
                            var14 >= w(0)
                        } {
                            self.flat_scanline(off.0, rgb, (var13 >> 14).0, (var30 >> 14).0)?;
                            var13 += var17;
                            var30 += var18;
                            off += stride;
                        }
                        while {
                            var16 -= 1;
                            var16 >= w(0)
                        } {
                            self.flat_scanline(off.0, rgb, (var11 >> 14).0, (var30 >> 14).0)?;
                            var11 += var19;
                            var30 += var18;
                            off += stride;
                        }
                    } else {
                        var16 -= var14;
                        var14 -= var15;
                        let mut off = w(self.state.row_offset(var15.0));
                        while {
                            var14 -= 1;
                            var14 >= w(0)
                        } {
                            self.flat_scanline(off.0, rgb, (var30 >> 14).0, (var13 >> 14).0)?;
                            var13 += var17;
                            var30 += var18;
                            off += stride;
                        }
                        while {
                            var16 -= 1;
                            var16 >= w(0)
                        } {
                            self.flat_scanline(off.0, rgb, (var30 >> 14).0, (var11 >> 14).0)?;
                            var11 += var19;
                            var30 += var18;
                            off += stride;
                        }
                    }
                }
            }
        } else if var16 < var21 {
            if var14 > var21 {
                var14 = var21;
            }
            if var15 > var21 {
                var15 = var21;
            }
            if var14 < var15 {
                let mut var35 = var13 << 14;
                var12 = var35;
                if var16 < w(0) {
                    var12 -= var18 * var16;
                    var35 -= var19 * var16;
                    var16 = w(0);
                }
                var11 <<= 14;
                if var14 < w(0) {
                    var11 -= var17 * var14;
                    var14 = w(0);
                }
                if var18 < var19 {
                    var15 -= var14;
                    var14 -= var16;
                    let mut off = w(self.state.row_offset(var16.0));
                    while {
                        var14 -= 1;
                        var14 >= w(0)
                    } {
                        self.flat_scanline(off.0, rgb, (var12 >> 14).0, (var35 >> 14).0)?;
                        var12 += var18;
                        var35 += var19;
                        off += stride;
                    }
                    while {
                        var15 -= 1;
                        var15 >= w(0)
                    } {
                        self.flat_scanline(off.0, rgb, (var12 >> 14).0, (var11 >> 14).0)?;
                        var12 += var18;
                        var11 += var17;
                        off += stride;
                    }
                } else {
                    var15 -= var14;
                    var14 -= var16;
                    let mut off = w(self.state.row_offset(var16.0));
                    while {
                        var14 -= 1;
                        var14 >= w(0)
                    } {
                        self.flat_scanline(off.0, rgb, (var35 >> 14).0, (var12 >> 14).0)?;
                        var12 += var18;
                        var35 += var19;
                        off += stride;
                    }
                    while {
                        var15 -= 1;
                        var15 >= w(0)
                    } {
                        self.flat_scanline(off.0, rgb, (var11 >> 14).0, (var12 >> 14).0)?;
                        var12 += var18;
                        var11 += var17;
                        off += stride;
                    }
                }
            } else {
                let mut var34 = var13 << 14;
                var11 = var34;
                if var16 < w(0) {
                    var11 -= var18 * var16;
                    var34 -= var19 * var16;
                    var16 = w(0);
                }
                var12 <<= 14;
                if var15 < w(0) {
                    var12 -= var17 * var15;
                    var15 = w(0);
                }
                if var18 < var19 {
                    var14 -= var15;
                    var15 -= var16;
                    let mut off = w(self.state.row_offset(var16.0));
                    while {
                        var15 -= 1;
                        var15 >= w(0)
                    } {
                        self.flat_scanline(off.0, rgb, (var11 >> 14).0, (var34 >> 14).0)?;
                        var11 += var18;
                        var34 += var19;
                        off += stride;
                    }
                    while {
                        var14 -= 1;
                        var14 >= w(0)
                    } {
                        self.flat_scanline(off.0, rgb, (var12 >> 14).0, (var34 >> 14).0)?;
                        var12 += var17;
                        var34 += var19;
                        off += stride;
                    }
                } else {
                    var14 -= var15;
                    var15 -= var16;
                    let mut off = w(self.state.row_offset(var16.0));
                    while {
                        var15 -= 1;
                        var15 >= w(0)
                    } {
                        self.flat_scanline(off.0, rgb, (var34 >> 14).0, (var11 >> 14).0)?;
                        var11 += var18;
                        var34 += var19;
                        off += stride;
                    }
                    while {
                        var14 -= 1;
                        var14 >= w(0)
                    } {
                        self.flat_scanline(off.0, rgb, (var34 >> 14).0, (var12 >> 14).0)?;
                        var12 += var17;
                        var34 += var19;
                        off += stride;
                    }
                }
            }
        }
        Ok(())
    }

    /// `ft.px`: flat scanline; alpha 254 reproduces the original's shift-copy quirk.
    fn flat_scanline(&mut self, offset: i32, rgb: i32, x_start: i32, x_end: i32) -> Result<(), Abort> {
        let mut var5 = x_start;
        let mut var6 = x_end;
        if self.clip_x {
            if var6 > self.state.width {
                var6 = self.state.width;
            }
            if var5 < 0 {
                var5 = 0;
            }
        }
        if var5 >= var6 {
            return Ok(());
        }
        let mut var2 = w(offset) + w(var5);
        let mut var4 = (var6 - var5) >> 2;
        let alpha = self.alpha;
        if alpha != 0 {
            if alpha == 254 {
                let total = ((var6 - var5) >> 2) * 4 + ((var6 - var5) & 3);
                for _ in 0..total {
                    let next = self.get(var2.0 + 1)?;
                    self.put(var2.0, next)?;
                    var2 += 1;
                }
            } else {
                let var7 = w(alpha);
                let var8 = w(256 - alpha);
                let var3 = (((w(rgb) & w(16711935)) * var8 >> 8) & w(16711935)) + (((w(rgb) & w(0xFF00)) * var8 >> 8) & w(0xFF00));
                let total = var4 * 4 + ((var6 - var5) & 3);
                for _ in 0..total {
                    let var9 = w(self.get(var2.0)?);
                    let out = var3 + (((var9 & w(16711935)) * var7 >> 8) & w(16711935)) + (((var9 & w(0xFF00)) * var7 >> 8) & w(0xFF00));
                    self.put(var2.0, out.0)?;
                    var2 += 1;
                }
            }
        } else {
            while var4 > 0 {
                var4 -= 1;
                for _ in 0..4 {
                    self.put(var2.0, rgb)?;
                    var2 += 1;
                }
            }
            var4 = (var6 - var5) & 3;
            while var4 > 0 {
                var4 -= 1;
                self.put(var2.0, rgb)?;
                var2 += 1;
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------- Textured (model: ft.aj / bq)

    /// `ft.aj`: model textured triangle. Colors are HSL shades; P/M/N are camera-space texture
    /// plane vertices. Without a loaded texture the original falls back to Gouraud with the
    /// average texture color merged into each shade (`fq.af`).
    pub fn textured_model(
        &mut self, y1: i32, y2: i32, y3: i32, x1: i32, x2: i32, x3: i32, c1: i32, c2: i32, c3: i32, px1: i32, px2: i32, px3: i32,
        py1: i32, py2: i32, py3: i32, pz1: i32, pz2: i32, pz3: i32, texture: i32,
    ) -> Result<(), Abort> {
        let Some(texels) = self.textures.texels(texture) else {
            let avg = self.textures.average(texture);
            return self.gouraud(y1, y2, y3, x1, x2, x3, merge_texture_shade(avg, c1), merge_texture_shade(avg, c2), merge_texture_shade(avg, c3));
        };
        let texels: Vec<i32> = texels.to_vec();
        self.tex_opaque = self.textures.opaque(texture);
        self.textured_common(&texels, true, y1, y2, y3, x1, x2, x3, c1, c2, c3, px1, px2, px3, py1, py2, py3, pz1, pz2, pz3)
    }

    /// `ft.ay`: tile textured triangle (affine texture across each scanline).
    pub fn textured_tile(
        &mut self, y1: i32, y2: i32, y3: i32, x1: i32, x2: i32, x3: i32, c1: i32, c2: i32, c3: i32, px1: i32, px2: i32, px3: i32,
        py1: i32, py2: i32, py3: i32, pz1: i32, pz2: i32, pz3: i32, texture: i32,
    ) -> Result<(), Abort> {
        let Some(texels) = self.textures.texels(texture) else {
            let avg = self.textures.average(texture);
            return self.gouraud(y1, y2, y3, x1, x2, x3, merge_texture_shade(avg, c1), merge_texture_shade(avg, c2), merge_texture_shade(avg, c3));
        };
        let texels: Vec<i32> = texels.to_vec();
        self.tex_opaque = self.textures.opaque(texture);
        self.textured_common(&texels, false, y1, y2, y3, x1, x2, x3, c1, c2, c3, px1, px2, px3, py1, py2, py3, pz1, pz2, pz3)
    }

    /// Shared edge walk of `aj`/`ay`; only the plane-step precision and scanline differ.
    fn textured_common(
        &mut self, texels: &[i32], model: bool, y1: i32, y2: i32, y3: i32, x1: i32, x2: i32, x3: i32, c1: i32, c2: i32, c3: i32,
        var13: i32, var14: i32, var15: i32, var16: i32, var17: i32, var18: i32, var19: i32, var20: i32, var21: i32,
    ) -> Result<(), Abort> {
        let (mut var24, mut var25, mut var26) = (w(x1), w(x2), w(x3));
        let (mut var27, mut var28, mut var29) = (w(y1), w(y2), w(y3));
        let (mut var10, mut var11, mut var12) = (w(c1), w(c2), w(c3));
        let var30 = var25 - var24;
        let var31 = var28 - var27;
        let var32 = var26 - var24;
        let var33 = var29 - var27;
        let var34 = var11 - var10;
        let var35 = var12 - var10;
        let var36 = if var28 != var27 { ((var25 - var24) << 14) / (var28 - var27) } else { w(0) };
        let var37 = if var29 != var28 { ((var26 - var25) << 14) / (var29 - var28) } else { w(0) };
        let var38 = if var29 != var27 { ((var24 - var26) << 14) / (var27 - var29) } else { w(0) };
        let var39 = var30 * var33 - var32 * var31;
        if var39 == w(0) {
            return Ok(());
        }
        let var40 = ((var34 * var33 - var35 * var31) << 9) / var39;
        let var41 = ((var35 * var30 - var34 * var32) << 9) / var39;
        let var42 = i64::from(self.state.zoom);
        let (mut var13, mut var14, mut var15) = (w(var13), w(var14), w(var15));
        let (mut var16, mut var17, mut var18) = (w(var16), w(var17), w(var18));
        let (mut var19, mut var20, mut var21) = (w(var19), w(var20), w(var21));
        var14 = var13 - var14;
        var17 = var16 - var17;
        var20 = var19 - var20;
        var15 -= var13;
        var18 -= var16;
        var21 -= var19;
        let shift3: u32 = if model { 3 } else { 0 };
        let long_div = |v: W| -> W { w((((i64::from(v.0)) << shift3 << 14) / var42) as i32) };
        let long_div_plain = |v: W| -> W { w((((i64::from(v.0)) << 14) / var42) as i32) };
        let mut var43 = (var15 * var16 - var18 * var13) << 14;
        let var44 = long_div(var18 * var19 - var21 * var16);
        let var45 = long_div_plain(var21 * var13 - var15 * var19);
        let mut var46 = (var14 * var16 - var17 * var13) << 14;
        let var47 = long_div(var17 * var19 - var20 * var16);
        let var48 = long_div_plain(var20 * var13 - var14 * var19);
        let mut var49 = (var17 * var15 - var14 * var18) << 14;
        let var50 = long_div(var20 * var18 - var17 * var21);
        let var51 = long_div_plain(var14 * var21 - var20 * var15);
        let var53 = w(self.state.height);
        let var54 = w(self.state.center_y);
        let stride = w(self.state.stride);
        // Row emitter closure replaced by a macro-like helper to keep the six branches literal.
        macro_rules! row {
            ($off:expr, $xa:expr, $xb:expr, $shade:expr) => {
                self.textured_scanline(texels, model, $off.0, ($xa >> 14).0, ($xb >> 14).0, $shade.0, var40.0, var43.0, var46.0, var49.0, var44.0, var47.0, var50.0)?;
            };
        }
        if var27 <= var28 && var27 <= var29 {
            if var27 < var53 {
                if var28 > var53 {
                    var28 = var53;
                }
                if var29 > var53 {
                    var29 = var53;
                }
                var10 = (var10 << 9) - var40 * var24 + var40;
                if var28 < var29 {
                    let mut var70 = var24 << 14;
                    var26 = var70;
                    if var27 < w(0) {
                        var26 -= var38 * var27;
                        var70 -= var36 * var27;
                        var10 -= var41 * var27;
                        var27 = w(0);
                    }
                    var25 <<= 14;
                    if var28 < w(0) {
                        var25 -= var37 * var28;
                        var28 = w(0);
                    }
                    let var142 = var27 - var54;
                    var43 += var45 * var142;
                    var46 += var48 * var142;
                    var49 += var51 * var142;
                    if (var27 == var28 || var38 >= var36) && (var27 != var28 || var38 <= var37) {
                        var29 -= var28;
                        var28 -= var27;
                        let mut off = w(self.state.row_offset(var27.0));
                        while {
                            var28 -= 1;
                            var28 >= w(0)
                        } {
                            row!(off, var70, var26, var10);
                            var26 += var38;
                            var70 += var36;
                            var10 += var41;
                            off += stride;
                            var43 += var45;
                            var46 += var48;
                            var49 += var51;
                        }
                        while {
                            var29 -= 1;
                            var29 >= w(0)
                        } {
                            row!(off, var25, var26, var10);
                            var26 += var38;
                            var25 += var37;
                            var10 += var41;
                            off += stride;
                            var43 += var45;
                            var46 += var48;
                            var49 += var51;
                        }
                    } else {
                        var29 -= var28;
                        var28 -= var27;
                        let mut off = w(self.state.row_offset(var27.0));
                        while {
                            var28 -= 1;
                            var28 >= w(0)
                        } {
                            row!(off, var26, var70, var10);
                            var26 += var38;
                            var70 += var36;
                            var10 += var41;
                            off += stride;
                            var43 += var45;
                            var46 += var48;
                            var49 += var51;
                        }
                        while {
                            var29 -= 1;
                            var29 >= w(0)
                        } {
                            row!(off, var26, var25, var10);
                            var26 += var38;
                            var25 += var37;
                            var10 += var41;
                            off += stride;
                            var43 += var45;
                            var46 += var48;
                            var49 += var51;
                        }
                    }
                } else {
                    let mut var69 = var24 << 14;
                    var25 = var69;
                    if var27 < w(0) {
                        var25 -= var38 * var27;
                        var69 -= var36 * var27;
                        var10 -= var41 * var27;
                        var27 = w(0);
                    }
                    var26 <<= 14;
                    if var29 < w(0) {
                        var26 -= var37 * var29;
                        var29 = w(0);
                    }
                    let var141 = var27 - var54;
                    var43 += var45 * var141;
                    var46 += var48 * var141;
                    var49 += var51 * var141;
                    if (var27 == var29 || var38 >= var36) && (var27 != var29 || var37 <= var36) {
                        var28 -= var29;
                        var29 -= var27;
                        let mut off = w(self.state.row_offset(var27.0));
                        while {
                            var29 -= 1;
                            var29 >= w(0)
                        } {
                            row!(off, var69, var25, var10);
                            var25 += var38;
                            var69 += var36;
                            var10 += var41;
                            off += stride;
                            var43 += var45;
                            var46 += var48;
                            var49 += var51;
                        }
                        while {
                            var28 -= 1;
                            var28 >= w(0)
                        } {
                            row!(off, var69, var26, var10);
                            var26 += var37;
                            var69 += var36;
                            var10 += var41;
                            off += stride;
                            var43 += var45;
                            var46 += var48;
                            var49 += var51;
                        }
                    } else {
                        var28 -= var29;
                        var29 -= var27;
                        let mut off = w(self.state.row_offset(var27.0));
                        while {
                            var29 -= 1;
                            var29 >= w(0)
                        } {
                            row!(off, var25, var69, var10);
                            var25 += var38;
                            var69 += var36;
                            var10 += var41;
                            off += stride;
                            var43 += var45;
                            var46 += var48;
                            var49 += var51;
                        }
                        while {
                            var28 -= 1;
                            var28 >= w(0)
                        } {
                            row!(off, var26, var69, var10);
                            var26 += var37;
                            var69 += var36;
                            var10 += var41;
                            off += stride;
                            var43 += var45;
                            var46 += var48;
                            var49 += var51;
                        }
                    }
                }
            }
        } else if var28 <= var29 {
            if var28 < var53 {
                if var29 > var53 {
                    var29 = var53;
                }
                if var27 > var53 {
                    var27 = var53;
                }
                var11 = (var11 << 9) - var40 * var25 + var40;
                if var29 < var27 {
                    let mut var75 = var25 << 14;
                    var24 = var75;
                    if var28 < w(0) {
                        var24 -= var36 * var28;
                        var75 -= var37 * var28;
                        var11 -= var41 * var28;
                        var28 = w(0);
                    }
                    var26 <<= 14;
                    if var29 < w(0) {
                        var26 -= var38 * var29;
                        var29 = w(0);
                    }
                    let var140 = var28 - var54;
                    var43 += var45 * var140;
                    var46 += var48 * var140;
                    var49 += var51 * var140;
                    if (var28 == var29 || var36 >= var37) && (var28 != var29 || var36 <= var38) {
                        var27 -= var29;
                        var29 -= var28;
                        let mut off = w(self.state.row_offset(var28.0));
                        while {
                            var29 -= 1;
                            var29 >= w(0)
                        } {
                            row!(off, var75, var24, var11);
                            var24 += var36;
                            var75 += var37;
                            var11 += var41;
                            off += stride;
                            var43 += var45;
                            var46 += var48;
                            var49 += var51;
                        }
                        while {
                            var27 -= 1;
                            var27 >= w(0)
                        } {
                            row!(off, var26, var24, var11);
                            var24 += var36;
                            var26 += var38;
                            var11 += var41;
                            off += stride;
                            var43 += var45;
                            var46 += var48;
                            var49 += var51;
                        }
                    } else {
                        var27 -= var29;
                        var29 -= var28;
                        let mut off = w(self.state.row_offset(var28.0));
                        while {
                            var29 -= 1;
                            var29 >= w(0)
                        } {
                            row!(off, var24, var75, var11);
                            var24 += var36;
                            var75 += var37;
                            var11 += var41;
                            off += stride;
                            var43 += var45;
                            var46 += var48;
                            var49 += var51;
                        }
                        while {
                            var27 -= 1;
                            var27 >= w(0)
                        } {
                            row!(off, var24, var26, var11);
                            var24 += var36;
                            var26 += var38;
                            var11 += var41;
                            off += stride;
                            var43 += var45;
                            var46 += var48;
                            var49 += var51;
                        }
                    }
                } else {
                    let mut var74 = var25 << 14;
                    var26 = var74;
                    if var28 < w(0) {
                        var26 -= var36 * var28;
                        var74 -= var37 * var28;
                        var11 -= var41 * var28;
                        var28 = w(0);
                    }
                    var24 <<= 14;
                    if var27 < w(0) {
                        var24 -= var38 * var27;
                        var27 = w(0);
                    }
                    let var139 = var28 - var54;
                    var43 += var45 * var139;
                    var46 += var48 * var139;
                    var49 += var51 * var139;
                    if var36 < var37 {
                        var29 -= var27;
                        var27 -= var28;
                        let mut off = w(self.state.row_offset(var28.0));
                        while {
                            var27 -= 1;
                            var27 >= w(0)
                        } {
                            row!(off, var26, var74, var11);
                            var26 += var36;
                            var74 += var37;
                            var11 += var41;
                            off += stride;
                            var43 += var45;
                            var46 += var48;
                            var49 += var51;
                        }
                        while {
                            var29 -= 1;
                            var29 >= w(0)
                        } {
                            row!(off, var24, var74, var11);
                            var24 += var38;
                            var74 += var37;
                            var11 += var41;
                            off += stride;
                            var43 += var45;
                            var46 += var48;
                            var49 += var51;
                        }
                    } else {
                        var29 -= var27;
                        var27 -= var28;
                        let mut off = w(self.state.row_offset(var28.0));
                        while {
                            var27 -= 1;
                            var27 >= w(0)
                        } {
                            row!(off, var74, var26, var11);
                            var26 += var36;
                            var74 += var37;
                            var11 += var41;
                            off += stride;
                            var43 += var45;
                            var46 += var48;
                            var49 += var51;
                        }
                        while {
                            var29 -= 1;
                            var29 >= w(0)
                        } {
                            row!(off, var74, var24, var11);
                            var24 += var38;
                            var74 += var37;
                            var11 += var41;
                            off += stride;
                            var43 += var45;
                            var46 += var48;
                            var49 += var51;
                        }
                    }
                }
            }
        } else if var29 < var53 {
            if var27 > var53 {
                var27 = var53;
            }
            if var28 > var53 {
                var28 = var53;
            }
            var12 = (var12 << 9) - var40 * var26 + var40;
            if var27 < var28 {
                let mut var79 = var26 << 14;
                var25 = var79;
                if var29 < w(0) {
                    var25 -= var37 * var29;
                    var79 -= var38 * var29;
                    var12 -= var41 * var29;
                    var29 = w(0);
                }
                var24 <<= 14;
                if var27 < w(0) {
                    var24 -= var36 * var27;
                    var27 = w(0);
                }
                let var138 = var29 - var54;
                var43 += var45 * var138;
                var46 += var48 * var138;
                var49 += var51 * var138;
                if var37 < var38 {
                    var28 -= var27;
                    var27 -= var29;
                    let mut off = w(self.state.row_offset(var29.0));
                    while {
                        var27 -= 1;
                        var27 >= w(0)
                    } {
                        row!(off, var25, var79, var12);
                        var25 += var37;
                        var79 += var38;
                        var12 += var41;
                        off += stride;
                        var43 += var45;
                        var46 += var48;
                        var49 += var51;
                    }
                    while {
                        var28 -= 1;
                        var28 >= w(0)
                    } {
                        row!(off, var25, var24, var12);
                        var25 += var37;
                        var24 += var36;
                        var12 += var41;
                        off += stride;
                        var43 += var45;
                        var46 += var48;
                        var49 += var51;
                    }
                } else {
                    var28 -= var27;
                    var27 -= var29;
                    let mut off = w(self.state.row_offset(var29.0));
                    while {
                        var27 -= 1;
                        var27 >= w(0)
                    } {
                        row!(off, var79, var25, var12);
                        var25 += var37;
                        var79 += var38;
                        var12 += var41;
                        off += stride;
                        var43 += var45;
                        var46 += var48;
                        var49 += var51;
                    }
                    while {
                        var28 -= 1;
                        var28 >= w(0)
                    } {
                        row!(off, var24, var25, var12);
                        var25 += var37;
                        var24 += var36;
                        var12 += var41;
                        off += stride;
                        var43 += var45;
                        var46 += var48;
                        var49 += var51;
                    }
                }
            } else {
                let mut var78 = var26 << 14;
                var24 = var78;
                if var29 < w(0) {
                    var24 -= var37 * var29;
                    var78 -= var38 * var29;
                    var12 -= var41 * var29;
                    var29 = w(0);
                }
                var25 <<= 14;
                if var28 < w(0) {
                    var25 -= var36 * var28;
                    var28 = w(0);
                }
                let var137 = var29 - var54;
                var43 += var45 * var137;
                var46 += var48 * var137;
                var49 += var51 * var137;
                if var37 < var38 {
                    var27 -= var28;
                    var28 -= var29;
                    let mut off = w(self.state.row_offset(var29.0));
                    while {
                        var28 -= 1;
                        var28 >= w(0)
                    } {
                        row!(off, var24, var78, var12);
                        var24 += var37;
                        var78 += var38;
                        var12 += var41;
                        off += stride;
                        var43 += var45;
                        var46 += var48;
                        var49 += var51;
                    }
                    while {
                        var27 -= 1;
                        var27 >= w(0)
                    } {
                        row!(off, var25, var78, var12);
                        var25 += var36;
                        var78 += var38;
                        var12 += var41;
                        off += stride;
                        var43 += var45;
                        var46 += var48;
                        var49 += var51;
                    }
                } else {
                    var27 -= var28;
                    var28 -= var29;
                    let mut off = w(self.state.row_offset(var29.0));
                    while {
                        var28 -= 1;
                        var28 >= w(0)
                    } {
                        row!(off, var78, var24, var12);
                        var24 += var37;
                        var78 += var38;
                        var12 += var41;
                        off += stride;
                        var43 += var45;
                        var46 += var48;
                        var49 += var51;
                    }
                    while {
                        var27 -= 1;
                        var27 >= w(0)
                    } {
                        row!(off, var78, var25, var12);
                        var25 += var36;
                        var78 += var38;
                        var12 += var41;
                        off += stride;
                        var43 += var45;
                        var46 += var48;
                        var49 += var51;
                    }
                }
            }
        }
        Ok(())
    }

    /// Dispatches to `bq` (model: perspective per 8 pixels) or `bf` (tile: affine per span).
    fn textured_scanline(
        &mut self, texels: &[i32], model: bool, offset: i32, x_start: i32, x_end: i32, shade: i32, shade_step: i32, var10: i32,
        var11: i32, var12: i32, var13: i32, var14: i32, var15: i32,
    ) -> Result<(), Abort> {
        if model {
            self.textured_scanline_model(texels, offset, x_start, x_end, shade, shade_step, var10, var11, var12, var13, var14, var15)
        } else {
            self.textured_scanline_tile(texels, offset, x_start, x_end, shade, shade_step, var10, var11, var12, var13, var14, var15)
        }
    }

    #[inline]
    fn texel(texels: &[i32], var3: i32) -> Result<i32, Abort> {
        let index = (var3 & 16256) + ((var3 as u32) >> 25) as i32;
        texels.get(index as usize).copied().ok_or(Abort)
    }

    #[inline]
    fn shade_texel(var4: W, var16: W) -> i32 {
        ((((var4 & w(16711935)) * var16) & w(-16711936)) + (((var4 & w(0xFF00)) * var16) & w(0xFF0000)) >> 8 | w(0xFF000000u32 as i32)).0
    }

    /// Alpha blend used by `bq`: `((texel * premultipliedShade) >> 8) + ((dst * alpha) >> 8) | 0xFF000000`.
    #[inline]
    fn blend_texel(var4: W, premul: W, dst: W, alpha: W) -> i32 {
        let src = ((((var4 & w(16711935)) * premul) & w(-16711936)) | (((var4 & w(0xFF00)) * premul) & w(0xFF0000))) >> 8;
        let d = ((((dst & w(16711935)) * alpha) & w(-16711936)) | (((dst & w(0xFF00)) * alpha) & w(0xFF0000))) >> 8;
        (src + d | w(0xFF000000u32 as i32)).0
    }

    /// `ft.bq`: model textured scanline, perspective-correct every 8 pixels.
    fn textured_scanline_model(
        &mut self, texels: &[i32], offset: i32, x_start: i32, x_end: i32, shade: i32, shade_step: i32, var10: i32, var11: i32,
        var12: i32, var13: i32, var14: i32, var15: i32,
    ) -> Result<(), Abort> {
        let mut var6 = x_start;
        let mut var7 = x_end;
        if self.clip_x {
            if var7 > self.state.width {
                var7 = self.state.width;
            }
            if var6 < 0 {
                var6 = 0;
            }
        }
        if var6 >= var7 {
            return Ok(());
        }
        let mut var5 = w(offset) + w(var6);
        let mut var8 = w(shade) + w(shade_step) * w(var6);
        let mut var9 = w(shade_step);
        let mut var18 = var7 - var6;
        let var24 = w(var6 - self.state.center_x);
        let (mut var10, mut var11, mut var12) = (w(var10), w(var11), w(var12));
        let (var13, var14, var15) = (w(var13), w(var14), w(var15));
        var10 += (var13 >> 3) * var24;
        var11 += (var14 >> 3) * var24;
        var12 += (var15 >> 3) * var24;
        let mut var23 = var12 >> 14;
        let (mut var19, mut var20);
        if var23 != w(0) {
            var19 = var10 / var23;
            var20 = var11 / var23;
            if var19 < w(0) {
                var19 = w(0);
            } else if var19 > w(16256) {
                var19 = w(16256);
            }
        } else {
            var19 = w(0);
            var20 = w(0);
        }
        var10 += var13;
        var11 += var14;
        var12 += var15;
        var23 = var12 >> 14;
        let (mut var21, mut var22);
        if var23 != w(0) {
            var21 = var10 / var23;
            var22 = var11 / var23;
            if var21 < w(0) {
                var21 = w(0);
            } else if var21 > w(16256) {
                var21 = w(16256);
            }
        } else {
            var21 = w(0);
            var22 = w(0);
        }
        let mut var3 = (var19 << 18) + var20;
        let mut var17 = (((var21 - var19) >> 3) << 18) + ((var22 - var20) >> 3);
        var18 >>= 3;
        var9 <<= 3;
        let mut var16 = var8 >> 8;
        let alpha = self.alpha;
        let opaque = self.tex_opaque;
        macro_rules! advance_block {
            () => {
                var19 = var21;
                var20 = var22;
                var10 += var13;
                var11 += var14;
                var12 += var15;
                var23 = var12 >> 14;
                if var23 != w(0) {
                    var21 = var10 / var23;
                    var22 = var11 / var23;
                    if var21 < w(0) {
                        var21 = w(0);
                    } else if var21 > w(16256) {
                        var21 = w(16256);
                    }
                } else {
                    var21 = w(0);
                    var22 = w(0);
                }
                var3 = (var19 << 18) + var20;
                var17 = (((var21 - var19) >> 3) << 18) + ((var22 - var20) >> 3);
                var8 += var9;
                var16 = var8 >> 8;
            };
        }
        if opaque {
            if alpha == 0 {
                while var18 > 0 {
                    for _ in 0..8 {
                        let var4 = w(Self::texel(texels, var3.0)?);
                        self.put(var5.0, Self::shade_texel(var4, var16))?;
                        var5 += 1;
                        var3 += var17;
                    }
                    advance_block!();
                    var18 -= 1;
                }
                var18 = (var7 - var6) & 7;
                while var18 > 0 {
                    let var4 = w(Self::texel(texels, var3.0)?);
                    self.put(var5.0, Self::shade_texel(var4, var16))?;
                    var5 += 1;
                    var3 += var17;
                    var18 -= 1;
                }
            } else {
                let var25 = w(alpha);
                let var26 = w(256 - alpha);
                // Original quirk: the premultiplied shade is computed once per scanline.
                let var27 = var16 * var26 >> 8;
                while var18 > 0 {
                    for _ in 0..8 {
                        let var4 = w(Self::texel(texels, var3.0)?);
                        let var28 = w(self.get(var5.0)?);
                        self.put(var5.0, Self::blend_texel(var4, var27, var28, var25))?;
                        var5 += 1;
                        var3 += var17;
                    }
                    advance_block!();
                    var18 -= 1;
                }
                var18 = (var7 - var6) & 7;
                while var18 > 0 {
                    let var4 = w(Self::texel(texels, var3.0)?);
                    let var28 = w(self.get(var5.0)?);
                    self.put(var5.0, Self::blend_texel(var4, var27, var28, var25))?;
                    var5 += 1;
                    var3 += var17;
                    var18 -= 1;
                }
            }
        } else if alpha == 0 {
            while var18 > 0 {
                for _ in 0..8 {
                    let var4 = w(Self::texel(texels, var3.0)?);
                    if var4 != w(0) {
                        self.put(var5.0, Self::shade_texel(var4, var16))?;
                    }
                    var5 += 1;
                    var3 += var17;
                }
                advance_block!();
                var18 -= 1;
            }
            var18 = (var7 - var6) & 7;
            while var18 > 0 {
                let var4 = w(Self::texel(texels, var3.0)?);
                if var4 != w(0) {
                    self.put(var5.0, Self::shade_texel(var4, var16))?;
                }
                var5 += 1;
                var3 += var17;
                var18 -= 1;
            }
        } else {
            let var173 = w(alpha);
            let var174 = w(256 - alpha);
            // Original quirk: computed once per scanline, not per 8-pixel block.
            let var175 = var16 * var174 >> 8;
            while var18 > 0 {
                for _ in 0..8 {
                    let var4 = w(Self::texel(texels, var3.0)?);
                    if var4 != w(0) {
                        let var184 = w(self.get(var5.0)?);
                        self.put(var5.0, Self::blend_texel(var4, var175, var184, var173))?;
                    }
                    var5 += 1;
                    var3 += var17;
                }
                advance_block!();
                var18 -= 1;
            }
            var18 = (var7 - var6) & 7;
            while var18 > 0 {
                let var4 = w(Self::texel(texels, var3.0)?);
                if var4 != w(0) {
                    let var184 = w(self.get(var5.0)?);
                    self.put(var5.0, Self::blend_texel(var4, var175, var184, var173))?;
                }
                var5 += 1;
                var3 += var17;
                var18 -= 1;
            }
        }
        Ok(())
    }

    /// `ft.bf`: tile textured scanline, affine u/v across the span, shade per 8 pixels.
    fn textured_scanline_tile(
        &mut self, texels: &[i32], offset: i32, x_start: i32, x_end: i32, shade: i32, shade_step: i32, var10: i32, var11: i32,
        var12: i32, var13: i32, var14: i32, var15: i32,
    ) -> Result<(), Abort> {
        let mut var6 = x_start;
        let mut var7 = x_end;
        if self.clip_x {
            if var7 > self.state.width {
                var7 = self.state.width;
            }
            if var6 < 0 {
                var6 = 0;
            }
        }
        if var6 >= var7 {
            return Ok(());
        }
        let mut var5 = w(offset) + w(var6);
        let mut var8 = w(shade) + w(shade_step) * w(var6);
        let mut var9 = w(shade_step);
        let mut var18 = w(var7 - var6);
        let var24 = w(var6 - self.state.center_x);
        let (mut var10, mut var11, mut var12) = (w(var10), w(var11), w(var12));
        let (var13, var14, var15) = (w(var13), w(var14), w(var15));
        var10 += var13 * var24;
        var11 += var14 * var24;
        var12 += var15 * var24;
        let mut var23 = var12 >> 14;
        let (var19, var20) = if var23 != w(0) { (var10 / var23, var11 / var23) } else { (w(0), w(0)) };
        var10 += var13 * var18;
        var11 += var14 * var18;
        var12 += var15 * var18;
        var23 = var12 >> 14;
        let (var21, var22) = if var23 != w(0) { (var10 / var23, var11 / var23) } else { (w(0), w(0)) };
        let mut var3 = (var19 << 18) + var20;
        let var17 = (((var21 - var19) / var18) << 18) + (var22 - var20) / var18;
        let mut blocks = (var18 >> 3).0;
        var9 <<= 3;
        let mut var16 = var8 >> 8;
        if self.tex_opaque {
            while blocks > 0 {
                for _ in 0..8 {
                    let var4 = w(Self::texel(texels, var3.0)?);
                    self.put(var5.0, Self::shade_texel(var4, var16))?;
                    var5 += 1;
                    var3 += var17;
                }
                var8 += var9;
                var16 = var8 >> 8;
                blocks -= 1;
            }
            let mut rest = (var7 - var6) & 7;
            while rest > 0 {
                let var4 = w(Self::texel(texels, var3.0)?);
                self.put(var5.0, Self::shade_texel(var4, var16))?;
                var5 += 1;
                var3 += var17;
                rest -= 1;
            }
        } else {
            while blocks > 0 {
                for _ in 0..8 {
                    let var4 = w(Self::texel(texels, var3.0)?);
                    if var4 != w(0) {
                        self.put(var5.0, Self::shade_texel(var4, var16))?;
                    }
                    var5 += 1;
                    var3 += var17;
                }
                var8 += var9;
                var16 = var8 >> 8;
                blocks -= 1;
            }
            let mut rest = (var7 - var6) & 7;
            while rest > 0 {
                let var4 = w(Self::texel(texels, var3.0)?);
                if var4 != w(0) {
                    self.put(var5.0, Self::shade_texel(var4, var16))?;
                }
                var5 += 1;
                var3 += var17;
                rest -= 1;
            }
        }
        let _ = var18;
        Ok(())
    }
}

/// `fq.af(average, shade)`: when a texture is unavailable the face is Gouraud filled with the
/// texture's average hue/saturation and `lum = shade * (average & 127) >> 7` clamped to 2..126.
pub fn merge_texture_shade(average: i32, shade: i32) -> i32 {
    let mut l = shade.wrapping_mul(average & 127) >> 7;
    if l < 2 {
        l = 2;
    } else if l > 126 {
        l = 126;
    }
    (average & 65408) + l
}
