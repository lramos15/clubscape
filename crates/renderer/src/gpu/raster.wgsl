// Exact original triangle fills evaluated per pixel in painter order.
//
// One workgroup handles a 16x8 pixel bin plus one ghost column (17x8 invocations) so the
// original flat alpha-254 shift-copy quirk (pixel[x] = pixel[x+1]) can read the neighbor's
// current color through workgroup memory. Every thread walks the bin's triangle list in the
// original draw order and reproduces the scanline algorithms (`ft.ao/al/aj/ay` and their
// scanline routines) in closed form: edge positions are `base + slope * (y - row)` in 14-bit
// fixed point and colors `base + dy * (y - row) + dx * x_start`, with identical truncations.
// All integer arithmetic wraps like Java int.

struct Params {
    width: u32,
    height: u32,
    bins_x: u32,
    bins_y: u32,
    center_x: i32,
    center_y: i32,
    zoom: i32,
    clear_color: u32,
};

@group(0) @binding(0) var<storage, read> tris: array<i32>;
@group(0) @binding(1) var<storage, read> bin_offsets: array<u32>;
@group(0) @binding(2) var<storage, read> bin_tris: array<u32>;
@group(0) @binding(3) var<storage, read> palette: array<u32>;
@group(0) @binding(4) var<storage, read> texels: array<u32>;
// texture_table[id]: 0 = not loaded, otherwise (base_offset + 1) | opaque_flag << 30
@group(0) @binding(5) var<storage, read> texture_table: array<u32>;
@group(0) @binding(6) var<uniform> params: Params;
@group(0) @binding(7) var output: texture_storage_2d<rgba8unorm, write>;

const TRI_STRIDE: u32 = 20u;
const KIND_GOURAUD: u32 = 0u;
const KIND_FLAT: u32 = 1u;
const KIND_TEX_MODEL: u32 = 2u;
const KIND_TEX_TILE: u32 = 3u;

var<workgroup> shared_colors: array<u32, 136>;

struct Row {
    valid: bool,
    xs: i32,
    xe: i32,
    color: i32,
};

struct Plane {
    p43: i32,
    p44: i32,
    p45: i32,
    p46: i32,
    p47: i32,
    p48: i32,
    p49: i32,
    p50: i32,
    p51: i32,
};

// Java `(int)(((long) v * mult) / zoom)` without 64-bit integers: exact because q*mult is an
// integer and the remainder term shares its sign (truncation distributes).
fn long_div(v: i32, mult: i32, zoom: i32) -> i32 {
    let q = v / zoom;
    let rem = v - q * zoom;
    return q * mult + (rem * mult) / zoom;
}

fn edge_at(x: i32, row: i32, slope: i32, y: i32) -> i32 {
    return (x << 14u) + slope * (y - row);
}

// dcolor/dx for Gouraud (shift 8) or textured shade (shift 9); area is nonzero when called.
fn color_dx(base: u32, shift: u32) -> i32 {
    let y1 = tris[base + 0u]; let y2 = tris[base + 1u]; let y3 = tris[base + 2u];
    let x1 = tris[base + 3u]; let x2 = tris[base + 4u]; let x3 = tris[base + 5u];
    let c1 = tris[base + 6u]; let c2 = tris[base + 7u]; let c3 = tris[base + 8u];
    let v19 = x2 - x1; let v20 = y2 - y1; let v21 = x3 - x1; let v22 = y3 - y1;
    let v23 = c2 - c1; let v24 = c3 - c1;
    let area = v19 * v22 - v21 * v20;
    return ((v23 * v22 - v24 * v20) << shift) / area;
}

fn plane_setup(base: u32, model_variant: bool, py: i32) -> Plane {
    var p: Plane;
    let zoom = params.zoom;
    var v13 = tris[base + 11u]; var v14 = tris[base + 12u]; var v15 = tris[base + 13u];
    var v16 = tris[base + 14u]; var v17 = tris[base + 15u]; var v18 = tris[base + 16u];
    var v19 = tris[base + 17u]; var v20 = tris[base + 18u]; var v21 = tris[base + 19u];
    v14 = v13 - v14;
    v17 = v16 - v17;
    v20 = v19 - v20;
    v15 = v15 - v13;
    v18 = v18 - v16;
    v21 = v21 - v19;
    var mult = 16384;
    if (model_variant) { mult = 131072; }
    let p43_0 = (v15 * v16 - v18 * v13) << 14u;
    p.p44 = long_div(v18 * v19 - v21 * v16, mult, zoom);
    p.p45 = long_div(v21 * v13 - v15 * v19, 16384, zoom);
    let p46_0 = (v14 * v16 - v17 * v13) << 14u;
    p.p47 = long_div(v17 * v19 - v20 * v16, mult, zoom);
    p.p48 = long_div(v20 * v13 - v14 * v19, 16384, zoom);
    let p49_0 = (v17 * v15 - v14 * v18) << 14u;
    p.p50 = long_div(v20 * v18 - v17 * v21, mult, zoom);
    p.p51 = long_div(v14 * v21 - v20 * v15, 16384, zoom);
    let dy = py - params.center_y;
    p.p43 = p43_0 + p.p45 * dy;
    p.p46 = p46_0 + p.p48 * dy;
    p.p49 = p49_0 + p.p51 * dy;
    return p;
}

