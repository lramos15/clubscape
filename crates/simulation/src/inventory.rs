use std::collections::BTreeSet;

use clubscape_game_types::{
    GameError, GameErrorCode, GameResult, Inventory, ItemId, ItemStack, Quantity,
};

use crate::{ItemDefinitions, add_quantities, item_definition};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InventoryOperation {
    Add(ItemStack),
    Remove(ItemStack),
}

/// Rejects unknown items, nonstackable stacks, and split stackable stacks.
pub fn validate(inventory: &Inventory, items: &ItemDefinitions) -> GameResult<()> {
    let mut stackable_items = BTreeSet::new();
    for stack in inventory.slots.iter().flatten() {
        let definition = item_definition(items, &stack.item)?;
        if definition.stackable {
            if !stackable_items.insert(&stack.item) {
                return Err(GameError::new(
                    GameErrorCode::InvalidInput,
                    format!("Inventory contains multiple stacks of {}.", stack.item),
                ));
            }
        } else if stack.quantity.get() != 1 {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                format!(
                    "Nonstackable item {} must occupy separate slots.",
                    stack.item
                ),
            ));
        }
    }
    Ok(())
}

pub fn count(inventory: &Inventory, items: &ItemDefinitions, item: &ItemId) -> GameResult<u32> {
    item_definition(items, item)?;
    validate(inventory, items)?;
    count_validated(inventory, item)
}

pub fn stack_at(inventory: &Inventory, slot: usize) -> GameResult<&ItemStack> {
    inventory
        .slots
        .get(slot)
        .ok_or_else(|| invalid_slot(slot))?
        .as_ref()
        .ok_or_else(|| {
            GameError::new(
                GameErrorCode::NotOwned,
                format!("Inventory slot {slot} is empty."),
            )
        })
}

/// Applies operations in order, committing only if every operation succeeds.
pub fn apply_operations(
    inventory: &mut Inventory,
    items: &ItemDefinitions,
    operations: &[InventoryOperation],
) -> GameResult<()> {
    validate(inventory, items)?;
    let mut draft = inventory.clone();
    for operation in operations {
        match operation {
            InventoryOperation::Add(stack) => add_to_draft(&mut draft, items, stack)?,
            InventoryOperation::Remove(stack) => remove_from_draft(&mut draft, items, stack)?,
        }
    }
    *inventory = draft;
    Ok(())
}

pub fn add(
    inventory: &mut Inventory,
    items: &ItemDefinitions,
    stack: &ItemStack,
) -> GameResult<()> {
    apply_operations(inventory, items, &[InventoryOperation::Add(stack.clone())])
}

pub fn remove(
    inventory: &mut Inventory,
    items: &ItemDefinitions,
    stack: &ItemStack,
) -> GameResult<()> {
    apply_operations(
        inventory,
        items,
        &[InventoryOperation::Remove(stack.clone())],
    )
}

pub fn add_batch(
    inventory: &mut Inventory,
    items: &ItemDefinitions,
    stacks: &[ItemStack],
) -> GameResult<()> {
    let operations: Vec<_> = stacks
        .iter()
        .cloned()
        .map(InventoryOperation::Add)
        .collect();
    apply_operations(inventory, items, &operations)
}

pub fn remove_batch(
    inventory: &mut Inventory,
    items: &ItemDefinitions,
    stacks: &[ItemStack],
) -> GameResult<()> {
    let operations: Vec<_> = stacks
        .iter()
        .cloned()
        .map(InventoryOperation::Remove)
        .collect();
    apply_operations(inventory, items, &operations)
}

/// Removes only from this slot, rather than selecting other copies of its item.
pub fn remove_from_slot(
    inventory: &mut Inventory,
    items: &ItemDefinitions,
    slot: usize,
    quantity: Quantity,
) -> GameResult<ItemStack> {
    validate(inventory, items)?;
    let stored = stack_at(inventory, slot)?;
    let remaining = stored
        .quantity
        .get()
        .checked_sub(quantity.get())
        .ok_or_else(|| {
            GameError::new(
                GameErrorCode::InsufficientItems,
                "Inventory slot has too few items.",
            )
        })?;
    let removed = ItemStack {
        item: stored.item.clone(),
        quantity,
    };
    let replacement = if remaining == 0 {
        None
    } else {
        Some(ItemStack {
            item: stored.item.clone(),
            quantity: Quantity::new(remaining)?,
        })
    };
    *inventory
        .slots
        .get_mut(slot)
        .ok_or_else(|| invalid_slot(slot))? = replacement;
    Ok(removed)
}

