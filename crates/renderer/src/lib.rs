//! ClubScape source-faithful renderer core.
//!
//! * [`raster`]: original triangle fill algorithms (exact CPU reference) and the triangle
//!   command stream consumed by the GPU compute rasterizer.
//! * [`model_draw`]: original model projection, culling, depth/priority ordering.
//! * [`scene`]: original scene traversal (tile order, occlusion, walls/decor/objects).
//! * [`palette`], [`texture`], [`tables`]: original color tables.
//! * `gpu` (feature `gpu`): wgpu compute rasterizer and canvas presentation.
//! * `wasm` (feature `web`): wasm-bindgen ABI used by `web/renderer`.
//!
//! Nothing here is a source reference image. All geometry and colors come from the exported
//! original data (`assets/compiled/render`) and the authoritative world view supplied by the
//! browser shell.

pub mod chunk;
pub mod error;
pub mod model;
pub mod model_draw;
pub mod palette;
pub mod raster;
pub mod tables;
pub mod texture;

pub use error::RenderError;
