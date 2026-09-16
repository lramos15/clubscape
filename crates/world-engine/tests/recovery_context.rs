#[path = "support/recovery_context.rs"]
mod fixture;

use clubscape_game_types::*;
use fixture::{add_record, apply, definition, entry, next, setup, source, view};

fn all(context: &RecoveryContextView) -> GameplayUiRequest {
    GameplayUiRequest::RecoveryTakeAll {
        selection: context.take_all.selection.clone(),
    }
}

#[test]
fn empty_office_keeps_real_context_and_policy_capacity_without_a_death_panel() {
    let (engine, mut world) = setup(definition(), 12_345);
    let before = world.clone();
    let context = view(&engine, &world);
    assert_eq!(world, before);
    assert!(matches!(
        context.identity,
        RecoveryContextIdentity::DeathOffice {
            instance: Some(_),
            ..
        }
    ));
    assert_eq!(context.counts.entries, 0);
    assert_eq!(context.counts.native_item_types, Some(0));
    assert_eq!(context.counts.capacity, 120);
    assert_eq!(context.counts.stored, 0);
    assert_eq!(context.counts.offered, 0);
    assert!(context.slots.is_empty());
    assert!(!context.take_all.permission.allowed);
    assert!(context.take_all.plan.is_none());
    assert!(context.take_all.selection.records.is_empty());
    next(&engine, &mut world);
    let before = world.clone();
    assert_eq!(
        apply(&engine, &mut world, all(&context)).unwrap_err().code,
        GameErrorCode::NotOwned
    );
    assert_eq!(world, before);
}

#[test]
fn native_duplicate_type_caption_keeps_slot_seven_but_displays_thirty_five_arrows() {
    let mut content = definition();
    let names = [
        "pick", "coins", "cooked", "rune", "ore_note", "dagger", "hammer", "arrow", "bar", "raw",
        "burnt", "flour", "bucket", "pot", "ore", "tin",
    ];
    let originals = [
        1265, 995, 315, 556, 558, 1277, 1171, 882, 1351, 303, 590, 1511, 1925, 1931, 436, 438,
    ];
    for (name, original) in names.iter().zip(originals) {
        content
            .items
            .get_mut(&source::item(name))
            .unwrap()
            .source_id = Some(original);
    }
    let (engine, mut world) = setup(content, 12_345);
    for record in 0..2 {
        let items = (record * 40..(record + 1) * 40)
            .map(|slot| {
                entry(
                    &format!("capture.{slot}"),
                    names[slot % 16],
                    if slot % 16 == 7 { 7 } else { 1 },
                    840,
                )
            })
            .collect();
        add_record(&engine, &mut world, &format!("capture.{record}"), items);
    }
    let before = world.clone();
    let context = view(&engine, &world);
    assert_eq!(world, before);
    assert_eq!(context.counts.entries, 80);
    assert_eq!(context.counts.native_item_types, Some(16));
    assert_eq!(context.counts.stored, 16);
    assert_eq!(context.counts.offered, 16);
    assert_eq!(context.counts.capacity, 120);
    assert_eq!(
        context.counts.capacity_unit,
        RecoveryCapacityUnit::ItemTypesOrInstances
    );
    let selected = &context.slots[7];
    assert_eq!(selected.slot, 7);
    assert_eq!(selected.entry.item.quantity, 7);
    assert_eq!(selected.entry.full_stack_fee, "294");
    assert_eq!(
        selected.selected_type_caption,
        RecoveryTypeCaption::Source {
            source_id: 882,
            quantity: "35".into(),
            unit_fee: "42".into(),
            total_fee: "1470".into(),
        }
    );
    for (index, slot) in context.slots.iter().enumerate() {
        assert_eq!(slot.slot as usize, index);
        assert_eq!(
            slot.entry.id.as_str(),
            format!("recovery_item.synthetic.capture.{index}")
        );
    }
    let restored = serde_json::from_slice(&serde_json::to_vec(&world).unwrap()).unwrap();
    assert_eq!(view(&engine, &restored), context);
}

