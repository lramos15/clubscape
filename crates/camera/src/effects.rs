use serde::{Deserialize, Serialize};

use crate::{CameraError, CameraOutput, MAX_PITCH, MIN_PITCH, Result, math};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShakeChannel {
    pub random_radius: i32,
    pub sine_amplitude: i32,
    pub sine_frequency: i32,
    pub phase: i32,
    /// One original Math.random-compatible draw, required only when jitter is not suppressed.
    pub random_sample: Option<f64>,
}

/// Source client.vk/pc/ph/pf/po and hg, sampled before client.tz.
/// Channel order: horizontal X, height, horizontal Y, yaw, pitch.
/// Phase and random draws are caller-owned inputs, never invented from render time.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameEffects {
    pub shake: [Option<ShakeChannel>; 5],
    pub suppress_jitter: bool,
}

impl FrameEffects {
    pub const NONE: Self = Self {
        shake: [None; 5],
        suppress_jitter: false,
    };

    pub(crate) fn pitch_floor(self) -> Result<i32> {
        for (axis, channel) in self.shake.into_iter().enumerate() {
            if let Some(channel) = channel {
                if channel.random_radius < 0
                    || channel.random_radius > (i32::MAX - 1) / 2
                    || channel.sine_amplitude < 0
                    || channel.sine_frequency < 0
                {
                    return Err(CameraError::InvalidFrameEffects);
                }
                if !self.suppress_jitter {
                    let sample = channel
                        .random_sample
                        .ok_or(CameraError::MissingShakeSample { axis: axis as u8 })?;
                    if !sample.is_finite() || !(0.0..1.0).contains(&sample) {
                        return Err(CameraError::InvalidFrameEffects);
                    }
                }
            }
        }
        let pitch = match self.shake[4] {
            Some(channel) => MIN_PITCH
                .checked_add(channel.sine_amplitude)
                .ok_or(CameraError::InvalidFrameEffects)?,
            None => 0,
        };
        if pitch > MAX_PITCH {
            return Err(CameraError::InvalidFrameEffects);
        }
        Ok(pitch)
    }

    pub(crate) fn apply(self, output: &mut CameraOutput) -> Result<()> {
        if self.suppress_jitter {
            return Ok(());
        }
        for (axis, channel) in self.shake.into_iter().enumerate() {
            let Some(channel) = channel else {
                continue;
            };
            let sample = channel
                .random_sample
                .ok_or(CameraError::MissingShakeSample { axis: axis as u8 })?;
            let wave =
                libm::sin(f64::from(channel.phase) * (f64::from(channel.sine_frequency) / 100.0))
                    * f64::from(channel.sine_amplitude);
            let delta = (sample * f64::from(channel.random_radius * 2 + 1)
                - f64::from(channel.random_radius)
                + wave) as i32;
            if axis < 3 {
                output.eye_native[axis] = output.eye_native[axis]
                    .checked_add(delta)
                    .ok_or(CameraError::ArithmeticOverflow)?;
                output.eye_float[axis] += delta as f32;
            } else if axis == 3 {
                output.yaw_native = output.yaw_native.wrapping_add(delta) & 16383;
                output.yaw_radians = math::radians(output.yaw_native);
            } else {
                output.pitch_native = output
                    .pitch_native
                    .checked_add(delta)
                    .ok_or(CameraError::ArithmeticOverflow)?
                    .clamp(MIN_PITCH, MAX_PITCH);
                output.pitch_radians = math::radians(output.pitch_native);
            }
        }
        Ok(())
    }
}
