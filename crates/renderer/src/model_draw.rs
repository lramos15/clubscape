//! Exact port of the original model draw path (`fx.be` legacy projection, `fx.xm` scene
//! projection, `fx.ja` culling, `si.av`/`fx.kx` depth and priority ordering, `fx.bz`/`fx.gb`
//! fill dispatch and `fx.cb` near-plane clipping). Output is a [`Tri`] stream in draw order.

#![allow(clippy::too_many_arguments)]

use std::num::Wrapping;

use crate::model::Model;
use crate::raster::{Fill, RasterState, Tri};
use crate::tables::tables;

/// Near plane distance used by every original projection (`50`).
pub const NEAR_Z: i32 = 50;

/// Scratch buffers mirroring the static arrays of `fx`.
#[derive(Default)]
pub struct ModelScratch {
    screen_x: Vec<f32>,
    screen_y: Vec<f32>,
    screen_z: Vec<f32>,
    depth: Vec<i32>,
    cam_x: Vec<i32>,
    cam_y: Vec<i32>,
    cam_z: Vec<i32>,
    culled: Vec<bool>,
    clipped: Vec<bool>,
    clip_x: Vec<bool>,
    order: Vec<(i32, u32)>,
    priority_lists: [Vec<u32>; 12],
    priority_depth_sum: [i32; 12],
    depth10: Vec<i32>,
    depth11: Vec<i32>,
}

impl ModelScratch {
    fn ensure(&mut self, vertices: usize, faces: usize) {
        if self.screen_x.len() < vertices {
            self.screen_x.resize(vertices, 0.0);
            self.screen_y.resize(vertices, 0.0);
            self.screen_z.resize(vertices, 0.0);
            self.depth.resize(vertices, 0);
            self.cam_x.resize(vertices, 0);
            self.cam_y.resize(vertices, 0);
            self.cam_z.resize(vertices, 0);
        }
        if self.culled.len() < faces {
            self.culled.resize(faces, false);
            self.clipped.resize(faces, false);
            self.clip_x.resize(faces, false);
        }
    }
}

/// Camera transform inputs for the scene model path (`fx.xm`).
#[derive(Clone, Copy, Debug)]
pub struct SceneCamera {
    /// 16384-unit pitch/yaw sin/cos as integers (`65536` scale) and floats.
    pub pitch_sin: i32,
    pub pitch_cos: i32,
    pub yaw_sin: i32,
    pub yaw_cos: i32,
    pub pitch_sin_f: f32,
    pub pitch_cos_f: f32,
    pub yaw_sin_f: f32,
    pub yaw_cos_f: f32,
    /// `fq.ae()`: far clip in source units (fixtures use 32768; default is 3500).
    pub far_clip: i32,
}

impl SceneCamera {
    pub fn from_angles(pitch: i32, yaw: i32, far_clip: i32) -> Self {
        let t = tables();
        let p = (pitch & 16383) as usize;
        let y = (yaw & 16383) as usize;
        Self {
            pitch_sin: t.sin16384[p],
            pitch_cos: t.cos16384[p],
            yaw_sin: t.sin16384[y],
            yaw_cos: t.cos16384[y],
            pitch_sin_f: t.sinf16384[p],
            pitch_cos_f: t.cosf16384[p],
            yaw_sin_f: t.sinf16384[y],
            yaw_cos_f: t.cosf16384[y],
            far_clip,
        }
    }
}

/// The draw aborted like the original swallowed exception (division by zero / bad index).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawAbort;

pub struct ModelDrawer<'a> {
    pub state: RasterState,
    pub palette: &'a [i32],
    pub scratch: &'a mut ModelScratch,
    /// `fd.aj`: 2 draws all faces in one pass (stock software path).
    pub alpha_pass: i32,
}

