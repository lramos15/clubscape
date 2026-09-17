//! Source-data and integer renderer ABI adapters; camera mechanics remain in clubscape-camera.

use std::collections::{BTreeMap, BTreeSet};

use clubscape_camera::{
    CameraError, CameraOutput, Focus, FrameEffects, InitializationProvenance, Preferences,
    SurfaceTriangle, Terrain, Viewport, WorldBase,
};
use serde::{Deserialize, Serialize};

use crate::core::Camera;
use crate::error::RenderError;
use crate::scene::{SceneData, flag, tag_object_id};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CameraContext {
    pub actor_id: String,
    pub region: String,
    pub instance: Option<String>,
    pub revision: String,
    pub tick: String,
    pub scene_id: String,
    pub scene_generation: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RenderedActorPlacement {
    pub actor_id: String,
    pub local: [i32; 2],
    pub plane: i32,
    pub size_tiles: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CameraSourceSample {
    pub context: CameraContext,
    pub rendered_actor: Option<RenderedActorPlacement>,
    pub focus: Option<Focus>,
    pub effects: Option<FrameEffects>,
    pub missing: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CameraDelivery {
    pub context: CameraContext,
    pub initialization: InitializationProvenance,
    pub output: CameraOutput,
    pub cycle: i32,
    pub preferences: Preferences,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CameraSurface {
    pub triangles: Vec<SurfaceTriangle>,
    pub decoration: Option<i32>,
}

/// Dense plane/X/Y arrays. Null is unavailable, never an original zero or an absent surface.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CameraScene {
    pub version: u32,
    pub id: String,
    pub generation: String,
    pub base: WorldBase,
    pub dimensions: [u32; 2],
    pub heights: Vec<Option<i32>>,
    pub settings: Vec<Option<u8>>,
    pub surfaces: Vec<Option<CameraSurface>>,
}

impl CameraScene {
    pub fn from_scene(scene: &SceneData, id: &str, generation: u64) -> Result<Self, RenderError> {
        if scene.min_x != 0
            || scene.min_y != 0
            || !(1..=104).contains(&scene.max_x)
            || !(1..=104).contains(&scene.max_y)
            || scene.planes != 4
        {
            return Err(RenderError::Scene(
                "normal camera requires the original four-plane, at-most-104-tile main area".into(),
            ));
        }
        let width = scene.max_x;
        let height = scene.max_y;
        let mut heights = Vec::new();
        let mut settings = Vec::new();
        let mut surfaces = Vec::new();
        for plane in 0..4 {
            for x in 0..=width {
                for y in 0..=height {
                    heights.push(scene.camera_height(plane, x + scene.offset, y + scene.offset));
                }
            }
            for x in 0..width {
                for y in 0..height {
                    settings.push(scene.camera_setting(plane, x + scene.offset, y + scene.offset));
                    surfaces.push(surface(scene, plane, x, y)?);
                }
            }
        }
        let result = Self {
            version: 1,
            id: id.to_owned(),
            generation: generation.to_string(),
            base: WorldBase {
                x: scene.base_x,
                y: scene.base_y,
            },
            dimensions: [width as u32, height as u32],
            heights,
            settings,
            surfaces,
        };
        result.validate().map_err(RenderError::Scene)?;
        Ok(result)
    }

    pub fn validate(&self) -> Result<(), String> {
        let [w, h] = self.dimensions;
        if self.version != 1
            || self.id.is_empty()
            || self.id.len() > 256
            || !decimal(&self.generation)
            || !(1..=104).contains(&w)
            || !(1..=104).contains(&h)
            || self.heights.len() != (4 * (w + 1) * (h + 1)) as usize
            || self.settings.len() != (4 * w * h) as usize
            || self.surfaces.len() != self.settings.len()
        {
            return Err(
                "invalid original camera scene identity, dimensions or array lengths".into(),
            );
        }
        for tile in self.surfaces.iter().flatten() {
            if tile.triangles.len() > 16 || tile.decoration.is_some_and(|id| id < 0) {
                return Err("invalid original camera surface topology or object identity".into());
            }
            for triangle in &tile.triangles {
                if triangle
                    .horizontal
                    .iter()
                    .any(|p| p[0] < 0 || p[1] < 0 || p[0] > w as i32 * 128 || p[1] > h as i32 * 128)
                {
                    return Err("original camera surface vertex is outside the source scene".into());
                }
                triangle
                    .height_at(triangle.horizontal[0])
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    pub fn required_objects(&self) -> Vec<i32> {
        self.surfaces
            .iter()
            .flatten()
            .filter_map(|s| s.decoration)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
}

fn surface(
    scene: &SceneData,
    plane: i32,
    x: i32,
    y: i32,
) -> Result<Option<CameraSurface>, RenderError> {
    let ex = x + scene.offset;
    let ey = y + scene.offset;
    if scene.camera_setting(plane, ex, ey).is_none() {
        return Ok(None);
    }
    let Some(settings) = scene.camera_setting(1, ex, ey) else {
        return Ok(None);
    };
    let index = scene.tile_index(plane, ex, ey);
    let base_index = scene.tile_index(0, ex, ey);
    let bridge = settings & 2 != 0;
    let base_flags = scene.flags.get(base_index).ok_or_else(|| {
        RenderError::Scene("camera source bridge index is outside the scene".into())
    })?;
    if (base_flags & flag::BRIDGE_BELOW != 0) != bridge {
        return Err(RenderError::Scene(format!(
            "camera source bridge settings/geometry disagree at {},{}",
            scene.base_x + x,
            scene.base_y + y
        )));
    }
    // On bridge tiles slot 3 contains the below-bridge plane, not a source plane-3 surface.
    if bridge && plane == 3 {
        return Ok(None);
    }
    let height_plane = plane + i32::from(bridge);
    let flags = *scene.flags.get(index).ok_or_else(|| {
        RenderError::Scene("camera source tile index is outside the scene".into())
    })?;
    let mut triangles = Vec::new();
    if flags & flag::PAINT != 0 {
        if !scene.paints.contains_key(&index) {
            return Err(RenderError::Scene(
                "camera tile flag names an absent original paint".into(),
            ));
        }
        let corners = [(0, 0), (1, 0), (1, 1), (0, 1)]
            .map(|(dx, dy)| scene.camera_height(height_plane, ex + dx, ey + dy));
        let [Some(sw), Some(se), Some(ne), Some(nw)] = corners else {
            return Ok(None);
        };
        let x = x * 128;
        let y = y * 128;
        triangles.extend([
            SurfaceTriangle {
                horizontal: [[x + 128, y + 128], [x, y + 128], [x + 128, y]],
                heights: [ne, nw, se],
            },
            SurfaceTriangle {
                horizontal: [[x, y], [x + 128, y], [x, y + 128]],
                heights: [sw, se, nw],
            },
        ]);
    } else if flags & flag::TILE_MODEL != 0 {
        let model = scene.tile_models.get(&index).ok_or_else(|| {
            RenderError::Scene("camera tile flag names an absent original model".into())
        })?;
        if model.xs.len() != model.ys.len()
            || model.xs.len() != model.zs.len()
            || model.face_a.len() != model.face_b.len()
            || model.face_a.len() != model.face_c.len()
        {
            return Err(RenderError::Scene(
                "camera original tile model arrays disagree".into(),
            ));
        }
        for ((&a, &b), &c) in model.face_a.iter().zip(&model.face_b).zip(&model.face_c) {
            let indices = [a, b, c];
            if indices
                .iter()
                .any(|&i| i < 0 || i as usize >= model.xs.len())
            {
                return Err(RenderError::Scene(
                    "camera original tile model face is out of bounds".into(),
                ));
            }
            let indices = indices.map(|i| i as usize);
            triangles.push(SurfaceTriangle {
                horizontal: indices.map(|i| [model.xs[i], model.zs[i]]),
                heights: indices.map(|i| model.ys[i]),
            });
        }
    }
    let decoration = scene
        .floor_decorations
        .get(&index)
        .map(|d| tag_object_id(d.hash));
    if flags & flag::FLOOR_DECOR != 0 && decoration.is_none() {
        return Err(RenderError::Scene(
            "camera ground decoration lacks its original object tag".into(),
        ));
    }
    Ok(Some(CameraSurface {
        triangles,
        decoration,
    }))
}

/// Parsed from actual decoded object metadata. No model bound, height or ID-derived substitute.
#[derive(Clone, Debug, Deserialize)]
pub struct CameraObjectDefinition {
    pub id: i32,
    pub raise: i32,
}

pub struct CameraTerrain {
    scene: CameraScene,
    raises: BTreeMap<i32, i32>,
}

impl CameraTerrain {
    pub fn scene(&self) -> &CameraScene {
        &self.scene
    }

    pub fn new(
        scene: CameraScene,
        definitions: Vec<CameraObjectDefinition>,
    ) -> Result<Self, String> {
        scene.validate()?;
        let mut raises = BTreeMap::new();
        for definition in definitions {
            if definition.id < 0 || raises.insert(definition.id, definition.raise).is_some() {
                return Err("duplicate/invalid source ground-decoration definition".into());
            }
        }
        Ok(Self { scene, raises })
    }

    fn tile_index(&self, plane: u8, x: u32, y: u32) -> Option<usize> {
        let [w, h] = self.scene.dimensions;
        (plane < 4 && x < w && y < h).then(|| ((u32::from(plane) * w + x) * h + y) as usize)
    }

    pub fn require_surface(&self, plane: u8, point: [i32; 2]) -> Result<(), String> {
        let tile = self
            .tile_index(plane, (point[0] >> 7) as u32, (point[1] >> 7) as u32)
            .and_then(|i| self.scene.surfaces[i].as_ref())
            .ok_or_else(|| {
                format!(
                    "original camera surface unavailable on plane {plane} at local {},{}",
                    point[0], point[1]
                )
            })?;
        if tile.triangles.is_empty() {
            return Err("original rendered camera surface has no triangles; no bilinear/zero substitute is enabled".into());
        }
        if let Some(id) = tile.decoration
            && !self.raises.contains_key(&id)
        {
            return Err(format!(
                "original ground-decoration object {id} is missing its decoded raise"
            ));
        }
        self.surface_height(plane, point[0], point[1])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl Terrain for CameraTerrain {
    fn world_base(&self) -> WorldBase {
        self.scene.base
    }
    fn dimensions(&self) -> [u32; 2] {
        self.scene.dimensions
    }

    fn height_corner(&self, plane: u8, x: u32, y: u32) -> clubscape_camera::Result<i32> {
        let [w, h] = self.scene.dimensions;
        if plane < 4
            && x <= w
            && y <= h
            && let Some(value) =
                self.scene.heights[((u32::from(plane) * (w + 1) + x) * (h + 1) + y) as usize]
        {
            return Ok(value);
        }
        Err(CameraError::MissingHeight { plane, x, y })
    }

    fn tile_settings(&self, plane: u8, x: u32, y: u32) -> clubscape_camera::Result<u8> {
        self.tile_index(plane, x, y)
            .and_then(|i| self.scene.settings[i])
            .ok_or(CameraError::MissingTileSettings { plane, x, y })
    }

    fn surface_height(&self, plane: u8, x: i32, y: i32) -> clubscape_camera::Result<i32> {
        let tile = self
            .tile_index(plane, (x >> 7) as u32, (y >> 7) as u32)
            .and_then(|i| self.scene.surfaces[i].as_ref())
            .ok_or(CameraError::InvalidSurface)?;
        for triangle in &tile.triangles {
            if let Some(height) = triangle.height_at([x, y])? {
                let raise = match tile.decoration {
                    Some(id) => *self.raises.get(&id).ok_or(CameraError::InvalidSurface)?,
                    None => 0,
                };
                return height
                    .checked_sub(raise)
                    .ok_or(CameraError::ArithmeticOverflow);
            }
        }
        Err(CameraError::InvalidSurface)
    }
}

pub fn decimal(value: &str) -> bool {
    value.parse::<u64>().is_ok_and(|n| n.to_string() == value)
}

/// The selected renderer consumes the integer/table lane, with absolute horizontal positions.
pub fn renderer_camera(
    output: CameraOutput,
    viewport: Viewport,
    far: i32,
) -> Result<Camera, String> {
    if output.projection.viewport != viewport || viewport.x != 0 || viewport.y != 0 {
        return Err(
            "native camera projection needs unsupported renderer subviewport/letterboxing".into(),
        );
    }
    if output.projection.zoom <= 0
        || far < 50
        || !(1024..=3064).contains(&output.pitch_native)
        || !(0..16384).contains(&output.yaw_native)
    {
        return Err("invalid native camera projection, angles or renderer clipping".into());
    }
    let world = |base: i32, local: i32| {
        base.checked_mul(128)
            .and_then(|b| b.checked_add(local))
            .ok_or_else(|| "native camera absolute-coordinate overflow".to_owned())
    };
    Ok(Camera {
        x: world(output.world_base.x, output.eye_native[0])?,
        height: output.eye_native[1],
        y: world(output.world_base.y, output.eye_native[2])?,
        pitch: output.pitch_native,
        yaw: output.yaw_native,
        zoom: output.projection.zoom,
        far,
    })
}

pub fn camera_json(camera: Camera) -> serde_json::Value {
    serde_json::json!({
        "x": camera.x, "height": camera.height, "y": camera.y,
        "pitch": camera.pitch, "yaw": camera.yaw, "unitsPerTurn": 16384,
        "zoom": camera.zoom, "near": 50, "far": camera.far,
    })
}
