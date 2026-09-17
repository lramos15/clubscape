mod support;

use clubscape_game_types::*;
use support::*;

#[test]
fn bank_requires_open_authenticated_session_not_proximity_alone() {
    let (engine, mut world) = setup(content());
    let deposit = GameIntent::BankDeposit {
        banker: spawn("bank"),
        inventory_slot: 0,
        quantity: quantity(1),
    };
    error_unchanged(
        &engine,
        &mut world,
        deposit.clone(),
        GameErrorCode::RequirementNotMet,
    );
    apply(&engine, &mut world, interact("bank"));
    next(&engine, &mut world);
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::BankDeposit {
            banker: spawn("store"),
            inventory_slot: 0,
            quantity: quantity(1),
        },
        GameErrorCode::NotOwned,
    );
    apply(&engine, &mut world, deposit);
    assert_eq!(count(&engine, &world, "pick"), 0);
    assert_eq!(state(&world).bank.slots[0], Some(stack("pick", 1)));
}

#[test]
fn bank_session_survives_restart_but_movement_and_range_revoke_access() {
    let (engine, mut world) = setup(content());
    apply(&engine, &mut world, interact("bank"));
    next(&engine, &mut world);
    world = serde_json::from_str(&serde_json::to_string(&world).unwrap()).unwrap();
    apply(
        &engine,
        &mut world,
        GameIntent::BankDeposit {
            banker: spawn("bank"),
            inventory_slot: 0,
            quantity: quantity(1),
        },
    );
    next(&engine, &mut world);
    apply(
        &engine,
        &mut world,
        GameIntent::Walk {
            destination: tile(10, 11, 0),
            running: false,
        },
    );
    next(&engine, &mut world);
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::BankWithdraw {
            banker: spawn("bank"),
            bank_slot: 0,
            quantity: quantity(1),
            noted: false,
        },
        GameErrorCode::RequirementNotMet,
    );
}

#[test]
fn bank_transfers_up_to_quantity_and_capacity_without_losing_remainder() {
    let mut content = content();
    content
        .initial_state
        .bank
        .slots
        .push(Some(stack("ore", 50)));
    give_initial(&mut content, &[stack("pebble", 24)]);
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("bank"));
    next(&engine, &mut world);
    apply(
        &engine,
        &mut world,
        GameIntent::BankWithdraw {
            banker: spawn("bank"),
            bank_slot: 0,
            quantity: quantity(MAX_STACK_QUANTITY),
            noted: false,
        },
    );
    assert_eq!(count(&engine, &world, "ore"), 2);
    assert_eq!(state(&world).bank.slots[0], Some(stack("ore", 48)));
    next(&engine, &mut world);
    let slot = locate(&world, "ore");
    apply(
        &engine,
        &mut world,
        GameIntent::BankDeposit {
            banker: spawn("bank"),
            inventory_slot: slot,
            quantity: quantity(50),
        },
    );
    assert_eq!(count(&engine, &world, "ore"), 0);
    assert_eq!(state(&world).bank.slots[0], Some(stack("ore", 50)));
}

#[test]
fn bank_note_conversion_uses_shared_primitives() {
    let mut content = content();
    content
        .initial_state
        .bank
        .slots
        .push(Some(stack("ore", 10)));
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("bank"));
    next(&engine, &mut world);
    apply(
        &engine,
        &mut world,
        GameIntent::BankWithdraw {
            banker: spawn("bank"),
            bank_slot: 0,
            quantity: quantity(10),
            noted: true,
        },
    );
    assert_eq!(count(&engine, &world, "ore_note"), 10);
    next(&engine, &mut world);
    let slot = locate(&world, "ore_note");
    apply(
        &engine,
        &mut world,
        GameIntent::BankDeposit {
            banker: spawn("bank"),
            inventory_slot: slot,
            quantity: quantity(10),
        },
    );
    assert_eq!(count(&engine, &world, "ore_note"), 0);
    assert_eq!(state(&world).bank.slots[0], Some(stack("ore", 10)));
}

#[test]
fn open_interface_event_is_not_bank_or_shop_authorization() {
    let (engine, mut world) = setup(content());
    apply(
        &engine,
        &mut world,
        GameIntent::OpenInterface {
            interface: interface(),
        },
    );
    next(&engine, &mut world);
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::ShopBuy {
            shop: shop(),
            item_index: 0,
            quantity: quantity(1),
            expected_item: None,
        },
        GameErrorCode::RequirementNotMet,
    );
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::BankDeposit {
            banker: spawn("bank"),
            inventory_slot: 0,
            quantity: quantity(1),
        },
        GameErrorCode::RequirementNotMet,
    );
}

