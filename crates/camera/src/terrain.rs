use serde::{Deserialize, Serialize};

use crate::{CameraError, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldBase {
    pub x: i32,
    pub y: i32,
}

/// Original local render and logical coordinates, not an eye or inferred tile centre.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Focus {
    pub identity: i32,
    pub world_base: WorldBase,
    pub logical: [i32; 2],
    pub rendered: [f32; 2],
    pub plane: u8,
    pub footprint: u16,
    pub kind: FocusKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FocusKind {
    Actor,
    Point,
}

pub trait FocusProvider {
    fn camera_focus(&self) -> Result<Focus>;
}

impl FocusProvider for Focus {
    fn camera_focus(&self) -> Result<Focus> {
        Ok(*self)
    }
}

/// Height corners include the far edge (width+1, height+1).
/// Missing source data must return an error, never a synthesized height/flag.
pub trait Terrain {
    fn world_base(&self) -> WorldBase;
    fn dimensions(&self) -> [u32; 2];
    fn height_corner(&self, plane: u8, x: u32, y: u32) -> Result<i32>;
    fn tile_settings(&self, plane: u8, x: u32, y: u32) -> Result<u8>;
    /// Original rendered tile surface query (dz.ad), including source triangle order
    /// and ground-decoration raise. This is not interchangeable with bilinear height.
    fn surface_height(&self, plane: u8, local_x: i32, local_y: i32) -> Result<i32>;
}

pub(crate) fn validate_focus(focus: Focus, terrain: &impl Terrain, base: WorldBase) -> Result<()> {
    if focus.identity < 0 {
        return Err(CameraError::MissingFocus);
    }
    if focus.world_base != base || terrain.world_base() != base {
        return Err(CameraError::WorldBaseMismatch);
    }
    if focus.plane > 3 || !focus.rendered.iter().all(|v| v.is_finite()) {
        return Err(CameraError::InvalidFocus);
    }
    let dimensions = terrain.dimensions();
    if dimensions.contains(&0) {
        return Err(CameraError::EmptyTerrain);
    }
    for (axis, dimension) in dimensions.into_iter().enumerate() {
        let bound = dimension as f64 * 128.0;
        if focus.logical[axis] < 0
            || f64::from(focus.logical[axis]) >= bound
            || focus.rendered[axis] < 0.0
            || f64::from(focus.rendered[axis]) >= bound
        {
            return Err(CameraError::FocusOutsideTerrain);
        }
    }
    Ok(())
}

fn tile(terrain: &impl Terrain, x: f32, y: f32, plane: u8) -> Result<(u32, u32, u8)> {
    if !x.is_finite() || !y.is_finite() || plane > 3 {
        return Err(CameraError::InvalidFocus);
    }
    let dimensions = terrain.dimensions();
    if x < 0.0
        || y < 0.0
        || f64::from(x) >= f64::from(dimensions[0]) * 128.0
        || f64::from(y) >= f64::from(dimensions[1]) * 128.0
    {
        return Err(CameraError::FocusOutsideTerrain);
    }
    let tx = (x / 128.0) as u32;
    let ty = (y / 128.0) as u32;
    let effective = if plane < 3 && terrain.tile_settings(1, tx, ty)? & 2 != 0 {
        plane + 1
    } else {
        plane
    };
    Ok((tx, ty, effective))
}

pub fn bilinear_height(terrain: &impl Terrain, x: f32, y: f32, plane: u8) -> Result<f32> {
    let (tx, ty, p) = tile(terrain, x, y, plane)?;
    let dx = x % 128.0;
    let dy = y % 128.0;
    let a = ((128.0 - dx) * terrain.height_corner(p, tx, ty)? as f32
        + dx * terrain.height_corner(p, tx + 1, ty)? as f32)
        / 128.0;
    let b = (terrain.height_corner(p, tx, ty + 1)? as f32 * (128.0 - dx)
        + dx * terrain.height_corner(p, tx + 1, ty + 1)? as f32)
        / 128.0;
    Ok((dy * b + a * (128.0 - dy)) / 128.0)
}

pub fn integer_height(terrain: &impl Terrain, x: i32, y: i32, plane: u8) -> Result<i32> {
    tile(terrain, x as f32, y as f32, plane)?;
    terrain.surface_height(plane, x, y)
}

pub fn integer_bilinear_height(terrain: &impl Terrain, x: i32, y: i32, plane: u8) -> Result<i32> {
    let (tx, ty, p) = tile(terrain, x as f32, y as f32, plane)?;
    let dx = i64::from(x & 127);
    let dy = i64::from(y & 127);
    let a = ((128 - dx) * i64::from(terrain.height_corner(p, tx, ty)?)
        + dx * i64::from(terrain.height_corner(p, tx + 1, ty)?))
        >> 7;
    let b = ((128 - dx) * i64::from(terrain.height_corner(p, tx, ty + 1)?)
        + dx * i64::from(terrain.height_corner(p, tx + 1, ty + 1)?))
        >> 7;
    i32::try_from(((128 - dy) * a + dy * b) >> 7).map_err(|_| CameraError::ArithmeticOverflow)
}

/// One original surface triangle, in source draw/query order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceTriangle {
    pub horizontal: [[i32; 2]; 3],
    pub heights: [i32; 3],
}