impl<'a> ModelDrawer<'a> {
    /// `fx.be` / `Model.drawFrustum(rotX, yaw, rotZ, cameraPitch, x, y, z)` with 2048-unit angles.
    /// Emits triangles into `out`; returns `Err` where the original would abort the model.
    pub fn draw_legacy(
        &mut self, model: &Model, rot_x: i32, yaw: i32, rot_z: i32, camera_pitch: i32, x: i32, y: i32, z: i32, pick: u32,
        out: &mut Vec<Tri>,
    ) -> Result<(), DrawAbort> {
        let t = tables();
        self.scratch.ensure(model.vertex_count, model.face_count);
        let cx = self.state.center_x;
        let cy = self.state.center_y;
        let (sin_x, cos_x) = (t.sin2048[(rot_x & 2047) as usize], t.cos2048[(rot_x & 2047) as usize]);
        let (sin_y, cos_y) = (t.sin2048[(yaw & 2047) as usize], t.cos2048[(yaw & 2047) as usize]);
        let (sin_z, cos_z) = (t.sin2048[(rot_z & 2047) as usize], t.cos2048[(rot_z & 2047) as usize]);
        let (sin_p, cos_p) = (t.sin2048[(camera_pitch & 2047) as usize], t.cos2048[(camera_pitch & 2047) as usize]);
        let var27 = (y.wrapping_mul(sin_p).wrapping_add(z.wrapping_mul(cos_p))) >> 16;
        let zoom = self.state.zoom;
        let textured = model.textures.is_some();
        for i in 0..model.vertex_count {
            let mut vx = model.xs[i] as i32;
            let mut vy = model.ys[i] as i32;
            let mut vz = model.zs[i] as i32;
            if rot_z != 0 {
                let t0 = (vy.wrapping_mul(sin_z).wrapping_add(vx.wrapping_mul(cos_z))) >> 16;
                vy = (vy.wrapping_mul(cos_z).wrapping_sub(vx.wrapping_mul(sin_z))) >> 16;
                vx = t0;
            }
            if rot_x != 0 {
                let t0 = (vy.wrapping_mul(cos_x).wrapping_sub(vz.wrapping_mul(sin_x))) >> 16;
                vz = (vy.wrapping_mul(sin_x).wrapping_add(vz.wrapping_mul(cos_x))) >> 16;
                vy = t0;
            }
            if yaw != 0 {
                let t0 = (vz.wrapping_mul(sin_y).wrapping_add(vx.wrapping_mul(cos_y))) >> 16;
                vz = (vz.wrapping_mul(cos_y).wrapping_sub(vx.wrapping_mul(sin_y))) >> 16;
                vx = t0;
            }
            vx = vx.wrapping_add(x);
            vy = vy.wrapping_add(y);
            vz = vz.wrapping_add(z);
            let var41 = (vy.wrapping_mul(cos_p).wrapping_sub(vz.wrapping_mul(sin_p))) >> 16;
            vz = (vy.wrapping_mul(sin_p).wrapping_add(vz.wrapping_mul(cos_p))) >> 16;
            self.scratch.depth[i] = vz.wrapping_sub(var27);
            if vz == 0 {
                return Err(DrawAbort);
            }
            self.scratch.screen_x[i] = (cx.wrapping_add(vx.wrapping_mul(zoom).wrapping_div(vz))) as f32;
            self.scratch.screen_y[i] = (cy.wrapping_add(var41.wrapping_mul(zoom).wrapping_div(vz))) as f32;
            self.scratch.screen_z[i] = vz as f32;
            if textured {
                self.scratch.cam_x[i] = vx;
                self.scratch.cam_y[i] = var41;
                self.scratch.cam_z[i] = vz;
            }
        }
        let (radius, range) = model.sphere_bounds();
        self.draw_faces(model, false, radius, range, pick, out)
    }

