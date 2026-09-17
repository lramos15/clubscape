use std::{collections::BTreeMap, io::Read, sync::Arc};

use clubscape_content::{ValidationMode, load_compiled, sha256};
use clubscape_game_types::*;
use clubscape_world_engine::{ActorEvent, LifecycleTransition, RandomSource, WorldEngine};

#[path = "../../content/tests/common/mod.rs"]
mod fixtures;

struct NoActionRandom;
impl RandomSource for NoActionRandom {
    fn draw_below(&mut self, _: u32) -> GameResult<u32> {
        panic!("These UI and travel inputs must not draw gameplay randomness")
    }
}

struct TickRandom;
impl RandomSource for TickRandom {
    fn draw_below(&mut self, upper: u32) -> GameResult<u32> {
        assert!(upper > 0);
        Ok(0)
    }
}

fn actor() -> ActorId {
    ActorId::new("actor.ui.travel").unwrap()
}

fn bone() -> ItemId {
    ItemId::new("item.bones.tutorial").unwrap()
}

fn prayer() -> SkillId {
    SkillId::new("skill.prayer").unwrap()
}

fn travel() -> TravelId {
    TravelId::new("travel.tutorial.departure").unwrap()
}

fn setup(deny_another_action: bool) -> (WorldEngine, WorldState) {
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(&include_bytes!("../../../content/m1/game-content.csc.gz")[..])
        .read_to_end(&mut bytes)
        .unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(include_bytes!("../../../content/m1/manifest.json")).unwrap();
    assert_eq!(
        sha256(&bytes),
        manifest["compiled_artifact"]["uncompressed_sha256"]
            .as_str()
            .unwrap()
    );
    let mut content = load_compiled(&bytes, ValidationMode::Runtime)
        .unwrap()
        .definition()
        .clone();
    assert!(
        content.mechanics.travels[&travel()]
            .interruptions
            .contains(&InterruptionCause::AnotherAction)
    );
    if deny_another_action {
        // A controlled refusal-policy fixture, never a change to the published source pack.
        content
            .mechanics
            .travels
            .get_mut(&travel())
            .unwrap()
            .interruptions
            .remove(&InterruptionCause::AnotherAction);
    }
    let engine = WorldEngine::new(Arc::new(content)).unwrap();
    let mut world = engine.initial_world().unwrap();
    let mut character = engine
        .character_from_initial(
            actor(),
            "UI travel fixture",
            BTreeMap::from([("body_type".into(), 0)]),
        )
        .unwrap();
    // Source-valid component preconditions, not legitimate journey/acquisition evidence.
    character.tutorial_stage = StageId::new("stage.tutorial.teleport_channel").unwrap();
    character.runtime.settings.experience =
        Some(ExperienceId::new("experience.brand_new").unwrap());
    character.runtime.counters.insert(
        CounterId::new("counter.tutorial.departure_authorized").unwrap(),
        CounterValue::Boolean(true),
    );
    character.interfaces = engine
        .content()
        .interfaces
        .values()
        .filter(|definition| definition.access == InterfaceAccess::Tab)
        .map(|definition| definition.id.clone())
        .collect();
    let ui = character.runtime.ui.as_mut().unwrap();
    ui.active_interface = None;
    ui.active_tab = Some(InterfaceId::new("interface.magic").unwrap());
    for slot in [5, 6] {
        character.inventory.slots[slot] = Some(ItemStack {
            item: bone(),
            quantity: Quantity::new(1).unwrap(),
            instance: None,
        });
    }
    world.characters.insert(actor(), character);
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Join)
        .unwrap();
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::Cast {
                spell: "spell.lumbridge_home_teleport".into(),
                target: None,
            },
            &mut NoActionRandom,
        )
        .unwrap();
    assert_eq!(
        world.characters[&actor()]
            .runtime
            .pending_travel
            .as_ref()
            .unwrap()
            .travel,
        travel()
    );
    (engine, world)
}

fn bury(slot: u8) -> GameplayUiRequest {
    GameplayUiRequest::ItemAction {
        inventory_slot: slot,
        expected_item: bone(),
        expected_instance: None,
        action: "bury".into(),
    }
}

