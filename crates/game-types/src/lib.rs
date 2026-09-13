mod content;
mod error;
mod ids;
mod intent;
mod state;

pub use content::*;
pub use error::*;
pub use ids::*;
pub use intent::*;
pub use state::*;

pub const GAME_SCHEMA_VERSION: u32 = 1;
pub const TICK_MILLISECONDS: u64 = 600;
pub const INVENTORY_SLOTS: usize = 28;
pub const MAX_STACK_QUANTITY: u32 = i32::MAX as u32;
