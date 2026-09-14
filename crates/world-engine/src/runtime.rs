use clubscape_game_types::*;

use crate::{invalid_state, unknown};

pub(crate) const PREFIX: &str = "__world_engine.";
pub(crate) const COMMAND_SEEN: &str = "__world_engine.command_seen";
pub(crate) const GATHER_INTERACTION: &str = "__world_engine.gather_interaction";
pub(crate) const DIALOGUE_INTERACTION: &str = "__world_engine.dialogue_interaction";
pub(crate) const FOOD_READY: &str = "__world_engine.food_ready";
pub(crate) const ATTACK_READY: &str = "__world_engine.attack_ready";

pub(crate) fn deadline(now: u64, delay: u64) -> GameResult<u64> {
    now.checked_add(delay)
        .filter(|tick| *tick <= i64::MAX as u64)
        .ok_or_else(|| invalid_state("Tick deadline overflow."))
}

pub(crate) fn read_counter(character: &CharacterState, key: &str) -> GameResult<u64> {
    match character.flags.get(key) {
        Some(value) => {
            u64::try_from(*value).map_err(|_| invalid_state("Negative runtime counter."))
        }
        None => Ok(0),
    }
}

pub(crate) fn set_counter(character: &mut CharacterState, key: &str, value: u64) -> GameResult<()> {
    character.flags.insert(
        key.into(),
        i64::try_from(value).map_err(|_| invalid_state("Runtime counter overflow."))?,
    );
    Ok(())
}

pub(crate) fn close_access(character: &mut CharacterState) {
    character
        .flags
        .retain(|key, _| !key.starts_with("__world_engine.access."));
}

pub(crate) fn interrupt(character: &mut CharacterState) {
    character.activity = Activity::Idle;
    character.flags.remove(GATHER_INTERACTION);
    character.flags.remove(DIALOGUE_INTERACTION);
    character.dialogue = None;
    close_access(character);
}

pub(crate) fn open_access(
    character: &mut CharacterState,
    kind: &str,
    spawn: &SpawnId,
    interaction_index: usize,
) -> GameResult<()> {
    close_access(character);
    set_counter(
        character,
        &format!("{PREFIX}access.{kind}:{spawn}"),
        interaction_index as u64 + 1,
    )
}

pub(crate) fn access(character: &CharacterState, kind: &str) -> GameResult<(SpawnId, usize)> {
    let prefix = format!("{PREFIX}access.{kind}:");
    let mut entries = character
        .flags
        .iter()
        .filter(|(key, _)| key.starts_with(&prefix));
    let (key, value) = entries.next().ok_or_else(|| {
        GameError::new(
            GameErrorCode::RequirementNotMet,
            format!("No open {kind} interaction."),
        )
    })?;
    if entries.next().is_some() || *value <= 0 {
        return Err(invalid_state("Invalid persisted access session."));
    }
    let spawn = SpawnId::new(&key[prefix.len()..])?;
    let index = usize::try_from(*value - 1).map_err(|_| invalid_state("Invalid session index."))?;
    Ok((spawn, index))
}

pub(crate) fn interaction<'a>(
    content: &'a GameContent,
    spawn: &SpawnId,
    index: usize,
) -> GameResult<&'a InteractionDefinition> {
    content
        .spawns
        .get(spawn)
        .ok_or_else(|| unknown(format!("Unknown spawn {spawn}.")))?
        .interactions
        .get(index)
        .ok_or_else(|| invalid_state("Persisted interaction no longer exists in this revision."))
}
