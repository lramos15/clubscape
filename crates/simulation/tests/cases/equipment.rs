use clubscape_game_types::{GameErrorCode::*, GameEvent, MAX_STACK_QUANTITY, SkillRequirement};
use clubscape_simulation::{equipment as gear, inventory, skills::LevelBasis};

use crate::support::*;

#[test]
fn equip_and_unequip_require_owned_items_and_preserve_other_character_state() {
    let content = content();
    let mut character = character(&content);
    put(&mut character.inventory, 5, stack("blade", 1));
    let before = character.clone();
    let total = normalized_totals(&character, &content);
    assert_eq!(
        gear::equip(&mut character, &content, 5).unwrap(),
        GameEvent::Equipped {
            slot: slot("weapon"),
            stack: stack("blade", 1)
        },
    );
    assert!(character.inventory.slots.get(5).unwrap().is_none());
    assert_eq!(
        character.equipment.get(&slot("weapon")).unwrap(),
        &stack("blade", 1)
    );
    assert_eq!(normalized_totals(&character, &content), total);
    let equipped = character.clone();
    assert_error(gear::equip(&mut character, &content, 5), NotOwned);
    assert_eq!(character, equipped);
    assert_eq!(
        gear::unequip(&mut character, &content, &slot("weapon")).unwrap(),
        stack("blade", 1)
    );
    let mut expected = before;
    expected.inventory = character.inventory.clone();
    assert_eq!(character, expected);
    assert_eq!(normalized_totals(&character, &content), total);
}

#[test]
fn every_data_defined_functional_slot_is_preserved_without_cosmetic_layers() {
    let mut content = content();
    let mut character = character(&content);
    let slots = content.equipment_slots.clone();
    for (index, primary) in slots.iter().enumerate() {
        let name = format!("garment_{index}");
        let mut definition = item_definition(&name, false);
        let mut equipment = equipment_definition("head", &[]);
        equipment.slot = primary.clone();
        definition.equipment = Some(equipment);
        content.items.insert(definition.id.clone(), definition);
        put(&mut character.inventory, 0, stack(&name, 1));
        gear::equip(&mut character, &content, 0).unwrap();
    }
    assert_eq!(character.equipment.len(), slots.len());
    for (index, primary) in slots.iter().enumerate() {
        let name = format!("garment_{index}");
        assert_eq!(character.equipment.get(primary).unwrap(), &stack(&name, 1));
        assert_eq!(
            gear::occupant(&character.equipment, &content, primary)
                .unwrap()
                .unwrap()
                .0,
            primary,
        );
    }
}

#[test]
fn two_handed_equipment_displaces_each_conflicting_owner_exactly_once() {
    let content = content();
    let mut character = character(&content);
    character
        .equipment
        .insert(slot("weapon"), stack("blade", 1));
    character
        .equipment
        .insert(slot("offhand"), stack("shield", 1));
    character.equipment.insert(slot("ammo"), stack("ammo", 9));
    put(&mut character.inventory, 27, stack("staff", 1));
    let total = normalized_totals(&character, &content);
    gear::equip(&mut character, &content, 27).unwrap();
    assert_eq!(character.equipment.len(), 2);
    assert_eq!(
        character.equipment.get(&slot("weapon")).unwrap(),
        &stack("staff", 1)
    );
    assert!(!character.equipment.contains_key(&slot("offhand")));
    assert_eq!(
        inventory::count(&character.inventory, &content.items, &item("blade")).unwrap(),
        1
    );
    assert_eq!(
        inventory::count(&character.inventory, &content.items, &item("shield")).unwrap(),
        1
    );
    let owner = gear::occupant(&character.equipment, &content, &slot("offhand"))
        .unwrap()
        .unwrap();
    assert_eq!(owner.0, &slot("weapon"));
    assert_eq!(owner.1, &stack("staff", 1));
    assert_eq!(normalized_totals(&character, &content), total);
    gear::unequip(&mut character, &content, &slot("offhand")).unwrap();
    assert!(!character.equipment.contains_key(&slot("weapon")));
    assert_eq!(normalized_totals(&character, &content), total);
}

