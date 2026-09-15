mod support;

use std::collections::BTreeMap;

use clubscape_game_types::*;
use clubscape_world_engine::WorldEngine;
use support::*;

fn shop_mechanics(interval_ticks: u32) -> ShopLineMechanics {
    ShopLineMechanics {
        pricing: ShopPricing::StockSensitive {
            buy: StockPriceFormula {
                base_per_mille: 1000,
                change_per_stock: 0,
                minimum_per_mille: 1000,
                maximum_per_mille: 1000,
                minimum_price: 1,
                rounding: IntegerRounding::Floor,
            },
            sell: StockPriceFormula {
                base_per_mille: 500,
                change_per_stock: 0,
                minimum_per_mille: 500,
                maximum_per_mille: 500,
                minimum_price: 0,
                rounding: IntegerRounding::Floor,
            },
            overstock: SourceBinding::Bound {
                value: OverstockPricing::LinearToClamp,
                source: source(),
            },
        },
        restock: StockRestockRule {
            interval_ticks,
            amount: quantity(1),
            phase: SourceBinding::Bound {
                value: RestockPhase::SinceLastStockChange,
                source: source(),
            },
        },
    }
}

fn shop_content(maximum_lines: u16, base_stock: u32, interval_ticks: u32) -> GameContent {
    let mut content = content();
    content.items.get_mut(&item("ore")).unwrap().base_value = 6;
    content.items.get_mut(&item("tin")).unwrap().base_value = 10;
    content.shops.get_mut(&shop()).unwrap().unstocked = Some(UnstockedShopPolicy::Accept {
        maximum_lines,
        base_stock,
        rule: shop_mechanics(interval_ticks),
    });
    give_initial(
        &mut content,
        &[stack("coins", 30), stack("ore", 2), stack("tin", 1)],
    );
    content
}

fn opened_shop(content: GameContent) -> (WorldEngine, WorldState) {
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("store"));
    next(&engine, &mut world);
    (engine, world)
}

fn sell(world: &WorldState, name: &str, amount: u32) -> GameIntent {
    GameIntent::ShopSell {
        shop: shop(),
        inventory_slot: locate(world, name),
        quantity: quantity(amount),
    }
}

fn buy(index: u16, amount: u32) -> GameIntent {
    GameIntent::ShopBuy {
        shop: shop(),
        item_index: index,
        quantity: quantity(amount),
    }
}

fn old_extra(world: &mut WorldState, name: &str, stock: u32, deadline: u64) {
    world
        .shops
        .get_mut(&shop())
        .unwrap()
        .stock
        .insert(item(name), stock);
    world
        .runtime
        .stock_deadlines
        .entry(shop())
        .or_default()
        .insert(item(name), deadline);
}

fn restarted(world: &WorldState) -> WorldState {
    serde_json::from_str(&serde_json::to_string(world).unwrap()).unwrap()
}

fn assert_reclaimed(world: &WorldState, name: &str) {
    assert!(!world.shops[&shop()].stock.contains_key(&item(name)));
    assert!(
        world
            .runtime
            .stock_deadlines
            .get(&shop())
            .is_none_or(|clocks| !clocks.contains_key(&item(name)))
    );
}

#[test]
fn buying_last_extra_removes_row_and_clock_and_reuses_capacity() {
    let (engine, mut world) = opened_shop(shop_content(1, 0, 10));
    let intent = sell(&world, "ore", 1);
    apply(&engine, &mut world, intent);
    assert_eq!(count(&engine, &world, "coins"), 33);
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("ore")], 11);
    next(&engine, &mut world);

    apply(&engine, &mut world, buy(2, 1));
    assert_reclaimed(&world, "ore");
    assert_eq!(count(&engine, &world, "ore"), 2);
    assert_eq!(count(&engine, &world, "coins"), 27);
    world = restarted(&world);
    world.validate_runtime(engine.content()).unwrap();
    next(&engine, &mut world);

    let intent = sell(&world, "tin", 1);
    apply(&engine, &mut world, intent);
    assert_eq!(count(&engine, &world, "tin"), 0);
    assert_eq!(count(&engine, &world, "coins"), 32);
    let view = engine.shop_view(&world, &actor()).unwrap();
    assert_eq!(view.lines.len(), 3);
    assert_eq!(view.lines[2].index, 2);
    assert_eq!(view.lines[2].item, item("tin"));
    assert_eq!(view.lines[2].stock, 1);
}

