use clubscape_game_types::{Bank, GameErrorCode::*, Inventory, MAX_STACK_QUANTITY};
use clubscape_simulation::{bank, inventory};

use crate::support::*;

#[test]
fn deposits_aggregate_nonstackable_items_and_consume_selected_slot_first() {
    let content = content();
    let mut character = character(&content);
    for index in [0, 4, 8] {
        put(&mut character.inventory, index, stack("shard", 1));
    }
    let total = normalized_totals(&character, &content);
    assert_eq!(
        bank::deposit(&mut character, &content, 8, quantity(2)).unwrap(),
        stack("shard", 2)
    );
    assert_eq!(character.bank.slots, vec![Some(stack("shard", 2))]);
    assert!(character.inventory.slots.get(8).unwrap().is_none());
    assert!(character.inventory.slots.first().unwrap().is_none());
    assert_eq!(
        inventory::stack_at(&character.inventory, 4).unwrap(),
        &stack("shard", 1)
    );
    assert_eq!(normalized_totals(&character, &content), total);
}

#[test]
fn partial_stack_deposit_and_withdraw_use_real_bank_balances() {
    let content = content();
    let mut character = character(&content);
    put(&mut character.inventory, 9, stack("tokens", 10));
    bank::deposit(&mut character, &content, 9, quantity(6)).unwrap();
    assert_eq!(
        inventory::stack_at(&character.inventory, 9).unwrap(),
        &stack("tokens", 4)
    );
    assert_eq!(
        bank::count(&character.bank, &content.items, &item("tokens")).unwrap(),
        6
    );
    assert_eq!(
        bank::withdraw(&mut character, &content, 0, quantity(2), false).unwrap(),
        stack("tokens", 2)
    );
    assert_eq!(
        inventory::stack_at(&character.inventory, 9).unwrap(),
        &stack("tokens", 6)
    );
    assert_eq!(
        bank::count(&character.bank, &content.items, &item("tokens")).unwrap(),
        4
    );
}

#[test]
fn noted_deposit_unnotes_and_merges_with_existing_unnoted_stack() {
    let content = content();
    let mut character = character(&content);
    character.bank.slots = vec![Some(stack("shard", 5))];
    put(&mut character.inventory, 12, stack("shard_note", 4));
    put(&mut character.inventory, 2, stack("shard", 1));
    let total = normalized_totals(&character, &content);
    assert_eq!(
        bank::deposit(&mut character, &content, 12, quantity(3)).unwrap(),
        stack("shard", 3),
    );
    assert_eq!(character.bank.slots, vec![Some(stack("shard", 8))]);
    assert_eq!(
        inventory::stack_at(&character.inventory, 12).unwrap(),
        &stack("shard_note", 1)
    );
    assert_eq!(
        inventory::stack_at(&character.inventory, 2).unwrap(),
        &stack("shard", 1)
    );
    assert_eq!(
        bank::count(&character.bank, &content.items, &item("shard_note")).unwrap(),
        8
    );
    assert_eq!(normalized_totals(&character, &content), total);
}

#[test]
fn unnoted_withdrawal_splits_into_distinct_stable_inventory_slots() {
    let content = content();
    let mut character = character(&content);
    character.bank.slots = vec![Some(stack("shard", 5))];
    put(&mut character.inventory, 1, stack("tokens", 4));
    let total = normalized_totals(&character, &content);
    bank::withdraw(&mut character, &content, 0, quantity(3), false).unwrap();
    for index in [0, 2, 3] {
        assert_eq!(
            inventory::stack_at(&character.inventory, index).unwrap(),
            &stack("shard", 1)
        );
    }
    assert_eq!(
        inventory::stack_at(&character.inventory, 1).unwrap(),
        &stack("tokens", 4)
    );
    assert_eq!(character.bank.slots, vec![Some(stack("shard", 2))]);
    assert_eq!(normalized_totals(&character, &content), total);
}