#[test]
fn equipping_into_a_secondary_occupied_slot_displaces_the_primary_owner() {
    let content = content();
    let mut character = character(&content);
    character
        .equipment
        .insert(slot("weapon"), stack("staff", 1));
    put(&mut character.inventory, 5, stack("shield", 1));
    let total = normalized_totals(&character, &content);
    gear::equip(&mut character, &content, 5).unwrap();
    assert_eq!(character.equipment.len(), 1);
    assert_eq!(
        character.equipment.get(&slot("offhand")).unwrap(),
        &stack("shield", 1)
    );
    assert_eq!(
        inventory::count(&character.inventory, &content.items, &item("staff")).unwrap(),
        1
    );
    assert_eq!(normalized_totals(&character, &content), total);
}

#[test]
fn one_displacement_fits_the_vacated_slot_in_a_full_inventory() {
    let content = content();
    let mut character = character(&content);
    character.inventory = filled("pebble");
    put(&mut character.inventory, 19, stack("blade", 1));
    character
        .equipment
        .insert(slot("weapon"), stack("staff", 1));
    let total = normalized_totals(&character, &content);
    gear::equip(&mut character, &content, 19).unwrap();
    assert_eq!(
        inventory::stack_at(&character.inventory, 19).unwrap(),
        &stack("staff", 1)
    );
    assert_eq!(normalized_totals(&character, &content), total);
}

#[test]
fn insufficient_space_for_two_displacements_rolls_back_everything() {
    let content = content();
    let mut character = character(&content);
    character.inventory = filled("pebble");
    put(&mut character.inventory, 19, stack("staff", 1));
    character
        .equipment
        .insert(slot("weapon"), stack("blade", 1));
    character
        .equipment
        .insert(slot("offhand"), stack("shield", 1));
    let before = character.clone();
    assert_error(gear::equip(&mut character, &content, 19), InventoryFull);
    assert_eq!(character, before);
}

#[test]
fn requirements_use_base_levels_by_default_not_temporary_boosts() {
    let content = content();
    let mut character = character(&content);
    put(&mut character.inventory, 0, stack("blade", 1));
    let state = character.skills.get_mut(&skill("practice")).unwrap();
    state.xp_tenths = 0;
    state.current_level = 5;
    let before = character.clone();
    assert_error(gear::equip(&mut character, &content, 0), RequirementNotMet);
    assert_eq!(character, before);
    gear::equip_with_level_basis(&mut character, &content, 0, LevelBasis::Current).unwrap();
    assert_eq!(
        character.equipment.get(&slot("weapon")).unwrap(),
        &stack("blade", 1)
    );
}

#[test]
fn drained_levels_are_not_silently_restored_by_equipping() {
    let content = content();
    let mut character = character(&content);
    put(&mut character.inventory, 0, stack("blade", 1));
    character
        .skills
        .get_mut(&skill("practice"))
        .unwrap()
        .current_level = 0;
    let before = character.clone();
    assert_error(
        gear::equip_with_level_basis(&mut character, &content, 0, LevelBasis::Current),
        RequirementNotMet,
    );
    assert_eq!(character, before);
    gear::equip(&mut character, &content, 0).unwrap();
    assert_eq!(character.skills, before.skills);
    assert_eq!(character.hitpoints, before.hitpoints);
}

#[test]
fn missing_skill_state_and_unknown_requirement_definitions_fail_atomically() {
    let mut content = content();
    let mut character = character(&content);
    put(&mut character.inventory, 0, stack("blade", 1));
    character.skills.clear();
    let before = character.clone();
    assert_error(gear::equip(&mut character, &content, 0), RequirementNotMet);
    assert_eq!(character, before);
    content.skills.clear();
    assert_error(gear::equip(&mut character, &content, 0), UnknownContent);
    assert_eq!(character, before);
}

