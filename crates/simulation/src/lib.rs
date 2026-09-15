//! Deterministic, headless mechanics over caller-owned state and explicit content.
//!
//! Operations do not persist state, schedule ticks, load content, or authorize
//! access to interfaces. Callers retain those responsibilities.

pub mod bank;
pub mod bank_layout;
pub mod equipment;
pub mod inventory;
pub mod navigation;
pub mod skills;

use std::collections::BTreeMap;

use clubscape_game_types::{
    CharacterState, GameError, GameErrorCode, GameResult, ItemDefinition, ItemId,
    MAX_STACK_QUANTITY, Quantity,
};

pub type ItemDefinitions = BTreeMap<ItemId, ItemDefinition>;

/// Composes pure character-state operations into one all-or-nothing mutation.
/// External side effects inside the closure cannot be rolled back by this helper.
pub fn transact_character<T>(
    character: &mut CharacterState,
    operation: impl FnOnce(&mut CharacterState) -> GameResult<T>,
) -> GameResult<T> {
    let mut draft = character.clone();
    let result = operation(&mut draft)?;
    *character = draft;
    Ok(result)
}

pub(crate) fn item_definition<'a>(
    items: &'a ItemDefinitions,
    item: &ItemId,
) -> GameResult<&'a ItemDefinition> {
    let definition = items.get(item).ok_or_else(|| {
        GameError::new(
            GameErrorCode::UnknownContent,
            format!("Unknown item {item}."),
        )
    })?;
    if definition.id != *item {
        return Err(GameError::new(
            GameErrorCode::InvalidContent,
            format!("Item definition key does not match {item}."),
        ));
    }
    definition.stackable.fixed()?;
    if definition.charges.is_some() {
        return Err(GameError::new(
            GameErrorCode::Unavailable,
            "Charged item instances require source-aware container operations.",
        ));
    }
    Ok(definition)
}

pub(crate) fn require_ordinary_stack(stack: &clubscape_game_types::ItemStack) -> GameResult<()> {
    if stack.instance.is_some() {
        return Err(GameError::new(
            GameErrorCode::Unavailable,
            "Per-item instances require instance-aware ownership and transfer operations.",
        ));
    }
    Ok(())
}

pub(crate) fn add_quantities(left: Quantity, right: Quantity) -> GameResult<Quantity> {
    let total = left
        .get()
        .checked_add(right.get())
        .filter(|total| *total <= MAX_STACK_QUANTITY)
        .ok_or_else(|| {
            GameError::new(GameErrorCode::StackOverflow, "Item stack limit exceeded.")
        })?;
    Quantity::new(total)
}
