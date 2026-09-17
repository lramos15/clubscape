#[path = "../../content/tests/common/mod.rs"]
mod source;

use clubscape_game_types::*;
use clubscape_world_engine::{LifecycleTransition, RandomSource, WorldEngine};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

struct NoRandom;
impl RandomSource for NoRandom {
    fn draw_below(&mut self, _: u32) -> GameResult<u32> {
        panic!("this control must not sample gameplay RNG")
    }
}
fn actor() -> ActorId {
    ActorId::new("actor.ui.owner").unwrap()
}
fn data() -> GameContent {
    let mut content = source::fixture();
    source::ui::enable(&mut content);
    for stage in content.tutorial.values_mut() {
        stage.allowed_actions = vec!["*".into()];
    }
    content.initial_state.tile = source::tile(1002, 1002);
    content.initial_state.interfaces = content.interfaces.keys().cloned().collect();
    content
}
fn setup(content: GameContent) -> (WorldEngine, WorldState) {
    clubscape_content::compile_content(
        content.clone(),
        clubscape_content::ValidationMode::TestFixture,
    )
    .unwrap();
    let engine = WorldEngine::new(Arc::new(content)).unwrap();
    let mut world = engine.initial_world().unwrap();
    world.characters.insert(
        actor(),
        engine
            .character_from_initial(actor(), "Owner", BTreeMap::new())
            .unwrap(),
    );
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Join)
        .unwrap();
    (engine, world)
}
fn ui(
    engine: &WorldEngine,
    world: &mut WorldState,
    request: GameplayUiRequest,
) -> GameResult<Vec<clubscape_world_engine::ActorEvent>> {
    engine.apply_intent(world, &actor(), &GameIntent::Ui { request }, &mut NoRandom)
}
fn next(engine: &WorldEngine, world: &mut WorldState) {
    let context = engine.tick_context(world).unwrap();
    engine
        .tick_with_context(world, &mut NoRandom, &context)
        .unwrap();
}
fn interact(engine: &WorldEngine, world: &mut WorldState, action: &str) {
    engine
        .apply_intent(
            world,
            &actor(),
            &GameIntent::Interact {
                target: SpawnId::new("spawn.test.guide").unwrap(),
                action: action.into(),
            },
            &mut NoRandom,
        )
        .unwrap();
}

#[test]
fn versioned_query_is_immutable_and_never_invents_equipment_weight_or_availability() {
    let (engine, world) = setup(data());
    let before = world.clone();
    let view = engine.ui_view(&world, &actor()).unwrap();
    assert_eq!(view.version, 1);
    assert!(view.production.is_none() && view.reward.is_none() && view.bank.is_none());
    assert_eq!(view.equipment.slots, engine.content().equipment_slots);
    assert_eq!(view.equipment.weight_grams, "124");
    assert_eq!(world, before);
}

#[test]
fn source_drink_replaces_only_selected_dose_with_its_independent_timer() {
    let mut content = data();
    content.initial_state.inventory.slots[5] = Some(source::stack("item.test.potion_2", 1));
    content.initial_state.inventory.slots[6] = Some(source::stack("item.test.potion_2", 1));
    content.initial_state.run_energy = 1000;
    let (engine, mut world) = setup(content);
    world
        .characters
        .get_mut(&actor())
        .unwrap()
        .runtime
        .food_ready = 900;
    world
        .characters
        .get_mut(&actor())
        .unwrap()
        .runtime
        .combat
        .attack_ready = 800;
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::ItemAction {
            inventory_slot: 6,
            expected_item: ItemId::new("item.test.potion_2").unwrap(),
            expected_instance: None,
            action: "drink".into(),
        },
    )
    .unwrap();
    let state = &world.characters[&actor()];
    assert_eq!(
        state.inventory.slots[5],
        Some(source::stack("item.test.potion_2", 1))
    );
    assert_eq!(
        state.inventory.slots[6],
        Some(source::stack("item.test.potion_1", 1))
    );
    assert_eq!(state.run_energy, 2500);
    assert_eq!(state.runtime.food_ready, 900);
    assert_eq!(state.runtime.combat.attack_ready, 800);
    let before = world.clone();
    assert!(
        ui(
            &engine,
            &mut world,
            GameplayUiRequest::ItemAction {
                inventory_slot: 6,
                expected_item: ItemId::new("item.test.potion_2").unwrap(),
                expected_instance: None,
                action: "drink".into(),
            }
        )
        .is_err()
    );
    assert_eq!(world, before);
    for _ in 0..3 {
        next(&engine, &mut world);
    }
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::ItemAction {
            inventory_slot: 6,
            expected_item: ItemId::new("item.test.potion_1").unwrap(),
            expected_instance: None,
            action: "drink".into(),
        },
    )
    .unwrap();
    assert_eq!(
        world.characters[&actor()].inventory.slots[6],
        Some(source::stack("item.test.vial", 1))
    );
    assert_eq!(world.characters[&actor()].run_energy, 4000);
}