#[test]
fn invalid_slot_unequippable_and_unknown_items_do_not_change_state() {
    let content = content();
    let mut character = character(&content);
    let before = character.clone();
    assert_error(gear::equip(&mut character, &content, 28), InvalidInput);
    assert_error(
        gear::equip(&mut character, &content, usize::MAX),
        InvalidInput,
    );
    assert_error(gear::equip(&mut character, &content, 0), NotOwned);
    assert_error(
        gear::unequip(&mut character, &content, &slot("unknown")),
        UnknownContent,
    );
    assert_error(
        gear::unequip(&mut character, &content, &slot("head")),
        NotOwned,
    );
    assert_eq!(character, before);
    put(&mut character.inventory, 0, stack("pebble", 1));
    let before = character.clone();
    assert_error(gear::equip(&mut character, &content, 0), InvalidInput);
    assert_eq!(character, before);
    put(&mut character.inventory, 0, stack("unknown", 1));
    let before = character.clone();
    assert_error(gear::equip(&mut character, &content, 0), UnknownContent);
    assert_eq!(character, before);
}

#[test]
fn stackable_equipment_moves_the_full_owned_stack_and_merges_without_duplication() {
    let content = content();
    let mut character = character(&content);
    character.equipment.insert(slot("ammo"), stack("ammo", 7));
    put(&mut character.inventory, 4, stack("ammo", 5));
    let total = normalized_totals(&character, &content);
    assert_eq!(
        gear::equip(&mut character, &content, 4).unwrap(),
        GameEvent::Equipped {
            slot: slot("ammo"),
            stack: stack("ammo", 12)
        },
    );
    assert_eq!(character.inventory.slots.iter().flatten().count(), 0);
    assert_eq!(normalized_totals(&character, &content), total);
    gear::unequip(&mut character, &content, &slot("ammo")).unwrap();
    assert_eq!(
        inventory::count(&character.inventory, &content.items, &item("ammo")).unwrap(),
        12
    );
    assert_eq!(normalized_totals(&character, &content), total);
}

#[test]
fn stackable_equipment_merge_overflow_rolls_back() {
    let content = content();
    let mut character = character(&content);
    character
        .equipment
        .insert(slot("ammo"), stack("ammo", MAX_STACK_QUANTITY));
    put(&mut character.inventory, 4, stack("ammo", 1));
    let before = character.clone();
    assert_error(gear::equip(&mut character, &content, 4), StackOverflow);
    assert_eq!(character, before);
}

#[test]
fn full_inventory_unequip_fails_without_hidden_or_lost_gear() {
    let content = content();
    let mut character = character(&content);
    character.inventory = filled("pebble");
    character
        .equipment
        .insert(slot("weapon"), stack("staff", 1));
    let before = character.clone();
    assert_error(
        gear::unequip(&mut character, &content, &slot("offhand")),
        InventoryFull,
    );
    assert_eq!(character, before);
}

#[test]
fn full_inventory_unequip_can_merge_stack_but_never_overflow_it() {
    let content = content();
    let mut character = character(&content);
    character.inventory = filled("pebble");
    put(
        &mut character.inventory,
        4,
        stack("ammo", MAX_STACK_QUANTITY - 2),
    );
    character.equipment.insert(slot("ammo"), stack("ammo", 3));
    let before = character.clone();
    assert_error(
        gear::unequip(&mut character, &content, &slot("ammo")),
        StackOverflow,
    );
    assert_eq!(character, before);
    character.equipment.insert(slot("ammo"), stack("ammo", 2));
    gear::unequip(&mut character, &content, &slot("ammo")).unwrap();
    assert_eq!(
        inventory::count(&character.inventory, &content.items, &item("ammo")).unwrap(),
        MAX_STACK_QUANTITY,
    );
}

#[test]
fn malformed_equipment_state_is_rejected_instead_of_normalized() {
    let content = content();
    for entries in [
        vec![(slot("head"), stack("blade", 1))],
        vec![(slot("weapon"), stack("blade", 2))],
        vec![(slot("weapon"), stack("pebble", 1))],
        vec![(slot("weapon"), stack("unknown", 1))],
        vec![
            (slot("weapon"), stack("staff", 1)),
            (slot("offhand"), stack("shield", 1)),
        ],
    ] {
        let mut character = character(&content);
        character.equipment = entries.into_iter().collect();
        put(&mut character.inventory, 0, stack("ammo", 1));
        let before = character.clone();
        assert!(gear::equip(&mut character, &content, 0).is_err());
        assert_eq!(character, before);
    }
}

