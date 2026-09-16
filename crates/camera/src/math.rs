pub(crate) const UNIT: f32 = 3.834_952e-4;
pub(crate) const LEGACY_UNIT: f32 = 0.003_067_961_7;
pub(crate) const MIN_PITCH_RAD: f32 = std::f32::consts::FRAC_PI_8;
pub(crate) const MAX_PITCH_RAD: f32 = 1.175_029_3;
pub(crate) const TURN: f32 = std::f32::consts::TAU;
const TABLE_UNIT: f64 = 3.834951969714103e-4;

pub(crate) fn java_round(value: f32) -> i32 {
    libm::floor(f64::from(value) + 0.5) as i32
}

pub(crate) fn angle(value: f32) -> i32 {
    java_round(value * 2607.5945) & 16383
}

pub(crate) fn legacy_angle(value: f32) -> i32 {
    java_round(value * 325.9493) & 2047
}

pub(crate) fn radians(value: i32) -> f32 {
    value as f32 * UNIT
}

pub(crate) fn trig(angle: i32) -> (i32, i32) {
    let radians = f64::from(angle & 16383) * TABLE_UNIT;
    (
        (65536.0 * libm::sin(radians)) as i32,
        (65536.0 * libm::cos(radians)) as i32,
    )
}

pub(crate) fn native_orbit(focal: [i32; 3], pitch: i32, yaw: i32, distance: i32) -> [i32; 3] {
    let (sp, cp) = trig(16384 - pitch);
    let (sy, cy) = trig(16384 - yaw);
    // cz.az uses JVM imul/ishr, including overflow at large native distance scales.
    let height = sp.wrapping_mul(distance).wrapping_neg() >> 16;
    let horizontal = cp.wrapping_mul(distance) >> 16;
    let x = sy.wrapping_mul(horizontal) >> 16;
    let y = cy.wrapping_mul(horizontal) >> 16;
    [
        focal[0].wrapping_sub(x),
        focal[1].wrapping_sub(height),
        focal[2].wrapping_sub(y),
    ]
}

pub(crate) fn float_orbit(focal: [f32; 3], pitch: f32, yaw: f32, distance: i32) -> [f32; 3] {
    let inverse_pitch = TURN - pitch;
    let inverse_yaw = TURN - yaw;
    let sp = libm::sin(f64::from(inverse_pitch)) as f32;
    let cp = libm::cos(f64::from(inverse_pitch)) as f32;
    let height = cp * 0.0 - sp * distance as f32;
    let horizontal = cp * distance as f32 + sp * 0.0;
    let sy = libm::sin(f64::from(inverse_yaw)) as f32;
    let cy = libm::cos(f64::from(inverse_yaw)) as f32;
    let x = sy * horizontal + cy * 0.0;
    let y = cy * horizontal - sy * 0.0;
    [focal[0] - x, focal[1] - height, focal[2] - y]
}

#[cfg(test)]
mod tests {
    use super::java_round;

    #[test]
    fn source_rounding_uses_ties_toward_positive_infinity() {
        for (input, expected) in [
            (-1.5, -1),
            (-0.5, 0),
            (0.5, 1),
            (1.5, 2),
            (8_388_609.0, 8_388_609),
        ] {
            assert_eq!(java_round(input), expected);
        }
    }
}