#[test]
fn whole_context_uses_one_combined_plan_and_preserves_both_records_receipts() {
    let (engine, mut world) = setup(definition(), 1000);
    let first = add_record(
        &engine,
        &mut world,
        "one",
        vec![entry("one", "arrow", 7, 840)],
    );
    let second = add_record(
        &engine,
        &mut world,
        "two",
        vec![entry("two", "arrow", 7, 880)],
    );
    let context = view(&engine, &world);
    let preview = context.take_all.plan.as_ref().unwrap();
    assert_eq!(preview.total_fee, "602");
    assert_eq!(preview.transfers.len(), 2);
    assert!(!preview.partial);
    assert_eq!(context.counts.entries, 2);
    assert_eq!(context.counts.stored, 1);
    assert_eq!(
        context.slots[0].selected_type_caption,
        RecoveryTypeCaption::Source {
            source_id: 882,
            quantity: "14".into(),
            unit_fee: "42".into(),
            total_fee: "588".into(),
        }
    );
    next(&engine, &mut world);
    let before = world.clone();
    let events = apply(&engine, &mut world, all(&context)).unwrap();
    let receipts: Vec<_> = events
        .iter()
        .filter_map(|event| match &event.event {
            GameEvent::RecoveryCompleted { death, fee, .. } => Some((death.clone(), *fee)),
            _ => None,
        })
        .collect();
    assert_eq!(receipts, vec![(first.clone(), 294), (second.clone(), 308)]);
    assert_eq!(world.characters[&source::actor()].runtime.death_coffer, 398);
    assert_eq!(
        world.characters[&source::actor()].skills,
        before.characters[&source::actor()].skills
    );
    assert_eq!(world.tick, before.tick);
    for death in [first, second] {
        assert!(world.runtime.deaths[&death].office.is_empty());
        assert_eq!(world.runtime.deaths[&death].reclaimed.len(), 1);
    }
    assert_eq!(
        clubscape_simulation::inventory::count(
            &world.characters[&source::actor()].inventory,
            &engine.content().items,
            &source::item("arrow")
        )
        .unwrap(),
        14
    );
    next(&engine, &mut world);
    let before = world.clone();
    assert_eq!(
        apply(&engine, &mut world, all(&context)).unwrap_err().code,
        GameErrorCode::StaleCommand
    );
    assert_eq!(world, before);
}

#[test]
fn funds_and_capacity_are_combined_not_summed_and_partial_rows_keep_identity() {
    let (engine, mut world) = setup(definition(), 420);
    add_record(
        &engine,
        &mut world,
        "one",
        vec![entry("one", "arrow", 7, 840)],
    );
    let second = add_record(
        &engine,
        &mut world,
        "two",
        vec![entry("two", "arrow", 7, 840)],
    );
    let context = view(&engine, &world);
    assert!(
        context
            .slots
            .iter()
            .all(|slot| slot.entry.inventory_capacity == 7)
    );
    let preview = context.take_all.plan.as_ref().unwrap();
    assert_eq!(preview.total_fee, "420");
    assert_eq!(
        preview
            .transfers
            .iter()
            .map(|row| row.quantity.get())
            .collect::<Vec<_>>(),
        vec![7, 3]
    );
    assert!(preview.partial);
    next(&engine, &mut world);
    let events = apply(&engine, &mut world, all(&context)).unwrap();
    assert!(events.iter().any(|event| matches!(
        &event.event, GameEvent::Message { text } if text.starts_with("Some recovery items remain:")
    )));
    let remaining = &world.runtime.deaths[&second].office[0];
    assert_eq!(remaining.id.as_str(), "recovery_item.synthetic.two");
    assert_eq!(remaining.stack.quantity.get(), 4);
    assert!(
        !world.runtime.deaths[&second]
            .reclaimed
            .contains(&remaining.id)
    );
    assert_eq!(world.characters[&source::actor()].runtime.death_coffer, 0);
}

