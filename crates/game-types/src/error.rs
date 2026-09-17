use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GameErrorCode {
    InvalidInput,
    UnknownContent,
    InvalidContent,
    Unavailable,
    NotOwned,
    InsufficientItems,
    InventoryFull,
    StackOverflow,
    RequirementNotMet,
    OutOfReach,
    Blocked,
    Busy,
    StaleCommand,
    SessionConflict,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{message}")]
pub struct GameError {
    pub code: GameErrorCode,
    pub message: String,
}

impl GameError {
    pub fn new(code: GameErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

pub type GameResult<T> = Result<T, GameError>;