#[test]
fn note_withdrawal_uses_one_stack_and_roundtrips_without_loss() {
    let content = content();
    let mut character = character(&content);
    character.bank.slots = vec![Some(stack("shard", MAX_STACK_QUANTITY))];
    let total = normalized_totals(&character, &content);
    assert_eq!(
        bank::withdraw(
            &mut character,
            &content,
            0,
            quantity(MAX_STACK_QUANTITY),
            true
        )
        .unwrap(),
        stack("shard_note", MAX_STACK_QUANTITY),
    );
    assert_eq!(character.inventory.slots.iter().flatten().count(), 1);
    assert_eq!(character.bank.slots, vec![None]);
    bank::deposit(&mut character, &content, 0, quantity(MAX_STACK_QUANTITY)).unwrap();
    assert_eq!(character.inventory, Inventory::default());
    assert_eq!(normalized_totals(&character, &content), total);
}

#[test]
fn full_bank_accepts_existing_items_but_rejects_new_stacks_atomically() {
    let content = content();
    let mut character = character(&content);
    character.bank = Bank {
        capacity: 1,
        slots: vec![Some(stack("shard", 5))],
    };
    put(&mut character.inventory, 4, stack("shard", 1));
    put(&mut character.inventory, 7, stack("tokens", 3));
    bank::deposit(&mut character, &content, 4, quantity(1)).unwrap();
    assert_eq!(character.bank.slots, vec![Some(stack("shard", 6))]);
    let before = character.clone();
    let error = bank::deposit(&mut character, &content, 7, quantity(2)).unwrap_err();
    assert_eq!(error.code, InventoryFull);
    assert!(error.message.contains("Bank"));
    assert_eq!(character, before);
}

#[test]
fn zero_capacity_bank_has_no_hidden_storage() {
    let content = content();
    let mut character = character(&content);
    character.bank.capacity = 0;
    put(&mut character.inventory, 0, stack("tokens", 5));
    let before = character.clone();
    assert_error(
        bank::deposit(&mut character, &content, 0, quantity(1)),
        InventoryFull,
    );
    assert_eq!(character, before);
    assert_error(
        bank::withdraw(&mut character, &content, 0, quantity(1), false),
        InvalidInput,
    );
    assert_eq!(character, before);
}

#[test]
fn bank_stack_overflow_rolls_back_note_consumption() {
    let content = content();
    let mut character = character(&content);
    character.bank.slots = vec![Some(stack("shard", MAX_STACK_QUANTITY))];
    put(&mut character.inventory, 7, stack("shard_note", 2));
    let before = character.clone();
    assert_error(
        bank::deposit(&mut character, &content, 7, quantity(1)),
        StackOverflow,
    );
    assert_eq!(character, before);
}

#[test]
fn withdrawal_does_not_partially_fill_an_inventory_when_request_cannot_fit() {
    let content = content();
    let mut character = character(&content);
    character.inventory = filled("pebble");
    *character.inventory.slots.get_mut(10).unwrap() = None;
    character.bank.slots = vec![Some(stack("shard", 3))];
    let before = character.clone();
    assert_error(
        bank::withdraw(&mut character, &content, 0, quantity(2), false),
        InventoryFull,
    );
    assert_eq!(character, before);
    bank::withdraw(&mut character, &content, 0, quantity(1), false).unwrap();
    assert_eq!(
        inventory::stack_at(&character.inventory, 10).unwrap(),
        &stack("shard", 1)
    );
    let before = character.clone();
    assert_error(
        bank::withdraw(&mut character, &content, 0, quantity(1), true),
        InventoryFull,
    );
    assert_eq!(character, before);
}

#[test]
fn huge_nonstackable_withdrawal_fails_bounded_without_debiting_bank() {
    let content = content();
    let mut character = character(&content);
    character.bank.slots = vec![Some(stack("shard", MAX_STACK_QUANTITY))];
    let before = character.clone();
    assert_error(
        bank::withdraw(
            &mut character,
            &content,
            0,
            quantity(MAX_STACK_QUANTITY),
            false,
        ),
        InventoryFull,
    );
    assert_eq!(character, before);
}