fn apply(
    engine: &WorldEngine,
    world: &mut WorldState,
    request: GameplayUiRequest,
) -> GameResult<Vec<ActorEvent>> {
    engine.apply_intent(
        world,
        &actor(),
        &GameIntent::Ui { request },
        &mut NoActionRandom,
    )
}

fn tick(engine: &WorldEngine, world: &mut WorldState) -> Vec<ActorEvent> {
    engine.tick(world, &mut TickRandom).unwrap()
}

fn read_only_channel() -> (WorldEngine, WorldState) {
    let mut content = fixtures::fixture();
    fixtures::ui::enable(&mut content);
    for stage in content.tutorial.values_mut() {
        stage.allowed_actions = vec!["*".into()];
    }
    let guide: SpawnId = fixtures::id("spawn.test.guide");
    content.initial_state.tile = content.spawns[&guide].tile;
    content.initial_state.inventory.slots[5] = Some(fixtures::stack("item.test.food", 1));
    content.initial_state.interfaces = content.interfaces.keys().cloned().collect();
    let id: TravelId = fixtures::id("travel.test.ui_channel");
    let bound = |value| SourceBinding::Bound {
        value,
        source: fixtures::sources(),
    };
    content.mechanics.travels.insert(
        id.clone(),
        TravelDefinition {
            id: id.clone(),
            guard: Guard::Always,
            destination: SourceBinding::Bound {
                value: TravelDestination::Fixed {
                    location: WorldLocation {
                        region: content.initial_state.region.clone(),
                        tile: content.initial_state.tile,
                        instance: None,
                    },
                },
                source: fixtures::sources(),
            },
            channel_ticks: bound(6),
            cooldown_ticks: bound(0),
            cooldown_start: SourceBinding::Bound {
                value: CooldownStart::Completed,
                source: fixtures::sources(),
            },
            interruptions: [InterruptionCause::AnotherAction].into(),
            completion_effects: Vec::new(),
            source: fixtures::sources(),
        },
    );
    content
        .spawns
        .get_mut(&guide)
        .unwrap()
        .interactions
        .push(InteractionDefinition {
            name: "Channel".into(),
            reach: 1,
            guard: Guard::Always,
            action: InteractionAction::TravelVia { travel: id },
        });
    clubscape_content::compile_content(content.clone(), ValidationMode::TestFixture).unwrap();
    let engine = WorldEngine::new(Arc::new(content)).unwrap();
    let mut world = engine.initial_world().unwrap();
    world.characters.insert(
        actor(),
        engine
            .character_from_initial(actor(), "UI channel fixture", BTreeMap::new())
            .unwrap(),
    );
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Join)
        .unwrap();
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::Interact {
                target: guide,
                action: "Channel".into(),
            },
            &mut NoActionRandom,
        )
        .unwrap();
    (engine, world)
}

#[test]
fn canonical_burial_interrupts_home_teleport_and_keeps_its_two_tick_cadence() {
    let (engine, mut world) = setup(false);
    let original = world.characters[&actor()].clone();
    let arrival_tick = original
        .runtime
        .pending_travel
        .as_ref()
        .unwrap()
        .completes_at_tick;
    tick(&engine, &mut world);
    let events = apply(&engine, &mut world, bury(5)).unwrap();
    let character = &world.characters[&actor()];
    assert!(
        character.runtime.pending_travel.is_none(),
        "An acknowledged burial must not remain behind pending source travel"
    );
    assert!(events.iter().any(|event| matches!(
        &event.event,
        GameEvent::Teleport {
            travel: id,
            phase: TeleportPhase::Interrupted { reason: InterruptionCause::AnotherAction },
        } if id == &travel()
    )));
    assert!(matches!(
        character.activity,
        Activity::InventoryAction {
            slot: 5,
            completes_at: 3,
            ..
        }
    ));
    assert_eq!(character.inventory, original.inventory);
    assert_eq!(character.skills[&prayer()].xp_tenths, 0);
    assert_eq!(
        character.runtime.travel_cooldowns,
        original.runtime.travel_cooldowns
    );
    let persisted = serde_json::to_vec(&world).unwrap();
    world = serde_json::from_slice(&persisted).unwrap();
    world.validate_runtime(engine.content()).unwrap();
    assert!(tick(&engine, &mut world).iter().all(|event| !matches!(
        &event.event,
        GameEvent::ProductionResolved { recipe, .. }
            if recipe.as_str() == "recipe.prayer.bones.tutorial"
    )));
    assert!(world.characters[&actor()].inventory.slots[5].is_some());
    let resolved = tick(&engine, &mut world);
    assert_eq!(
        resolved
            .iter()
            .filter(|event| matches!(
                &event.event,
                GameEvent::ProductionResolved { recipe, .. }
                    if recipe.as_str() == "recipe.prayer.bones.tutorial"
            ))
            .count(),
        1
    );
    let character = &world.characters[&actor()];
    assert!(character.inventory.slots[5].is_none());
    assert_eq!(character.inventory.slots[6], original.inventory.slots[6]);
    assert_eq!(character.skills[&prayer()].xp_tenths, 45);
    while world.tick <= arrival_tick {
        tick(&engine, &mut world);
    }
    let character = &world.characters[&actor()];
    assert_eq!(character.tile, original.tile);
    assert_eq!(
        character.tutorial_stage.as_str(),
        "stage.tutorial.home_teleport"
    );
    assert_eq!(
        character.runtime.entitlements,
        original.runtime.entitlements
    );
    assert_eq!(character.skills[&prayer()].xp_tenths, 45);
}

