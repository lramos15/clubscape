#[path = "mod.rs"]
pub mod source;

use crate::engine_content as ui_source;
use clubscape_game_types::*;
use clubscape_world_engine::{ActorEvent, LifecycleTransition, RandomSource, WorldEngine};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub struct NoRandom;
impl RandomSource for NoRandom {
    fn draw_below(&mut self, _: u32) -> GameResult<u32> {
        panic!("Recovery context controls must not draw gameplay RNG")
    }
}

pub fn definition() -> GameContent {
    let mut content = source::v2::content();
    source::v2::with_combat(&mut content);
    source::v2::with_death(&mut content);
    for (name, original) in [("grave", 602), ("office", 669)] {
        let id = InterfaceId::new(format!("interface.synthetic.recovery.{name}")).unwrap();
        content.interfaces.insert(
            id.clone(),
            InterfaceDefinition {
                id,
                name: format!("Controlled recovery {name}"),
                access: InterfaceAccess::Contextual,
                source_ids: vec![original],
                source: source::source(),
            },
        );
    }
    let policy = content.mechanics.death.as_mut().unwrap();
    policy.interfaces = Some(RecoveryInterfaces {
        grave: InterfaceId::new("interface.synthetic.recovery.grave").unwrap(),
        office: InterfaceId::new("interface.synthetic.recovery.office").unwrap(),
    });
    policy.office_fee = source::v2::bound(RecoveryFee::Percentage {
        free_below: 0,
        rate: Ratio {
            numerator: 5,
            denominator: 100,
        },
        rounding: IntegerRounding::Floor,
    });
    ui_source::ui::projection(&mut content);
    for stage in content.tutorial.values_mut() {
        stage.allowed_actions = vec!["*".into()];
    }
    for (index, item) in content.items.values_mut().enumerate() {
        item.source_id = Some(20_000 + u32::try_from(index).unwrap());
    }
    let arrow = content.items.get_mut(&source::item("arrow")).unwrap();
    arrow.source_id = Some(882);
    arrow.name = "Bronze arrow".into();
    content.initial_state.inventory = Inventory::default();
    content.initial_state.equipment.clear();
    content.initial_state.interfaces = content.interfaces.keys().cloned().collect();
    content
}

pub fn setup(content: GameContent, funds: u64) -> (WorldEngine, WorldState) {
    let engine = WorldEngine::new(Arc::new(content)).unwrap();
    let mut world = engine.initial_world().unwrap();
    let mut character = engine
        .character_from_initial(
            source::actor(),
            "Controlled recovery context",
            BTreeMap::new(),
        )
        .unwrap();
    let office = engine
        .content()
        .mechanics
        .death
        .as_ref()
        .unwrap()
        .first_office
        .require()
        .unwrap();
    let instance = InstanceId::new("instance.synthetic.recovery_context").unwrap();
    world.runtime.instances.insert(
        instance.clone(),
        InstanceState {
            template: office.instance.clone().unwrap(),
            owner: Some(source::actor()),
            counters: BTreeMap::new(),
            entities: BTreeMap::new(),
            object_states: BTreeMap::new(),
        },
    );
    character.region = office.region.clone();
    character.tile = office.tile;
    character.runtime.instance = Some(instance);
    character.runtime.death_coffer = funds;
    world.characters.insert(source::actor(), character);
    engine
        .apply_lifecycle(&mut world, &source::actor(), LifecycleTransition::Join)
        .unwrap();
    engine
        .apply_intent(
            &mut world,
            &source::actor(),
            &GameIntent::OpenDeathOffice,
            &mut NoRandom,
        )
        .unwrap();
    (engine, world)
}

pub fn entry(label: &str, item: &str, quantity: u32, value: u64) -> RecoveryItem {
    RecoveryItem {
        id: RecoveryItemId::new(format!("recovery_item.synthetic.{label}")).unwrap(),
        stack: source::stack(item, quantity),
        layout: ItemLayout::Inventory { slot: 0 },
        effective_unit_value: value,
        fee_paid: 0,
    }
}

pub fn add_record(
    engine: &WorldEngine,
    world: &mut WorldState,
    label: &str,
    items: Vec<RecoveryItem>,
) -> DeathId {
    let content = engine.content();
    let policy = content.mechanics.death.as_ref().unwrap();
    let location = RuntimeLocation {
        region: content.initial_state.region.clone(),
        tile: content.initial_state.tile,
        instance: None,
    };
    let id = DeathId::new(format!("death.synthetic.{label}")).unwrap();
    world.runtime.deaths.insert(
        id.clone(),
        DeathRecord {
            owner: source::actor(),
            occurred_at_tick: world.tick,
            origin: location.clone(),
            respawn: location,
            value_provider: policy.value_provider.clone(),
            value_revision: content.mechanics.value_providers[&policy.value_provider]
                .revision
                .clone(),
            retained: Vec::new(),
            grave: None,
            office: items,
            reclaimed: BTreeSet::new(),
            discarded: BTreeSet::new(),
            arrival: None,
        },
    );
    id
}

pub fn view(engine: &WorldEngine, world: &WorldState) -> RecoveryContextView {
    engine
        .ui_view(world, &source::actor())
        .unwrap()
        .recovery
        .unwrap()
        .management
        .unwrap()
        .context
        .unwrap()
}

pub fn apply(
    engine: &WorldEngine,
    world: &mut WorldState,
    request: GameplayUiRequest,
) -> GameResult<Vec<ActorEvent>> {
    engine.apply_intent(
        world,
        &source::actor(),
        &GameIntent::Ui { request },
        &mut NoRandom,
    )
}

pub fn next(engine: &WorldEngine, world: &mut WorldState) {
    engine.tick(world, &mut NoRandom).unwrap();
}
