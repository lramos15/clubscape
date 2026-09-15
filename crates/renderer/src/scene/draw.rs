//! Exact port of the original scene draw (`ez.dh` → `bh` → `ei` → `ee`): visible-tile marking,
//! the two-pass spiral over tiles around the camera, the per-tile linked draw queue and the
//! original wall/decoration/object ordering rules. Emits the painter-ordered triangle stream.
//!
//! Occluder culling (`ez.mw`) is unreachable from this draw path in the pinned client (the
//! activation routine is only called by unused draw variants), so `yi`/`zo`/`xg` are always
//! false here exactly as in the source captures.

// The nested flag/lookup `if`s mirror the original `ee` control flow one-to-one.
#![allow(clippy::collapsible_if)]
#![allow(clippy::too_many_arguments, clippy::cognitive_complexity)]

use std::collections::{HashMap, VecDeque};

use super::flag;
use super::tile::{TileCamera, TileScratch, draw_tile_model, draw_tile_paint};
use super::visibility::Visibility;
use super::{GameObject, SceneData};
use crate::model::Model;
use crate::model_draw::{ModelDrawer, ModelScratch, SceneCamera};
use crate::raster::{RasterState, Tri};
use crate::tables::tables;

/// Original wall direction tables (`ez.ad/ap/au/ai/ar/aw/ak`), indexed by the camera octant.
const WALL_DRAW_MASK: [i32; 9] = [19, 55, 38, 155, 255, 110, 137, 205, 76];
const WALL_HIDE_MASK: [i32; 9] = [160, 192, 80, 96, 0, 144, 80, 48, 160];
const WALL_DEFER_MASK: [i32; 9] = [76, 8, 137, 4, 0, 1, 38, 2, 19];
const DIAG_16: [i32; 9] = [0, 0, 2, 0, 0, 2, 1, 1, 0];
const DIAG_32: [i32; 9] = [2, 0, 0, 2, 0, 0, 0, 4, 4];
const DIAG_64: [i32; 9] = [0, 4, 4, 8, 0, 0, 8, 0, 0];
const DIAG_128: [i32; 9] = [1, 1, 0, 0, 0, 8, 0, 0, 8];

/// Camera for one frame in original scene units (`ez.dh` arguments).
#[derive(Clone, Copy, Debug)]
pub struct SceneView {
    /// Camera position in main-area local units (X/Y horizontal, height negative-up).
    pub camera_x: i32,
    pub camera_height: i32,
    pub camera_z: i32,
    /// 16384 units per turn.
    pub pitch: i32,
    pub yaw: i32,
    /// Draw plane (`br`).
    pub plane: i32,
    /// Focal point in local units (used for the tile range when `center_on_camera` is false).
    pub focal_x: i32,
    pub focal_z: i32,
    /// `client.kb`: range centered on the camera tile rather than the focal tile.
    pub center_on_camera: bool,
    /// `fq.ae()`: far clip in units.
    pub far_clip: i32,
}

/// What a triangle belongs to, for picking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickTarget {
    Tile {
        plane: i32,
        x: i32,
        y: i32,
    },
    Object {
        hash: i64,
        plane: i32,
        x: i32,
        y: i32,
    },
}

pub trait ModelSource {
    fn model(&self, index: i32) -> Option<&Model>;
}

impl ModelSource for Vec<Option<Model>> {
    fn model(&self, index: i32) -> Option<&Model> {
        if index < 0 {
            None
        } else {
            self.get(index as usize).and_then(|m| m.as_ref())
        }
    }
}

impl ModelSource for HashMap<i32, Model> {
    fn model(&self, index: i32) -> Option<&Model> {
        self.get(&index)
    }
}

#[derive(Clone, Copy, Default)]
struct ObjectState {
    drawn_frame: u32,
    distance: i32,
}

/// A temporary actor placed into the scene for one frame (`ez.bo` with the temporary flag).
#[derive(Clone, Debug)]
pub struct TemporaryEntity {
    pub plane: i32,
    /// Main-area tile of the south-west corner and the tile footprint.
    pub tile_x: i32,
    pub tile_y: i32,
    pub size_x: i32,
    pub size_y: i32,
    /// Position in local units and orientation (2048 units per turn).
    pub x: i32,
    pub height: i32,
    pub z: i32,
    pub orientation: i32,
    pub hash: i64,
    /// Index into the per-frame temporary model list.
    pub model: usize,
}

/// Model index space for temporaries appended after the scene's static models.
pub const TEMP_MODEL_BASE: i32 = 1 << 24;

pub struct SceneDrawer {
    pub state: RasterState,
    palette: Vec<i32>,
    flags: Vec<i32>,
    object_count: Vec<i8>,
    link: Vec<i8>,
    object_flags: Vec<i8>,
    slots: HashMap<usize, usize>,
    temp_objects: Vec<GameObject>,
    objects: Vec<ObjectState>,
    frame: u32,
    remaining: i32,
    queue: VecDeque<usize>,
    queued: Vec<bool>,
    visibility: Visibility,
    pitch_bucket: i32,
    yaw_bucket: i32,
    // camera (`cp cq cl cd cv cs cy br cr cu cb ct dz dn`)
    cp: i32,
    cq: i32,
    cl: i32,
    cd: i32,
    cv: i32,
    cs: i32,
    cy: i32,
    br: i32,
    cr: i32,
    cu: i32,
    cb: i32,
    ct: i32,
    dz: i32,
    dn: i32,
    /// `cg`: plane threshold for drawing paints without the visible flag.
    pub paint_plane: i32,
    tile_camera: TileCamera,
    scene_camera: SceneCamera,
    tile_scratch: TileScratch,
    model_scratch: ModelScratch,
    pub picks: Vec<PickTarget>,
    /// Objects whose model reference was missing (asset failure surfaced, never silently skipped).
    pub missing_models: Vec<i32>,
    scratch_objects: Vec<usize>,
}

