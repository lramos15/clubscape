use std::collections::{BTreeMap, BTreeSet};

use clubscape_game_types::{
    CharacterState, EquipmentDefinition, GameContent, GameError, GameErrorCode, GameEvent,
    GameResult, ItemStack, SlotId,
};

use crate::{
    add_quantities, inventory, item_definition,
    skills::{self, LevelBasis},
};

/// Equipment is stored once at each item's primary slot. Other occupancy is derived.
pub fn validate(equipment: &BTreeMap<SlotId, ItemStack>, content: &GameContent) -> GameResult<()> {
    validate_slot_catalog(content)?;
    let mut occupied = BTreeSet::new();
    for (slot, stack) in equipment {
        crate::require_ordinary_stack(stack)?;
        let item = item_definition(&content.items, &stack.item)?;
        let definition = item.equipment.as_ref().ok_or_else(|| {
            GameError::new(
                GameErrorCode::InvalidInput,
                format!("Equipped item {} has no equipment definition.", stack.item),
            )
        })?;
        if definition.slot != *slot || (!item.stackable.fixed()? && stack.quantity.get() != 1) {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                format!(
                    "Invalid equipment placement or quantity for {}.",
                    stack.item
                ),
            ));
        }
        for claimed in occupied_slots(definition, content)? {
            if !occupied.insert(claimed) {
                return Err(GameError::new(
                    GameErrorCode::InvalidInput,
                    "Equipment entries overlap functional slots.",
                ));
            }
        }
    }
    Ok(())
}

/// Resolves either a primary slot or a secondary occupied slot to the single owner.
pub fn occupant<'a>(
    equipment: &'a BTreeMap<SlotId, ItemStack>,
    content: &GameContent,
    slot: &SlotId,
) -> GameResult<Option<(&'a SlotId, &'a ItemStack)>> {
    validate(equipment, content)?;
    require_known_slot(content, slot)?;
    for (primary, stack) in equipment {
        let definition = equipment_definition(content, stack)?;
        if occupied_slots(definition, content)?.contains(slot) {
            return Ok(Some((primary, stack)));
        }
    }
    Ok(None)
}

/// Equips the selected stack, checking requirements against XP-derived base levels.
pub fn equip(
    character: &mut CharacterState,
    content: &GameContent,
    inventory_slot: usize,
) -> GameResult<GameEvent> {
    let stack = inventory::stack_at(&character.inventory, inventory_slot)?;
    if equipment_definition(content, stack)?
        .requirements
        .iter()
        .any(|requirement| requirement.basis != clubscape_game_types::SkillLevelBasis::Base)
    {
        return Err(GameError::new(
            GameErrorCode::Unavailable,
            "Source equipment basis requires explicit level-basis selection.",
        ));
    }
    equip_with_level_basis(character, content, inventory_slot, LevelBasis::Base)
}

/// An explicit source policy may instead use temporary current levels.
pub fn equip_with_level_basis(
    character: &mut CharacterState,
    content: &GameContent,
    inventory_slot: usize,
    level_basis: LevelBasis,
) -> GameResult<GameEvent> {
    inventory::validate(&character.inventory, &content.items)?;
    validate(&character.equipment, content)?;
    let mut incoming = inventory::stack_at(&character.inventory, inventory_slot)?.clone();
    let item = item_definition(&content.items, &incoming.item)?;
    let definition = equipment_definition(content, &incoming)?;
    let claimed = occupied_slots(definition, content)?;
    skills::check_requirements(
        &character.skills,
        &content.skills,
        &definition.requirements,
        level_basis,
    )?;

    let mut inventory = character.inventory.clone();
    let mut equipment = character.equipment.clone();
    inventory::remove_from_slot(
        &mut inventory,
        &content.items,
        inventory_slot,
        incoming.quantity,
    )?;
    let mut conflicts = Vec::new();
    for (slot, stored) in &equipment {
        let existing = equipment_definition(content, stored)?;
        if !claimed.is_disjoint(&occupied_slots(existing, content)?) {
            conflicts.push(slot.clone());
        }
    }
    for slot in conflicts {
        let displaced = equipment.remove(&slot).ok_or_else(|| {
            GameError::new(
                GameErrorCode::InvalidInput,
                "Conflicting equipment disappeared.",
            )
        })?;
        if displaced.item == incoming.item && item.stackable.fixed()? {
            incoming.quantity = add_quantities(incoming.quantity, displaced.quantity)?;
        } else {
            inventory::add(&mut inventory, &content.items, &displaced)?;
        }
    }
    equipment.insert(definition.slot.clone(), incoming.clone());
    character.inventory = inventory;
    character.equipment = equipment;
    Ok(GameEvent::Equipped {
        slot: definition.slot.clone(),
        stack: incoming,
    })
}

/// Unequips the owner of a primary or occupied slot, returning its entire stack.
pub fn unequip(
    character: &mut CharacterState,
    content: &GameContent,
    slot: &SlotId,
) -> GameResult<ItemStack> {
    inventory::validate(&character.inventory, &content.items)?;
    let (primary, stored) = occupant(&character.equipment, content, slot)?.ok_or_else(|| {
        GameError::new(
            GameErrorCode::NotOwned,
            format!("Equipment slot {slot} is empty."),
        )
    })?;
    let primary = primary.clone();
    let stack = stored.clone();
    let mut inventory = character.inventory.clone();
    inventory::add(&mut inventory, &content.items, &stack)?;
    character.inventory = inventory;
    character.equipment.remove(&primary);
    Ok(stack)
}

fn equipment_definition<'a>(
    content: &'a GameContent,
    stack: &ItemStack,
) -> GameResult<&'a EquipmentDefinition> {
    item_definition(&content.items, &stack.item)?
        .equipment
        .as_ref()
        .ok_or_else(|| {
            GameError::new(
                GameErrorCode::InvalidInput,
                format!("Item {} cannot be equipped.", stack.item),
            )
        })
}

fn occupied_slots(
    definition: &EquipmentDefinition,
    content: &GameContent,
) -> GameResult<BTreeSet<SlotId>> {
    require_known_slot(content, &definition.slot)?;
    let mut listed = BTreeSet::new();
    for slot in &definition.occupied_slots {
        require_known_slot(content, slot)?;
        if !listed.insert(slot.clone()) {
            return Err(GameError::new(
                GameErrorCode::InvalidContent,
                "Equipment definition repeats an occupied slot.",
            ));
        }
    }
    listed.insert(definition.slot.clone());
    Ok(listed)
}

fn validate_slot_catalog(content: &GameContent) -> GameResult<()> {
    let unique: BTreeSet<_> = content.equipment_slots.iter().collect();
    if unique.len() != content.equipment_slots.len() {
        return Err(GameError::new(
            GameErrorCode::InvalidContent,
            "Equipment slot catalog contains duplicates.",
        ));
    }
    Ok(())
}

fn require_known_slot(content: &GameContent, slot: &SlotId) -> GameResult<()> {
    if !content.equipment_slots.contains(slot) {
        return Err(GameError::new(
            GameErrorCode::UnknownContent,
            format!("Unknown equipment slot {slot}."),
        ));
    }
    Ok(())
}
