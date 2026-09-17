use clubscape_game_types::{GameErrorCode::*, Inventory, MAX_STACK_QUANTITY, Quantity};
use clubscape_simulation::inventory::{self as inv, InventoryOperation::*};

use crate::support::*;

#[test]
fn add_respects_stackability_first_free_slots_and_existing_positions() {
    let content = content();
    let mut inventory = Inventory::default();
    put(&mut inventory, 8, stack("tokens", 2));
    put(&mut inventory, 1, stack("pebble", 1));
    inv::add(&mut inventory, &content.items, &stack("tokens", 3)).unwrap();
    inv::add(&mut inventory, &content.items, &stack("shard", 3)).unwrap();
    assert_eq!(inv::stack_at(&inventory, 8).unwrap(), &stack("tokens", 5));
    assert_eq!(inv::stack_at(&inventory, 1).unwrap(), &stack("pebble", 1));
    for index in [0, 2, 3] {
        assert_eq!(
            inv::stack_at(&inventory, index).unwrap(),
            &stack("shard", 1)
        );
    }
    assert_eq!(
        inv::count(&inventory, &content.items, &item("shard")).unwrap(),
        3
    );
    assert_eq!(
        inv::count(&inventory, &content.items, &item("blade")).unwrap(),
        0
    );
}

#[test]
fn full_inventory_can_grow_existing_stack_but_cannot_add_new_items() {
    let content = content();
    let mut inventory = filled("pebble");
    put(&mut inventory, 7, stack("tokens", 1));
    inv::add(&mut inventory, &content.items, &stack("tokens", 8)).unwrap();
    let before = inventory.clone();
    for requested in [stack("pebble", 1), stack("shard_note", 1)] {
        assert_error(
            inv::add(&mut inventory, &content.items, &requested),
            InventoryFull,
        );
        assert_eq!(inventory, before);
    }
    assert_eq!(inv::stack_at(&inventory, 7).unwrap(), &stack("tokens", 9));
}

#[test]
fn stack_limit_is_exact_and_does_not_spill_into_another_slot() {
    let content = content();
    let mut inventory = Inventory::default();
    inv::add(
        &mut inventory,
        &content.items,
        &stack("tokens", MAX_STACK_QUANTITY),
    )
    .unwrap();
    let before = inventory.clone();
    assert_error(
        inv::add(&mut inventory, &content.items, &stack("tokens", 1)),
        StackOverflow,
    );
    assert_eq!(inventory, before);
    assert_eq!(inventory.slots.iter().flatten().count(), 1);
    assert_error(Quantity::new(0), InvalidInput);
    assert_error(Quantity::new(MAX_STACK_QUANTITY + 1), InvalidInput);
}

#[test]
fn huge_nonstackable_add_is_bounded_and_atomic() {
    let content = content();
    let mut inventory = Inventory::default();
    assert_error(
        inv::add(
            &mut inventory,
            &content.items,
            &stack("shard", MAX_STACK_QUANTITY),
        ),
        InventoryFull,
    );
    assert_eq!(inventory, Inventory::default());
}

#[test]
fn unknown_item_ids_and_mismatched_definition_keys_are_rejected() {
    let mut content = content();
    let mut inventory = Inventory::default();
    assert_error(
        inv::count(&inventory, &content.items, &item("unknown")),
        UnknownContent,
    );
    assert_error(
        inv::add(&mut inventory, &content.items, &stack("unknown", 1)),
        UnknownContent,
    );
    assert_error(
        inv::remove(&mut inventory, &content.items, &stack("unknown", 1)),
        UnknownContent,
    );
    content.items.get_mut(&item("tokens")).unwrap().id = item("different");
    assert_error(
        inv::add(&mut inventory, &content.items, &stack("tokens", 1)),
        InvalidContent,
    );
    assert_eq!(inventory, Inventory::default());
}

#[test]
fn invalid_persisted_inventory_is_not_silently_normalized() {
    let content = content();
    for stacks in [
        vec![stack("shard", 2)],
        vec![stack("tokens", 1), stack("tokens", 2)],
        vec![stack("unknown", 1)],
    ] {
        let mut inventory = Inventory::default();
        for (index, stack) in stacks.into_iter().enumerate() {
            put(&mut inventory, index, stack);
        }
        let before = inventory.clone();
        assert!(inv::add(&mut inventory, &content.items, &stack("pebble", 1)).is_err());
        assert_eq!(inventory, before);
    }
}