/// Transfers between distinct inventories atomically, including capacity checks.
pub fn transfer(
    source: &mut Inventory,
    destination: &mut Inventory,
    items: &ItemDefinitions,
    stack: &ItemStack,
) -> GameResult<()> {
    validate(source, items)?;
    validate(destination, items)?;
    let mut source_draft = source.clone();
    let mut destination_draft = destination.clone();
    remove_from_draft(&mut source_draft, items, stack)?;
    add_to_draft(&mut destination_draft, items, stack)?;
    *source = source_draft;
    *destination = destination_draft;
    Ok(())
}

/// Swaps occupied or empty slots without compacting, merging, or losing items.
pub fn swap(
    inventory: &mut Inventory,
    items: &ItemDefinitions,
    from: usize,
    to: usize,
) -> GameResult<()> {
    validate(inventory, items)?;
    let from_stack = inventory
        .slots
        .get(from)
        .ok_or_else(|| invalid_slot(from))?
        .clone();
    let to_stack = inventory
        .slots
        .get(to)
        .ok_or_else(|| invalid_slot(to))?
        .clone();
    let mut draft = inventory.clone();
    *draft
        .slots
        .get_mut(from)
        .ok_or_else(|| invalid_slot(from))? = to_stack;
    *draft.slots.get_mut(to).ok_or_else(|| invalid_slot(to))? = from_stack;
    *inventory = draft;
    Ok(())
}

fn count_validated(inventory: &Inventory, item: &ItemId) -> GameResult<u32> {
    inventory
        .slots
        .iter()
        .flatten()
        .filter(|stack| stack.item == *item)
        .try_fold(0_u32, |total, stack| {
            total.checked_add(stack.quantity.get()).ok_or_else(|| {
                GameError::new(
                    GameErrorCode::StackOverflow,
                    "Inventory item count overflow.",
                )
            })
        })
}

fn add_to_draft(
    inventory: &mut Inventory,
    items: &ItemDefinitions,
    stack: &ItemStack,
) -> GameResult<()> {
    let definition = item_definition(items, &stack.item)?;
    if definition.stackable {
        if let Some(stored) = inventory
            .slots
            .iter_mut()
            .flatten()
            .find(|stored| stored.item == stack.item)
        {
            stored.quantity = add_quantities(stored.quantity, stack.quantity)?;
            return Ok(());
        }
        let slot = inventory
            .slots
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or_else(full)?;
        *slot = Some(stack.clone());
    } else {
        let needed = usize::try_from(stack.quantity.get()).map_err(|_| full())?;
        if needed > inventory.slots.iter().filter(|slot| slot.is_none()).count() {
            return Err(full());
        }
        let single = ItemStack {
            item: stack.item.clone(),
            quantity: Quantity::new(1)?,
        };
        for slot in inventory
            .slots
            .iter_mut()
            .filter(|slot| slot.is_none())
            .take(needed)
        {
            *slot = Some(single.clone());
        }
    }
    Ok(())
}

fn remove_from_draft(
    inventory: &mut Inventory,
    items: &ItemDefinitions,
    stack: &ItemStack,
) -> GameResult<()> {
    item_definition(items, &stack.item)?;
    if count_validated(inventory, &stack.item)? < stack.quantity.get() {
        return Err(GameError::new(
            GameErrorCode::InsufficientItems,
            format!("Insufficient inventory items: {}.", stack.item),
        ));
    }
    let mut remaining = stack.quantity.get();
    for slot in &mut inventory.slots {
        if remaining == 0 {
            break;
        }
        if let Some(stored) = slot.as_mut().filter(|stored| stored.item == stack.item) {
            let taken = remaining.min(stored.quantity.get());
            let left = stored.quantity.get() - taken;
            remaining -= taken;
            if left == 0 {
                *slot = None;
            } else {
                stored.quantity = Quantity::new(left)?;
            }
        }
    }
    Ok(())
}

fn invalid_slot(slot: usize) -> GameError {
    GameError::new(
        GameErrorCode::InvalidInput,
        format!("Invalid inventory slot {slot}."),
    )
}

fn full() -> GameError {
    GameError::new(
        GameErrorCode::InventoryFull,
        "Inventory has insufficient free slots.",
    )
}