    /// `fx.xm`: scene model draw with 16384-unit camera angles and float projection. `x/y/z`
    /// are already camera-relative (`position - camera`). Returns `Ok(false)` when culled.
    pub fn draw_scene(
        &mut self, model: &Model, orientation: i32, camera: &SceneCamera, x: i32, y: i32, z: i32, pick: u32, out: &mut Vec<Tri>,
    ) -> Result<bool, DrawAbort> {
        let t = tables();
        self.scratch.ensure(model.vertex_count, model.face_count);
        let b = model.bounds;
        let n9 = camera.pitch_sin;
        let n10 = camera.pitch_cos;
        let n11 = camera.yaw_sin;
        let n12 = camera.yaw_cos;
        let w = Wrapping;
        let n13 = ((w(z) * w(n12) - w(x) * w(n11)) >> 16).0;
        let n14 = ((w(y) * w(n9) + w(n13) * w(n10)) >> 16).0;
        let n15 = ((w(b.radius) * w(n10)) >> 16).0;
        let n16 = n14.wrapping_add(n15);
        if n16 <= NEAR_Z || n14 >= camera.far_clip {
            return Ok(false);
        }
        let zoom = self.state.zoom;
        let n17 = ((w(z) * w(n11) + w(x) * w(n12)) >> 16).0;
        let n18 = (w(n17) - w(b.radius)).0.wrapping_mul(zoom);
        if n18 / n16 >= self.state.clip_pos_x() {
            return Ok(false);
        }
        let n19 = (w(n17) + w(b.radius)).0.wrapping_mul(zoom);
        if n19 / n16 <= self.state.clip_neg_x() {
            return Ok(false);
        }
        let n20 = ((w(y) * w(n10) - w(n13) * w(n9)) >> 16).0;
        let n21 = ((w(b.radius) * w(n9)) >> 16).0;
        let n22 = n21.wrapping_add((w(b.bottom) * w(n10) >> 16).0);
        let n23 = n20.wrapping_add(n22).wrapping_mul(zoom);
        if n23 / n16 <= self.state.clip_neg_y() {
            return Ok(false);
        }
        let n24 = n21.wrapping_add((w(b.height) * w(n10) >> 16).0);
        let n25 = n20.wrapping_sub(n24).wrapping_mul(zoom);
        if n25 / n16 >= self.state.clip_pos_y() {
            return Ok(false);
        }
        let n26 = n15.wrapping_add((w(b.height) * w(n9) >> 16).0);
        let near = n14.wrapping_sub(n26) <= NEAR_Z;
        let store_camera = near || model.textures.is_some();
        let cx = self.state.center_x;
        let cy = self.state.center_y;
        let (f5, f6) = if orientation != 0 {
            (t.sinf2048[(orientation & 2047) as usize], t.cosf2048[(orientation & 2047) as usize])
        } else {
            (0.0f32, 0.0f32)
        };
        let (f, f2, f3, f4) = (camera.pitch_sin_f, camera.pitch_cos_f, camera.yaw_sin_f, camera.yaw_cos_f);
        let zoom_f = zoom as f32;
        let mut any_clipped = false;
        for i in 0..model.vertex_count {
            let mut f8 = (model.xs[i] as i32) as f32;
            let mut f9 = (model.ys[i] as i32) as f32;
            let mut f10 = (model.zs[i] as i32) as f32;
            if orientation != 0 {
                let f7 = f10 * f5 + f8 * f6;
                f10 = f10 * f6 - f8 * f5;
                f8 = f7;
            }
            f10 += z as f32;
            f8 += x as f32;
            let f7 = f10 * f3 + f8 * f4;
            f10 = f10 * f4 - f8 * f3;
            f8 = f7;
            f9 += y as f32;
            let f7 = f9 * f2 - f10 * f;
            f10 = f9 * f + f10 * f2;
            f9 = f7;
            // fx.bf
            self.scratch.depth[i] = (f10 as i32).wrapping_sub(n14);
            if store_camera {
                self.scratch.cam_x[i] = f8 as i32;
                self.scratch.cam_y[i] = f9 as i32;
                self.scratch.cam_z[i] = f10 as i32;
            }
            if f10 >= 50.0 {
                self.scratch.screen_x[i] = cx as f32 + f8 * zoom_f / f10;
                self.scratch.screen_y[i] = cy as f32 + f9 * zoom_f / f10;
                self.scratch.screen_z[i] = f10;
            } else {
                self.scratch.screen_x[i] = -5000.0;
                any_clipped = true;
            }
        }
        self.draw_faces(model, any_clipped, b.bucket_offset, b.bucket_range, pick, out)?;
        Ok(true)
    }