#[test]
fn production_menu_has_real_target_and_single_make_x_one_remain_distinct() {
    let mut content = data();
    content.initial_state.tile = source::tile(1004, 1002);
    let recipe = content
        .recipes
        .get_mut(&RecipeId::new("recipe.test.bar").unwrap())
        .unwrap();
    recipe.ticks = None;
    recipe.mechanics = Some(RecipeMechanics {
        method: ActionId::new("action.test.smith").unwrap(),
        guard: Guard::Always,
        chance_skill: None,
        cadence: ActionCadence {
            single: SourceBinding::Bound {
                value: 1,
                source: source::sources(),
            },
            first: SourceBinding::Bound {
                value: 3,
                source: source::sources(),
            },
            repeat: SourceBinding::Bound {
                value: 3,
                source: source::sources(),
            },
            menu_delay: SourceBinding::Bound {
                value: 0,
                source: source::sources(),
            },
        },
        tool_ownership: OwnershipScope::InventoryAndEquipment,
        failed_xp: Vec::new(),
        success_effects: Vec::new(),
        failure_effects: Vec::new(),
        lifecycle: RecipeLifecycle::InventoryConversion,
    });
    for (mode, delay, all) in [
        (ProductionMode::Single, 1, false),
        (ProductionMode::MakeX, 3, false),
        (ProductionMode::MakeX, 3, true),
    ] {
        let (engine, mut world) = setup(content.clone());
        engine
            .apply_intent(
                &mut world,
                &actor(),
                &GameIntent::Interact {
                    target: SpawnId::new("spawn.test.furnace").unwrap(),
                    action: "Smelt".into(),
                },
                &mut NoRandom,
            )
            .unwrap();
        let menu = engine
            .ui_view(&world, &actor())
            .unwrap()
            .production
            .unwrap();
        assert_eq!(
            menu.target,
            Some(WorldTarget::Spawn {
                spawn: SpawnId::new("spawn.test.furnace").unwrap()
            })
        );
        let before = world.clone();
        assert!(
            ui(
                &engine,
                &mut world,
                GameplayUiRequest::ProductionSelect {
                    menu_id: menu.id.clone(),
                    recipe: RecipeId::new("recipe.test.invalid").unwrap(),
                    quantity: 1,
                    mode,
                }
            )
            .is_err()
        );
        assert_eq!(world, before);
        next(&engine, &mut world);
        let starts_at = world.tick;
        ui(
            &engine,
            &mut world,
            if all {
                GameplayUiRequest::ProductionSelectAll {
                    menu_id: menu.id,
                    recipe: RecipeId::new("recipe.test.bar").unwrap(),
                }
            } else {
                GameplayUiRequest::ProductionSelect {
                    menu_id: menu.id,
                    recipe: RecipeId::new("recipe.test.bar").unwrap(),
                    quantity: 1,
                    mode,
                }
            },
        )
        .unwrap();
        assert!(
            matches!(&world.characters[&actor()].activity, Activity::ProducingSelected { next_tick, .. } if *next_tick == starts_at + delay)
        );
    }
}

#[test]
fn make_all_derives_current_complete_input_sets_and_rejects_stale_menus() {
    let mut content = data();
    content.initial_state.tile = source::tile(1004, 1002);
    let recipe = RecipeId::new("recipe.test.bar").unwrap();
    for input in &content.recipes[&recipe].inputs {
        for slot in &mut content.initial_state.inventory.slots {
            if slot.as_ref().is_some_and(|stack| stack.item == input.item) {
                *slot = None;
            }
        }
        clubscape_simulation::inventory::add(
            &mut content.initial_state.inventory,
            &content.items,
            &ItemStack {
                quantity: Quantity::new(input.quantity.get() * 3).unwrap(),
                ..input.clone()
            },
        )
        .unwrap();
    }
    let (engine, mut world) = setup(content);
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::Interact {
                target: SpawnId::new("spawn.test.furnace").unwrap(),
                action: "Smelt".into(),
            },
            &mut NoRandom,
        )
        .unwrap();
    let menu = engine
        .ui_view(&world, &actor())
        .unwrap()
        .production
        .unwrap();
    assert!(menu.recipes[0].all.as_ref().unwrap().allowed);
    next(&engine, &mut world);
    let inventory = world.characters[&actor()].inventory.clone();
    let request = GameplayUiRequest::ProductionSelectAll {
        menu_id: menu.id,
        recipe,
    };
    ui(&engine, &mut world, request.clone()).unwrap();
    assert_eq!(world.characters[&actor()].inventory, inventory);
    assert!(matches!(
        &world.characters[&actor()].activity,
        Activity::ProducingSelected {
            remaining: 3,
            mode: ProductionMode::MakeX,
            ..
        }
    ));
    let before = world.clone();
    assert!(ui(&engine, &mut world, request).is_err());
    assert_eq!(world, before);
}

#[test]
fn semantic_bank_all_survives_serialization_and_literal_options_clear_it() {
    let mut content = data();
    content.initial_state.tile = source::tile(1002, 1001);
    let (engine, mut world) = setup(content);
    interact(&engine, &mut world, "Bank");
    let before = world.characters[&actor()].clone();
    let initial = engine.ui_view(&world, &actor()).unwrap().bank.unwrap();
    assert_eq!(
        initial.amount_selection,
        Some(UiAmount::Quantity {
            quantity: Quantity::new(1).unwrap()
        })
    );
    let legacy = serde_json::to_value(&before.runtime.ui).unwrap();
    assert!(legacy["bank"].get("amount_all").is_none());
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::BankSetAmount {
            amount: UiAmount::All {},
            noted: true,
        },
    )
    .unwrap();
    let saved = serde_json::to_vec(&world).unwrap();
    let mut damaged = serde_json::to_value(&world).unwrap();
    let bank = &mut damaged["characters"][actor().as_str()]["runtime"]["ui"]["bank"];
    assert_eq!(bank["version"], BANK_LAYOUT_AMOUNT_VERSION);
    bank.as_object_mut().unwrap().remove("amount_all");
    let damaged: WorldState = serde_json::from_value(damaged).unwrap();
    assert!(
        engine.ui_view(&damaged, &actor()).is_err(),
        "versioned All history cannot become a legacy default"
    );
    let mut restored: WorldState = serde_json::from_slice(&saved).unwrap();
    let bank = engine.ui_view(&restored, &actor()).unwrap().bank.unwrap();
    assert_eq!(bank.amount_selection, Some(UiAmount::All {}));
    assert_eq!(bank.amount, 1, "the old literal is not an All sentinel");
    assert!(bank.noted);
    assert_ne!(bank.revision, initial.revision);
    let after = &restored.characters[&actor()];
    assert_eq!(before.inventory, after.inventory);
    assert_eq!(before.bank, after.bank);
    assert_eq!(before.skills, after.skills);
    assert_eq!(before.runtime.entitlements, after.runtime.entitlements);
    assert_eq!(before.last_action_tick, after.last_action_tick);
    ui(
        &engine,
        &mut restored,
        GameplayUiRequest::BankSetOptions {
            amount: 5,
            noted: false,
        },
    )
    .unwrap();
    let bank = engine.ui_view(&restored, &actor()).unwrap().bank.unwrap();
    assert_eq!(
        bank.amount_selection,
        Some(UiAmount::Quantity {
            quantity: Quantity::new(5).unwrap()
        })
    );
    assert_eq!(bank.amount, 5);
    assert!(!bank.noted);
}