#[test]
fn withdrawal_stack_overflow_preserves_bank_and_inventory() {
    let content = content();
    let mut character = character(&content);
    character.bank.slots = vec![Some(stack("shard", 2))];
    put(
        &mut character.inventory,
        7,
        stack("shard_note", MAX_STACK_QUANTITY),
    );
    let before = character.clone();
    assert_error(
        bank::withdraw(&mut character, &content, 0, quantity(1), true),
        StackOverflow,
    );
    assert_eq!(character, before);
}

#[test]
fn deposit_and_withdraw_insufficient_quantities_do_not_grant_or_consume_partially() {
    let content = content();
    let mut character = character(&content);
    put(&mut character.inventory, 4, stack("shard", 1));
    put(&mut character.inventory, 8, stack("shard_note", 9));
    character.bank.slots = vec![Some(stack("tokens", 2))];
    let before = character.clone();
    assert_error(
        bank::deposit(&mut character, &content, 4, quantity(2)),
        InsufficientItems,
    );
    assert_eq!(character, before);
    assert_error(
        bank::deposit(&mut character, &content, 8, quantity(10)),
        InsufficientItems,
    );
    assert_eq!(character, before);
    assert_error(
        bank::withdraw(&mut character, &content, 0, quantity(3), false),
        InsufficientItems,
    );
    assert_eq!(character, before);
}

#[test]
fn bank_slots_are_stable_and_holes_are_reused_without_compaction() {
    let content = content();
    let mut character = character(&content);
    character.bank = Bank {
        capacity: 4,
        slots: vec![
            None,
            Some(stack("tokens", 2)),
            None,
            Some(stack("shard", 3)),
        ],
    };
    bank::withdraw(&mut character, &content, 1, quantity(2), false).unwrap();
    assert_eq!(
        character.bank.slots,
        vec![None, None, None, Some(stack("shard", 3))]
    );
    bank::deposit(&mut character, &content, 0, quantity(2)).unwrap();
    assert_eq!(
        character.bank.slots,
        vec![
            Some(stack("tokens", 2)),
            None,
            None,
            Some(stack("shard", 3))
        ],
    );
}

#[test]
fn bank_short_prefix_can_grow_only_up_to_explicit_capacity() {
    let content = content();
    let mut character = character(&content);
    character.bank.capacity = 2;
    put(&mut character.inventory, 0, stack("tokens", 1));
    put(&mut character.inventory, 1, stack("shard", 1));
    put(&mut character.inventory, 2, stack("pebble", 1));
    bank::deposit(&mut character, &content, 0, quantity(1)).unwrap();
    bank::deposit(&mut character, &content, 1, quantity(1)).unwrap();
    assert_eq!(character.bank.slots.len(), 2);
    let before = character.clone();
    assert_error(
        bank::deposit(&mut character, &content, 2, quantity(1)),
        InventoryFull,
    );
    assert_eq!(character, before);
}

#[test]
fn invalid_empty_or_unallocated_bank_and_inventory_slots_are_typed_errors() {
    let content = content();
    let mut character = character(&content);
    character.bank.slots = vec![None];
    let before = character.clone();
    for (slot, code) in [
        (0, NotOwned),
        (7, NotOwned),
        (8, InvalidInput),
        (usize::MAX, InvalidInput),
    ] {
        assert_error(
            bank::withdraw(&mut character, &content, slot, quantity(1), false),
            code,
        );
        assert_eq!(character, before);
    }
    assert_error(
        bank::deposit(&mut character, &content, 0, quantity(1)),
        NotOwned,
    );
    assert_error(
        bank::deposit(&mut character, &content, 28, quantity(1)),
        InvalidInput,
    );
    assert_eq!(character, before);
}

#[test]
fn note_request_without_mapping_is_explicitly_rejected() {
    let content = content();
    let mut character = character(&content);
    character.bank.slots = vec![Some(stack("pebble", 2)), Some(stack("tokens", 3))];
    let before = character.clone();
    for index in [0, 1] {
        assert_error(
            bank::withdraw(&mut character, &content, index, quantity(1), true),
            InvalidInput,
        );
        assert_eq!(character, before);
    }
}