impl SurfaceTriangle {
    /// Original vi.aq float barycentric evaluation; the final conversion truncates.
    pub fn height_at(self, point: [i32; 2]) -> Result<Option<i32>> {
        let [[ax, ay], [bx, by], [cx, cy]] = self.horizontal.map(|p| p.map(i128::from));
        let [x, y] = point.map(i128::from);
        let denominator = (ay - cy) * (cx - bx) + (ax - cx) * (by - cy);
        if denominator == 0 {
            return Err(CameraError::InvalidSurface);
        }
        let numerator_a = (x - cx) * (by - cy) + (y - cy) * (cx - bx);
        let numerator_b = (ax - cx) * (y - cy) + (x - cx) * (cy - ay);
        let numerator_c = denominator - numerator_a - numerator_b;
        if [numerator_a, numerator_b, numerator_c]
            .iter()
            .any(|n| if denominator > 0 { *n < 0 } else { *n > 0 })
        {
            return Ok(None);
        }
        let a = numerator_a as f32 / denominator as f32;
        let b = numerator_b as f32 / denominator as f32;
        let c = 1.0 - a - b;
        Ok(Some(
            (b * self.heights[1] as f32 + a * self.heights[0] as f32 + c * self.heights[2] as f32)
                as i32,
        ))
    }
}

pub fn footprint_height(terrain: &impl Terrain, focus: Focus) -> Result<f32> {
    let [x, y] = focus.rendered;
    if focus.footprint == 0 {
        return bilinear_height(terrain, x, y, focus.plane);
    }
    let half = f32::from(focus.footprint / 2);
    let mut height = f32::MAX;
    let mut grid_x = (x - half) / 128.0 + 1.0;
    while grid_x <= (x + half) / 128.0 {
        let mut grid_y = (y - half) / 128.0 + 1.0;
        while grid_y <= (y + half) / 128.0 {
            height = height.min(bilinear_height(
                terrain,
                grid_x * 128.0,
                grid_y * 128.0,
                focus.plane,
            )?);
            grid_y += 1.0;
        }
        grid_x += 1.0;
    }
    for [sx, sy] in [
        [x, y],
        [x - half, y - half],
        [x - half, y + half],
        [x + half, y - half],
        [x + half, y + half],
    ] {
        height = height.min(bilinear_height(terrain, sx, sy, focus.plane)?);
    }
    Ok(height)
}

pub(crate) fn terrain_pitch_target(
    terrain: &impl Terrain,
    focal: [i32; 2],
    plane: u8,
) -> Result<i32> {
    let ground = integer_height(terrain, focal[0], focal[1], plane)?;
    let x = focal[0] >> 7;
    let y = focal[1] >> 7;
    let mut drop = 0_i32;
    if x > 3 && y > 3 && x < 100 && y < 100 {
        for sx in x - 4..=x + 4 {
            for sy in y - 4..=y + 4 {
                let sx = sx as u32;
                let sy = sy as u32;
                if sx >= terrain.dimensions()[0] || sy >= terrain.dimensions()[1] {
                    return Err(CameraError::FocusOutsideTerrain);
                }
                let p = if plane < 3 && terrain.tile_settings(1, sx, sy)? & 2 != 0 {
                    plane + 1
                } else {
                    plane
                };
                drop = drop.max(
                    ground
                        .checked_sub(terrain.height_corner(p, sx, sy)?)
                        .ok_or(CameraError::ArithmeticOverflow)?,
                );
            }
        }
    }
    Ok((i64::from(drop) * 1536).clamp(1024 * 256, 3064 * 256) as i32)
}