#[test]
fn bank_placeholders_and_tabs_use_same_values_and_stale_identity_fails_atomically() {
    let mut content = data();
    content.initial_state.tile = source::tile(1002, 1001);
    let (engine, mut world) = setup(content);
    interact(&engine, &mut world, "Bank");
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::BankSetPlaceholders { enabled: true },
    )
    .unwrap();
    next(&engine, &mut world);
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::BankDeposit {
                banker: SpawnId::new("spawn.test.guide").unwrap(),
                inventory_slot: 2,
                quantity: Quantity::new(2).unwrap(),
            },
            &mut NoRandom,
        )
        .unwrap();
    let bank = engine.ui_view(&world, &actor()).unwrap().bank.unwrap();
    let entry = bank
        .entries
        .iter()
        .find(|entry| entry.item.as_str() == "item.test.ore")
        .unwrap()
        .id
        .clone();
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::BankCreateTab {
            entry_id: entry.clone(),
        },
    )
    .unwrap();
    assert_eq!(
        engine
            .ui_view(&world, &actor())
            .unwrap()
            .bank
            .unwrap()
            .selected_tab,
        1
    );
    next(&engine, &mut world);
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::BankWithdrawEntry {
            entry_id: entry.clone(),
            quantity: 2,
            noted: true,
        },
    )
    .unwrap();
    let bank = engine.ui_view(&world, &actor()).unwrap().bank.unwrap();
    let placeholder = bank.entries.iter().find(|row| row.id == entry).unwrap();
    assert!(placeholder.placeholder && placeholder.value.is_none());
    assert!(world.characters[&actor()].bank.slots[usize::from(placeholder.slot)].is_none());
    let before = world.clone();
    assert!(
        ui(
            &engine,
            &mut world,
            GameplayUiRequest::BankWithdrawEntry {
                entry_id: entry.clone(),
                quantity: 1,
                noted: false
            }
        )
        .is_err()
    );
    assert_eq!(world, before);
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::BankReleasePlaceholder {
            entry_id: entry.clone(),
        },
    )
    .unwrap();
    let before = world.clone();
    assert!(
        ui(
            &engine,
            &mut world,
            GameplayUiRequest::BankCreateTab { entry_id: entry }
        )
        .is_err()
    );
    assert_eq!(world, before);
}

#[test]
fn reward_payload_is_entitlement_owned_and_dismisses_before_actual_level_up() {
    let mut content = data();
    content.initial_state.tile = source::tile(1002, 1001);
    content
        .initial_state
        .skills
        .get_mut(&SkillId::new("skill.test.smithing").unwrap())
        .unwrap()
        .xp_tenths = 900;
    let (engine, mut world) = setup(content);
    interact(&engine, &mut world, "Talk");
    next(&engine, &mut world);
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::SelectDialogue {
                speaker: SpawnId::new("spawn.test.guide").unwrap(),
                choice: "ask".into(),
            },
            &mut NoRandom,
        )
        .unwrap();
    let reward = engine.ui_view(&world, &actor()).unwrap().reward.unwrap();
    assert_eq!(reward.kind, RewardUiKind::Quest);
    assert_eq!(reward.quest_points, 1);
    assert_eq!(reward.xp[0].amount_tenths, "200");
    let points = world.characters[&actor()].quest_points;
    ui(&engine, &mut world, reward.continuation).unwrap();
    let level = engine.ui_view(&world, &actor()).unwrap().reward.unwrap();
    assert_eq!(level.kind, RewardUiKind::LevelUp);
    assert_eq!(level.level, Some(2));
    let stale = level.continuation.clone();
    ui(&engine, &mut world, level.continuation).unwrap();
    assert!(engine.ui_view(&world, &actor()).unwrap().reward.is_none());
    assert_eq!(world.characters[&actor()].quest_points, points);
    assert!(ui(&engine, &mut world, stale).is_err());
}

#[test]
fn public_chat_has_authored_audience_and_does_not_interrupt_activity_or_cooldowns() {
    let (engine, mut world) = setup(data());
    let nearby = ActorId::new("actor.ui.near").unwrap();
    let mut character = engine
        .character_from_initial(nearby.clone(), "Near", BTreeMap::new())
        .unwrap();
    character.tile = source::tile(1003, 1002);
    world.characters.insert(nearby.clone(), character);
    engine
        .apply_lifecycle(&mut world, &nearby, LifecycleTransition::Join)
        .unwrap();
    world.characters.get_mut(&actor()).unwrap().activity = Activity::Walking {
        path: vec![source::tile(1001, 1002)],
        running: false,
    };
    let activity = world.characters[&actor()].activity.clone();
    let events = ui(
        &engine,
        &mut world,
        GameplayUiRequest::PublicChat {
            channel: "public".into(),
            text: "red:wave:Hello €".into(),
        },
    )
    .unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(world.characters[&actor()].activity, activity);
    assert_eq!(world.characters[&actor()].last_action_tick, 0);
    let line = &world.characters[&nearby]
        .runtime
        .ui
        .as_ref()
        .unwrap()
        .chat_messages[0];
    assert_eq!(line.sender, "Owner");
    assert_eq!(line.text, "Hello €");
    assert_eq!((line.colour, line.effect), (1, 1));
    for invalid in [
        "<col=ff0000>spoof",
        "private\nline",
        "🐧",
        "/private message",
    ] {
        let before = world.clone();
        assert!(
            ui(
                &engine,
                &mut world,
                GameplayUiRequest::PublicChat {
                    channel: "public".into(),
                    text: invalid.into()
                }
            )
            .is_err()
        );
        assert_eq!(world, before);
    }
    let sent = world.characters[&actor()]
        .runtime
        .ui
        .as_ref()
        .unwrap()
        .chat_messages
        .len();
    assert_eq!(sent, 1);
    assert_eq!(
        BTreeSet::from_iter(events.into_iter().map(|event| event.actor_id)),
        BTreeSet::from([actor(), nearby])
    );
}

