use std::collections::BTreeSet;

use clubscape_game_types::{
    Bank, CharacterState, GameContent, GameError, GameErrorCode, GameResult, ItemDefinition,
    ItemId, ItemStack, Quantity,
};

use crate::{ItemDefinitions, add_quantities, inventory, item_definition};

/// Bank slots form a stable, possibly shorter-than-capacity prefix.
/// Each unnoted item has at most one stack, even when nonstackable in an inventory.
pub fn validate(bank: &Bank, items: &ItemDefinitions) -> GameResult<()> {
    if bank.slots.len() > usize::from(bank.capacity) {
        return Err(GameError::new(
            GameErrorCode::InvalidInput,
            "Bank slot storage exceeds its declared capacity.",
        ));
    }
    let mut seen = BTreeSet::new();
    for stack in bank.slots.iter().flatten() {
        crate::require_ordinary_stack(stack)?;
        let (unnoted, _) = forms(items, &stack.item)?;
        if unnoted.id != stack.item || !seen.insert(&stack.item) {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                "Bank must contain one unnoted stack per item.",
            ));
        }
    }
    Ok(())
}

/// Counts a base item or its explicitly mapped notes in the same bank stack.
pub fn count(bank: &Bank, items: &ItemDefinitions, item: &ItemId) -> GameResult<u32> {
    validate(bank, items)?;
    let (unnoted, _) = forms(items, item)?;
    Ok(bank
        .slots
        .iter()
        .flatten()
        .find(|stack| stack.item == unnoted.id)
        .map_or(0, |stack| stack.quantity.get()))
}

/// The selected slot chooses the item; quantities may span other copies.
/// The selected slot is consumed first, then other copies in ascending slot order.
/// Notes are converted only through an explicit reciprocal content mapping.
pub fn deposit(
    character: &mut CharacterState,
    content: &GameContent,
    inventory_slot: usize,
    quantity: Quantity,
) -> GameResult<ItemStack> {
    let mut draft = character.clone();
    let result = deposit_inner(&mut draft, content, inventory_slot, quantity)?;
    *character = draft;
    Ok(result)
}

fn deposit_inner(
    character: &mut CharacterState,
    content: &GameContent,
    inventory_slot: usize,
    quantity: Quantity,
) -> GameResult<ItemStack> {
    inventory::validate(&character.inventory, &content.items)?;
    validate(&character.bank, &content.items)?;
    let selected = inventory::stack_at(&character.inventory, inventory_slot)?.clone();
    let (unnoted, _) = forms(&content.items, &selected.item)?;
    if inventory::count(&character.inventory, &content.items, &selected.item)? < quantity.get() {
        return Err(GameError::new(
            GameErrorCode::InsufficientItems,
            "Insufficient inventory items for this deposit.",
        ));
    }
    let deposited = ItemStack {
        item: unnoted.id.clone(),
        quantity,
        instance: selected.instance.clone(),
    };
    let mut inventory = character.inventory.clone();
    let mut bank = character.bank.clone();
    let selected_quantity = quantity.get().min(selected.quantity.get());
    inventory::remove_from_slot(
        &mut inventory,
        &content.items,
        inventory_slot,
        Quantity::new(selected_quantity)?,
    )?;
    let remaining = quantity.get() - selected_quantity;
    if remaining != 0 {
        inventory::remove(
            &mut inventory,
            &content.items,
            &ItemStack {
                item: selected.item,
                quantity: Quantity::new(remaining)?,
                instance: selected.instance.clone(),
            },
        )?;
    }
    add_to_bank(
        &mut bank,
        &deposited,
        character.runtime.ui.as_ref().map(|ui| &ui.bank),
    )?;
    character.inventory = inventory;
    character.bank = bank;
    if let Some(ui) = &mut character.runtime.ui {
        super::bank_layout::reconcile(&character.bank, &mut ui.bank, true)?;
    }
    Ok(deposited)
}