impl SceneDrawer {
    pub fn new(scene: &SceneData, state: RasterState, palette: &[i32], far_clip: i32) -> Self {
        let visibility = Visibility::new(state.width, state.height, far_clip, scene.draw_distance);
        Self {
            state,
            palette: palette.to_vec(),
            flags: scene.flags.clone(),
            object_count: scene.object_count.clone(),
            link: scene.link.clone(),
            object_flags: scene.object_flags.clone(),
            slots: scene.slots.clone(),
            temp_objects: Vec::new(),
            objects: vec![ObjectState::default(); scene.game_objects.len()],
            frame: 0,
            remaining: 0,
            queue: VecDeque::new(),
            queued: vec![false; scene.tile_count],
            visibility,
            pitch_bucket: 0,
            yaw_bucket: 0,
            cp: 0,
            cq: 0,
            cl: 0,
            cd: 0,
            cv: 0,
            cs: 0,
            cy: 0,
            br: 0,
            cr: 0,
            cu: 0,
            cb: 0,
            ct: 0,
            dz: 0,
            dn: 0,
            paint_plane: 0,
            tile_camera: TileCamera {
                x: 0,
                height: 0,
                z: 0,
                pitch_sin: 0.0,
                pitch_cos: 0.0,
                yaw_sin: 0.0,
                yaw_cos: 0.0,
            },
            scene_camera: SceneCamera::from_angles(0, 0, far_clip),
            tile_scratch: TileScratch::default(),
            model_scratch: ModelScratch::default(),
            picks: Vec::new(),
            missing_models: Vec::new(),
            scratch_objects: Vec::new(),
        }
    }

    // ------------------------------------------------------------------ flag helpers (xj)

    #[inline]
    fn exists(&self, i: usize) -> bool {
        self.flags[i] & flag::EXISTS != 0
    }
    #[inline]
    fn draw_primary(&self, i: usize) -> bool {
        self.flags[i] & flag::DRAW_PRIMARY != 0
    }
    #[inline]
    fn visible(&self, i: usize) -> bool {
        self.flags[i] & flag::VISIBLE != 0
    }
    /// `ez.ym`: logical plane (0 when forced).
    #[inline]
    fn logical_plane(&self, scene: &SceneData, i: usize) -> i32 {
        let plane = (i as i32 >> scene.plane_shift) & 3;
        if self.flags[i] & flag::FORCE_PLANE_0 != 0 {
            0
        } else {
            plane
        }
    }
    /// `ez.xm`: plane adjusted by the bridge flag of the plane-0 tile.
    #[inline]
    fn bridge_plane(&self, scene: &SceneData, i: usize) -> i32 {
        let plane = (i as i32 >> scene.plane_shift) & 3;
        let base = i & (scene.plane_stride as usize - 1);
        (plane + ((self.flags[base] >> 5) & 1)) & 3
    }

    /// Static or temporary game object by combined id.
    #[inline]
    fn object<'s>(&'s self, scene: &'s SceneData, id: usize) -> &'s GameObject {
        if id < scene.game_objects.len() {
            &scene.game_objects[id]
        } else {
            &self.temp_objects[id - scene.game_objects.len()]
        }
    }

    /// Restores the static object slots/links/counts before placing this frame's actors
    /// (`ez.gc` clears temporaries after the frame in the original).
    pub fn begin_frame(&mut self, scene: &SceneData) {
        self.object_count.copy_from_slice(&scene.object_count);
        self.link.copy_from_slice(&scene.link);
        self.object_flags.copy_from_slice(&scene.object_flags);
        self.slots.clone_from(&scene.slots);
        self.temp_objects.clear();
        self.objects.truncate(scene.game_objects.len());
        self.objects
            .resize(scene.game_objects.len(), ObjectState::default());
    }