#[test]
fn production_and_equipment_statistics_emit_real_source_interface_events() {
    for (interface, intent, location) in [
        (
            "interface.test.ui_production",
            GameIntent::Interact {
                target: source::id("spawn.test.furnace"),
                action: "Smelt".into(),
            },
            source::tile(1004, 1002),
        ),
        (
            "interface.test.ui_equipment_stats",
            GameIntent::OpenInterface {
                interface: source::id("interface.test.ui_equipment_stats"),
            },
            source::tile(1002, 1002),
        ),
    ] {
        let mut content = data();
        content.initial_state.tile = location;
        content
            .initial_state
            .flags
            .insert("source_ui_seen".into(), 0);
        content
            .tutorial
            .get_mut(&content.initial_state.tutorial_stage)
            .unwrap()
            .transitions
            .push(ProgressTransition {
                event: "interface_opened".into(),
                target: Some(interface.into()),
                guard: Guard::Always,
                effects: vec![Effect::SetFlag {
                    name: "source_ui_seen".into(),
                    value: 1,
                }],
            });
        let (engine, mut world) = setup(content);
        let events = engine
            .apply_intent(&mut world, &actor(), &intent, &mut NoRandom)
            .unwrap();
        assert!(events.iter().any(|event| matches!(&event.event, GameEvent::InterfaceOpened { interface: id } if id.as_str() == interface)));
        assert_eq!(world.characters[&actor()].flags["source_ui_seen"], 1);
        assert_eq!(
            engine
                .ui_view(&world, &actor())
                .unwrap()
                .active_interface
                .unwrap()
                .as_str(),
            interface
        );
        engine
            .apply_lifecycle(&mut world, &actor(), LifecycleTransition::RequestedLogout)
            .unwrap();
        let shown = engine.ui_view(&world, &actor()).unwrap();
        assert!(shown.production.is_none() && shown.active_interface.is_none());
        engine
            .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Rejoin)
            .unwrap();
        assert!(
            engine
                .ui_view(&world, &actor())
                .unwrap()
                .production
                .is_none()
        );
    }
}

#[test]
fn one_click_source_production_does_not_fabricate_a_selection_menu() {
    let mut content = data();
    content.initial_state.tile = source::tile(1004, 1002);
    let definition = content.ui.as_mut().unwrap();
    definition
        .production_interfaces
        .remove(&source::id("recipe.test.bar"));
    definition
        .direct_production
        .insert(source::id("recipe.test.bar"));
    let (engine, mut world) = setup(content);
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::Interact {
                target: source::id("spawn.test.furnace"),
                action: "Smelt".into(),
            },
            &mut NoRandom,
        )
        .unwrap();
    assert!(
        engine
            .ui_view(&world, &actor())
            .unwrap()
            .production
            .is_none()
    );
    assert!(matches!(
        world.characters[&actor()].activity,
        Activity::ProducingSelected {
            remaining: 1,
            mode: ProductionMode::Single,
            ..
        }
    ));
    for _ in 0..3 {
        next(&engine, &mut world);
    }
    assert_eq!(
        clubscape_simulation::inventory::count(
            &world.characters[&actor()].inventory,
            &engine.content().items,
            &source::id("item.test.bar")
        )
        .unwrap(),
        2
    );
}

#[test]
fn self_equipment_statistics_never_open_a_bank_or_erase_the_selected_tab() {
    let (engine, mut world) = setup(data());
    let closed = engine
        .ui_view(&world, &actor())
        .unwrap()
        .interfaces
        .into_iter()
        .find(|interface| interface.interface.as_str() == "interface.test.ui_equipment_stats")
        .unwrap();
    assert_eq!(closed.visibility, UiVisibility::Hidden);
    assert!(
        closed.permission.allowed,
        "a closed self-owned panel still has a real source open action"
    );
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::OpenInterface {
                interface: source::id("interface.test.inventory"),
            },
            &mut NoRandom,
        )
        .unwrap();
    next(&engine, &mut world);
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::OpenInterface {
                interface: source::id("interface.test.ui_equipment_stats"),
            },
            &mut NoRandom,
        )
        .unwrap();
    let view = engine.ui_view(&world, &actor()).unwrap();
    assert_eq!(
        view.active_tab.as_ref().unwrap().as_str(),
        "interface.test.inventory"
    );
    assert_eq!(
        view.active_interface.as_ref().unwrap().as_str(),
        "interface.test.ui_equipment_stats"
    );
    next(&engine, &mut world);
    let before = world.clone();
    assert!(
        engine
            .apply_intent(
                &mut world,
                &actor(),
                &GameIntent::OpenInterface {
                    interface: source::id("interface.test.ui_book"),
                },
                &mut NoRandom
            )
            .is_err()
    );
    assert_eq!(world, before);
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::CloseInterface,
            &mut NoRandom,
        )
        .unwrap();
    let view = engine.ui_view(&world, &actor()).unwrap();
    assert_eq!(view.active_interface, view.active_tab);
    let mut content = data();
    content
        .initial_state
        .interfaces
        .retain(|id| id.as_str() != "interface.test.ui_equipment_stats");
    let (engine, mut world) = setup(content);
    assert_eq!(
        engine
            .apply_intent(
                &mut world,
                &actor(),
                &GameIntent::OpenInterface {
                    interface: source::id("interface.test.ui_equipment_stats"),
                },
                &mut NoRandom
            )
            .unwrap_err()
            .code,
        GameErrorCode::RequirementNotMet
    );
}