// Per-row state of the original edge walk for pixel row `py`. `shift` is 8 (Gouraud), 9
// (textured shade) or 0 (flat: no color interpolation, no area check).
fn row_state(base: u32, py: i32, shift: u32) -> Row {
    var r: Row;
    r.valid = false;
    r.xs = 0;
    r.xe = 0;
    r.color = 0;
    let y1 = tris[base + 0u]; let y2 = tris[base + 1u]; let y3 = tris[base + 2u];
    let x1 = tris[base + 3u]; let x2 = tris[base + 4u]; let x3 = tris[base + 5u];
    let c1 = tris[base + 6u]; let c2 = tris[base + 7u]; let c3 = tris[base + 8u];
    let height = i32(params.height);

    var s23 = 0;
    if (y3 != y2) { s23 = ((x3 - x2) << 14u) / (y3 - y2); }
    var s12 = 0;
    if (y2 != y1) { s12 = ((x2 - x1) << 14u) / (y2 - y1); }
    var s13 = 0;
    if (y3 != y1) { s13 = ((x3 - x1) << 14u) / (y3 - y1); }

    var cdx = 0;
    var cdy = 0;
    if (shift > 0u) {
        let v19 = x2 - x1; let v20 = y2 - y1; let v21 = x3 - x1; let v22 = y3 - y1;
        let v23 = c2 - c1; let v24 = c3 - c1;
        let area = v19 * v22 - v21 * v20;
        if (area == 0) { return r; }
        cdx = ((v23 * v22 - v24 * v20) << shift) / area;
        cdy = ((v24 * v19 - v23 * v21) << shift) / area;
    }

    var xs_edge = 0;
    var xe_edge = 0;
    if (y1 <= y2 && y1 <= y3) {
        if (y1 >= height) { return r; }
        let y2c = min(y2, height);
        let y3c = min(y3, height);
        r.color = ((c1 << shift) - cdx * x1 + cdx) + cdy * (py - y1);
        let e13 = edge_at(x1, y1, s13, py);
        let e12 = edge_at(x1, y1, s12, py);
        if (y2c < y3c) {
            let e23 = edge_at(x2, y2, s23, py);
            let first = py >= max(y1, 0) && py < y2c;
            let second = py >= max(y2c, 0) && py < y3c;
            if (!first && !second) { return r; }
            let cond = (y1 == y2 || s13 >= s12) && (y1 != y2 || s13 <= s23);
            if (first) {
                if (cond) { xs_edge = e12; xe_edge = e13; } else { xs_edge = e13; xe_edge = e12; }
            } else {
                if (cond) { xs_edge = e23; xe_edge = e13; } else { xs_edge = e13; xe_edge = e23; }
            }
        } else {
            let e32 = edge_at(x3, y3, s23, py);
            let first = py >= max(y1, 0) && py < y3c;
            let second = py >= max(y3c, 0) && py < y2c;
            if (!first && !second) { return r; }
            let cond = (y1 == y3 || s13 >= s12) && (y1 != y3 || s23 <= s12);
            if (first) {
                if (cond) { xs_edge = e12; xe_edge = e13; } else { xs_edge = e13; xe_edge = e12; }
            } else {
                if (cond) { xs_edge = e12; xe_edge = e32; } else { xs_edge = e32; xe_edge = e12; }
            }
        }
    } else if (y2 <= y3) {
        if (y2 >= height) { return r; }
        let y3c = min(y3, height);
        let y1c = min(y1, height);
        r.color = ((c2 << shift) - cdx * x2 + cdx) + cdy * (py - y2);
        let e21 = edge_at(x2, y2, s12, py);
        let e23 = edge_at(x2, y2, s23, py);
        if (y3c < y1c) {
            let e31 = edge_at(x3, y3, s13, py);
            let first = py >= max(y2, 0) && py < y3c;
            let second = py >= max(y3c, 0) && py < y1c;
            if (!first && !second) { return r; }
            let cond = (y2 == y3 || s12 >= s23) && (y2 != y3 || s12 <= s13);
            if (first) {
                if (cond) { xs_edge = e23; xe_edge = e21; } else { xs_edge = e21; xe_edge = e23; }
            } else {
                if (cond) { xs_edge = e31; xe_edge = e21; } else { xs_edge = e21; xe_edge = e31; }
            }
        } else {
            let e13 = edge_at(x1, y1, s13, py);
            let first = py >= max(y2, 0) && py < y1c;
            let second = py >= max(y1c, 0) && py < y3c;
            if (!first && !second) { return r; }
            let cond = s12 < s23;
            if (first) {
                if (cond) { xs_edge = e21; xe_edge = e23; } else { xs_edge = e23; xe_edge = e21; }
            } else {
                if (cond) { xs_edge = e13; xe_edge = e23; } else { xs_edge = e23; xe_edge = e13; }
            }
        }
    } else {
        if (y3 >= height) { return r; }
        let y1c = min(y1, height);
        let y2c = min(y2, height);
        r.color = ((c3 << shift) - cdx * x3 + cdx) + cdy * (py - y3);
        let e32 = edge_at(x3, y3, s23, py);
        let e31 = edge_at(x3, y3, s13, py);
        if (y1c < y2c) {
            let e12 = edge_at(x1, y1, s12, py);
            let first = py >= max(y3, 0) && py < y1c;
            let second = py >= max(y1c, 0) && py < y2c;
            if (!first && !second) { return r; }
            let cond = s23 < s13;
            if (first) {
                if (cond) { xs_edge = e32; xe_edge = e31; } else { xs_edge = e31; xe_edge = e32; }
            } else {
                if (cond) { xs_edge = e32; xe_edge = e12; } else { xs_edge = e12; xe_edge = e32; }
            }
        } else {
            let e21 = edge_at(x2, y2, s12, py);
            let first = py >= max(y3, 0) && py < y2c;
            let second = py >= max(y2c, 0) && py < y1c;
            if (!first && !second) { return r; }
            let cond = s23 < s13;
            if (first) {
                if (cond) { xs_edge = e32; xe_edge = e31; } else { xs_edge = e31; xe_edge = e32; }
            } else {
                if (cond) { xs_edge = e21; xe_edge = e31; } else { xs_edge = e31; xe_edge = e21; }
            }
        }
    }
    r.valid = true;
    r.xs = xs_edge >> 14u;
    r.xe = xe_edge >> 14u;
    return r;
}