    /// `ez.bo(..., temporary = true)`: registers an actor in every tile it spans for this frame.
    /// Returns false when the original would refuse (out of range or a tile already has 5 objects).
    pub fn add_temporary(&mut self, scene: &SceneData, entity: &TemporaryEntity) -> bool {
        let oy = scene.offset;
        let n13 = entity.tile_x + oy;
        let n14 = entity.tile_y + oy;
        for n12 in n13..n13 + entity.size_x {
            for i in n14..n14 + entity.size_y {
                if n12 < 0 || i < 0 || n12 >= scene.width || i >= scene.height {
                    return false;
                }
                let n11 = scene.tile_index(entity.plane, n12, i);
                if self.flags[n11] & flag::EXISTS != 0 && self.object_count[n11] >= 5 {
                    return false;
                }
            }
        }
        let id = scene.game_objects.len() + self.temp_objects.len();
        self.temp_objects.push(GameObject {
            model: TEMP_MODEL_BASE + entity.model as i32,
            orientation: entity.orientation,
            x: entity.x,
            height: entity.height,
            z: entity.z,
            min_x: entity.tile_x,
            max_x: entity.tile_x + entity.size_x - 1,
            min_y: entity.tile_y,
            max_y: entity.tile_y + entity.size_y - 1,
            config: 0,
            slot_flag: 0,
            dynamic: false,
            hash: entity.hash,
        });
        self.objects.push(ObjectState::default());
        for i in n13..n13 + entity.size_x {
            for n11 in n14..n14 + entity.size_y {
                let mut n19 = 0i32;
                if i > n13 {
                    n19 |= 1;
                }
                if i < n13 + entity.size_x - 1 {
                    n19 |= 4;
                }
                if n11 > n14 {
                    n19 |= 8;
                }
                if n11 < n14 + entity.size_y - 1 {
                    n19 |= 2;
                }
                // Original also creates tile records on lower planes; the exported flags already
                // have EXISTS for every ground tile the loader created, so only set it here.
                for n18 in (0..=entity.plane).rev() {
                    let n17 = scene.tile_index(n18, i, n11);
                    self.flags[n17] |= flag::EXISTS;
                }
                let n18 = scene.tile_index(entity.plane, i, n11);
                let n17 = self.object_count[n18] as usize;
                self.slots.insert(n18 * 5 + n17, id);
                self.object_flags[n18 * 5 + n17] = n19 as i8;
                self.link[n18] |= n19 as i8;
                self.object_count[n18] += 1;
            }
        }
        true
    }
    #[inline]
    fn wall_cull_a(&self, i: usize) -> i32 {
        (self.flags[i] >> 16) & 15
    }
    #[inline]
    fn wall_cull_b(&self, i: usize) -> i32 {
        (self.flags[i] >> 20) & 15
    }
    #[inline]
    fn wall_direction(&self, i: usize) -> i32 {
        ((self.flags[i] as u32) >> 24) as i32 & 255
    }
    fn set_wall_direction(&mut self, i: usize, value: i32) {
        self.flags[i] = (self.flags[i] & 0xFFFFFF) | (value << 24);
    }
    fn set_wall_cull_a(&mut self, i: usize, value: i32) {
        self.flags[i] = (self.flags[i] & 0xFFF0FFFFu32 as i32) | (value << 16);
    }
    fn set_wall_cull_b(&mut self, i: usize, value: i32) {
        self.flags[i] = (self.flags[i] & 0xFF0FFFFFu32 as i32) | (value << 20);
    }

    // ------------------------------------------------------------------ queue (fa / ew / rj)

    /// `ez.ew`: move to the back of the queue (or append).
    fn enqueue(&mut self, i: usize) {
        if self.queued[i] {
            if let Some(pos) = self.queue.iter().position(|&q| q == i) {
                self.queue.remove(pos);
            }
        }
        self.queued[i] = true;
        self.queue.push_back(i);
    }

    /// `ez.rj`: pop the front or `None`.
    fn dequeue(&mut self) -> Option<usize> {
        let i = self.queue.pop_front()?;
        self.queued[i] = false;
        Some(i)
    }

    // ------------------------------------------------------------------ frame