#[test]
fn decaying_last_extra_removes_row_and_clock_and_reuses_capacity() {
    let (engine, mut world) = opened_shop(shop_content(1, 0, 4));
    let intent = sell(&world, "ore", 1);
    apply(&engine, &mut world, intent);
    ticks(&engine, &mut world, 3, &mut NeverDraw);
    assert_eq!(world.tick, 4);
    assert_eq!(world.shops[&shop()].stock[&item("ore")], 1);
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("ore")], 5);

    next(&engine, &mut world);
    assert_reclaimed(&world, "ore");
    assert_eq!(count(&engine, &world, "ore"), 1);
    assert_eq!(count(&engine, &world, "coins"), 33);
    let intent = sell(&world, "tin", 1);
    apply(&engine, &mut world, intent);
    assert_eq!(world.shops[&shop()].stock[&item("tin")], 1);
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("tin")], 9);
    assert_eq!(count(&engine, &world, "coins"), 38);
    world.validate_runtime(engine.content()).unwrap();
}

#[test]
fn serialized_tombstones_are_reclaimed_on_tick_without_waiting_for_their_clocks() {
    let (engine, mut world) = opened_shop(shop_content(2, 0, 10));
    old_extra(&mut world, "ore", 0, 1000);
    old_extra(&mut world, "tin", 0, 2000);
    world = restarted(&world);
    world.validate_runtime(engine.content()).unwrap();
    let characters = world.characters.clone();
    let ground = world.ground_items.clone();
    let provenance = world.runtime.ground_provenance.clone();

    next(&engine, &mut world);
    assert_reclaimed(&world, "ore");
    assert_reclaimed(&world, "tin");
    assert_eq!(world.characters, characters);
    assert_eq!(world.ground_items, ground);
    assert_eq!(world.runtime.ground_provenance, provenance);
    assert_eq!(engine.shop_view(&world, &actor()).unwrap().lines.len(), 2);
    world.validate_runtime(engine.content()).unwrap();
}

#[test]
fn serialized_tombstones_do_not_consume_sale_quote_or_execution_capacity() {
    let (engine, mut world) = opened_shop(shop_content(1, 0, 10));
    old_extra(&mut world, "ore", 0, 1000);
    world = restarted(&world);
    let before = world.clone();
    let quote = engine
        .shop_sell_quote(
            &world,
            &actor(),
            &shop(),
            locate(&world, "tin"),
            quantity(1),
        )
        .unwrap();
    assert_eq!(quote.item, item("tin"));
    assert_eq!(quote.quantity, 1);
    assert_eq!(quote.total_price, 5);
    assert_eq!(world, before);

    let intent = sell(&world, "tin", 1);
    apply(&engine, &mut world, intent);
    assert_reclaimed(&world, "ore");
    assert_eq!(world.shops[&shop()].stock[&item("tin")], quote.stock_after);
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("tin")], 11);
    assert_eq!(count(&engine, &world, "tin"), 0);
    assert_eq!(count(&engine, &world, "coins"), 35);
    world.validate_runtime(engine.content()).unwrap();
}

#[test]
fn legacy_fixed_source_buy_also_reclaims_old_empty_extras() {
    let (engine, mut world) = opened_shop(shop_content(1, 0, 10));
    old_extra(&mut world, "ore", 0, 1000);
    world = restarted(&world);
    let before = world.clone();
    let quote = engine
        .shop_buy_quote(&world, &actor(), &shop(), 0, quantity(1))
        .unwrap();
    assert_eq!(quote.item, item("pot"));
    assert_eq!(quote.total_price, 2);
    assert_eq!(world, before);

    apply(&engine, &mut world, buy(0, 1));
    assert_reclaimed(&world, "ore");
    assert_eq!(count(&engine, &world, "pot"), 1);
    assert_eq!(count(&engine, &world, "coins"), 28);
    assert_eq!(world.shops[&shop()].stock[&item("pot")], 4);
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("pot")], 2);
}

#[test]
fn live_extra_capacity_refuses_another_item_but_accepts_more_of_the_same_item() {
    let (engine, mut world) = opened_shop(shop_content(1, 0, 10));
    let intent = sell(&world, "ore", 1);
    apply(&engine, &mut world, intent);
    next(&engine, &mut world);
    let before = world.clone();
    assert_eq!(
        engine
            .shop_sell_quote(
                &world,
                &actor(),
                &shop(),
                locate(&world, "tin"),
                quantity(1)
            )
            .unwrap_err()
            .code,
        GameErrorCode::InventoryFull
    );
    assert_eq!(world, before);
    let intent = sell(&world, "tin", 1);
    error_unchanged(&engine, &mut world, intent, GameErrorCode::InventoryFull);

    let intent = sell(&world, "ore", 1);
    apply(&engine, &mut world, intent);
    assert_eq!(world.shops[&shop()].stock[&item("ore")], 2);
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("ore")], 12);
    assert_eq!(count(&engine, &world, "ore"), 0);
    assert_eq!(count(&engine, &world, "tin"), 1);
    assert_eq!(count(&engine, &world, "coins"), 36);
}

