use std::io::{self, Write};

use axum::http::StatusCode;
use clubscape_game_types::{
    Activity, CharacterState, GAME_SCHEMA_VERSION, GameError, GameErrorCode, GameEvent, GameIntent,
    INVENTORY_SLOTS, WorldState,
};
use clubscape_protocol::ErrorCode;
use serde::{Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::ApiError;

pub(super) const MAX_WORLD_BYTES: usize = 4 * 1024 * 1024;
pub(super) const MAX_CHARACTER_BYTES: usize = 128 * 1024;
pub(super) const MAX_RESULT_BYTES: usize = 256 * 1024;
pub(super) const MAX_CHARACTERS: usize = 256;
const MAX_INTENT_BYTES: usize = 4096;

struct BoundedJson {
    bytes: Vec<u8>,
    limit: usize,
}

impl Write for BoundedJson {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("JSON storage bound exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn encode(value: &impl Serialize, limit: usize) -> Result<String, ApiError> {
    let mut writer = BoundedJson {
        bytes: Vec::new(),
        limit,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| {
        ApiError::invalid("The game data exceeds its storage or serialization bounds.")
    })?;
    String::from_utf8(writer.bytes).map_err(|_| ApiError::internal("game_json_encoding"))
}

pub(super) fn decode<T: DeserializeOwned>(json: Option<&str>, limit: usize) -> Result<T, ApiError> {
    // JSONB's canonical text includes whitespace; reserve twice the compact write bound.
    let json = json
        .filter(|json| json.len() <= limit * 2)
        .ok_or_else(|| ApiError::internal("game_stored_data_size"))?;
    serde_json::from_str(json).map_err(|_| ApiError::internal("game_stored_data_decode"))
}

pub(super) fn conflict(message: &'static str) -> ApiError {
    ApiError::new(StatusCode::CONFLICT, ErrorCode::Conflict, message)
}

pub(super) fn uuid(id: Uuid) -> Result<(), ApiError> {
    if id.is_nil() {
        return Err(ApiError::invalid("A non-nil UUID is required."));
    }
    Ok(())
}

pub(super) fn number(value: u64) -> Result<i64, ApiError> {
    i64::try_from(value)
        .map_err(|_| ApiError::invalid("The game revision or sequence is out of range."))
}

pub(super) fn increment(value: u64) -> Result<u64, ApiError> {
    value
        .checked_add(1)
        .filter(|value| *value <= i64::MAX as u64)
        .ok_or_else(|| conflict("The game revision or sequence is exhausted."))
}

pub(super) fn text(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}

pub(super) fn validate_character(character: &CharacterState, tick: u64) -> Result<(), ApiError> {
    let valid = character.schema_version == GAME_SCHEMA_VERSION
        && text(&character.display_name, 80)
        && character.appearance.len() <= 64
        && character.appearance.keys().all(|key| text(key, 160))
        && character.equipment.len() <= 64
        && character.bank.slots.len() <= usize::from(character.bank.capacity)
        && character.bank.capacity <= 4096
        && character.skills.len() <= 256
        && character.quests.len() <= 2048
        && character.flags.len() <= 2048
        && character.flags.keys().all(|key| text(key, 160))
        && character.interfaces.len() <= 2048
        && character.last_action_tick <= tick
        && character.last_command_sequence <= i64::MAX as u64
        && character
            .quests
            .values()
            .all(|quest| quest.flags.len() <= 2048 && quest.flags.keys().all(|key| text(key, 160)))
        && character
            .dialogue
            .as_ref()
            .is_none_or(|dialogue| text(&dialogue.node, 160))
        && match &character.activity {
            Activity::Walking { path, .. } => path.len() <= 4096,
            Activity::Fighting { style, .. } => text(style, 160),
            Activity::Casting { spell, .. } => text(spell, 160),
            _ => true,
        };
    if !valid {
        return Err(ApiError::invalid(
            "The character state has an unsupported shape or bound.",
        ));
    }
    encode(character, MAX_CHARACTER_BYTES)?;
    Ok(())
}

pub(super) fn validate_world(world: &WorldState) -> Result<(), ApiError> {
    if world.schema_version != GAME_SCHEMA_VERSION
        || !text(&world.content_revision, 256)
        || world.characters.len() > MAX_CHARACTERS
        || world.entities.len() > 32_768
        || world.shops.len() > 4096
        || world.ground_items.len() > 32_768
    {
        return Err(ApiError::invalid(
            "The world state has an unsupported shape or bound.",
        ));
    }
    number(world.tick)?;
    number(world.revision)?;
    for (actor, character) in &world.characters {
        if *actor != character.actor_id {
            return Err(ApiError::invalid(
                "Character identities must match their world keys.",
            ));
        }
        validate_character(character, world.tick)?;
    }
    if world
        .entities
        .values()
        .any(|entity| entity.flags.len() > 2048 || entity.flags.keys().any(|key| !text(key, 160)))
        || world.shops.values().any(|shop| shop.stock.len() > 4096)
        || world.ground_items.iter().any(|item| !text(&item.id, 160))
    {
        return Err(ApiError::invalid(
            "The dynamic world collections exceed their storage bounds.",
        ));
    }
    Ok(())
}

pub(super) fn validate_events(events: &[GameEvent]) -> Result<(), ApiError> {
    if events.len() > 1024 {
        return Err(ApiError::invalid(
            "The event result exceeds its storage bounds.",
        ));
    }
    Ok(())
}

pub(super) fn intent_hash(intent: &GameIntent) -> Result<[u8; 32], ApiError> {
    let slot = |slot: u8| usize::from(slot) < INVENTORY_SLOTS;
    let valid = match intent {
        GameIntent::Equip { inventory_slot }
        | GameIntent::Drop { inventory_slot, .. }
        | GameIntent::Eat { inventory_slot }
        | GameIntent::BankDeposit { inventory_slot, .. }
        | GameIntent::ShopSell { inventory_slot, .. } => slot(*inventory_slot),
        GameIntent::UseItem {
            inventory_slot,
            target,
        } => {
            slot(*inventory_slot)
                && match target {
                    clubscape_game_types::ItemTarget::Inventory { slot: target } => slot(*target),
                    clubscape_game_types::ItemTarget::World { .. } => true,
                }
        }
        GameIntent::MoveInventory { from, to } => slot(*from) && slot(*to),
        GameIntent::Interact { action, .. } => text(action, 160),
        GameIntent::SelectDialogue { choice, .. } => text(choice, 160),
        GameIntent::TakeGroundItem { ground_item_id } => text(ground_item_id, 160),
        GameIntent::SetCombatStyle { style } => text(style, 160),
        GameIntent::Cast { spell, .. } => text(spell, 160),
        GameIntent::SetPrayer { prayer, .. } => text(prayer, 160),
        _ => true,
    };
    if !valid {
        return Err(ApiError::invalid("The game intent is out of range."));
    }
    let json = encode(intent, MAX_INTENT_BYTES)?;
    let mut canonical: serde_json::Value =
        serde_json::from_str(&json).map_err(|_| ApiError::internal("game_intent_encoding"))?;
    canonical.sort_all_objects();
    let canonical = encode(&canonical, MAX_INTENT_BYTES)?;
    let mut hash = Sha256::new();
    hash.update(b"clubscape.game-intent.v1\0");
    hash.update(canonical.as_bytes());
    Ok(hash.finalize().into())
}

pub(super) fn callback_error(source: GameError) -> ApiError {
    let (status, code, message) = match source.code {
        GameErrorCode::InvalidInput | GameErrorCode::UnknownContent => (
            StatusCode::BAD_REQUEST,
            ErrorCode::InvalidArgument,
            "The game intent is not supported.",
        ),
        GameErrorCode::InvalidContent => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::Internal,
            "The configured game content is invalid.",
        ),
        GameErrorCode::Unavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::Unavailable,
            "The game operation is unavailable.",
        ),
        _ => (
            StatusCode::CONFLICT,
            ErrorCode::Conflict,
            "The game operation failed its validation requirements.",
        ),
    };
    let error = ApiError::new(status, code, message);
    tracing::warn!(
        event = "game_validation_failure",
        error_id = %error.error_id,
        game_error_code = ?source.code,
        "game command validation failed"
    );
    error
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialization_and_intents_have_explicit_bounds() {
        assert!(encode(&"x".repeat(64), 16).is_err());
        assert!(decode::<Vec<u8>>(Some("[]"), 1).is_ok());
        assert!(decode::<Vec<u8>>(None, 1).is_err());
        assert!(intent_hash(&GameIntent::Equip { inventory_slot: 28 }).is_err());
        assert!(
            intent_hash(&GameIntent::SetPrayer {
                prayer: "secret".repeat(100),
                enabled: true,
            })
            .is_err()
        );
        assert_eq!(
            intent_hash(&GameIntent::CancelActivity).unwrap(),
            intent_hash(&GameIntent::CancelActivity).unwrap()
        );
        assert_ne!(
            intent_hash(&GameIntent::CancelActivity).unwrap(),
            intent_hash(&GameIntent::RequestLogout).unwrap()
        );
    }

    #[test]
    fn callback_failures_never_expose_source_messages() {
        let error = callback_error(GameError::new(
            GameErrorCode::InvalidContent,
            "secret SQL/token",
        ));
        assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!format!("{error:?}").contains("secret"));
        assert!(!error.error_id.is_nil());
    }
}