#[test]
fn noninterruptible_source_travel_refuses_timed_ui_atomically() {
    let (engine, mut world) = setup(true);
    tick(&engine, &mut world);
    let before = world.clone();
    let error = apply(&engine, &mut world, bury(5)).unwrap_err();
    assert_eq!(error.code, GameErrorCode::Busy);
    assert!(
        world == before,
        "A refused interruption must leave all state unchanged"
    );
}

#[test]
fn invalid_selected_item_does_not_cancel_an_interruptible_source_travel() {
    let (engine, mut world) = setup(false);
    tick(&engine, &mut world);
    let before = world.clone();
    assert!(apply(&engine, &mut world, bury(4)).is_err());
    assert!(
        world == before,
        "Failed item validation must roll back travel interruption"
    );
}

#[test]
fn same_tick_refusal_does_not_cancel_source_travel() {
    let (engine, mut world) = setup(false);
    let before = world.clone();
    let error = apply(&engine, &mut world, bury(5)).unwrap_err();
    assert_eq!(error.code, GameErrorCode::Busy);
    assert!(
        world == before,
        "A phase refusal must not cancel the preceding travel"
    );
}

#[test]
fn read_only_ui_and_chat_do_not_interrupt_or_consume_the_travel_action_phase() {
    for request in [
        GameplayUiRequest::PublicChat {
            channel: "public".into(),
            text: "Source travel is continuing.".into(),
        },
        GameplayUiRequest::ItemAction {
            inventory_slot: 5,
            expected_item: fixtures::id("item.test.food"),
            expected_instance: None,
            action: "read".into(),
        },
    ] {
        let (engine, mut world) = read_only_channel();
        tick(&engine, &mut world);
        let before = world.characters[&actor()].clone();
        assert!(!engine.ui_request_requires_tick(&request).unwrap());
        let events = apply(&engine, &mut world, request).unwrap();
        let character = &world.characters[&actor()];
        assert_eq!(
            character.runtime.pending_travel,
            before.runtime.pending_travel
        );
        assert_eq!(character.last_action_tick, before.last_action_tick);
        assert_eq!(character.inventory, before.inventory);
        assert!(events.iter().all(|event| !matches!(
            event.event,
            GameEvent::Teleport {
                phase: TeleportPhase::Interrupted { .. },
                ..
            }
        )));
        if let Some(document) = character.runtime.ui.as_ref().unwrap().document.as_ref() {
            let id = document.id.clone();
            apply(
                &engine,
                &mut world,
                GameplayUiRequest::UiDocumentPage {
                    document_id: id.clone(),
                    page: 1,
                },
            )
            .unwrap();
            apply(
                &engine,
                &mut world,
                GameplayUiRequest::UiDismiss {
                    presentation_id: id,
                },
            )
            .unwrap();
            assert_eq!(
                world.characters[&actor()].runtime.pending_travel,
                before.runtime.pending_travel
            );
            assert_eq!(
                world.characters[&actor()].last_action_tick,
                before.last_action_tick
            );
        }
        tick(&engine, &mut world);
        assert!(world.characters[&actor()].runtime.pending_travel.is_some());
    }
}
