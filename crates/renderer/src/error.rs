//! Error type shared by loaders, the CPU rasterizer and the GPU path.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// Malformed or truncated exported buffer.
    Format(String),
    /// A required asset (model, texture, region, palette) is missing or failed its hash check.
    MissingAsset(String),
    /// Asset content is inconsistent (index out of range, wrong counts, unsupported feature).
    InvalidAsset(String),
    /// A GPU capability, device or submission failure. Never silently recovered.
    Gpu(String),
    /// Scene state problem (unknown scene id, region not loaded, bad camera).
    Scene(String),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::Format(m) => write!(f, "format: {m}"),
            RenderError::MissingAsset(m) => write!(f, "missing asset: {m}"),
            RenderError::InvalidAsset(m) => write!(f, "invalid asset: {m}"),
            RenderError::Gpu(m) => write!(f, "gpu: {m}"),
            RenderError::Scene(m) => write!(f, "scene: {m}"),
        }
    }
}

impl std::error::Error for RenderError {}
