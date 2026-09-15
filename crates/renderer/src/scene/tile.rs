//! Exact port of the original tile paint (`eu.qu` + `fv.zw`) and tile model (`eu.ae` + `ei.pv`)
//! drawing: float camera transform, float projection, backface test and fill dispatch.

use super::{SceneData, TileModel, TilePaint};
use crate::raster::{Fill, RasterState, Tri};

/// The hidden-color marker used by the original for invisible paint corners.
pub const HIDDEN_COLOR: i32 = 12345678;

/// Float camera used by the tile drawing (`eu`).
#[derive(Clone, Copy, Debug)]
pub struct TileCamera {
    pub x: i32,
    pub height: i32,
    pub z: i32,
    pub pitch_sin: f32,
    pub pitch_cos: f32,
    pub yaw_sin: f32,
    pub yaw_cos: f32,
}

pub struct TileScratch {
    screen_x: Vec<f32>,
    screen_y: Vec<f32>,
    cam_x: Vec<i32>,
    cam_y: Vec<i32>,
    cam_z: Vec<i32>,
}

impl Default for TileScratch {
    fn default() -> Self {
        Self {
            screen_x: vec![0.0; 64],
            screen_y: vec![0.0; 64],
            cam_x: vec![0; 64],
            cam_y: vec![0; 64],
            cam_z: vec![0; 64],
        }
    }
}