    /// `fx.ja` + `si.av`/`fx.kx`: cull, order and emit faces.
    fn draw_faces(
        &mut self, model: &Model, needs_clipping: bool, bucket_offset: i32, bucket_range: i32, pick: u32, out: &mut Vec<Tri>,
    ) -> Result<(), DrawAbort> {
        if bucket_range >= 6000 {
            return Ok(());
        }
        let width = self.state.width as f32;
        let s = &mut *self.scratch;
        for face in 0..model.face_count {
            if model.color_c[face] == -2 {
                s.culled[face] = true;
                continue;
            }
            if model.render_mode != 2 {
                let alpha_face = model.alphas.as_ref().is_some_and(|a| a[face] != 0);
                if self.alpha_pass == 1 && !alpha_face && model.transparency == 0 {
                    s.culled[face] = true;
                    continue;
                }
                if self.alpha_pass == 0 && (alpha_face || model.transparency != 0) {
                    s.culled[face] = true;
                    continue;
                }
            }
            let a = model.face_a[face] as usize;
            let b = model.face_b[face] as usize;
            let c = model.face_c[face] as usize;
            let xa = s.screen_x[a];
            let xb = s.screen_x[b];
            let xc = s.screen_x[c];
            let clipped = needs_clipping && (xa == -5000.0 || xb == -5000.0 || xc == -5000.0);
            s.clipped[face] = clipped;
            if clipped {
                let w = Wrapping;
                let var15 = w(s.cam_x[a]) - w(s.cam_x[b]);
                let var17 = w(s.cam_x[c]) - w(s.cam_x[b]);
                let var18 = w(s.cam_y[a]) - w(s.cam_y[b]);
                let var20 = w(s.cam_y[c]) - w(s.cam_y[b]);
                let var21 = w(s.cam_z[a]) - w(s.cam_z[b]);
                let var23 = w(s.cam_z[c]) - w(s.cam_z[b]);
                let var24 = var18 * var23 - var21 * var20;
                let var25 = var21 * var17 - var15 * var23;
                let var26 = var15 * var20 - var18 * var17;
                let dot = w(s.cam_x[b]) * var24 + w(s.cam_y[b]) * var25 + w(s.cam_z[b]) * var26;
                s.culled[face] = dot.0 <= 0;
            } else {
                s.culled[face] = (xa - xb) * (s.screen_y[c] - s.screen_y[b]) - (s.screen_y[a] - s.screen_y[b]) * (xc - xb) <= 0.0;
                s.clip_x[face] = xa < 0.0 || xb < 0.0 || xc < 0.0 || xa > width || xb > width || xc > width;
            }
        }
        // si.av(model, true): bucket by average depth (stable), far to near.
        s.order.clear();
        let range = bucket_range;
        for face in 0..model.face_count {
            if s.culled[face] {
                continue;
            }
            let a = model.face_a[face] as usize;
            let b = model.face_b[face] as usize;
            let c = model.face_c[face] as usize;
            let depth = (Wrapping(s.depth[a]) + Wrapping(s.depth[b]) + Wrapping(s.depth[c])).0 / 3 + bucket_offset;
            if !(0..6000).contains(&depth) {
                return Err(DrawAbort);
            }
            if depth < range {
                s.order.push((depth, face as u32));
            }
        }
        s.order.sort_by(|l, r| r.0.cmp(&l.0));
        if let Some(priorities) = &model.priorities {
            // fx.kx
            for list in s.priority_lists.iter_mut() {
                list.clear();
            }
            s.priority_depth_sum = [0; 12];
            s.depth10.clear();
            s.depth11.clear();
            for &(depth, face) in s.order.iter() {
                let p = priorities[face as usize] as usize;
                if p >= 12 {
                    return Err(DrawAbort);
                }
                s.priority_lists[p].push(face);
                if p < 10 {
                    s.priority_depth_sum[p] = s.priority_depth_sum[p].wrapping_add(depth);
                } else if p == 10 {
                    s.depth10.push(depth);
                } else {
                    s.depth11.push(depth);
                }
            }
            let counts: [usize; 12] = std::array::from_fn(|i| s.priority_lists[i].len());
            let sums = s.priority_depth_sum;
            let avg = |i: usize, j: usize| -> i32 {
                if counts[i] > 0 || counts[j] > 0 {
                    sums[i].wrapping_add(sums[j]) / (counts[i] + counts[j]) as i32
                } else {
                    0
                }
            };
            let var14 = avg(1, 2);
            let var15 = avg(3, 4);
            let var16 = avg(6, 8);
            let list10: Vec<u32> = s.priority_lists[10].clone();
            let list11: Vec<u32> = s.priority_lists[11].clone();
            let d10: Vec<i32> = s.depth10.clone();
            let d11: Vec<i32> = s.depth11.clone();
            let ordered: Vec<Vec<u32>> = (0..10).map(|p| s.priority_lists[p].clone()).collect();
            // Cursor over the priority-10 list followed by the priority-11 list.
            let mut using11 = list10.is_empty();
            let mut idx = 0usize;
            let current_depth = |using11: bool, idx: usize| -> i32 {
                let (list, depths) = if using11 { (&list11, &d11) } else { (&list10, &d10) };
                if idx < list.len() { depths[idx] } else { -1000 }
            };
            let mut var17 = current_depth(using11, idx);
            let mut emit_dynamic = |this: &mut Self, using11: &mut bool, idx: &mut usize, var17: &mut i32, out: &mut Vec<Tri>| -> Result<(), DrawAbort> {
                let face = if *using11 { list11[*idx] } else { list10[*idx] };
                this.draw_face(model, face as usize, pick, out)?;
                *idx += 1;
                if !*using11 && *idx == list10.len() {
                    *using11 = true;
                    *idx = 0;
                }
                *var17 = current_depth(*using11, *idx);
                Ok(())
            };
            for var9 in 0..10usize {
                while var9 == 0 && var17 > var14 {
                    emit_dynamic(self, &mut using11, &mut idx, &mut var17, out)?;
                }
                while var9 == 3 && var17 > var15 {
                    emit_dynamic(self, &mut using11, &mut idx, &mut var17, out)?;
                }
                while var9 == 5 && var17 > var16 {
                    emit_dynamic(self, &mut using11, &mut idx, &mut var17, out)?;
                }
                for &face in &ordered[var9] {
                    self.draw_face(model, face as usize, pick, out)?;
                }
            }
            while var17 != -1000 {
                emit_dynamic(self, &mut using11, &mut idx, &mut var17, out)?;
            }
        } else {
            let order: Vec<u32> = s.order.iter().map(|&(_, f)| f).collect();
            for face in order {
                self.draw_face(model, face as usize, pick, out)?;
            }
        }
        Ok(())
    }