#[test]
fn invalid_bank_state_is_not_silently_repaired_or_used_for_withdrawals() {
    let content = content();
    for (bank, code) in [
        (
            Bank {
                capacity: 0,
                slots: vec![None],
            },
            InvalidInput,
        ),
        (
            Bank {
                capacity: 2,
                slots: vec![Some(stack("shard", 1)), Some(stack("shard", 2))],
            },
            InvalidInput,
        ),
        (
            Bank {
                capacity: 1,
                slots: vec![Some(stack("shard_note", 1))],
            },
            InvalidInput,
        ),
        (
            Bank {
                capacity: 1,
                slots: vec![Some(stack("unknown", 1))],
            },
            UnknownContent,
        ),
    ] {
        let mut character = character(&content);
        character.bank = bank;
        let before = character.clone();
        assert_error(
            bank::withdraw(&mut character, &content, 0, quantity(1), false),
            code,
        );
        assert_eq!(character, before);
    }
}

#[test]
fn broken_missing_nonstackable_or_cyclic_note_pairs_are_rejected_atomically() {
    for case in 0..6 {
        let mut content = content();
        let mut character = character(&content);
        put(&mut character.inventory, 0, stack("shard_note", 1));
        match case {
            0 => content.items.get_mut(&item("shard")).unwrap().noted_variant = None,
            1 => {
                content
                    .items
                    .get_mut(&item("shard_note"))
                    .unwrap()
                    .stackable = false
            }
            2 => {
                content
                    .items
                    .get_mut(&item("shard_note"))
                    .unwrap()
                    .unnoted_variant = Some(item("unknown"))
            }
            3 => {
                content
                    .items
                    .get_mut(&item("shard_note"))
                    .unwrap()
                    .unnoted_variant = Some(item("shard_note"))
            }
            4 => {
                content.items.get_mut(&item("shard")).unwrap().noted_variant = Some(item("tokens"))
            }
            5 => {
                content
                    .items
                    .get_mut(&item("shard_note"))
                    .unwrap()
                    .noted_variant = Some(item("shard"))
            }
            _ => unreachable!(),
        }
        let before = character.clone();
        assert_error(
            bank::deposit(&mut character, &content, 0, quantity(1)),
            if case == 2 {
                UnknownContent
            } else {
                InvalidContent
            },
        );
        assert_eq!(character, before);
    }
}

#[test]
fn unknown_ids_and_invalid_inventory_cannot_be_banked() {
    let content = content();
    let mut character = character(&content);
    assert_error(
        bank::count(&character.bank, &content.items, &item("unknown")),
        UnknownContent,
    );
    put(&mut character.inventory, 0, stack("unknown", 1));
    let before = character.clone();
    assert_error(
        bank::deposit(&mut character, &content, 0, quantity(1)),
        UnknownContent,
    );
    assert_eq!(character, before);
    put(&mut character.inventory, 0, stack("shard", 2));
    let before = character.clone();
    assert_error(
        bank::deposit(&mut character, &content, 0, quantity(1)),
        InvalidInput,
    );
    assert_eq!(character, before);
}

#[test]
fn repeated_note_and_item_roundtrips_preserve_all_progress_and_owned_value() {
    let content = content();
    let mut character = character(&content);
    character.bank.slots = vec![Some(stack("shard", 28))];
    character
        .equipment
        .insert(slot("weapon"), stack("blade", 1));
    let before = character.clone();
    let total = normalized_totals(&character, &content);
    for _ in 0..16 {
        bank::withdraw(&mut character, &content, 0, quantity(28), false).unwrap();
        assert_eq!(normalized_totals(&character, &content), total);
        bank::deposit(&mut character, &content, 17, quantity(28)).unwrap();
        assert_eq!(normalized_totals(&character, &content), total);
        bank::withdraw(&mut character, &content, 0, quantity(28), true).unwrap();
        bank::deposit(&mut character, &content, 0, quantity(28)).unwrap();
        assert_eq!(character, before);
    }
}