#[test]
fn legacy_partial_take_stays_single_entry_and_invalidates_the_old_whole_context_echo() {
    let (engine, mut world) = setup(definition(), 1000);
    let death = add_record(
        &engine,
        &mut world,
        "one",
        vec![entry("one", "arrow", 7, 840)],
    );
    add_record(
        &engine,
        &mut world,
        "two",
        vec![entry("two", "arrow", 7, 840)],
    );
    let context = view(&engine, &world);
    next(&engine, &mut world);
    apply(
        &engine,
        &mut world,
        GameplayUiRequest::RecoveryTake {
            death: death.clone(),
            storage: RecoveryStorage::DeathOffice,
            items: vec![RecoveryItemAmount {
                id: context.slots[0].entry.id.clone(),
                amount: UiAmount::Quantity {
                    quantity: Quantity::new(1).unwrap(),
                },
            }],
        },
    )
    .unwrap();
    assert_eq!(
        world.runtime.deaths[&death].office[0].stack.quantity.get(),
        6
    );
    next(&engine, &mut world);
    let before = world.clone();
    assert_eq!(
        apply(&engine, &mut world, all(&context)).unwrap_err().code,
        GameErrorCode::StaleCommand
    );
    assert_eq!(world, before);
    let fresh = view(&engine, &world);
    assert_eq!(fresh.slots[1].entry.item.quantity, 7);
    apply(&engine, &mut world, all(&fresh)).unwrap();
}

#[test]
fn wrong_context_missing_entries_and_foreign_records_are_refused_atomically() {
    let (engine, mut world) = setup(definition(), 1000);
    add_record(
        &engine,
        &mut world,
        "one",
        vec![entry("one", "arrow", 7, 840)],
    );
    let context = view(&engine, &world);
    next(&engine, &mut world);
    let mut missing = context.take_all.selection.clone();
    missing.records.clear();
    let mut wrong_context = context.take_all.selection.clone();
    wrong_context.context = RecoveryContextIdentity::DeathOffice {
        interface: InterfaceId::new("interface.synthetic.wrong").unwrap(),
        instance: None,
    };
    for selection in [missing, wrong_context] {
        let before = world.clone();
        assert_eq!(
            apply(
                &engine,
                &mut world,
                GameplayUiRequest::RecoveryTakeAll { selection }
            )
            .unwrap_err()
            .code,
            GameErrorCode::StaleCommand
        );
        assert_eq!(world, before);
    }
    let foreign = add_record(
        &engine,
        &mut world,
        "foreign",
        vec![entry("foreign", "arrow", 1, 840)],
    );
    world.runtime.deaths.get_mut(&foreign).unwrap().owner = source::actor_two();
    let before = world.clone();
    assert_eq!(view(&engine, &world), context);
    let mut selected = context.take_all.selection.clone();
    selected.records[0].death = foreign;
    assert_eq!(
        apply(
            &engine,
            &mut world,
            GameplayUiRequest::RecoveryTakeAll {
                selection: selected
            }
        )
        .unwrap_err()
        .code,
        GameErrorCode::NotOwned
    );
    assert_eq!(world, before);
}

#[test]
fn incoming_grave_counts_are_not_stored_capacity_and_overflow_refusal_does_not_move_items() {
    let mut content = definition();
    content.mechanics.death.as_mut().unwrap().office_capacity = 1;
    let (engine, mut world) = setup(content, 1000);
    add_record(
        &engine,
        &mut world,
        "one",
        vec![entry("one", "arrow", 7, 840)],
    );
    let death = add_record(
        &engine,
        &mut world,
        "two",
        vec![entry("two", "ore", 1, 840)],
    );
    let record = world.runtime.deaths.get_mut(&death).unwrap();
    record.grave = Some(GraveState {
        location: record.origin.clone(),
        active_ticks_remaining: 1500,
        clock_started: false,
        started_at_tick: None,
        paused: Default::default(),
        items: std::mem::take(&mut record.office),
    });
    let before = world.clone();
    let context = view(&engine, &world);
    assert_eq!(world, before);
    assert_eq!(context.counts.capacity, 1);
    assert_eq!(context.counts.stored, 1);
    assert_eq!(context.counts.offered, 2);
    assert_eq!(context.slots[1].current_storage, RecoveryStorage::Grave);
    assert!(!context.take_all.permission.allowed);
    assert_eq!(
        context.take_all.permission.code,
        Some(GameErrorCode::InventoryFull)
    );
    next(&engine, &mut world);
    let before = world.clone();
    assert_eq!(
        apply(&engine, &mut world, all(&context)).unwrap_err().code,
        GameErrorCode::InventoryFull
    );
    assert_eq!(world, before);
}