#[test]
fn selected_burial_has_two_source_phases_and_rejects_a_replaced_slot() {
    for replace in [false, true] {
        let mut content = data();
        content.initial_state.inventory.slots[5] = Some(source::stack("item.test.bones", 1));
        content.initial_state.inventory.slots[6] = Some(source::stack("item.test.bones", 1));
        let (engine, mut world) = setup(content);
        ui(
            &engine,
            &mut world,
            GameplayUiRequest::ItemAction {
                inventory_slot: 6,
                expected_item: source::id("item.test.bones"),
                expected_instance: None,
                action: "bury".into(),
            },
        )
        .unwrap();
        next(&engine, &mut world);
        assert_eq!(
            world.characters[&actor()].skills[&source::id("skill.test.smithing")].xp_tenths,
            0
        );
        assert!(world.characters[&actor()].inventory.slots[6].is_some());
        if replace {
            world
                .characters
                .get_mut(&actor())
                .unwrap()
                .inventory
                .slots
                .swap(4, 6);
        }
        next(&engine, &mut world);
        let character = &world.characters[&actor()];
        assert_eq!(
            character.skills[&source::id("skill.test.smithing")].xp_tenths,
            if replace { 0 } else { 45 }
        );
        assert_eq!(
            character.inventory.slots[5],
            Some(source::stack("item.test.bones", 1))
        );
        assert!(matches!(character.activity, Activity::Idle));
        if replace {
            assert_eq!(
                character.inventory.slots[6],
                Some(source::stack("item.test.bar", 1))
            );
            assert_eq!(
                character.inventory.slots[4],
                Some(source::stack("item.test.bones", 1))
            );
        } else {
            assert!(character.inventory.slots[6].is_none());
        }
    }
}

#[test]
fn reading_paging_dismissal_and_emptying_keep_selected_item_identity() {
    let mut content = data();
    content.initial_state.inventory.slots[5] = Some(source::stack("item.test.food", 1));
    content.initial_state.inventory.slots[6] = Some(source::stack("item.test.potion_2", 1));
    let (engine, mut world) = setup(content);
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::ItemAction {
            inventory_slot: 5,
            expected_item: source::id("item.test.food"),
            expected_instance: None,
            action: "read".into(),
        },
    )
    .unwrap();
    let document = engine.ui_view(&world, &actor()).unwrap().document.unwrap();
    assert_eq!(document.pages.len(), 2);
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::UiDocumentPage {
            document_id: document.id.clone(),
            page: 1,
        },
    )
    .unwrap();
    assert_eq!(
        engine
            .ui_view(&world, &actor())
            .unwrap()
            .document
            .unwrap()
            .page,
        1
    );
    let before = world.clone();
    assert!(
        ui(
            &engine,
            &mut world,
            GameplayUiRequest::UiDocumentPage {
                document_id: document.id.clone(),
                page: 2
            }
        )
        .is_err()
    );
    assert_eq!(world, before);
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::UiDismiss {
            presentation_id: document.id,
        },
    )
    .unwrap();
    assert!(engine.ui_view(&world, &actor()).unwrap().document.is_none());
    assert_eq!(
        world.characters[&actor()].inventory.slots[5],
        Some(source::stack("item.test.food", 1))
    );
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::ItemAction {
            inventory_slot: 6,
            expected_item: source::id("item.test.potion_2"),
            expected_instance: None,
            action: "empty".into(),
        },
    )
    .unwrap();
    assert_eq!(
        world.characters[&actor()].inventory.slots[6],
        Some(source::stack("item.test.vial", 1))
    );
    assert_eq!(world.characters[&actor()].run_energy, MAX_RUN_ENERGY);
}

#[test]
fn bank_swap_insert_and_tab_moves_preserve_values_and_guard_destinations() {
    let mut content = data();
    content.initial_state.tile = source::tile(1002, 1001);
    content.initial_state.bank.slots = vec![
        Some(source::stack("item.test.ore", 4)),
        Some(source::stack("item.test.bar", 3)),
        Some(source::stack("item.test.coins", 20)),
    ];
    let (engine, mut world) = setup(content);
    interact(&engine, &mut world, "Bank");
    let entries = engine
        .ui_view(&world, &actor())
        .unwrap()
        .bank
        .unwrap()
        .entries;
    let ore = entries[0].id.clone();
    let bar = entries[1].id.clone();
    let coins = entries[2].id.clone();
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::BankMove {
            entry_id: ore.clone(),
            before_entry_id: Some(coins.clone()),
            tab: 0,
        },
    )
    .unwrap();
    assert_eq!(
        world.characters[&actor()].bank.slots[0],
        Some(source::stack("item.test.coins", 20))
    );
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::BankSetInsert { enabled: true },
    )
    .unwrap();
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::BankMove {
            entry_id: ore.clone(),
            before_entry_id: Some(bar.clone()),
            tab: 0,
        },
    )
    .unwrap();
    assert_eq!(
        world.characters[&actor()].bank.slots[1],
        Some(source::stack("item.test.ore", 4))
    );
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::BankCreateTab {
            entry_id: bar.clone(),
        },
    )
    .unwrap();
    let before = world.clone();
    assert!(
        ui(
            &engine,
            &mut world,
            GameplayUiRequest::BankMove {
                entry_id: coins.clone(),
                before_entry_id: Some(ore.clone()),
                tab: 1
            }
        )
        .is_err()
    );
    assert_eq!(world, before);
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::BankMove {
            entry_id: coins.clone(),
            before_entry_id: Some(bar),
            tab: 1,
        },
    )
    .unwrap();
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::BankCollapseTab { tab: 1 },
    )
    .unwrap();
    let bank = engine.ui_view(&world, &actor()).unwrap().bank.unwrap();
    assert!(bank.entries.iter().all(|entry| entry.tab == 0));
    for (item, count) in [
        ("item.test.ore", 4),
        ("item.test.bar", 3),
        ("item.test.coins", 20),
    ] {
        assert_eq!(
            clubscape_simulation::bank::count(
                &world.characters[&actor()].bank,
                &engine.content().items,
                &source::id(item)
            )
            .unwrap(),
            count
        );
    }
    let character = world.characters.get_mut(&actor()).unwrap();
    character.runtime.ui.as_mut().unwrap().bank.revision = i64::MAX as u64;
    let before = character.clone();
    assert!(
        clubscape_simulation::bank_layout::create_tab(
            &mut character.bank,
            &mut character.runtime.ui.as_mut().unwrap().bank,
            &ore,
            9
        )
        .is_err()
    );
    assert_eq!(character, &before);
}