#[test]
fn removal_uses_ascending_slots_without_compacting_untouched_items() {
    let content = content();
    let mut inventory = Inventory::default();
    for index in [2, 6, 20] {
        put(&mut inventory, index, stack("shard", 1));
    }
    put(&mut inventory, 11, stack("tokens", 8));
    inv::remove(&mut inventory, &content.items, &stack("shard", 2)).unwrap();
    inv::remove(&mut inventory, &content.items, &stack("tokens", 3)).unwrap();
    assert!(inventory.slots.get(2).unwrap().is_none());
    assert!(inventory.slots.get(6).unwrap().is_none());
    assert_eq!(inv::stack_at(&inventory, 20).unwrap(), &stack("shard", 1));
    assert_eq!(inv::stack_at(&inventory, 11).unwrap(), &stack("tokens", 5));
    inv::remove(&mut inventory, &content.items, &stack("tokens", 5)).unwrap();
    assert!(inventory.slots.get(11).unwrap().is_none());
}

#[test]
fn insufficient_removal_and_repeated_batch_consumption_roll_back() {
    let content = content();
    let mut inventory = Inventory::default();
    put(&mut inventory, 10, stack("tokens", 5));
    let before = inventory.clone();
    assert_error(
        inv::remove(&mut inventory, &content.items, &stack("tokens", 6)),
        InsufficientItems,
    );
    assert_eq!(inventory, before);
    assert_error(
        inv::remove_batch(
            &mut inventory,
            &content.items,
            &[stack("tokens", 3), stack("tokens", 3)],
        ),
        InsufficientItems,
    );
    assert_eq!(inventory, before);
}

#[test]
fn add_batch_aggregates_repeated_items_and_rolls_back_later_failures() {
    let content = content();
    let mut inventory = Inventory::default();
    inv::add_batch(
        &mut inventory,
        &content.items,
        &[stack("tokens", 2), stack("tokens", 3)],
    )
    .unwrap();
    assert_eq!(
        inv::count(&inventory, &content.items, &item("tokens")).unwrap(),
        5
    );
    let before = inventory.clone();
    assert_error(
        inv::add_batch(
            &mut inventory,
            &content.items,
            &[stack("tokens", 1), stack("shard", 28)],
        ),
        InventoryFull,
    );
    assert_eq!(inventory, before);
    assert_error(
        inv::add_batch(
            &mut inventory,
            &content.items,
            &[stack("tokens", 1), stack("unknown", 1)],
        ),
        UnknownContent,
    );
    assert_eq!(inventory, before);
    assert_error(
        inv::add_batch(
            &mut inventory,
            &content.items,
            &[stack("pebble", 1), stack("tokens", MAX_STACK_QUANTITY)],
        ),
        StackOverflow,
    );
    assert_eq!(inventory, before);
}

#[test]
fn mixed_operations_are_ordered_and_atomic_for_production() {
    let content = content();
    let mut inventory = filled("pebble");
    inv::apply_operations(
        &mut inventory,
        &content.items,
        &[Remove(stack("pebble", 1)), Add(stack("shard", 1))],
    )
    .unwrap();
    assert_eq!(inv::stack_at(&inventory, 0).unwrap(), &stack("shard", 1));
    let before = inventory.clone();
    assert_error(
        inv::apply_operations(
            &mut inventory,
            &content.items,
            &[
                Remove(stack("shard", 1)),
                Add(stack("tokens", 1)),
                Add(stack("shard", 1)),
            ],
        ),
        InventoryFull,
    );
    assert_eq!(inventory, before);
    assert_error(
        inv::apply_operations(
            &mut inventory,
            &content.items,
            &[Add(stack("tokens", 1)), Remove(stack("pebble", 1))],
        ),
        InventoryFull,
    );
    assert_eq!(inventory, before);
}

#[test]
fn slot_removal_is_exact_and_rejects_empty_invalid_or_insufficient_slots() {
    let content = content();
    let mut inventory = Inventory::default();
    put(&mut inventory, 4, stack("tokens", 5));
    put(&mut inventory, 5, stack("shard", 1));
    put(&mut inventory, 6, stack("shard", 1));
    assert_eq!(
        inv::remove_from_slot(&mut inventory, &content.items, 4, quantity(2)).unwrap(),
        stack("tokens", 2),
    );
    let before = inventory.clone();
    for (slot, amount, code) in [
        (28, 1, InvalidInput),
        (usize::MAX, 1, InvalidInput),
        (0, 1, NotOwned),
        (5, 2, InsufficientItems),
        (4, 4, InsufficientItems),
    ] {
        assert_error(
            inv::remove_from_slot(&mut inventory, &content.items, slot, quantity(amount)),
            code,
        );
        assert_eq!(inventory, before);
    }
}