fn blend_channels(rgb: i32, weight: i32) -> i32 {
    return (((rgb & 16711935) * weight >> 8u) & 16711935) + (((rgb & 65280) * weight >> 8u) & 65280);
}

fn shade_texel(t: i32, shade: i32) -> i32 {
    return (((((t & 16711935) * shade) & -16711936) + (((t & 65280) * shade) & 16711680)) >> 8u) | i32(0xFF000000u);
}

fn blend_texel(t: i32, premul: i32, dst: i32, alpha: i32) -> i32 {
    let src = ((((t & 16711935) * premul) & -16711936) | (((t & 65280) * premul) & 16711680)) >> 8u;
    let d = ((((dst & 16711935) * alpha) & -16711936) | (((dst & 65280) * alpha) & 16711680)) >> 8u;
    return (src + d) | i32(0xFF000000u);
}

fn texel_at(tex_base: u32, v3: i32) -> i32 {
    let index = u32(v3 & 16256) + (u32(v3) >> 25u);
    return i32(texels[tex_base + index]);
}

fn clamp_u(u: i32) -> i32 {
    if (u < 0) { return 0; }
    if (u > 16256) { return 16256; }
    return u;
}

@compute @workgroup_size(17, 8, 1)
fn main(@builtin(workgroup_id) wg: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>) {
    let px = i32(wg.x * 16u + lid.x);
    let py = i32(wg.y * 8u + lid.y);
    let in_image = px < i32(params.width) && py < i32(params.height);
    let local_index = lid.y * 17u + lid.x;
    var color: i32 = i32(params.clear_color);
    let bin = wg.y * params.bins_x + wg.x;
    let begin = bin_offsets[bin];
    let end = bin_offsets[bin + 1u];
    let width = i32(params.width);
    for (var i = begin; i < end; i = i + 1u) {
        let t = bin_tris[i];
        let base = t * TRI_STRIDE;
        let flags = u32(tris[base + 9u]);
        let kind = flags & 3u;
        let alpha = i32((flags >> 8u) & 255u);
        let clip_x = (flags & 4u) != 0u;
        let quirk = kind == KIND_FLAT && alpha == 254;
        var shift: u32 = 8u;
        if (kind == KIND_FLAT) { shift = 0u; }
        if (kind >= KIND_TEX_MODEL) { shift = 9u; }
        var covered = false;
        var new_color = color;
        if (in_image) {
            let row = row_state(base, py, shift);
            if (row.valid) {
                var xs = row.xs;
                var xe = row.xe;
                if (clip_x) {
                    if (xe > width) { xe = width; }
                    if (xs < 0) { xs = 0; }
                }
                if (xs < xe && px >= xs && px < xe) {
                    covered = true;
                    if (kind == KIND_GOURAUD) {
                        let cdx = color_dx(base, 8u);
                        let var7 = row.color + cdx * xs;
                        let k = (px - xs) >> 2u;
                        let v = var7 + (cdx << 2u) * k;
                        let idx = (v & ~(v >> 31u)) >> 8u;
                        let rgb = i32(palette[u32(idx) & 65535u]);
                        if (alpha == 0) {
                            new_color = rgb;
                        } else {
                            new_color = blend_channels(rgb, 256 - alpha) + blend_channels(color, alpha);
                        }
                    } else if (kind == KIND_FLAT) {
                        let rgb = tris[base + 6u];
                        if (alpha == 0) {
                            new_color = rgb;
                        } else if (alpha == 254) {
                            new_color = color;
                        } else {
                            new_color = blend_channels(rgb, 256 - alpha) + blend_channels(color, alpha);
                        }
                    } else {
                        let texture_id = tris[base + 10u];
                        let entry = texture_table[u32(texture_id)];
                        let tex_base = (entry & 0x3FFFFFFFu) - 1u;
                        let tex_opaque = (entry & 0x40000000u) != 0u;
                        let model_variant = kind == KIND_TEX_MODEL;
                        let p = plane_setup(base, model_variant, py);
                        let cdx = color_dx(base, 9u);
                        let var8 = row.color + cdx * xs;
                        let v24 = xs - params.center_x;
                        let j = px - xs;
                        let k = j >> 3u;
                        var v3 = 0;
                        if (model_variant) {
                            let a10 = p.p43 + (p.p44 >> 3u) * v24;
                            let a11 = p.p46 + (p.p47 >> 3u) * v24;
                            let a12 = p.p49 + (p.p50 >> 3u) * v24;
                            let s10 = a10 + p.p44 * k;
                            let s11 = a11 + p.p47 * k;
                            let s12 = a12 + p.p50 * k;
                            let b10 = s10 + p.p44;
                            let b11 = s11 + p.p47;
                            let b12 = s12 + p.p50;
                            var u0 = 0; var w0 = 0;
                            let z0 = s12 >> 14u;
                            if (z0 != 0) { u0 = clamp_u(s10 / z0); w0 = s11 / z0; }
                            var u1 = 0; var w1 = 0;
                            let z1 = b12 >> 14u;
                            if (z1 != 0) { u1 = clamp_u(b10 / z1); w1 = b11 / z1; }
                            let step = (((u1 - u0) >> 3u) << 18u) + ((w1 - w0) >> 3u);
                            v3 = (u0 << 18u) + w0 + step * (j & 7);
                        } else {
                            let len = xe - xs;
                            let a10 = p.p43 + p.p44 * v24;
                            let a11 = p.p46 + p.p47 * v24;
                            let a12 = p.p49 + p.p50 * v24;
                            var u0 = 0; var w0 = 0;
                            let z0 = a12 >> 14u;
                            if (z0 != 0) { u0 = a10 / z0; w0 = a11 / z0; }
                            let b10 = a10 + p.p44 * len;
                            let b11 = a11 + p.p47 * len;
                            let b12 = a12 + p.p50 * len;
                            var u1 = 0; var w1 = 0;
                            let z1 = b12 >> 14u;
                            if (z1 != 0) { u1 = b10 / z1; w1 = b11 / z1; }
                            let step = (((u1 - u0) / len) << 18u) + (w1 - w0) / len;
                            v3 = (u0 << 18u) + w0 + step * j;
                        }
                        let shade = (var8 + (cdx << 3u) * k) >> 8u;
                        let texel = texel_at(tex_base, v3);
                        if (!model_variant || alpha == 0) {
                            if (tex_opaque || texel != 0) {
                                new_color = shade_texel(texel, shade);
                            }
                        } else {
                            let shade0 = var8 >> 8u;
                            let premul = (shade0 * (256 - alpha)) >> 8u;
                            if (tex_opaque || texel != 0) {
                                new_color = blend_texel(texel, premul, color, alpha);
                            }
                        }
                    }
                }
            }
        }
        if (quirk) {
            shared_colors[local_index] = u32(color);
            workgroupBarrier();
            var neighbor = color;
            if (lid.x < 16u) {
                neighbor = i32(shared_colors[local_index + 1u]);
            }
            workgroupBarrier();
            if (covered) {
                new_color = neighbor;
            }
        }
        if (covered) {
            color = new_color;
        }
    }
    if (in_image && lid.x < 16u) {
        let c = u32(color);
        let r = f32((c >> 16u) & 255u) / 255.0;
        let g = f32((c >> 8u) & 255u) / 255.0;
        let b = f32(c & 255u) / 255.0;
        textureStore(output, vec2<i32>(px, py), vec4<f32>(r, g, b, 1.0));
    }
}