#[test]
fn partial_buy_keeps_the_remaining_live_extra_and_conserves_items_and_currency() {
    let mut content = shop_content(1, 0, 10);
    content
        .initial_state
        .inventory
        .slots
        .iter_mut()
        .flatten()
        .find(|stack| stack.item == item("coins"))
        .unwrap()
        .quantity = quantity(3);
    let (engine, mut world) = opened_shop(content);
    let intent = sell(&world, "ore", 2);
    apply(&engine, &mut world, intent);
    next(&engine, &mut world);
    let quote = engine
        .shop_buy_quote(&world, &actor(), &shop(), 2, quantity(50))
        .unwrap();
    assert_eq!(quote.quantity, 1);
    assert_eq!(quote.total_price, 6);
    assert_eq!(quote.stock_after, 1);
    assert_eq!(
        quote.partial_reason.unwrap().code,
        GameErrorCode::InsufficientItems
    );

    apply(&engine, &mut world, buy(2, 50));
    assert_eq!(count(&engine, &world, "coins"), 3);
    assert_eq!(count(&engine, &world, "ore"), 1);
    assert_eq!(world.shops[&shop()].stock[&item("ore")], 1);
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("ore")], 12);
    world.validate_runtime(engine.content()).unwrap();
}

#[test]
fn failed_trade_does_not_commit_cleanup_or_change_any_owned_state() {
    let (engine, mut world) = opened_shop(shop_content(1, 0, 10));
    old_extra(&mut world, "ore", 0, 1000);
    let coins = locate(&world, "coins");
    state_mut(&mut world).inventory.slots[usize::from(coins)] =
        Some(stack("coins", MAX_STACK_QUANTITY));
    world = restarted(&world);
    let before = world.clone();
    assert_eq!(
        engine
            .shop_sell_quote(
                &world,
                &actor(),
                &shop(),
                locate(&world, "tin"),
                quantity(1)
            )
            .unwrap_err()
            .code,
        GameErrorCode::StackOverflow
    );
    assert_eq!(world, before);
    let intent = sell(&world, "tin", 1);
    error_unchanged(&engine, &mut world, intent, GameErrorCode::StackOverflow);
    assert_eq!(world.shops[&shop()].stock[&item("ore")], 0);
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("ore")], 1000);
}

#[test]
fn static_zero_base_row_survives_both_last_purchase_and_last_decay() {
    let mut content = shop_content(1, 0, 4);
    let mut mechanics = shop_mechanics(4);
    mechanics.pricing = ShopPricing::Fixed;
    content
        .shops
        .get_mut(&shop())
        .unwrap()
        .stock
        .push(ShopItem {
            item: item("egg"),
            base_stock: 0,
            restock_ticks: 4,
            buy_price: 2,
            sell_price: 1,
            mechanics: Some(mechanics),
        });
    give_initial(&mut content, &[stack("egg", 2)]);
    let (engine, mut world) = opened_shop(content);
    let intent = sell(&world, "egg", 1);
    apply(&engine, &mut world, intent);
    next(&engine, &mut world);
    apply(&engine, &mut world, buy(2, 1));
    assert_eq!(world.shops[&shop()].stock[&item("egg")], 0);
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("egg")], 6);
    next(&engine, &mut world);

    let intent = sell(&world, "egg", 1);
    apply(&engine, &mut world, intent);
    ticks(&engine, &mut world, 3, &mut NeverDraw);
    assert_eq!(world.shops[&shop()].stock[&item("egg")], 1);
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("egg")], 7);
    next(&engine, &mut world);
    assert_eq!(world.shops[&shop()].stock[&item("egg")], 0);
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("egg")], 11);
    assert_eq!(
        engine.shop_view(&world, &actor()).unwrap().lines[2].item,
        item("egg")
    );
    world.validate_runtime(engine.content()).unwrap();
}