#[test]
fn duplicate_or_unknown_equipment_slot_definitions_are_explicit_errors() {
    for duplicate_catalog in [false, true] {
        let mut content = content();
        let mut character = character(&content);
        put(&mut character.inventory, 0, stack("staff", 1));
        if duplicate_catalog {
            content.equipment_slots.push(slot("weapon"));
        } else {
            content
                .items
                .get_mut(&item("staff"))
                .unwrap()
                .equipment
                .as_mut()
                .unwrap()
                .occupied_slots
                .push(slot("offhand"));
        }
        let before = character.clone();
        assert_error(gear::equip(&mut character, &content, 0), InvalidContent);
        assert_eq!(character, before);
    }
    let mut content = content();
    let mut character = character(&content);
    put(&mut character.inventory, 0, stack("staff", 1));
    content
        .items
        .get_mut(&item("staff"))
        .unwrap()
        .equipment
        .as_mut()
        .unwrap()
        .occupied_slots
        .push(slot("unknown"));
    let before = character.clone();
    assert_error(gear::equip(&mut character, &content, 0), UnknownContent);
    assert_eq!(character, before);
}

#[test]
fn arbitrary_multi_slot_occupancy_is_not_hardcoded_to_hands() {
    let mut content = content();
    let mut character = character(&content);
    let mut first = item_definition("multi_first", false);
    first.equipment = Some(equipment_definition("head", &["neck", "cape"]));
    let mut second = item_definition("multi_second", false);
    second.equipment = Some(equipment_definition(
        "extra_functional_slot",
        &["neck", "ring"],
    ));
    content.items.insert(first.id.clone(), first);
    content.items.insert(second.id.clone(), second);
    character
        .equipment
        .insert(slot("head"), stack("multi_first", 1));
    put(&mut character.inventory, 0, stack("multi_second", 1));
    let total = normalized_totals(&character, &content);
    gear::equip(&mut character, &content, 0).unwrap();
    assert_eq!(character.equipment.len(), 1);
    assert!(
        gear::occupant(&character.equipment, &content, &slot("head"))
            .unwrap()
            .is_none()
    );
    assert_eq!(normalized_totals(&character, &content), total);
}

#[test]
fn zero_requirement_and_invalid_xp_state_fail_before_displacement() {
    let mut content = content();
    let mut character = character(&content);
    put(&mut character.inventory, 0, stack("blade", 1));
    content
        .items
        .get_mut(&item("blade"))
        .unwrap()
        .equipment
        .as_mut()
        .unwrap()
        .requirements = vec![SkillRequirement {
        skill: skill("practice"),
        level: 0,
    }];
    let before = character.clone();
    assert_error(gear::equip(&mut character, &content, 0), InvalidContent);
    assert_eq!(character, before);
    content
        .items
        .get_mut(&item("blade"))
        .unwrap()
        .equipment
        .as_mut()
        .unwrap()
        .requirements
        .first_mut()
        .unwrap()
        .level = 2;
    character
        .skills
        .get_mut(&skill("practice"))
        .unwrap()
        .xp_tenths = u64::MAX;
    let before = character.clone();
    assert_error(gear::equip(&mut character, &content, 0), InvalidInput);
    assert_eq!(character, before);
}

#[test]
fn repeated_equipment_cycles_preserve_all_ownership() {
    let content = content();
    let mut character = character(&content);
    for (index, name) in ["staff", "shield", "blade", "ammo"].into_iter().enumerate() {
        put(
            &mut character.inventory,
            index,
            stack(name, if name == "ammo" { 9 } else { 1 }),
        );
    }
    let total = normalized_totals(&character, &content);
    for _ in 0..16 {
        for name in ["staff", "shield", "blade", "ammo"] {
            let index = character
                .inventory
                .slots
                .iter()
                .position(|entry| entry.as_ref().is_some_and(|entry| entry.item == item(name)));
            if let Some(index) = index {
                gear::equip(&mut character, &content, index).unwrap();
            }
            assert_eq!(normalized_totals(&character, &content), total);
        }
        let slots: Vec<_> = character.equipment.keys().cloned().collect();
        for slot in slots {
            gear::unequip(&mut character, &content, &slot).unwrap();
            assert_eq!(normalized_totals(&character, &content), total);
        }
    }
}
