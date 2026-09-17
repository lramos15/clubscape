//! wgpu compute rasterizer executing the original fills per pixel (`raster.wgsl`), plus the
//! CPU-side triangle packing/binning it consumes. The output texture is presented to a canvas
//! by the web adapter or read back for differential tests against the CPU reference.

pub mod device;
pub mod pack;

pub use device::{FrameRecord, GpuFrame, GpuRasterizer, GpuTextures};
pub use pack::{PackedFrame, pack_frame};