/// `eu.qu`: transforms the four corners of a tile paint and emits its two triangles.
#[allow(clippy::too_many_arguments)]
pub fn draw_tile_paint(
    scene: &SceneData,
    state: &RasterState,
    camera: &TileCamera,
    paint: &TilePaint,
    plane: i32,
    x: i32,
    y: i32,
    pick: u32,
    out: &mut Vec<Tri>,
) {
    let ex = x + scene.offset;
    let ey = y + scene.offset;
    let f5 = ((x << 7) - camera.x) as f32;
    let f4 = f5;
    let f6 = ((y << 7) - camera.z) as f32;
    let f3 = f6;
    let f7 = f5 + 128.0;
    let f2 = f7;
    let f8 = f6 + 128.0;
    let f = f8;
    let f9 = (scene.height(plane, ex, ey) - camera.height) as f32;
    let f10 = (scene.height(plane, ex + 1, ey) - camera.height) as f32;
    let f11 = (scene.height(plane, ex + 1, ey + 1) - camera.height) as f32;
    let f12 = (scene.height(plane, ex, ey + 1) - camera.height) as f32;
    let (aa, ac, ax, as_) = (
        camera.yaw_cos,
        camera.yaw_sin,
        camera.pitch_cos,
        camera.pitch_sin,
    );
    // corner 1
    let f13 = f5 * aa + f6 * ac;
    let f6 = f6 * aa - f5 * ac;
    let f5 = f13;
    let f13 = f9 * ax - f6 * as_;
    let f6 = f6 * ax + f9 * as_;
    let f9 = f13;
    if f6 < 50.0 {
        return;
    }
    // corner 2
    let f13 = f3 * ac + f7 * aa;
    let f3 = f3 * aa - f7 * ac;
    let f7 = f13;
    let f13 = f10 * ax - f3 * as_;
    let f3 = f3 * ax + f10 * as_;
    let f10 = f13;
    if f3 < 50.0 {
        return;
    }
    // corner 3
    let f13 = f2 * aa + f8 * ac;
    let f8 = f8 * aa - f2 * ac;
    let f2 = f13;
    let f13 = f11 * ax - f8 * as_;
    let f8 = f11 * as_ + f8 * ax;
    let f11 = f13;
    if f8 < 50.0 {
        return;
    }
    // corner 4
    let f13 = f4 * aa + f * ac;
    let f = f * aa - f4 * ac;
    let f4 = f13;
    let f13 = f12 * ax - f * as_;
    let f = f12 * as_ + f * ax;
    if f < 50.0 {
        return;
    }
    // fv.zw(f=x1c, f2=x2c, f3=x3c, f4=x4c, f5..f8 = y's, f9..f12 = depths)
    emit_paint(
        state,
        paint,
        [f5, f7, f2, f4],
        [f9, f10, f11, f13],
        [f6, f3, f8, f],
        pick,
        out,
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_paint(
    state: &RasterState,
    paint: &TilePaint,
    xc: [f32; 4],
    yc: [f32; 4],
    zc: [f32; 4],
    pick: u32,
    out: &mut Vec<Tri>,
) {
    let zoom = state.zoom as f32;
    let cx = state.center_x as f32;
    let cy = state.center_y as f32;
    let width = state.width as f32;
    let f14 = cx + xc[0] * zoom / zc[0];
    let f15 = cy + yc[0] * zoom / zc[0];
    let f16 = cx + xc[1] * zoom / zc[1];
    let f17 = cy + yc[1] * zoom / zc[1];
    let f18 = cx + xc[2] * zoom / zc[2];
    let f19 = cy + yc[2] * zoom / zc[2];
    let f20 = cx + xc[3] * zoom / zc[3];
    let f21 = cy + yc[3] * zoom / zc[3];
    let outside = |a: f32, b: f32, c: f32| {
        a < 0.0 || b < 0.0 || c < 0.0 || a > width || b > width || c > width
    };
    if (f17 - f21) * (f18 - f20) - (f19 - f21) * (f16 - f20) > 0.0 {
        let clip_x = outside(f18, f20, f16);
        let y = [f19 as i32, f21 as i32, f17 as i32];
        let x = [f18 as i32, f20 as i32, f16 as i32];
        if paint.texture == -1 {
            if paint.ne != HIDDEN_COLOR {
                out.push(Tri {
                    y,
                    x,
                    fill: Fill::Gouraud {
                        colors: [paint.ne, paint.nw, paint.se],
                    },
                    alpha: 0,
                    clip_x,
                    pick,
                });
            }
        } else {
            let (px, py, pz) = if paint.flat {
                (
                    [xc[0] as i32, xc[1] as i32, xc[3] as i32],
                    [yc[0] as i32, yc[1] as i32, yc[3] as i32],
                    [zc[0] as i32, zc[1] as i32, zc[3] as i32],
                )
            } else {
                (
                    [xc[2] as i32, xc[3] as i32, xc[1] as i32],
                    [yc[2] as i32, yc[3] as i32, yc[1] as i32],
                    [zc[2] as i32, zc[3] as i32, zc[1] as i32],
                )
            };
            out.push(Tri {
                y,
                x,
                fill: Fill::Textured {
                    colors: [paint.ne, paint.nw, paint.se],
                    px,
                    py,
                    pz,
                    texture: paint.texture,
                    model_variant: false,
                },
                alpha: 0,
                clip_x,
                pick,
            });
        }
    }
    if (f21 - f17) * (f14 - f16) - (f15 - f17) * (f20 - f16) > 0.0 {
        let clip_x = outside(f14, f16, f20);
        let y = [f15 as i32, f17 as i32, f21 as i32];
        let x = [f14 as i32, f16 as i32, f20 as i32];
        if paint.texture == -1 {
            if paint.sw != HIDDEN_COLOR {
                out.push(Tri {
                    y,
                    x,
                    fill: Fill::Gouraud {
                        colors: [paint.sw, paint.se, paint.nw],
                    },
                    alpha: 0,
                    clip_x,
                    pick,
                });
            }
        } else {
            out.push(Tri {
                y,
                x,
                fill: Fill::Textured {
                    colors: [paint.sw, paint.se, paint.nw],
                    px: [xc[0] as i32, xc[1] as i32, xc[3] as i32],
                    py: [yc[0] as i32, yc[1] as i32, yc[3] as i32],
                    pz: [zc[0] as i32, zc[1] as i32, zc[3] as i32],
                    texture: paint.texture,
                    model_variant: false,
                },
                alpha: 0,
                clip_x,
                pick,
            });
        }
    }
}

/// `eu.ae` + `ei.pv`: transforms and emits a shaped tile model.
#[allow(clippy::too_many_arguments)]
pub fn draw_tile_model(
    state: &RasterState,
    camera: &TileCamera,
    model: &TileModel,
    scratch: &mut TileScratch,
    pick: u32,
    out: &mut Vec<Tri>,
) {
    let count = model.xs.len();
    if scratch.screen_x.len() < count {
        scratch.screen_x.resize(count, 0.0);
        scratch.screen_y.resize(count, 0.0);
        scratch.cam_x.resize(count, 0);
        scratch.cam_y.resize(count, 0);
        scratch.cam_z.resize(count, 0);
    }
    let zoom = state.zoom as f32;
    let cx = state.center_x as f32;
    let cy = state.center_y as f32;
    let textured = model.textures.is_some();
    for i in 0..count {
        let f8 = (model.xs[i] - camera.x) as f32;
        let f9 = (model.ys[i] - camera.height) as f32;
        let mut f10 = (model.zs[i] - camera.z) as f32;
        let mut f11 = f10 * camera.yaw_sin + camera.yaw_cos * f8;
        f10 = camera.yaw_cos * f10 - f8 * camera.yaw_sin;
        let rot_x = f11;
        f11 = f9 * camera.pitch_cos - f10 * camera.pitch_sin;
        f10 = f9 * camera.pitch_sin + f10 * camera.pitch_cos;
        if f10 < 50.0 {
            return;
        }
        if textured {
            scratch.cam_x[i] = rot_x as i32;
            scratch.cam_y[i] = f11 as i32;
            scratch.cam_z[i] = f10 as i32;
        }
        scratch.screen_x[i] = cx + rot_x * zoom / f10;
        scratch.screen_y[i] = cy + f11 * zoom / f10;
    }
    let width = state.width as f32;
    for i in 0..model.face_a.len() {
        let n5 = model.face_a[i] as usize;
        let n6 = model.face_b[i] as usize;
        let n7 = model.face_c[i] as usize;
        let f = scratch.screen_x[n5];
        let f2 = scratch.screen_x[n6];
        let f3 = scratch.screen_x[n7];
        let f4 = scratch.screen_y[n5];
        let f5 = scratch.screen_y[n6];
        let f6 = scratch.screen_y[n7];
        // Original `!(cross > 0)` keeps NaN faces (the compare is false for NaN → continue).
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        if !((f - f2) * (f6 - f5) - (f4 - f5) * (f3 - f2) > 0.0) {
            continue;
        }
        let clip_x = f < 0.0 || f2 < 0.0 || f3 < 0.0 || f > width || f2 > width || f3 > width;
        let y = [f4 as i32, f5 as i32, f6 as i32];
        let x = [f as i32, f2 as i32, f3 as i32];
        let colors = [model.color_a[i], model.color_b[i], model.color_c[i]];
        let texture = model.textures.as_ref().map(|t| t[i]).unwrap_or(-1);
        if texture != -1 {
            let (p, m, n) = if model.flat {
                (0usize, 1usize, 3usize)
            } else {
                (n5, n6, n7)
            };
            out.push(Tri {
                y,
                x,
                fill: Fill::Textured {
                    colors,
                    px: [scratch.cam_x[p], scratch.cam_x[m], scratch.cam_x[n]],
                    py: [scratch.cam_y[p], scratch.cam_y[m], scratch.cam_y[n]],
                    pz: [scratch.cam_z[p], scratch.cam_z[m], scratch.cam_z[n]],
                    texture,
                    model_variant: false,
                },
                alpha: 0,
                clip_x,
                pick,
            });
            continue;
        }
        if model.color_a[i] == HIDDEN_COLOR {
            continue;
        }
        out.push(Tri {
            y,
            x,
            fill: Fill::Gouraud { colors },
            alpha: 0,
            clip_x,
            pick,
        });
    }
}