#[test]
fn bank_reopening_never_reseeds_source_starting_coins() {
    let mut content = content();
    content
        .initial_state
        .bank
        .slots
        .push(Some(stack("coins", 25)));
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("bank"));
    next(&engine, &mut world);
    apply(
        &engine,
        &mut world,
        GameIntent::BankWithdraw {
            banker: spawn("bank"),
            bank_slot: 0,
            quantity: quantity(25),
            noted: false,
        },
    );
    next(&engine, &mut world);
    apply(&engine, &mut world, GameIntent::CloseInterface);
    next(&engine, &mut world);
    apply(&engine, &mut world, interact("bank"));
    assert_eq!(count(&engine, &world, "coins"), 25);
    assert_eq!(state(&world).bank.slots[0], None);
}

#[test]
fn fixed_price_shop_mutates_source_stock_and_currency_atomically() {
    let mut content = content();
    give_initial(&mut content, &[stack("coins", 7)]);
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("store"));
    next(&engine, &mut world);
    apply(
        &engine,
        &mut world,
        GameIntent::ShopBuy {
            shop: shop(),
            item_index: 0,
            quantity: quantity(50),
            expected_item: None,
        },
    );
    assert_eq!(count(&engine, &world, "pot"), 3);
    assert_eq!(count(&engine, &world, "coins"), 1);
    assert_eq!(world.shops[&shop()].stock[&item("pot")], 2);
    next(&engine, &mut world);
    let slot = locate(&world, "pot");
    apply(
        &engine,
        &mut world,
        GameIntent::ShopSell {
            shop: shop(),
            inventory_slot: slot,
            quantity: quantity(3),
        },
    );
    assert_eq!(count(&engine, &world, "pot"), 0);
    assert_eq!(count(&engine, &world, "coins"), 4);
    assert_eq!(world.shops[&shop()].stock[&item("pot")], 6);
}

#[test]
fn shop_restock_uses_each_declared_row_period_and_moves_toward_base() {
    let (engine, mut world) = setup(content());
    let state = world.shops.get_mut(&shop()).unwrap();
    state.stock.insert(item("pot"), 0);
    state.stock.insert(item("arrow"), 22);
    next(&engine, &mut world);
    assert_eq!(world.shops[&shop()].stock[&item("pot")], 0);
    next(&engine, &mut world);
    assert_eq!(world.shops[&shop()].stock[&item("pot")], 1);
    assert_eq!(world.shops[&shop()].stock[&item("arrow")], 22);
    ticks(&engine, &mut world, 2, &mut NeverDraw);
    assert_eq!(world.shops[&shop()].stock[&item("pot")], 2);
    assert_eq!(world.shops[&shop()].stock[&item("arrow")], 21);
}

#[test]
fn shop_capacity_failure_preserves_coins_stock_and_slots() {
    let mut content = content();
    give_initial(&mut content, &[stack("coins", 100), stack("pebble", 25)]);
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("store"));
    next(&engine, &mut world);
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::ShopBuy {
            shop: shop(),
            item_index: 0,
            quantity: quantity(2),
            expected_item: None,
        },
        GameErrorCode::InventoryFull,
    );
}

#[test]
fn exact_currency_stack_can_be_replaced_when_inventory_is_full() {
    let mut content = content();
    give_initial(&mut content, &[stack("coins", 2), stack("pebble", 25)]);
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("store"));
    next(&engine, &mut world);
    apply(
        &engine,
        &mut world,
        GameIntent::ShopBuy {
            shop: shop(),
            item_index: 0,
            quantity: quantity(50),
            expected_item: None,
        },
    );
    assert_eq!(count(&engine, &world, "pot"), 1);
    assert_eq!(count(&engine, &world, "coins"), 0);
}

#[test]
fn zero_price_sale_is_legal_and_general_unknown_price_is_not_invented() {
    let mut content = content();
    give_initial(&mut content, &[stack("arrow", 4)]);
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("store"));
    next(&engine, &mut world);
    let slot = locate(&world, "arrow");
    apply(
        &engine,
        &mut world,
        GameIntent::ShopSell {
            shop: shop(),
            inventory_slot: slot,
            quantity: quantity(4),
        },
    );
    assert_eq!(count(&engine, &world, "arrow"), 0);
    assert_eq!(count(&engine, &world, "coins"), 0);
    assert_eq!(world.shops[&shop()].stock[&item("arrow")], 24);
    next(&engine, &mut world);
    error_unchanged(
        &engine,
        &mut world,
        GameIntent::ShopSell {
            shop: shop(),
            inventory_slot: 0,
            quantity: quantity(1),
        },
        GameErrorCode::Unavailable,
    );
}

#[test]
fn full_inventory_sale_of_entire_stack_is_not_missed_by_partial_search() {
    let mut content = content();
    content.shops.get_mut(&shop()).unwrap().stock[1].sell_price = 1;
    give_initial(&mut content, &[stack("arrow", 7), stack("pebble", 25)]);
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("store"));
    next(&engine, &mut world);
    let slot = locate(&world, "arrow");
    apply(
        &engine,
        &mut world,
        GameIntent::ShopSell {
            shop: shop(),
            inventory_slot: slot,
            quantity: quantity(50),
        },
    );
    assert_eq!(count(&engine, &world, "arrow"), 0);
    assert_eq!(count(&engine, &world, "coins"), 7);
}
