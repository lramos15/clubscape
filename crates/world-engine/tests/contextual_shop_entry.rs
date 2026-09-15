use std::{collections::BTreeMap, io::Read, sync::Arc};

use clubscape_content::{ValidationMode, load_compiled, sha256};
use clubscape_game_types::*;
use clubscape_world_engine::{LifecycleTransition, RandomSource, WorldEngine};

struct NoRandom;
impl RandomSource for NoRandom {
    fn draw_below(&mut self, _: u32) -> GameResult<u32> {
        panic!("Shop context admission must not sample gameplay randomness")
    }
}

fn actor() -> ActorId {
    ActorId::new("actor.shop.context").unwrap()
}

fn interface() -> InterfaceId {
    InterfaceId::new("interface.shop").unwrap()
}

fn source_content() -> GameContent {
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
    load_compiled(&bytes, ValidationMode::Runtime)
        .unwrap()
        .definition()
        .clone()
}

fn setup(content: GameContent, keeper: &SpawnId, mainland: bool) -> (WorldEngine, WorldState) {
    let engine = WorldEngine::new(Arc::new(content)).unwrap();
    let mut world = engine.initial_world().unwrap();
    let mut character = engine
        .character_from_initial(
            actor(),
            "Source shop fixture",
            BTreeMap::from([("body_type".into(), 0)]),
        )
        .unwrap();
    // Controlled source preconditions, not legitimate tutorial or acquisition evidence.
    character.tutorial_stage = StageId::new(if mainland {
        "stage.tutorial.mainland"
    } else {
        "stage.tutorial.catch_shrimp"
    })
    .unwrap();
    character.runtime.settings.experience =
        Some(ExperienceId::new("experience.brand_new").unwrap());
    character.region = engine.content().spawns[keeper].region.clone();
    character.tile = world.entities[keeper].tile;
    character.inventory.slots[0] = Some(ItemStack {
        item: ItemId::new("item.coins").unwrap(),
        quantity: Quantity::new(10).unwrap(),
        instance: None,
    });
    character.runtime.ui.as_mut().unwrap().active_interface = None;
    assert!(!character.interfaces.contains(&interface()));
    world.characters.insert(actor(), character);
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Join)
        .unwrap();
    (engine, world)
}

fn trade(
    engine: &WorldEngine,
    world: &mut WorldState,
    keeper: &SpawnId,
) -> GameResult<Vec<clubscape_world_engine::ActorEvent>> {
    engine.apply_intent(
        world,
        &actor(),
        &GameIntent::Interact {
            target: keeper.clone(),
            action: "Trade".into(),
        },
        &mut NoRandom,
    )
}

#[test]
fn both_canonical_keepers_admit_and_present_owned_shop_context_without_a_tab_unlock() {
    for name in ["spawn.shopkeeper", "spawn.shop_assistant"] {
        let keeper = SpawnId::new(name).unwrap();
        let (engine, mut world) = setup(source_content(), &keeper, true);
        let before_query = world.clone();
        let options = engine
            .interaction_options(
                &world,
                &actor(),
                &WorldTarget::Spawn {
                    spawn: keeper.clone(),
                },
            )
            .unwrap();
        assert!(
            world == before_query,
            "Evaluating a shop entry must not mutate state"
        );
        assert!(
            options
                .iter()
                .any(|row| row.name == "Trade" && row.permission.allowed),
            "The guarded mainland keeper must offer Trade without an unearned shop-tab token"
        );
        let before = world.characters[&actor()].clone();
        let events = trade(&engine, &mut world, &keeper).unwrap();
        assert!(events.iter().any(|event| matches!(
            &event.event,
            GameEvent::InterfacePresented {
                interface: id,
                context: InterfaceContext::Shop { spawn, shop },
            } if id == &interface() && spawn == &keeper && shop.as_str() == "shop.lumbridge.general_store"
        )));
        let character = &world.characters[&actor()];
        assert!(
            !character.interfaces.contains(&interface()),
            "A shop context is not a fabricated permanent unlock"
        );
        assert_eq!(character.inventory, before.inventory);
        assert_eq!(character.bank, before.bank);
        assert_eq!(character.skills, before.skills);
        assert_eq!(character.runtime.entitlements, before.runtime.entitlements);
        assert_eq!(
            engine.ui_view(&world, &actor()).unwrap().active_interface,
            Some(interface())
        );
        let view = engine.shop_view(&world, &actor()).unwrap();
        assert_eq!(view.shop.as_str(), "shop.lumbridge.general_store");
        let serialized = serde_json::to_vec(&world).unwrap();
        let restored: WorldState = serde_json::from_slice(&serialized).unwrap();
        assert_eq!(
            engine.shop_view(&restored, &actor()).unwrap(),
            engine.shop_view(&world, &actor()).unwrap()
        );
    }
}

#[test]
fn tutorial_guard_refuses_trade_even_if_a_shop_interface_token_is_present() {
    let keeper = SpawnId::new("spawn.shopkeeper").unwrap();
    let (engine, mut world) = setup(source_content(), &keeper, false);
    world
        .characters
        .get_mut(&actor())
        .unwrap()
        .interfaces
        .push(interface());
    let before = world.clone();
    let options = engine
        .interaction_options(
            &world,
            &actor(),
            &WorldTarget::Spawn {
                spawn: keeper.clone(),
            },
        )
        .unwrap();
    assert!(
        options
            .iter()
            .any(|row| row.name == "Trade" && !row.permission.allowed)
    );
    assert!(trade(&engine, &mut world, &keeper).is_err());
    assert!(world == before, "A source-stage refusal must be atomic");
}

#[test]
fn range_refusal_preserves_all_state_and_does_not_create_shop_context() {
    let content = source_content();
    let start = content.initial_state.tile;
    let region = content.initial_state.region.clone();
    let keeper = SpawnId::new("spawn.shopkeeper").unwrap();
    let (engine, mut world) = setup(content, &keeper, true);
    let character = world.characters.get_mut(&actor()).unwrap();
    character.tile = start;
    character.region = region;
    let before = world.clone();
    assert!(trade(&engine, &mut world, &keeper).is_err());
    assert!(world == before);
    assert!(engine.shop_view(&world, &actor()).is_err());
}

#[test]
fn contextual_shop_cannot_be_opened_with_generic_tab_input() {
    let keeper = SpawnId::new("spawn.shopkeeper").unwrap();
    let (engine, mut world) = setup(source_content(), &keeper, true);
    world
        .characters
        .get_mut(&actor())
        .unwrap()
        .interfaces
        .push(interface());
    let before = world.clone();
    let result = engine.apply_intent(
        &mut world,
        &actor(),
        &GameIntent::OpenInterface {
            interface: interface(),
        },
        &mut NoRandom,
    );
    assert!(result.is_err());
    assert!(world == before);
}

#[test]
fn source_shop_action_cannot_present_a_tab_as_its_context() {
    let mut content = source_content();
    let keeper = SpawnId::new("spawn.shopkeeper").unwrap();
    let action = content
        .spawns
        .get_mut(&keeper)
        .unwrap()
        .interactions
        .iter_mut()
        .find(|row| row.name == "Trade")
        .unwrap();
    let InteractionAction::OpenShop { interface, .. } = &mut action.action else {
        panic!("Expected the original source OpenShop action")
    };
    *interface = InterfaceId::new("interface.inventory").unwrap();
    let (engine, mut world) = setup(content, &keeper, true);
    let before = world.clone();
    assert_eq!(
        trade(&engine, &mut world, &keeper).unwrap_err().code,
        GameErrorCode::InvalidContent
    );
    assert!(world == before);
}
