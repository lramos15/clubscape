//! Original visible-tile table (`ez.dx/dy/di` with the `client.cw` pitch heights).
//!
//! The runtime lazily marks which tile offsets around the camera can project into a virtual
//! 334-pixel-high viewport for the current pitch/yaw buckets; tiles outside are skipped unless
//! they are far below the camera.

use std::collections::HashMap;

use crate::tables::tables;

pub const PITCH_BUCKETS: usize = 17;

#[derive(Clone, Debug)]
pub struct Visibility {
    /// `ds[i]`: camera height for pitch bucket `i` (`client.cw`).
    heights: [i32; PITCH_BUCKETS],
    /// `dj`/`da`: height sweep below/above.
    min_offset: i32,
    max_offset: i32,
    /// `du`/`dt`/`dr`/`dk`: virtual viewport bounds.
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    center_x: i32,
    center_y: i32,
    far_clip: i32,
    draw_distance: i32,
    cache: HashMap<(i32, i32), Vec<u8>>,
}

impl Visibility {
    /// `client.cw(width, height)` followed by `ez.dd(heights, 500, 800, width * 334 / height, 334)`.
    pub fn new(
        viewport_width: i32,
        viewport_height: i32,
        far_clip: i32,
        draw_distance: i32,
    ) -> Self {
        let t = tables();
        let mut heights = [0i32; PITCH_BUCKETS];
        for (i, slot) in heights.iter_mut().enumerate() {
            let n3 = 1 + i as i32 * 256 + 127;
            let n4 = (n3 >> 3) * 3 + 600;
            let n5 = t.sin16384[n3 as usize];
            let n6 = (viewport_height - 334).clamp(0, 100);
            let n7 = (320 - 256) * n6 / 100 + 256;
            let n8 = n7 * n4 / 256;
            *slot = (n8.wrapping_mul(n5)) >> 16;
        }
        let right = viewport_width * 334 / viewport_height;
        Self {
            heights,
            min_offset: 500,
            max_offset: 800,
            left: 0,
            top: 0,
            right,
            bottom: 334,
            center_x: right / 2,
            center_y: 334 / 2,
            far_clip,
            draw_distance,
            cache: HashMap::new(),
        }
    }

    /// `ez.di`: does the camera-relative point project inside the virtual viewport?
    fn point_visible(&self, x: i32, y: i32, z: i32, pitch: i32, yaw: i32) -> bool {
        let t = tables();
        let sin_p = t.sin16384[(pitch & 16383) as usize];
        let cos_p = t.cos16384[(pitch & 16383) as usize];
        let sin_y = t.sin16384[(yaw & 16383) as usize];
        let cos_y = t.cos16384[(yaw & 16383) as usize];
        let n12 = (z.wrapping_mul(sin_y).wrapping_add(x.wrapping_mul(cos_y))) >> 16;
        let n13 = (z.wrapping_mul(cos_y).wrapping_sub(x.wrapping_mul(sin_y))) >> 16;
        let n14 = (y.wrapping_mul(sin_p).wrapping_add(n13.wrapping_mul(cos_p))) >> 16;
        let n15 = (y.wrapping_mul(cos_p).wrapping_sub(n13.wrapping_mul(sin_p))) >> 16;
        if n14 < 50 || n14 > self.far_clip {
            return false;
        }
        let sx = self.center_x + n12.wrapping_mul(128) / n14;
        let sy = self.center_y + n15.wrapping_mul(128) / n14;
        sx >= self.left && sx <= self.right && sy >= self.top && sy <= self.bottom
    }

    /// `ez.dy`: any height along the sweep visible for this tile corner offset.
    fn corner_visible(
        &self,
        pitch_bucket: i32,
        yaw_bucket: i32,
        x: i32,
        y: i32,
        dz: i32,
        dn: i32,
    ) -> bool {
        let Some(&height) = self.heights.get(pitch_bucket as usize) else {
            return false;
        };
        let pitch = pitch_bucket * 256 + 1;
        let yaw = yaw_bucket * 1024;
        let wx = (x - self.draw_distance - dz - 1) * 128;
        let wz = (y - self.draw_distance - dn - 1) * 128;
        let mut off = -self.min_offset;
        while off <= self.max_offset {
            if self.point_visible(wx, height + off, wz, pitch, yaw) {
                return true;
            }
            off += 128;
        }
        false
    }

    /// `ez.dx`: tile offset `(x, y)` in `0..=2 * draw_distance` visible for the current buckets.
    pub fn tile_visible(
        &mut self,
        pitch_bucket: i32,
        yaw_bucket: i32,
        x: i32,
        y: i32,
        dz: i32,
        dn: i32,
    ) -> bool {
        let side = (2 * self.draw_distance + 1) as usize;
        if x < 0 || y < 0 || x as usize >= side || y as usize >= side {
            return false;
        }
        let key = (pitch_bucket, yaw_bucket);
        let entry = self
            .cache
            .entry(key)
            .or_insert_with(|| vec![0u8; side * side]);
        let slot = x as usize * side + y as usize;
        if entry[slot] != 0 {
            return entry[slot] == 2;
        }
        let mut visible = false;
        'outer: for n3 in -1..=1 {
            for n4 in -1..=1 {
                let cx = x + n3 + 1;
                let cy = y + n4 + 1;
                if self.corner_visible(pitch_bucket, yaw_bucket, cx, cy, dz, dn)
                    || self.corner_visible(pitch_bucket, (yaw_bucket + 1) % 16, cx, cy, dz, dn)
                    || self.corner_visible(pitch_bucket + 1, yaw_bucket, cx, cy, dz, dn)
                    || self.corner_visible(pitch_bucket + 1, (yaw_bucket + 1) % 16, cx, cy, dz, dn)
                {
                    visible = true;
                    break 'outer;
                }
            }
        }
        let entry = self.cache.get_mut(&key).expect("cache entry");
        entry[slot] = if visible { 2 } else { 1 };
        visible
    }
}
