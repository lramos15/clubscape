mod content;
mod error;
mod execution;
mod gameplay_ui;
mod ids;
mod intent;
mod mechanics;
mod numeric_keys;
mod observer;
mod runtime_state;
mod runtime_validation;
mod state;
mod ui_state;

pub use content::*;
pub use error::*;
pub use execution::*;
pub use gameplay_ui::*;
pub use ids::*;
pub use intent::*;
pub use mechanics::*;
pub use observer::*;
pub use runtime_state::*;
pub use state::*;
pub use ui_state::*;

/// The additive persisted world/character envelope; not the content format.
pub const GAME_SCHEMA_VERSION: u32 = 1;
pub const CONTENT_SCHEMA_VERSION: u32 = 4;
pub const RUNTIME_SCHEMA_VERSION: u32 = 1;
pub const TICK_MILLISECONDS: u64 = 600;
pub const INVENTORY_SLOTS: usize = 28;
pub const MAX_RUN_ENERGY: u16 = 10_000;
pub const MAX_STACK_QUANTITY: u32 = i32::MAX as u32;