#[test]
fn placeholders_reserve_capacity_and_refill_the_same_nonspendable_entry() {
    let mut content = data();
    content.initial_state.tile = source::tile(1002, 1001);
    content.initial_state.bank.capacity = 1;
    content.initial_state.bank.slots = vec![Some(source::stack("item.test.ore", 2))];
    let (engine, mut world) = setup(content);
    interact(&engine, &mut world, "Bank");
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::BankSetPlaceholders { enabled: true },
    )
    .unwrap();
    let id = engine
        .ui_view(&world, &actor())
        .unwrap()
        .bank
        .unwrap()
        .entries[0]
        .id
        .clone();
    next(&engine, &mut world);
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::BankPlaceholder {
            entry_id: id.clone(),
        },
    )
    .unwrap();
    let row = engine
        .ui_view(&world, &actor())
        .unwrap()
        .bank
        .unwrap()
        .entries
        .remove(0);
    assert!(row.placeholder && row.value.is_none());
    next(&engine, &mut world);
    let before = world.clone();
    assert!(
        engine
            .apply_intent(
                &mut world,
                &actor(),
                &GameIntent::BankDeposit {
                    banker: source::id("spawn.test.guide"),
                    inventory_slot: 4,
                    quantity: Quantity::new(1).unwrap(),
                },
                &mut NoRandom
            )
            .is_err()
    );
    assert_eq!(world, before);
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::BankDeposit {
                banker: source::id("spawn.test.guide"),
                inventory_slot: 2,
                quantity: Quantity::new(2).unwrap(),
            },
            &mut NoRandom,
        )
        .unwrap();
    let row = engine
        .ui_view(&world, &actor())
        .unwrap()
        .bank
        .unwrap()
        .entries
        .remove(0);
    assert_eq!(row.id, id);
    assert!(!row.placeholder);
    assert_eq!(row.value.unwrap().quantity, 2);
}

#[test]
fn equipment_deposit_uses_real_bank_and_full_bank_rejects_without_item_loss() {
    let mut content = data();
    content.initial_state.tile = source::tile(1002, 1001);
    content.initial_state.bank.capacity = 1;
    let (engine, mut world) = setup(content);
    interact(&engine, &mut world, "Bank");
    next(&engine, &mut world);
    let events = ui(&engine, &mut world, GameplayUiRequest::BankDepositEquipment).unwrap();
    assert!(world.characters[&actor()].equipment.is_empty());
    assert_eq!(
        world.characters[&actor()].bank.slots[0],
        Some(source::stack("item.test.ammo", 20))
    );
    assert!(events.iter().any(|event| matches!(&event.event, GameEvent::ItemTransferred { from: ContainerKind::Equipment, to: ContainerKind::Bank, items } if items == &vec![source::stack("item.test.ammo", 20)])));
    next(&engine, &mut world);
    let before = world.clone();
    assert!(
        engine
            .apply_intent(
                &mut world,
                &actor(),
                &GameIntent::BankDeposit {
                    banker: source::id("spawn.test.guide"),
                    inventory_slot: 4,
                    quantity: Quantity::new(1).unwrap(),
                },
                &mut NoRandom
            )
            .is_err()
    );
    assert_eq!(world, before);
}

#[test]
fn chat_admission_guard_and_offline_audience_do_not_create_gameplay_cooldowns() {
    let mut content = data();
    content.ui.as_mut().unwrap().chat.guard = Guard::Flag {
        name: "test_seen".into(),
        equals: 1,
    };
    let (engine, mut world) = setup(content);
    let before = world.clone();
    let send = || GameplayUiRequest::PublicChat {
        channel: "public".into(),
        text: "Hello".into(),
    };
    assert!(ui(&engine, &mut world, send()).is_err());
    assert_eq!(world, before);
    world
        .characters
        .get_mut(&actor())
        .unwrap()
        .flags
        .insert("test_seen".into(), 1);
    let offline: ActorId = source::id("actor.ui.offline");
    world.characters.insert(
        offline.clone(),
        engine
            .character_from_initial(offline.clone(), "Offline", BTreeMap::new())
            .unwrap(),
    );
    engine
        .apply_lifecycle(&mut world, &offline, LifecycleTransition::Join)
        .unwrap();
    engine
        .apply_lifecycle(&mut world, &offline, LifecycleTransition::RequestedLogout)
        .unwrap();
    for _ in 0..5 {
        let events = ui(&engine, &mut world, send()).unwrap();
        assert!(events.iter().all(|event| event.actor_id == actor()));
    }
    let before = world.clone();
    assert_eq!(
        ui(&engine, &mut world, send()).unwrap_err().code,
        GameErrorCode::Busy
    );
    assert_eq!(world, before);
    assert!(
        world.characters[&actor()]
            .runtime
            .action_cooldowns
            .is_empty()
    );
    assert!(
        world.characters[&offline]
            .runtime
            .ui
            .as_ref()
            .unwrap()
            .chat_messages
            .is_empty()
    );
    for _ in 0..8 {
        next(&engine, &mut world);
    }
    ui(&engine, &mut world, send()).unwrap();
}