    /// `fx.bh`: combine face alpha with the model transparency override.
    fn combine_alpha(model: &Model, n: i32) -> i32 {
        if model.transparency == -1 {
            return 253;
        }
        let n2 = model.transparency & 0xff;
        if n2 <= 0 || n >= 253 {
            return n;
        }
        n + ((253 - n) * n2 >> 8)
    }

    /// `fx.bz`: per-face alpha, clip flag and dispatch.
    fn draw_face(&mut self, model: &Model, face: usize, pick: u32, out: &mut Vec<Tri>) -> Result<(), DrawAbort> {
        if model.transparency == -1 {
            return Ok(());
        }
        let mut alpha = match &model.alphas {
            None => 0,
            Some(a) => (if a[face] == -1 { 253 } else { a[face] as i32 }) & 255,
        };
        if model.transparency != 0 {
            alpha = Self::combine_alpha(model, alpha);
        }
        if !(self.alpha_pass != 1 || alpha != 0 || model.transparency != 0) {
            return Ok(());
        }
        if !(self.alpha_pass != 0 || alpha == 0) {
            return Ok(());
        }
        if self.scratch.clipped[face] {
            return self.draw_clipped_face(model, face, alpha, pick, out);
        }
        let a = model.face_a[face] as usize;
        let b = model.face_b[face] as usize;
        let c = model.face_c[face] as usize;
        let clip_x = self.scratch.clip_x[face];
        let s = &self.scratch;
        let y = [s.screen_y[a] as i32, s.screen_y[b] as i32, s.screen_y[c] as i32];
        let x = [s.screen_x[a] as i32, s.screen_x[b] as i32, s.screen_x[c] as i32];
        self.emit_face(model, face, y, x, [model.color_a[face], model.color_b[face], model.color_c[face]], alpha, clip_x, pick, out)
    }

