use serde::{Deserialize, Serialize};

use crate::{CameraError, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Viewport {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Viewport {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Result<Self> {
        if width == 0 || height == 0 || width > 32767 || height > 32767 || x < 0 || y < 0 {
            return Err(CameraError::InvalidViewport);
        }
        x.checked_add(width as i32)
            .ok_or(CameraError::ArithmeticOverflow)?;
        y.checked_add(height as i32)
            .ok_or(CameraError::ArithmeticOverflow)?;
        Ok(Self {
            x,
            y,
            width,
            height,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoomBounds {
    pub min_height: i32,
    pub max_height: i32,
    pub min_width: i32,
    pub max_width: i32,
}

impl ZoomBounds {
    pub const PRESET_626: Self = Self {
        min_height: 128,
        max_height: 896,
        min_width: 128,
        max_width: 896,
    };

    pub fn validate(self) -> Result<()> {
        if self.min_height > self.max_height || self.min_width > self.max_width {
            return Err(CameraError::InvalidZoomBounds);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AspectLimits {
    pub min_ratio: u16,
    pub max_ratio: u16,
    pub min_scale: u16,
    pub max_scale: u16,
}

impl Default for AspectLimits {
    fn default() -> Self {
        Self {
            min_ratio: 1,
            max_ratio: 32767,
            min_scale: 1,
            max_scale: 32767,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Projection {
    pub viewport: Viewport,
    pub zoom: i32,
}

pub fn encode_fov(logical: i32) -> i32 {
    let converted = libm::pow(2.0, f64::from(logical) / 256.0 + 7.0) as i32;
    let native_short = converted as i16;
    if native_short <= 0 {
        256
    } else {
        i32::from(native_short)
    }
}

pub fn decode_fov(encoded: i32) -> Result<i32> {
    if encoded <= 0 || encoded > 32767 {
        return Err(CameraError::InvalidFov);
    }
    Ok(((libm::log(f64::from(encoded)) / std::f64::consts::LN_2 - 7.0) * 256.0) as i32)
}

pub(crate) fn factor(height: u32, values: [i32; 2]) -> i32 {
    let weight = (height as i32 - 334).clamp(0, 100);
    // CS2 logical bounds are ints; retain source integer arithmetic before FOV encoding.
    let difference = values[1].wrapping_sub(values[0]).wrapping_mul(weight);
    values[0].wrapping_add(difference / 100)
}

pub fn projection(view: Viewport, fov: [i32; 2], limits: AspectLimits) -> Result<Projection> {
    Viewport::new(view.x, view.y, view.width, view.height)?;
    if fov.iter().any(|v| *v < 1 || *v > 32767) {
        return Err(CameraError::InvalidFov);
    }
    if limits.min_ratio == 0
        || limits.min_scale == 0
        || limits.max_ratio < limits.min_ratio
        || limits.max_scale < limits.min_scale
        || limits.max_ratio > 32767
        || limits.max_scale > 32767
    {
        return Err(CameraError::InvalidViewport);
    }
    let mut width = view.width as i32;
    let mut height = view.height as i32;
    let mut x = view.x;
    let mut y = view.y;
    let mut scale = f64::from(factor(view.height, fov));
    let ratio = f64::from(height) * scale * 512.0 / f64::from(width * 334);
    if ratio < f64::from(limits.min_ratio) {
        let ratio = f64::from(limits.min_ratio);
        scale = ratio * f64::from(width) * 334.0 / f64::from(height * 512);
        if scale > f64::from(limits.max_scale) {
            scale = f64::from(limits.max_scale);
            let fitted = f64::from(height) * scale * 512.0 / (ratio * 334.0);
            let inset = ((f64::from(width) - fitted) / 2.0) as i32;
            x += inset;
            width -= inset * 2;
        }
    } else if ratio > f64::from(limits.max_ratio) {
        let ratio = f64::from(limits.max_ratio);
        scale = ratio * f64::from(width) * 334.0 / f64::from(height * 512);
        if scale < f64::from(limits.min_scale) {
            scale = f64::from(limits.min_scale);
            let fitted = ratio * f64::from(width) * 334.0 / (scale * 512.0);
            let inset = ((f64::from(height) - fitted) / 2.0) as i32;
            y += inset;
            height -= inset * 2;
        }
    }
    let zoom = (f64::from(height) * scale / 334.0) as i32;
    if zoom <= 0 || width <= 0 || height <= 0 {
        return Err(CameraError::InvalidViewport);
    }
    Ok(Projection {
        viewport: Viewport::new(x, y, width as u32, height as u32)?,
        zoom,
    })
}