/// Withdraws exactly the requested quantity or changes nothing.
/// `noted = true` requires an explicit note mapping; it never silently changes form.
pub fn withdraw(
    character: &mut CharacterState,
    content: &GameContent,
    bank_slot: usize,
    quantity: Quantity,
    noted: bool,
) -> GameResult<ItemStack> {
    let mut draft = character.clone();
    let result = withdraw_inner(&mut draft, content, bank_slot, quantity, noted)?;
    *character = draft;
    Ok(result)
}

fn withdraw_inner(
    character: &mut CharacterState,
    content: &GameContent,
    bank_slot: usize,
    quantity: Quantity,
    noted: bool,
) -> GameResult<ItemStack> {
    inventory::validate(&character.inventory, &content.items)?;
    validate(&character.bank, &content.items)?;
    let stored = stack_at(&character.bank, bank_slot)?;
    let remaining = stored
        .quantity
        .get()
        .checked_sub(quantity.get())
        .ok_or_else(|| {
            GameError::new(
                GameErrorCode::InsufficientItems,
                "Bank stack has too few items.",
            )
        })?;
    let (unnoted, note) = forms(&content.items, &stored.item)?;
    let withdrawn_item = if noted {
        note.ok_or_else(|| {
            GameError::new(
                GameErrorCode::InvalidInput,
                format!("Item {} has no noted variant.", stored.item),
            )
        })?
    } else {
        unnoted
    };
    let withdrawn = ItemStack {
        item: withdrawn_item.id.clone(),
        quantity,
        instance: stored.instance.clone(),
    };
    let replacement = if remaining == 0 {
        None
    } else {
        Some(ItemStack {
            item: stored.item.clone(),
            quantity: Quantity::new(remaining)?,
            instance: stored.instance.clone(),
        })
    };
    let mut inventory = character.inventory.clone();
    let mut bank = character.bank.clone();
    inventory::add(&mut inventory, &content.items, &withdrawn)?;
    *bank
        .slots
        .get_mut(bank_slot)
        .ok_or_else(|| invalid_slot(bank_slot))? = replacement;
    character.inventory = inventory;
    character.bank = bank;
    if let Some(ui) = &mut character.runtime.ui {
        super::bank_layout::reconcile(&character.bank, &mut ui.bank, true)?;
    }
    Ok(withdrawn)
}

fn stack_at(bank: &Bank, slot: usize) -> GameResult<&ItemStack> {
    if slot >= usize::from(bank.capacity) {
        return Err(invalid_slot(slot));
    }
    bank.slots
        .get(slot)
        .and_then(Option::as_ref)
        .ok_or_else(|| {
            GameError::new(
                GameErrorCode::NotOwned,
                format!("Bank slot {slot} is empty."),
            )
        })
}

fn add_to_bank(
    bank: &mut Bank,
    stack: &ItemStack,
    layout: Option<&clubscape_game_types::BankLayout>,
) -> GameResult<()> {
    if let Some(stored) = bank
        .slots
        .iter_mut()
        .flatten()
        .find(|stored| stored.item == stack.item)
    {
        stored.quantity = add_quantities(stored.quantity, stack.quantity)?;
    } else if let Some(entry) = layout.and_then(|layout| {
        layout
            .entries
            .iter()
            .find(|entry| entry.placeholder && entry.item == stack.item)
    }) {
        let index = usize::from(entry.slot);
        if bank.slots.len() <= index {
            bank.slots.resize(index + 1, None);
        }
        bank.slots[index] = Some(stack.clone());
    } else if let Some(index) = (0..usize::from(bank.capacity)).find(|index| {
        bank.slots.get(*index).is_none_or(Option::is_none)
            && layout.is_none_or(|layout| {
                !layout
                    .entries
                    .iter()
                    .any(|entry| usize::from(entry.slot) == *index && entry.placeholder)
            })
    }) {
        if bank.slots.len() <= index {
            bank.slots.resize(index + 1, None);
        }
        bank.slots[index] = Some(stack.clone());
    } else {
        return Err(GameError::new(
            GameErrorCode::InventoryFull,
            "Bank has reached its explicit slot capacity.",
        ));
    }

    Ok(())
}