#[test]
fn transfers_preserve_totals_and_roll_back_both_inventories() {
    let content = content();
    let mut source = Inventory::default();
    let mut destination = Inventory::default();
    put(&mut source, 5, stack("tokens", 12));
    let before_total = totals(
        source
            .slots
            .iter()
            .flatten()
            .chain(destination.slots.iter().flatten()),
    );
    inv::transfer(
        &mut source,
        &mut destination,
        &content.items,
        &stack("tokens", 5),
    )
    .unwrap();
    assert_eq!(
        totals(
            source
                .slots
                .iter()
                .flatten()
                .chain(destination.slots.iter().flatten())
        ),
        before_total,
    );
    let before = (source.clone(), destination.clone());
    assert_error(
        inv::transfer(
            &mut source,
            &mut destination,
            &content.items,
            &stack("tokens", 8),
        ),
        InsufficientItems,
    );
    assert_eq!((source.clone(), destination.clone()), before);
    destination = filled("pebble");
    let before = (source.clone(), destination.clone());
    assert_error(
        inv::transfer(
            &mut source,
            &mut destination,
            &content.items,
            &stack("tokens", 1),
        ),
        InventoryFull,
    );
    assert_eq!((source.clone(), destination.clone()), before);
    put(&mut destination, 0, stack("tokens", MAX_STACK_QUANTITY));
    let before = (source.clone(), destination.clone());
    assert_error(
        inv::transfer(
            &mut source,
            &mut destination,
            &content.items,
            &stack("tokens", 1),
        ),
        StackOverflow,
    );
    assert_eq!((source, destination), before);
}

#[test]
fn swap_handles_empty_same_and_invalid_slots_without_loss() {
    let content = content();
    let mut inventory = Inventory::default();
    put(&mut inventory, 0, stack("tokens", 9));
    put(&mut inventory, 1, stack("shard", 1));
    inv::swap(&mut inventory, &content.items, 0, 1).unwrap();
    inv::swap(&mut inventory, &content.items, 1, 27).unwrap();
    assert_eq!(inv::stack_at(&inventory, 27).unwrap(), &stack("tokens", 9));
    assert!(inventory.slots.get(1).unwrap().is_none());
    let before = inventory.clone();
    inv::swap(&mut inventory, &content.items, 27, 27).unwrap();
    inv::swap(&mut inventory, &content.items, 2, 2).unwrap();
    assert_eq!(inventory, before);
    for (from, to) in [(28, 0), (0, 28), (usize::MAX, usize::MAX)] {
        assert_error(
            inv::swap(&mut inventory, &content.items, from, to),
            InvalidInput,
        );
        assert_eq!(inventory, before);
    }
}

#[test]
fn every_slot_pair_swap_is_reversible_and_count_preserving() {
    let content = content();
    let mut inventory = filled("pebble");
    put(&mut inventory, 9, stack("tokens", MAX_STACK_QUANTITY));
    *inventory.slots.get_mut(20).unwrap() = None;
    let before = inventory.clone();
    let counts = totals(before.slots.iter().flatten());
    for from in 0..28 {
        for to in 0..28 {
            inv::swap(&mut inventory, &content.items, from, to).unwrap();
            assert_eq!(totals(inventory.slots.iter().flatten()), counts);
            inv::swap(&mut inventory, &content.items, from, to).unwrap();
            assert_eq!(inventory, before);
        }
    }
}

#[test]
fn successful_repeated_removals_and_empty_batches_do_not_duplicate_items() {
    let content = content();
    let mut inventory = Inventory::default();
    put(&mut inventory, 0, stack("tokens", 6));
    inv::remove_batch(
        &mut inventory,
        &content.items,
        &[stack("tokens", 3), stack("tokens", 3)],
    )
    .unwrap();
    assert_eq!(inventory, Inventory::default());
    inv::add_batch(&mut inventory, &content.items, &[]).unwrap();
    inv::remove_batch(&mut inventory, &content.items, &[]).unwrap();
    assert_eq!(inventory, Inventory::default());
}