#[test]
fn legitimate_pretraining_precedes_atomic_quest_reward_and_actual_level_presentation() {
    let mut content = data();
    content.initial_state.tile = source::tile(1004, 1002);
    for slot in &mut content.initial_state.inventory.slots[2..10] {
        *slot = Some(source::stack("item.test.ore", 1));
    }
    let (engine, mut world) = setup(content);
    for _ in 0..4 {
        engine
            .apply_intent(
                &mut world,
                &actor(),
                &GameIntent::Produce {
                    recipe: source::id("recipe.test.bar"),
                    target: Some(source::id("spawn.test.furnace")),
                    quantity: Quantity::new(1).unwrap(),
                },
                &mut NoRandom,
            )
            .unwrap();
        for _ in 0..3 {
            next(&engine, &mut world);
        }
    }
    assert_eq!(
        world.characters[&actor()].skills[&source::id("skill.test.smithing")].xp_tenths,
        800
    );
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::Walk {
                destination: source::tile(1003, 1002),
                running: false,
            },
            &mut NoRandom,
        )
        .unwrap();
    next(&engine, &mut world);
    interact(&engine, &mut world, "Talk");
    next(&engine, &mut world);
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::SelectDialogue {
                speaker: source::id("spawn.test.guide"),
                choice: "ask".into(),
            },
            &mut NoRandom,
        )
        .unwrap();
    let reward = engine.ui_view(&world, &actor()).unwrap().reward.unwrap();
    assert_eq!(reward.kind, RewardUiKind::Quest);
    assert_eq!(
        world.characters[&actor()].skills[&source::id("skill.test.smithing")].xp_tenths,
        1000
    );
    ui(&engine, &mut world, reward.continuation).unwrap();
    assert_eq!(
        engine
            .ui_view(&world, &actor())
            .unwrap()
            .reward
            .unwrap()
            .level,
        Some(2)
    );
}

#[test]
fn missing_or_corrupt_ui_history_is_an_error_not_a_new_default_or_fabricated_reward() {
    let mut content = data();
    content.initial_state.tile = source::tile(1002, 1001);
    let (engine, mut world) = setup(content);
    interact(&engine, &mut world, "Talk");
    next(&engine, &mut world);
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::SelectDialogue {
                speaker: source::id("spawn.test.guide"),
                choice: "ask".into(),
            },
            &mut NoRandom,
        )
        .unwrap();
    assert!(engine.ui_view(&world, &actor()).unwrap().reward.is_some());
    let mut missing = world.clone();
    missing.characters.get_mut(&actor()).unwrap().runtime.ui = None;
    let before = missing.clone();
    assert!(engine.ui_view(&missing, &actor()).is_err());
    assert!(engine.migrate_ui_state(&mut missing).is_err());
    assert_eq!(missing, before);
    world
        .characters
        .get_mut(&actor())
        .unwrap()
        .runtime
        .ui
        .as_mut()
        .unwrap()
        .rewards[0]
        .quest_points += 1;
    let before = world.clone();
    assert!(engine.ui_view(&world, &actor()).is_err());
    assert!(
        ui(
            &engine,
            &mut world,
            GameplayUiRequest::UiDismiss {
                presentation_id: "ui.1".into()
            }
        )
        .is_err()
    );
    assert_eq!(world, before);
}

#[test]
fn empty_native_tabs_collapse_and_all_tab_counts_all_owned_entries() {
    let mut content = data();
    content.initial_state.tile = source::tile(1002, 1001);
    content.initial_state.bank.slots = vec![
        Some(source::stack("item.test.ore", 2)),
        Some(source::stack("item.test.bar", 1)),
    ];
    let (engine, mut world) = setup(content);
    interact(&engine, &mut world, "Bank");
    let rows = engine
        .ui_view(&world, &actor())
        .unwrap()
        .bank
        .unwrap()
        .entries;
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::BankCreateTab {
            entry_id: rows[0].id.clone(),
        },
    )
    .unwrap();
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::BankCreateTab {
            entry_id: rows[1].id.clone(),
        },
    )
    .unwrap();
    let view = engine.ui_view(&world, &actor()).unwrap().bank.unwrap();
    assert_eq!(view.tabs[0].entries, 2);
    assert_eq!(view.selected_tab, 2);
    next(&engine, &mut world);
    ui(
        &engine,
        &mut world,
        GameplayUiRequest::BankWithdrawEntry {
            entry_id: rows[0].id.clone(),
            quantity: 2,
            noted: false,
        },
    )
    .unwrap();
    let view = engine.ui_view(&world, &actor()).unwrap().bank.unwrap();
    assert_eq!(view.tabs.len(), 2);
    assert_eq!(view.tabs[1].tab, 1);
    assert_eq!(view.selected_tab, 1);
    assert_eq!(view.entries[0].id, rows[1].id);
}

fn inventory_menu() -> (WorldEngine, WorldState) {
    let mut content = data();
    source::ui::inventory_production(&mut content);
    for (slot, item) in [(5, "flour"), (6, "water"), (8, "flour"), (9, "water")] {
        content.initial_state.inventory.slots[slot] =
            Some(source::stack(&format!("item.test.{item}"), 1));
    }
    let (engine, mut world) = setup(content);
    let inventory = world.characters[&actor()].inventory.clone();
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::UseItem {
                inventory_slot: 8,
                target: ItemTarget::Inventory { slot: 9 },
            },
            &mut NoRandom,
        )
        .unwrap();
    assert_eq!(world.characters[&actor()].inventory, inventory);
    assert!(matches!(
        world.characters[&actor()].activity,
        Activity::Idle
    ));
    (engine, world)
}