    /// `fx.gb`: choose flat/Gouraud/textured fill for one triangle with given screen coords.
    fn emit_face(
        &mut self, model: &Model, face: usize, y: [i32; 3], x: [i32; 3], colors: [i32; 3], alpha: i32, clip_x: bool, pick: u32,
        out: &mut Vec<Tri>,
    ) -> Result<(), DrawAbort> {
        let texture = model.textures.as_ref().map(|t| t[face] as i32).unwrap_or(-1);
        if texture != -1 {
            let (p, m, n) = match &model.texture_coords {
                Some(coords) if coords[face] != -1 => {
                    let g = (coords[face] as u8) as usize;
                    (model.tex_p[g] as usize, model.tex_m[g] as usize, model.tex_n[g] as usize)
                }
                _ => (model.face_a[face] as usize, model.face_b[face] as usize, model.face_c[face] as usize),
            };
            let s = &self.scratch;
            let shades = if model.color_c[face] == -1 { [colors[0], colors[0], colors[0]] } else { colors };
            out.push(Tri {
                y,
                x,
                fill: Fill::Textured {
                    colors: shades,
                    px: [s.cam_x[p], s.cam_x[m], s.cam_x[n]],
                    py: [s.cam_y[p], s.cam_y[m], s.cam_y[n]],
                    pz: [s.cam_z[p], s.cam_z[m], s.cam_z[n]],
                    texture,
                    model_variant: true,
                },
                alpha,
                clip_x,
                pick,
            });
        } else if model.color_c[face] == -1 {
            let rgb = *self.palette.get((colors[0] & 0xffff) as usize).ok_or(DrawAbort)?;
            out.push(Tri { y, x, fill: Fill::Flat { rgb }, alpha, clip_x, pick });
        } else {
            out.push(Tri { y, x, fill: Fill::Gouraud { colors }, alpha, clip_x, pick });
        }
        Ok(())
    }