#[test]
fn cleanup_preserves_static_zero_source_rows_and_active_source_restock_phase() {
    let mut content = shop_content(1, 0, 3);
    let definition = content.shops.get_mut(&shop()).unwrap();
    let mut source_mechanics = shop_mechanics(10);
    source_mechanics.pricing = ShopPricing::Fixed;
    definition.stock[0].mechanics = Some(source_mechanics.clone());
    definition.stock.push(ShopItem {
        item: item("egg"),
        base_stock: 0,
        restock_ticks: 10,
        buy_price: 2,
        sell_price: 1,
        mechanics: Some(source_mechanics),
    });
    let (engine, mut world) = opened_shop(content);
    world
        .runtime
        .stock_deadlines
        .entry(shop())
        .or_default()
        .insert(item("egg"), 50);
    apply(&engine, &mut world, buy(0, 1));
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("pot")], 11);
    next(&engine, &mut world);
    let intent = sell(&world, "ore", 1);
    apply(&engine, &mut world, intent);
    ticks(&engine, &mut world, 3, &mut NeverDraw);
    assert_eq!(world.tick, 5);
    assert_reclaimed(&world, "ore");

    apply(&engine, &mut world, interact("store"));
    let before = world.clone();
    assert_eq!(engine.shop_view(&world, &actor()).unwrap().lines.len(), 3);
    assert_eq!(world, before);
    world = restarted(&world);
    ticks(&engine, &mut world, 5, &mut NeverDraw);
    assert_eq!(world.tick, 10);
    assert_eq!(world.shops[&shop()].stock[&item("pot")], 4);
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("pot")], 11);
    assert_eq!(world.shops[&shop()].stock[&item("egg")], 0);
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("egg")], 50);

    next(&engine, &mut world);
    assert_eq!(world.shops[&shop()].stock[&item("pot")], 5);
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("pot")], 21);
    assert_eq!(world.shops[&shop()].stock[&item("egg")], 0);
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("egg")], 50);
}

#[test]
fn positive_base_extra_at_zero_retains_capacity_and_its_restock_clock() {
    let (engine, mut world) = opened_shop(shop_content(1, 2, 4));
    let intent = sell(&world, "ore", 1);
    apply(&engine, &mut world, intent);
    assert_eq!(world.shops[&shop()].stock[&item("ore")], 3);
    next(&engine, &mut world);
    apply(&engine, &mut world, buy(2, 3));
    assert_eq!(world.shops[&shop()].stock[&item("ore")], 0);
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("ore")], 6);
    world = restarted(&world);
    next(&engine, &mut world);
    let intent = sell(&world, "tin", 1);
    error_unchanged(&engine, &mut world, intent, GameErrorCode::InventoryFull);

    ticks(&engine, &mut world, 2, &mut NeverDraw);
    assert_eq!(world.shops[&shop()].stock[&item("ore")], 0);
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("ore")], 6);
    next(&engine, &mut world);
    assert_eq!(world.shops[&shop()].stock[&item("ore")], 1);
    assert_eq!(world.runtime.stock_deadlines[&shop()][&item("ore")], 10);
    world.validate_runtime(engine.content()).unwrap();
}

#[test]
#[ignore = "Identity fix needs ownership of actions.rs, queries.rs and server game_service/view.rs."]
fn stale_expected_extra_identity_must_reject_without_charging_another_item() {
    let (engine, mut world) = setup(shop_content(2, 0, 10));
    world.characters.insert(
        actor_two(),
        engine
            .character_from_initial(actor_two(), "Other shop customer", BTreeMap::new())
            .unwrap(),
    );
    apply(&engine, &mut world, interact("store"));
    engine
        .apply_intent(&mut world, &actor_two(), &interact("store"), &mut NeverDraw)
        .unwrap();
    next(&engine, &mut world);
    let tin_slot = locate(&world, "tin");
    engine
        .apply_intent(
            &mut world,
            &actor_two(),
            &GameIntent::ShopSell {
                shop: shop(),
                inventory_slot: tin_slot,
                quantity: quantity(1),
            },
            &mut NeverDraw,
        )
        .unwrap();
    let viewed = engine.shop_view(&world, &actor()).unwrap().lines[2].clone();
    assert_eq!(viewed.item, item("tin"));
    assert_eq!(viewed.buy_price, 10);
    next(&engine, &mut world);
    let ore_slot = locate(&world, "ore");
    engine
        .apply_intent(
            &mut world,
            &actor_two(),
            &GameIntent::ShopSell {
                shop: shop(),
                inventory_slot: ore_slot,
                quantity: quantity(1),
            },
            &mut NeverDraw,
        )
        .unwrap();
    let request: GameIntent = serde_json::from_value(serde_json::json!({
        "kind": "shop_buy",
        "shop": shop(),
        "item_index": viewed.index,
        "quantity": 1,
        "expected_item": viewed.item,
    }))
    .unwrap();
    let before = world.clone();
    let result = engine.apply_intent(&mut world, &actor(), &request, &mut NeverDraw);
    assert!(
        result.is_err(),
        "stale row bought ore instead of tin: coins={}, ore={}, tin={}, result={result:?}",
        count(&engine, &world, "coins"),
        count(&engine, &world, "ore"),
        count(&engine, &world, "tin"),
    );
    assert_eq!(world, before);
}