    /// `ez.dh` + `bh(true, ..)` + `ei`: draws the whole frame into `out`. Temporary entities must
    /// have been registered with [`Self::add_temporary`] after [`Self::begin_frame`].
    pub fn draw<M: ModelSource>(
        &mut self,
        scene: &SceneData,
        models: &M,
        temp_models: &[Model],
        view: &SceneView,
        out: &mut Vec<Tri>,
    ) {
        let t = tables();
        self.picks.clear();
        self.missing_models.clear();
        // dh
        self.cp = view
            .camera_x
            .clamp(scene.min_x << 7, (scene.max_x << 7) - 1);
        self.cq = view.camera_height;
        self.cl = view
            .camera_z
            .clamp(scene.min_y << 7, (scene.max_y << 7) - 1);
        self.cd = (self.cp >> 7) + scene.offset;
        self.cv = (self.cl >> 7) + scene.offset;
        self.cs = (view.focal_x >> 7) + scene.offset;
        self.cy = (view.focal_z >> 7) + scene.offset;
        self.br = view.plane;
        let pitch = view.pitch.clamp(1, 4160);
        self.pitch_bucket = (pitch - 1) / 256;
        self.yaw_bucket = view.yaw / 1024;
        let p = (pitch & 16383) as usize;
        let y = (view.yaw & 16383) as usize;
        self.tile_camera = TileCamera {
            x: self.cp,
            height: self.cq,
            z: self.cl,
            pitch_sin: t.sinf16384[p],
            pitch_cos: t.cosf16384[p],
            yaw_sin: t.sinf16384[y],
            yaw_cos: t.cosf16384[y],
        };
        self.scene_camera = SceneCamera::from_angles(pitch, view.yaw, view.far_clip);
        // bh(true, kb)
        self.frame = self.frame.wrapping_add(1);
        let dd = scene.draw_distance;
        let n3 = if view.center_on_camera {
            self.cd
        } else {
            self.cs
        };
        let n = if view.center_on_camera {
            self.cv
        } else {
            self.cy
        };
        if scene.main_scene {
            self.cr = (n3 - dd).max(scene.min_x + scene.offset);
            self.cb = (n - dd).max(scene.min_y + scene.offset);
            self.cu = (n3 + dd).min(scene.max_x + scene.offset);
            self.ct = (n + dd).min(scene.max_y + scene.offset);
        } else {
            self.cr = 0;
            self.cb = 0;
            self.cu = scene.width;
            self.ct = scene.height;
        }
        self.dz = self.cd - n3;
        self.dn = self.cv - n;
        self.remaining = 0;
        // ei: mark visible tiles
        let roof_hiding = scene.roof_mode != 0 && scene.main_scene;
        let world_plane = 0;
        for plane in (scene.min_level..scene.planes).rev() {
            for x in self.cr..self.cu {
                for yy in self.cb..self.ct {
                    let i = scene.tile_index(plane, x, yy);
                    if !self.exists(i) {
                        continue;
                    }
                    let logical = self.logical_plane(scene, i);
                    let roof = scene.roof(world_plane, x, yy);
                    let vis_ok = !scene.main_scene
                        || self.visibility.tile_visible(
                            self.pitch_bucket,
                            self.yaw_bucket,
                            x - self.cd + self.dz + dd,
                            yy - self.cv + self.dn + dd,
                            self.dz,
                            self.dn,
                        )
                        || scene.height(plane, x, yy) - self.cq >= 2000;
                    let drawn = (logical <= self.br || roof_hiding)
                        && vis_ok
                        && (!roof_hiding || world_plane >= logical || roof == 0);
                    if drawn {
                        let mut f = self.flags[i];
                        f |= 6;
                        f |= if self.object_count[i] <= 0 && (f & 128) == 0 {
                            0
                        } else {
                            8
                        };
                        f &= 0xFF00FFEFu32 as i32;
                        self.flags[i] = f;
                        self.remaining += 1;
                    } else {
                        self.flags[i] &= !(2 | 4 | 16);
                    }
                }
            }
        }
        // ei: two passes over the spiral
        let n16 = self.dz.abs();
        let n17 = self.dn.abs();
        'outer: for pass in 0..2 {
            let first = pass == 0;
            for plane in scene.min_level..scene.planes {
                for var21 in -(n16 + dd)..=0 {
                    let x_a = var21 + self.cd;
                    let x_b = self.cd - var21;
                    if x_a < self.cr && x_b >= self.cu {
                        continue;
                    }
                    for var24 in -(n17 + dd)..=0 {
                        let y_a = var24 + self.cv;
                        let y_b = self.cv - var24;
                        if x_a >= self.cr && x_a < self.cu {
                            if y_a >= self.cb && y_a < self.ct {
                                let i = scene.tile_index(plane, x_a, y_a);
                                if (self.flags[i] & 3) == 3 {
                                    self.draw_tile(scene, models, temp_models, i, first, out);
                                }
                            }
                            if y_b >= self.cb && y_b < self.ct {
                                let i = scene.tile_index(plane, x_a, y_b);
                                if (self.flags[i] & 3) == 3 {
                                    self.draw_tile(scene, models, temp_models, i, first, out);
                                }
                            }
                        }
                        if x_b >= self.cr && x_b < self.cu {
                            if y_a >= self.cb && y_a < self.ct {
                                let i = scene.tile_index(plane, x_b, y_a);
                                if (self.flags[i] & 3) == 3 {
                                    self.draw_tile(scene, models, temp_models, i, first, out);
                                }
                            }
                            if y_b >= self.cb && y_b < self.ct {
                                let i = scene.tile_index(plane, x_b, y_b);
                                if (self.flags[i] & 3) == 3 {
                                    self.draw_tile(scene, models, temp_models, i, first, out);
                                }
                            }
                        }
                        if self.remaining == 0 {
                            break 'outer;
                        }
                    }
                }
            }
        }
    }

    fn pick_tile(&mut self, plane: i32, x: i32, y: i32) -> u32 {
        self.picks.push(PickTarget::Tile { plane, x, y });
        self.picks.len() as u32
    }

    fn pick_object(&mut self, hash: i64, plane: i32, x: i32, y: i32) -> u32 {
        self.picks.push(PickTarget::Object { hash, plane, x, y });
        self.picks.len() as u32
    }

    /// `ez.zm` → `fx.xm`: draw a model at a scene position.
    fn draw_model<M: ModelSource>(
        &mut self,
        models: &M,
        temp_models: &[Model],
        model_index: i32,
        orientation: i32,
        x: i32,
        height: i32,
        z: i32,
        pick: u32,
        out: &mut Vec<Tri>,
    ) {
        if model_index < 0 {
            return;
        }
        let model = if model_index >= TEMP_MODEL_BASE {
            temp_models.get((model_index - TEMP_MODEL_BASE) as usize)
        } else {
            models.model(model_index)
        };
        let Some(model) = model else {
            self.missing_models.push(model_index);
            return;
        };
        let mut drawer = ModelDrawer {
            state: self.state,
            palette: &self.palette,
            scratch: &mut self.model_scratch,
            alpha_pass: 2,
        };
        let _ = drawer.draw_scene(
            model,
            orientation,
            &self.scene_camera,
            x - self.cp,
            height - self.cq,
            z - self.cl,
            pick,
            out,
        );
    }

    fn draw_paint(
        &mut self,
        scene: &SceneData,
        tile_index: usize,
        plane: i32,
        x: i32,
        y: i32,
        out: &mut Vec<Tri>,
    ) {
        if let Some(paint) = scene.paints.get(&tile_index) {
            let pick = self.pick_tile(plane, x, y);
            draw_tile_paint(
                scene,
                &self.state,
                &self.tile_camera,
                paint,
                plane,
                x,
                y,
                pick,
                out,
            );
        }
    }

    fn draw_shaped(
        &mut self,
        scene: &SceneData,
        tile_index: usize,
        plane: i32,
        x: i32,
        y: i32,
        out: &mut Vec<Tri>,
    ) {
        if let Some(model) = scene.tile_models.get(&tile_index) {
            let pick = self.pick_tile(plane, x, y);
            draw_tile_model(
                &self.state,
                &self.tile_camera,
                model,
                &mut self.tile_scratch,
                pick,
                out,
            );
        }
    }

    /// `ez.tc`/`hz`: object distance metric from the camera tile.
    fn object_distance(object: &GameObject, cam_x: i32, cam_y: i32) -> i32 {
        let mut n6 = cam_x - object.min_x;
        let n5 = object.max_x - cam_x;
        if n5 > n6 {
            n6 = n5;
        }
        let n4 = object.max_y - cam_y;
        let n3 = cam_y - object.min_y;
        if n4 <= n3 { n6 + n3 } else { n6 + n4 }
    }

    /// `ez.ee`: the per-tile linked draw.
    fn draw_tile<M: ModelSource>(
        &mut self,
        scene: &SceneData,
        models: &M,
        temp_models: &[Model],
        start: usize,
        mut first_pass: bool,
        out: &mut Vec<Tri>,
    ) {
        let oy = scene.offset;
        let gf = scene.plane_stride as usize;
        let mh = scene.x_stride as usize;
        self.enqueue(start);
        while let Some(n2) = self.dequeue() {
            if !self.visible(n2) {
                continue;
            }
            let (n23, n21, n22) = scene.decode_index(n2);
            let n24 = self.bridge_plane(scene, n2);
            let n26 = n21 - oy;
            let n27 = n22 - oy;
            let mut n28 = self.flags[n2];
            if self.draw_primary(n2) {
                if first_pass {
                    let mut defer = false;
                    if n23 > 0 {
                        let n20 = n2 - gf;
                        if self.exists(n20) && self.visible(n20) {
                            defer = true;
                        }
                    }
                    if !defer && n21 <= self.cd && n21 > self.cr {
                        let n20 = n2 - mh;
                        if self.exists(n20)
                            && self.visible(n20)
                            && (self.draw_primary(n20) || (self.link[n2] & 1) == 0)
                        {
                            defer = true;
                        }
                    }
                    if !defer && n21 >= self.cd && n21 < self.cu - 1 {
                        let n20 = n2 + mh;
                        if self.exists(n20)
                            && self.visible(n20)
                            && (self.draw_primary(n20) || (self.link[n2] & 4) == 0)
                        {
                            defer = true;
                        }
                    }
                    if !defer && n22 <= self.cv && n22 > self.cb {
                        let n20 = n2 - 1;
                        if self.exists(n20)
                            && self.visible(n20)
                            && (self.draw_primary(n20) || (self.link[n2] & 8) == 0)
                        {
                            defer = true;
                        }
                    }
                    if !defer && n22 >= self.cv && n22 < self.ct - 1 {
                        let n20 = n2 + 1;
                        if self.exists(n20)
                            && self.visible(n20)
                            && (self.draw_primary(n20) || (self.link[n2] & 2) == 0)
                        {
                            defer = true;
                        }
                    }
                    if defer {
                        continue;
                    }
                } else {
                    first_pass = true;
                }
                self.flags[n2] &= !flag::DRAW_PRIMARY;
                n28 = self.flags[n2];
                if n28 & flag::BRIDGE_BELOW != 0 {
                    let n20 = scene.tile_index(3, n21, n22);
                    let n19 = self.flags[n20];
                    if n19 & flag::PAINT != 0 {
                        self.draw_paint(scene, n20, 0, n26, n27, out);
                    } else if n19 & flag::TILE_MODEL != 0 {
                        self.draw_shaped(scene, n20, 0, n26, n27, out);
                    }
                    if n19 & flag::WALL != 0 {
                        if let Some(w) = scene.walls.get(&n20).cloned() {
                            let pick = self.pick_object(w.hash, 0, n26, n27);
                            self.draw_model(
                                models,
                                temp_models,
                                w.model_a,
                                0,
                                w.x,
                                w.height,
                                w.z,
                                pick,
                                out,
                            );
                        }
                    }
                    let count = self.object_count[n20] as i32;
                    for slot in 0..count {
                        if let Some(&id) = self.slots.get(&(n20 * 5 + slot as usize)) {
                            let o = self.object(scene, id).clone();
                            let pick = self.pick_object(o.hash, 0, n26, n27);
                            self.draw_model(
                                models,
                                temp_models,
                                o.model,
                                o.orientation,
                                o.x,
                                o.height,
                                o.z,
                                pick,
                                out,
                            );
                        }
                    }
                }
                let mut drew_ground = false;
                if n28 & flag::PAINT != 0 {
                    drew_ground = true;
                    if n28 & flag::PAINT_VISIBLE != 0 || n23 <= self.paint_plane {
                        self.draw_paint(scene, n2, n24, n26, n27, out);
                    }
                } else if n28 & flag::TILE_MODEL != 0 {
                    drew_ground = true;
                    self.draw_shaped(scene, n2, n24, n26, n27, out);
                }
                let mut n19 = 0usize;
                let mut n30 = 0;
                let mut n18 = 0;
                if n28 & 0xC000 != 0 {
                    if n21 == self.cd {
                        n19 += 1;
                    } else if self.cd < n21 {
                        n19 += 2;
                    }
                    if n22 == self.cv {
                        n19 += 3;
                    } else if self.cv > n22 {
                        n19 += 6;
                    }
                    n30 = WALL_DRAW_MASK[n19];
                    let n17 = WALL_DEFER_MASK[n19];
                    self.set_wall_direction(n2, n17);
                    n18 = WALL_HIDE_MASK[n19];
                }
                if n28 & flag::WALL != 0 {
                    if let Some(w) = scene.walls.get(&n2).cloned() {
                        if w.orientation_a & n18 != 0 {
                            let (n16, n15) = match w.orientation_a {
                                16 => (3, DIAG_16[n19]),
                                32 => (6, DIAG_32[n19]),
                                64 => (12, DIAG_64[n19]),
                                128 => (9, DIAG_128[n19]),
                                _ => (0, 0),
                            };
                            if n16 != 0 {
                                self.set_wall_cull_a(n2, n16);
                                self.set_wall_cull_b(n2, n15);
                                self.flags[n2] |= flag::WALL_DEFERRED;
                                n28 = self.flags[n2];
                            }
                        }
                        if w.orientation_a & n30 != 0 {
                            let pick = self.pick_object(w.hash, n23, n26, n27);
                            self.draw_model(
                                models,
                                temp_models,
                                w.model_a,
                                0,
                                w.x,
                                w.height,
                                w.z,
                                pick,
                                out,
                            );
                        }
                        if w.orientation_b & n30 != 0 {
                            let pick = self.pick_object(w.hash, n23, n26, n27);
                            self.draw_model(
                                models,
                                temp_models,
                                w.model_b,
                                0,
                                w.x,
                                w.height,
                                w.z,
                                pick,
                                out,
                            );
                        }
                    }
                }
                if n28 & flag::WALL_DECOR != 0 {
                    if let Some(d) = scene.wall_decorations.get(&n2).cloned() {
                        if d.orientation & n30 != 0 {
                            let pick = self.pick_object(d.hash, n23, n26, n27);
                            self.draw_model(
                                models,
                                temp_models,
                                d.model_a,
                                0,
                                d.x + d.offset_x,
                                d.height,
                                d.z + d.offset_z,
                                pick,
                                out,
                            );
                        } else if d.orientation == 256 {
                            let n16 = d.x - self.cp;
                            let n15 = d.z - self.cl;
                            let n14 = d.orientation2;
                            let n12 = if n14 != 1 && n14 != 2 { n16 } else { -n16 };
                            let n13 = if n14 != 2 && n14 != 3 { n15 } else { -n15 };
                            if n13 < n12 {
                                let pick = self.pick_object(d.hash, n23, n26, n27);
                                self.draw_model(
                                    models,
                                    temp_models,
                                    d.model_a,
                                    0,
                                    d.x + d.offset_x,
                                    d.height,
                                    d.z + d.offset_z,
                                    pick,
                                    out,
                                );
                            } else if d.model_b >= 0 {
                                let pick = self.pick_object(d.hash, n23, n26, n27);
                                self.draw_model(
                                    models,
                                    temp_models,
                                    d.model_b,
                                    0,
                                    d.x + d.offset_x2,
                                    d.height,
                                    d.z + d.offset_z2,
                                    pick,
                                    out,
                                );
                            }
                        }
                    }
                }
                if drew_ground {
                    if self.flags[n2] & flag::FLOOR_DECOR != 0 {
                        if let Some(f) = scene.floor_decorations.get(&n2).cloned() {
                            let pick = self.pick_object(f.hash, n23, n26, n27);
                            self.draw_model(
                                models,
                                temp_models,
                                f.model,
                                0,
                                f.x,
                                f.height,
                                f.z,
                                pick,
                                out,
                            );
                        }
                    }
                    // Item layers (ground items) are not part of static scene exports.
                }
                let n17 = self.link[n2] as i32;
                if n21 < self.cd && n21 >= self.cr && n21 < self.cu - 1 && (n17 & 4) != 0 {
                    let n16 = n2 + mh;
                    if self.exists(n16) && self.visible(n16) {
                        self.enqueue(n16);
                    }
                }
                if n22 < self.cv && n22 >= self.cb && n22 < self.ct - 1 && (n17 & 2) != 0 {
                    let n16 = n2 + 1;
                    if self.exists(n16) && self.visible(n16) {
                        self.enqueue(n16);
                    }
                }
                if n21 > self.cd && n21 > self.cr && n21 < self.cu && (n17 & 1) != 0 {
                    let n16 = n2 - mh;
                    if self.exists(n16) && self.visible(n16) {
                        self.enqueue(n16);
                    }
                }
                if n22 > self.cv && n22 > self.cb && n22 < self.ct && (n17 & 8) != 0 {
                    let n16 = n2 - 1;
                    if self.exists(n16) && self.visible(n16) {
                        self.enqueue(n16);
                    }
                }
            }
            if n28 & flag::WALL_DEFERRED != 0 {
                let mut ready = true;
                let count = self.object_count[n2] as usize;
                for slot in 0..count {
                    let Some(&id) = self.slots.get(&(n2 * 5 + slot)) else {
                        continue;
                    };
                    let n18 = self.object_flags[n2 * 5 + slot] as i32;
                    if self.objects[id].drawn_frame != self.frame
                        && (n18 & self.wall_cull_a(n2)) == self.wall_cull_b(n2)
                    {
                        ready = false;
                        break;
                    }
                }
                if ready {
                    if let Some(w) = scene.walls.get(&n2).cloned() {
                        let pick = self.pick_object(w.hash, n23, n26, n27);
                        self.draw_model(
                            models,
                            temp_models,
                            w.model_a,
                            0,
                            w.x,
                            w.height,
                            w.z,
                            pick,
                            out,
                        );
                    }
                    self.flags[n2] &= !flag::WALL_DEFERRED;
                    n28 = self.flags[n2];
                }
            }
            if n28 & flag::DRAW_OBJECTS != 0 {
                self.flags[n2] &= !flag::DRAW_OBJECTS;
                n28 = self.flags[n2];
                self.scratch_objects.clear();
                let count = self.object_count[n2] as usize;
                'objects: for slot in 0..count {
                    let Some(&id) = self.slots.get(&(n2 * 5 + slot)) else {
                        continue;
                    };
                    if self.objects[id].drawn_frame == self.frame {
                        continue;
                    }
                    let o = self.object(scene, id).clone();
                    let o = &o;
                    for n18 in o.min_x..=o.max_x {
                        for n17 in o.min_y..=o.max_y {
                            let n16 = n18 + oy;
                            let n15 = n17 + oy;
                            let n14 = scene.tile_index(n23, n16, n15);
                            if self.draw_primary(n14) {
                                self.flags[n2] |= flag::DRAW_OBJECTS;
                                n28 = self.flags[n2];
                                continue 'objects;
                            }
                            if self.flags[n14] & flag::WALL_DEFERRED == 0 {
                                continue;
                            }
                            let mut n12 = 0;
                            if n18 > o.min_x {
                                n12 |= 1;
                            }
                            if n18 < o.max_x {
                                n12 |= 4;
                            }
                            if n17 > o.min_y {
                                n12 |= 8;
                            }
                            if n17 < o.max_y {
                                n12 |= 2;
                            }
                            let n13 = self.wall_cull_a(n2) ^ self.wall_cull_b(n2);
                            if (n12 & self.wall_cull_a(n14)) != n13 {
                                continue;
                            }
                            self.flags[n2] |= flag::DRAW_OBJECTS;
                            n28 = self.flags[n2];
                            continue 'objects;
                        }
                    }
                    self.scratch_objects.push(id);
                    self.objects[id].distance =
                        Self::object_distance(o, self.cd - oy, self.cv - oy);
                }
                if n28 & flag::ZONE_DYNAMIC != 0 {
                    if let Some(list) = scene.zone_dynamic.get(&(n21 >> 3, n22 >> 3)) {
                        for &id in list {
                            let o = &scene.game_objects[id];
                            if !o.dynamic
                                || self.objects[id].drawn_frame == self.frame
                                || o.min_x != n26
                                || o.min_y != n27
                                || self.scratch_objects.len() >= 55
                            {
                                continue;
                            }
                            self.scratch_objects.push(id);
                            self.objects[id].distance =
                                Self::object_distance(o, self.cd - oy, self.cv - oy);
                        }
                    }
                }
                loop {
                    let mut best_distance = -50;
                    let mut best: Option<usize> = None;
                    for pos in 0..self.scratch_objects.len() {
                        let id = self.scratch_objects[pos];
                        if self.objects[id].drawn_frame == self.frame {
                            continue;
                        }
                        let dist = self.objects[id].distance;
                        if dist > best_distance {
                            best_distance = dist;
                            best = Some(pos);
                            continue;
                        }
                        if best_distance != dist {
                            continue;
                        }
                        let Some(b) = best else { continue };
                        let o = self.object(scene, id);
                        let bo = self.object(scene, self.scratch_objects[b]);
                        let n16 = o.x - self.cp;
                        let n15 = o.z - self.cl;
                        let n14 = bo.x - self.cp;
                        let n12 = bo.z - self.cl;
                        if n16 * n16 + n15 * n15 > n14 * n14 + n12 * n12 {
                            best = Some(pos);
                        }
                    }
                    let Some(pos) = best else { break };
                    let id = self.scratch_objects[pos];
                    self.objects[id].drawn_frame = self.frame;
                    let o = self.object(scene, id).clone();
                    let pick = self.pick_object(o.hash, n23, o.min_x, o.min_y);
                    self.draw_model(
                        models,
                        temp_models,
                        o.model,
                        o.orientation,
                        o.x,
                        o.height,
                        o.z,
                        pick,
                        out,
                    );
                    for n43 in o.min_x..=o.max_x {
                        for n16 in o.min_y..=o.max_y {
                            let n15 = n43 + oy;
                            let n14 = n16 + oy;
                            let n12 = scene.tile_index(n23, n15, n14);
                            if self.flags[n12] & flag::WALL_DEFERRED != 0 {
                                self.enqueue(n12);
                                continue;
                            }
                            if (n15 == n21 && n14 == n22) || !self.visible(n12) {
                                continue;
                            }
                            self.enqueue(n12);
                        }
                    }
                }
                if n28 & flag::DRAW_OBJECTS != 0 {
                    continue;
                }
            }
            if !self.visible(n2) {
                continue;
            }
            if n28 & flag::WALL_DEFERRED != 0 {
                continue;
            }
            if n21 <= self.cd && n21 > self.cr {
                let n11 = n2 - mh;
                if self.exists(n11) && self.visible(n11) {
                    continue;
                }
            }
            if n21 >= self.cd && n21 < self.cu - 1 {
                let n10 = n2 + mh;
                if self.exists(n10) && self.visible(n10) {
                    continue;
                }
            }
            if n22 <= self.cv && n22 > self.cb {
                let n9 = n2 - 1;
                if self.exists(n9) && self.visible(n9) {
                    continue;
                }
            }
            if n22 >= self.cv && n22 < self.ct - 1 {
                let n8 = n2 + 1;
                if self.exists(n8) && self.visible(n8) {
                    continue;
                }
            }
            self.flags[n2] &= !flag::VISIBLE;
            n28 = self.flags[n2];
            self.remaining -= 1;
            // Deferred item layers are not part of static scene exports.
            if n28 & 0xC000 != 0 && self.wall_direction(n2) != 0 {
                if n28 & flag::WALL_DECOR != 0 {
                    if let Some(d) = scene.wall_decorations.get(&n2).cloned() {
                        if d.orientation & self.wall_direction(n2) != 0 {
                            let pick = self.pick_object(d.hash, n23, n26, n27);
                            self.draw_model(
                                models,
                                temp_models,
                                d.model_a,
                                0,
                                d.x + d.offset_x,
                                d.height,
                                d.z + d.offset_z,
                                pick,
                                out,
                            );
                        } else if d.orientation == 256 {
                            let n48 = d.x - self.cp;
                            let n49 = d.z - self.cl;
                            let n18 = d.orientation2;
                            let n17 = if n18 != 1 && n18 != 2 { n48 } else { -n48 };
                            let n16 = if n18 != 2 && n18 != 3 { n49 } else { -n49 };
                            if n16 >= n17 {
                                let pick = self.pick_object(d.hash, n23, n26, n27);
                                self.draw_model(
                                    models,
                                    temp_models,
                                    d.model_a,
                                    0,
                                    d.x + d.offset_x,
                                    d.height,
                                    d.z + d.offset_z,
                                    pick,
                                    out,
                                );
                            } else if d.model_b >= 0 {
                                let pick = self.pick_object(d.hash, n23, n26, n27);
                                self.draw_model(
                                    models,
                                    temp_models,
                                    d.model_b,
                                    0,
                                    d.x + d.offset_x2,
                                    d.height,
                                    d.z + d.offset_z2,
                                    pick,
                                    out,
                                );
                            }
                        }
                    }
                }
                if n28 & flag::WALL != 0 {
                    if let Some(w) = scene.walls.get(&n2).cloned() {
                        let n50 = self.wall_direction(n2);
                        if w.orientation_b & n50 != 0 {
                            let pick = self.pick_object(w.hash, n23, n26, n27);
                            self.draw_model(
                                models,
                                temp_models,
                                w.model_b,
                                0,
                                w.x,
                                w.height,
                                w.z,
                                pick,
                                out,
                            );
                        }
                        if w.orientation_a & n50 != 0 {
                            let pick = self.pick_object(w.hash, n23, n26, n27);
                            self.draw_model(
                                models,
                                temp_models,
                                w.model_a,
                                0,
                                w.x,
                                w.height,
                                w.z,
                                pick,
                                out,
                            );
                        }
                    }
                }
            }
            if n23 < scene.planes - 1 {
                let n7 = n2 + gf;
                if self.exists(n7) && self.visible(n7) {
                    self.enqueue(n7);
                }
            }
            if n21 < self.cd && n21 >= self.cr && n21 < self.cu - 1 {
                let n6 = n2 + mh;
                if self.exists(n6) && self.visible(n6) {
                    self.enqueue(n6);
                }
            }
            if n22 < self.cv && n22 >= self.cb && n22 < self.ct - 1 {
                let n5 = n2 + 1;
                if self.exists(n5) && self.visible(n5) {
                    self.enqueue(n5);
                }
            }
            if n21 > self.cd && n21 > self.cr && n21 < self.cu {
                let n4 = n2 - mh;
                if self.exists(n4) && self.visible(n4) {
                    self.enqueue(n4);
                }
            }
            if n22 > self.cv && n22 > self.cb && n22 < self.ct {
                let n3 = n2 - 1;
                if self.exists(n3) && self.visible(n3) {
                    self.enqueue(n3);
                }
            }
        }
    }
}