#[test]
fn inventory_only_menu_has_null_target_and_preserves_selected_inputs_through_completion() {
    for mode in [ProductionMode::Single, ProductionMode::MakeX] {
        let (engine, mut world) = inventory_menu();
        let menu = engine
            .ui_view(&world, &actor())
            .unwrap()
            .production
            .unwrap();
        assert!(menu.target.is_none());
        assert!(serde_json::to_value(&menu).unwrap()["target"].is_null());
        assert!(menu.recipes[0].single.allowed && menu.recipes[0].make_x.allowed);
        let selection = world.characters[&actor()]
            .runtime
            .ui
            .as_ref()
            .unwrap()
            .production
            .as_ref()
            .unwrap()
            .inventory_selection
            .as_ref()
            .unwrap();
        assert_eq!((selection.used_slot, selection.target_slot), (8, 9));
        world = serde_json::from_slice(&serde_json::to_vec(&world).unwrap()).unwrap();
        next(&engine, &mut world);
        let before = world.clone();
        assert!(
            engine
                .apply_intent(
                    &mut world,
                    &actor(),
                    &GameIntent::ProduceSelected {
                        recipe: source::id("recipe.test.dough"),
                        target: Some(WorldTarget::Spawn {
                            spawn: source::id("spawn.test.furnace")
                        }),
                        quantity: Quantity::new(1).unwrap(),
                        mode,
                    },
                    &mut NoRandom
                )
                .is_err()
        );
        assert_eq!(world, before);
        ui(
            &engine,
            &mut world,
            GameplayUiRequest::ProductionSelect {
                menu_id: menu.id,
                recipe: source::id("recipe.test.dough"),
                quantity: 1,
                mode,
            },
        )
        .unwrap();
        assert!(matches!(
            world.characters[&actor()].activity,
            Activity::ProducingSelected {
                target: None,
                next_tick: 2,
                ..
            }
        ));
        world = serde_json::from_slice(&serde_json::to_vec(&world).unwrap()).unwrap();
        next(&engine, &mut world);
        let character = &world.characters[&actor()];
        assert_eq!(
            character.inventory.slots[5],
            Some(source::stack("item.test.flour", 1))
        );
        assert_eq!(
            character.inventory.slots[6],
            Some(source::stack("item.test.water", 1))
        );
        assert_eq!(
            clubscape_simulation::inventory::count(
                &character.inventory,
                &engine.content().items,
                &source::id("item.test.dough")
            )
            .unwrap(),
            1
        );
        assert!(
            character
                .runtime
                .ui
                .as_ref()
                .unwrap()
                .production_input
                .is_none()
        );
        let inventory = character.inventory.clone();
        next(&engine, &mut world);
        assert_eq!(world.characters[&actor()].inventory, inventory);
    }
}

#[test]
fn inventory_menu_rejects_stale_slots_and_cannot_consume_a_different_matching_copy() {
    for after_selection in [false, true] {
        let (engine, mut world) = inventory_menu();
        let menu = engine
            .ui_view(&world, &actor())
            .unwrap()
            .production
            .unwrap();
        next(&engine, &mut world);
        if after_selection {
            ui(
                &engine,
                &mut world,
                GameplayUiRequest::ProductionSelect {
                    menu_id: menu.id.clone(),
                    recipe: source::id("recipe.test.dough"),
                    quantity: 1,
                    mode: ProductionMode::Single,
                },
            )
            .unwrap();
        }
        world
            .characters
            .get_mut(&actor())
            .unwrap()
            .inventory
            .slots
            .swap(8, 12);
        let before = world.clone();
        if !after_selection {
            assert!(
                !engine
                    .ui_view(&world, &actor())
                    .unwrap()
                    .production
                    .unwrap()
                    .recipes[0]
                    .single
                    .allowed
            );
            assert_eq!(
                ui(
                    &engine,
                    &mut world,
                    GameplayUiRequest::ProductionSelect {
                        menu_id: menu.id,
                        recipe: source::id("recipe.test.dough"),
                        quantity: 1,
                        mode: ProductionMode::Single,
                    }
                )
                .unwrap_err()
                .code,
                GameErrorCode::StaleCommand
            );
            assert_eq!(world, before);
        }
        next(&engine, &mut world);
        assert_eq!(
            world.characters[&actor()].inventory,
            before.characters[&actor()].inventory
        );
        assert!(matches!(
            world.characters[&actor()].activity,
            Activity::Idle
        ));
        assert!(
            world.characters[&actor()]
                .runtime
                .ui
                .as_ref()
                .unwrap()
                .production
                .is_none()
        );
        assert!(
            world.characters[&actor()]
                .runtime
                .ui
                .as_ref()
                .unwrap()
                .production_input
                .is_none()
        );
    }
}

#[test]
fn inventory_menu_closes_normally_and_legacy_targetless_production_remains_valid() {
    let (engine, mut world) = inventory_menu();
    next(&engine, &mut world);
    let before = world.clone();
    assert_eq!(
        engine
            .apply_intent(
                &mut world,
                &actor(),
                &GameIntent::UseItem {
                    inventory_slot: 8,
                    target: ItemTarget::Inventory { slot: 5 },
                },
                &mut NoRandom
            )
            .unwrap_err()
            .code,
        GameErrorCode::InvalidInput,
        "using flour on another flour stack is not the source flour-and-water selection"
    );
    assert_eq!(world, before);
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::CloseInterface,
            &mut NoRandom,
        )
        .unwrap();
    assert!(
        engine
            .ui_view(&world, &actor())
            .unwrap()
            .production
            .is_none()
    );
    next(&engine, &mut world);
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::ProduceSelected {
                recipe: source::id("recipe.test.dough"),
                target: None,
                quantity: Quantity::new(1).unwrap(),
                mode: ProductionMode::Single,
            },
            &mut NoRandom,
        )
        .unwrap();
    next(&engine, &mut world);
    assert_eq!(
        clubscape_simulation::inventory::count(
            &world.characters[&actor()].inventory,
            &engine.content().items,
            &source::id("item.test.dough")
        )
        .unwrap(),
        1
    );
}

#[test]
fn legacy_facility_menu_decodes_without_new_inventory_selection_metadata() {
    let menu: ProductionUiSession = serde_json::from_value(serde_json::json!({
        "id": "ui.1", "interface": "interface.test.ui_production",
        "target": {"kind": "spawn", "spawn": "spawn.test.furnace"},
        "instance": null, "recipes": ["recipe.test.bar"], "action": "Smelt"
    }))
    .unwrap();
    assert!(matches!(menu.target, Some(WorldTarget::Spawn { .. })));
    assert!(menu.inventory_selection.is_none());
}