    /// `fx.cb`: near-plane clipping producing one or two triangles.
    fn draw_clipped_face(&mut self, model: &Model, face: usize, alpha: i32, pick: u32, out: &mut Vec<Tri>) -> Result<(), DrawAbort> {
        let t = tables();
        let cx = self.state.center_x;
        let cy = self.state.center_y;
        let zoom = self.state.zoom;
        let n14 = model.face_a[face] as usize;
        let n13 = model.face_b[face] as usize;
        let n12 = model.face_c[face] as usize;
        let s = &self.scratch;
        let n18 = s.cam_z[n14];
        let n19 = s.cam_z[n13];
        let n20 = s.cam_z[n12];
        let w = Wrapping;
        let mut bo = [0i32; 4];
        let mut bu = [0i32; 4];
        let mut ba = [0i32; 4];
        let mut n17 = 0usize;
        let recip = |d: i32| -> Result<i32, DrawAbort> { t.reciprocal16.get(d as usize).copied().ok_or(DrawAbort) };
        let project = |v: i32, zoom: i32| -> i32 { v.wrapping_mul(zoom) / 50 };
        let colors = [model.color_a[face], model.color_b[face], model.color_c[face]];
        // vertex A
        if n18 >= NEAR_Z {
            bo[n17] = s.screen_x[n14] as i32;
            bu[n17] = s.screen_y[n14] as i32;
            ba[n17] = colors[0];
            n17 += 1;
        } else {
            let n11 = s.cam_x[n14];
            let n10 = s.cam_y[n14];
            let n9 = colors[0];
            if n20 >= NEAR_Z {
                let n8 = (w(NEAR_Z - n18) * w(recip(n20 - n18)?)).0;
                bo[n17] = cx.wrapping_add(project(n11.wrapping_add((w(s.cam_x[n12] - n11) * w(n8) >> 16).0), zoom));
                bu[n17] = cy.wrapping_add(project(n10.wrapping_add((w(s.cam_y[n12] - n10) * w(n8) >> 16).0), zoom));
                ba[n17] = n9.wrapping_add((w(colors[2] - n9) * w(n8) >> 16).0);
                n17 += 1;
            }
            if n19 >= NEAR_Z {
                let n8 = (w(NEAR_Z - n18) * w(recip(n19 - n18)?)).0;
                bo[n17] = cx.wrapping_add(project(n11.wrapping_add((w(s.cam_x[n13] - n11) * w(n8) >> 16).0), zoom));
                bu[n17] = cy.wrapping_add(project(n10.wrapping_add((w(s.cam_y[n13] - n10) * w(n8) >> 16).0), zoom));
                ba[n17] = n9.wrapping_add((w(colors[1] - n9) * w(n8) >> 16).0);
                n17 += 1;
            }
        }
        // vertex B
        if n19 >= NEAR_Z {
            bo[n17] = s.screen_x[n13] as i32;
            bu[n17] = s.screen_y[n13] as i32;
            ba[n17] = colors[1];
            n17 += 1;
        } else {
            let n11 = s.cam_x[n13];
            let n10 = s.cam_y[n13];
            let n9 = colors[1];
            if n18 >= NEAR_Z {
                let n8 = (w(NEAR_Z - n19) * w(recip(n18 - n19)?)).0;
                bo[n17] = cx.wrapping_add(project(n11.wrapping_add((w(s.cam_x[n14] - n11) * w(n8) >> 16).0), zoom));
                bu[n17] = cy.wrapping_add(project(n10.wrapping_add((w(s.cam_y[n14] - n10) * w(n8) >> 16).0), zoom));
                ba[n17] = n9.wrapping_add((w(colors[0] - n9) * w(n8) >> 16).0);
                n17 += 1;
            }
            if n20 >= NEAR_Z {
                let n8 = (w(NEAR_Z - n19) * w(recip(n20 - n19)?)).0;
                bo[n17] = cx.wrapping_add(project(n11.wrapping_add((w(s.cam_x[n12] - n11) * w(n8) >> 16).0), zoom));
                bu[n17] = cy.wrapping_add(project(n10.wrapping_add((w(s.cam_y[n12] - n10) * w(n8) >> 16).0), zoom));
                ba[n17] = n9.wrapping_add((w(colors[2] - n9) * w(n8) >> 16).0);
                n17 += 1;
            }
        }
        // vertex C
        if n20 >= NEAR_Z {
            bo[n17] = s.screen_x[n12] as i32;
            bu[n17] = s.screen_y[n12] as i32;
            ba[n17] = colors[2];
            n17 += 1;
        } else {
            let n11 = s.cam_x[n12];
            let n10 = s.cam_y[n12];
            let n9 = colors[2];
            if n19 >= NEAR_Z {
                let n8 = (w(NEAR_Z - n20) * w(recip(n19 - n20)?)).0;
                bo[n17] = cx.wrapping_add(project(n11.wrapping_add((w(s.cam_x[n13] - n11) * w(n8) >> 16).0), zoom));
                bu[n17] = cy.wrapping_add(project(n10.wrapping_add((w(s.cam_y[n13] - n10) * w(n8) >> 16).0), zoom));
                ba[n17] = n9.wrapping_add((w(colors[1] - n9) * w(n8) >> 16).0);
                n17 += 1;
            }
            if n18 >= NEAR_Z {
                let n8 = (w(NEAR_Z - n20) * w(recip(n18 - n20)?)).0;
                bo[n17] = cx.wrapping_add(project(n11.wrapping_add((w(s.cam_x[n14] - n11) * w(n8) >> 16).0), zoom));
                bu[n17] = cy.wrapping_add(project(n10.wrapping_add((w(s.cam_y[n14] - n10) * w(n8) >> 16).0), zoom));
                ba[n17] = n9.wrapping_add((w(colors[0] - n9) * w(n8) >> 16).0);
                n17 += 1;
            }
        }
        let width = self.state.width;
        let outside = |v: i32| v < 0 || v > width;
        if n17 == 3 {
            let clip_x = outside(bo[0]) || outside(bo[1]) || outside(bo[2]);
            return self.emit_face(model, face, [bu[0], bu[1], bu[2]], [bo[0], bo[1], bo[2]], [ba[0], ba[1], ba[2]], alpha, clip_x, pick, out);
        }
        if n17 != 4 {
            return Ok(());
        }
        let clip_x = outside(bo[0]) || outside(bo[1]) || outside(bo[2]) || outside(bo[3]);
        let texture = model.textures.as_ref().map(|t| t[face] as i32).unwrap_or(-1);
        if texture != -1 {
            let (p, m, n) = match &model.texture_coords {
                Some(coords) if coords[face] != -1 => {
                    let g = (coords[face] as u8) as usize;
                    (model.tex_p[g] as usize, model.tex_m[g] as usize, model.tex_n[g] as usize)
                }
                _ => (n14, n13, n12),
            };
            let s = &self.scratch;
            let plane = (
                [s.cam_x[p], s.cam_x[m], s.cam_x[n]],
                [s.cam_y[p], s.cam_y[m], s.cam_y[n]],
                [s.cam_z[p], s.cam_z[m], s.cam_z[n]],
            );
            let flat = model.color_c[face] == -1;
            let c0 = colors[0];
            let shades1 = if flat { [c0, c0, c0] } else { [ba[0], ba[1], ba[2]] };
            let shades2 = if flat { [c0, c0, c0] } else { [ba[0], ba[2], ba[3]] };
            out.push(Tri {
                y: [bu[0], bu[1], bu[2]],
                x: [bo[0], bo[1], bo[2]],
                fill: Fill::Textured { colors: shades1, px: plane.0, py: plane.1, pz: plane.2, texture, model_variant: true },
                alpha,
                clip_x,
                pick,
            });
            out.push(Tri {
                y: [bu[0], bu[2], bu[3]],
                x: [bo[0], bo[2], bo[3]],
                fill: Fill::Textured { colors: shades2, px: plane.0, py: plane.1, pz: plane.2, texture, model_variant: true },
                alpha,
                clip_x,
                pick,
            });
            return Ok(());
        }
        if model.color_c[face] == -1 {
            let rgb = *self.palette.get((colors[0] & 0xffff) as usize).ok_or(DrawAbort)?;
            out.push(Tri { y: [bu[0], bu[1], bu[2]], x: [bo[0], bo[1], bo[2]], fill: Fill::Flat { rgb }, alpha, clip_x, pick });
            out.push(Tri { y: [bu[0], bu[2], bu[3]], x: [bo[0], bo[2], bo[3]], fill: Fill::Flat { rgb }, alpha, clip_x, pick });
        } else {
            out.push(Tri { y: [bu[0], bu[1], bu[2]], x: [bo[0], bo[1], bo[2]], fill: Fill::Gouraud { colors: [ba[0], ba[1], ba[2]] }, alpha, clip_x, pick });
            out.push(Tri { y: [bu[0], bu[2], bu[3]], x: [bo[0], bo[2], bo[3]], fill: Fill::Gouraud { colors: [ba[0], ba[2], ba[3]] }, alpha, clip_x, pick });
        }
        Ok(())
    }
}