pub struct EquipmentDepositPlan {
    pub character: CharacterState,
    pub transferred: Vec<ItemStack>,
}

/// Ordered per-slot plans; failed slots retain their entire owned stack.
pub fn plan_equipment_deposit(
    character: &CharacterState,
    content: &GameContent,
) -> GameResult<EquipmentDepositPlan> {
    if character.equipment.is_empty() {
        return Err(GameError::new(
            GameErrorCode::NotOwned,
            "There are no equipped items to deposit.",
        ));
    }
    super::equipment::validate(&character.equipment, content)?;
    validate(&character.bank, &content.items)?;
    let mut draft = character.clone();
    let mut moved = Vec::new();
    for slot in &content.equipment_slots {
        let Some(stack) = draft.equipment.get(slot).cloned() else {
            continue;
        };
        let mut bank = draft.bank.clone();
        match add_to_bank(
            &mut bank,
            &stack,
            draft.runtime.ui.as_ref().map(|ui| &ui.bank),
        ) {
            Ok(()) => {
                draft.bank = bank;
                draft.equipment.remove(slot);
                if let Some(ui) = &mut draft.runtime.ui {
                    super::bank_layout::reconcile(&draft.bank, &mut ui.bank, true)?;
                }
                moved.push(stack);
            }
            Err(error)
                if matches!(
                    error.code,
                    GameErrorCode::InventoryFull | GameErrorCode::StackOverflow
                ) => {}
            Err(error) => return Err(error),
        }
    }
    if moved.is_empty() && !character.equipment.is_empty() {
        return Err(GameError::new(
            GameErrorCode::InventoryFull,
            "No equipped item fits in the bank.",
        ));
    }
    Ok(EquipmentDepositPlan {
        character: draft,
        transferred: moved,
    })
}

pub fn deposit_equipment(
    character: &mut CharacterState,
    content: &GameContent,
) -> GameResult<Vec<ItemStack>> {
    let plan = plan_equipment_deposit(character, content)?;
    *character = plan.character;
    Ok(plan.transferred)
}

pub fn reconcile_ui(character: &mut CharacterState, changed: bool) -> GameResult<()> {
    if let Some(ui) = &mut character.runtime.ui {
        super::bank_layout::reconcile(&character.bank, &mut ui.bank, changed)?;
    }
    Ok(())
}

fn forms<'a>(
    items: &'a ItemDefinitions,
    item: &ItemId,
) -> GameResult<(&'a ItemDefinition, Option<&'a ItemDefinition>)> {
    let selected = item_definition(items, item)?;
    let base = match &selected.unnoted_variant {
        Some(id) => item_definition(items, id)?,
        None => selected,
    };
    if base.unnoted_variant.is_some() {
        return Err(invalid_mapping(item));
    }
    let note = match &base.noted_variant {
        Some(id) => {
            let note = item_definition(items, id)?;
            if note.id == base.id
                || !note.stackable.fixed()?
                || note.noted_variant.is_some()
                || note.unnoted_variant.as_ref() != Some(&base.id)
            {
                return Err(invalid_mapping(item));
            }
            Some(note)
        }
        None => None,
    };
    if selected.unnoted_variant.is_some() && note.map(|definition| &definition.id) != Some(item) {
        return Err(invalid_mapping(item));
    }
    Ok((base, note))
}

fn invalid_mapping(item: &ItemId) -> GameError {
    GameError::new(
        GameErrorCode::InvalidContent,
        format!("Item {item} needs a reciprocal, noncyclic note mapping with a stackable note."),
    )
}

fn invalid_slot(slot: usize) -> GameError {
    GameError::new(
        GameErrorCode::InvalidInput,
        format!("Invalid bank slot {slot}."),
    )
}
