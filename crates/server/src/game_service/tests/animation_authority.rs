use super::*;
use std::io::Read;

struct NoDraw;
impl clubscape_world_engine::RandomSource for NoDraw {
    fn draw_below(&mut self, _: u32) -> GameResult<u32> {
        panic!("read-only/consumption animation work must not draw gameplay RNG")
    }
}

pub(super) fn source() -> Arc<WorldEngine> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../content/m1/game-content.csc.gz");
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(fs::File::open(path).unwrap())
        .read_to_end(&mut raw)
        .unwrap();
    let content = clubscape_content::load_compiled(&raw, ValidationMode::Runtime).unwrap();
    assert!(
        content
            .definition()
            .ui
            .as_ref()
            .unwrap()
            .actor_animations
            .is_some()
    );
    Arc::new(WorldEngine::new(Arc::new(content.definition().clone())).unwrap())
}

fn actor() -> ActorId {
    fixtures::id("actor.animation.source")
}

fn world(engine: &WorldEngine) -> WorldState {
    let mut world = engine.initial_world().unwrap();
    let mut character = engine
        .character_from_initial(actor(), "Animation", BTreeMap::new())
        .unwrap();
    character.tutorial_stage = fixtures::id("stage.tutorial.mainland");
    character.interfaces = engine.content().interfaces.keys().cloned().collect();
    world.characters.insert(actor(), character);
    engine
        .apply_lifecycle(&mut world, &actor(), LifecycleTransition::Join)
        .unwrap();
    world
}

fn tick(engine: &WorldEngine, world: &mut WorldState) {
    let context = engine.tick_context(world).unwrap();
    engine
        .tick_with_context(world, &mut engine_fixtures::v2::Hits(0), &context)
        .unwrap();
}

fn action(engine: &WorldEngine, world: &WorldState) -> ActorActionView {
    engine
        .actor_observer(world, &actor())
        .unwrap()
        .action
        .unwrap()
}

fn combat_setup(engine: &WorldEngine, style: &CombatStyleId) -> (WorldState, SpawnId) {
    let mut world = world(engine);
    let target = engine.content().spawns.values().find(|spawn| {
        matches!(&spawn.kind, SpawnKind::Npc { npc } if npc.as_str() == "npc.goblin.level_2")
    }).unwrap();
    let character = world.characters.get_mut(&actor()).unwrap();
    character.tile = target.tile.offset(-1, 0).unwrap();
    character.region = target.region.clone();
    character.equipment.clear();
    if let Some(item) = engine.content().items.values().find(|item| {
        item.equipment
            .as_ref()
            .and_then(|equipment| equipment.weapon.as_ref())
            .is_some_and(|weapon| weapon.styles.contains(style))
    }) {
        character.equipment.insert(
            item.equipment.as_ref().unwrap().slot.clone(),
            ItemStack {
                item: item.id.clone(),
                quantity: Quantity::new(1).unwrap(),
                instance: None,
            },
        );
    }
    if engine.content().mechanics.combat_styles[style].method == AttackMethod::Ranged {
        character.equipment.insert(
            fixtures::id("slot.ammo"),
            ItemStack {
                item: fixtures::id("item.arrow.bronze"),
                quantity: Quantity::new(50).unwrap(),
                instance: None,
            },
        );
    }
    character.runtime.combat.style = Some(style.clone());
    (world, target.id.clone())
}

fn attack(engine: &WorldEngine, target: &SpawnId) -> GameIntent {
    let name = engine.content().spawns[target]
        .interactions
        .iter()
        .find(|interaction| matches!(interaction.action, InteractionAction::Attack))
        .unwrap()
        .name
        .clone();
    GameIntent::Interact {
        target: target.clone(),
        action: name,
    }
}

#[test]
fn every_actual_legal_melee_ranged_style_emits_its_bound_source_animation() {
    let engine = source();
    for (style, binding) in &engine
        .content()
        .ui
        .as_ref()
        .unwrap()
        .actor_animations
        .as_ref()
        .unwrap()
        .styles
    {
        if style.as_str() == "style.magic.wind_strike" {
            continue;
        }
        let ActorAnimationRule::Sequence { sequence } = binding.rule else {
            panic!()
        };
        let (mut world, target) = combat_setup(&engine, style);
        engine
            .apply_intent(
                &mut world,
                &actor(),
                &attack(&engine, &target),
                &mut engine_fixtures::v2::Hits(0),
            )
            .unwrap();
        let observed = action(&engine, &world);
        assert_eq!(observed.style_id.as_ref(), Some(style), "{style}");
        assert_eq!(observed.target, Some(WorldTarget::Spawn { spawn: target }));
        assert_eq!(
            observed.animation.as_deref(),
            Some(sequence.to_string().as_str())
        );
        assert_eq!(observed.cycle_started_at_tick, "0");
        let before = world.clone();
        assert_eq!(action(&engine, &world), observed);
        assert_eq!(world, before);
    }
}

#[test]
fn whole_home_channel_has_stable_source_phase_clocks_and_cancels_without_arrival() {
    let engine = source();
    let mut world = world(&engine);
    let character = world.characters.get_mut(&actor()).unwrap();
    character.tutorial_stage = fixtures::id("stage.tutorial.teleport_channel");
    character.runtime.settings.experience = Some(fixtures::id("experience.brand_new"));
    character.runtime.counters.insert(
        fixtures::id("counter.tutorial.departure_authorized"),
        CounterValue::Boolean(true),
    );
    character.inventory.slots[5] = Some(ItemStack {
        item: fixtures::id("item.bones.tutorial"),
        quantity: Quantity::new(1).unwrap(),
        instance: None,
    });
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::Cast {
                spell: "spell.lumbridge_home_teleport".into(),
                target: None,
            },
            &mut NoDraw,
        )
        .unwrap();
    let id = action(&engine, &world).id;
    for at in 0..24 {
        let (start, sequence) = [(0, 4847), (6, 4850), (12, 4853), (16, 4855), (21, 4857)]
            .into_iter()
            .rev()
            .find(|(start, _)| *start <= at)
            .unwrap();
        let observed = action(&engine, &world);
        assert_eq!(observed.id, id);
        assert_eq!(observed.started_at_tick, "0");
        assert_eq!(observed.cycle_started_at_tick, start.to_string());
        assert_eq!(observed.next_action_tick.as_deref(), Some("24"));
        assert_eq!(
            observed.animation.as_deref(),
            Some(sequence.to_string().as_str())
        );
        let restored: WorldState =
            serde_json::from_slice(&serde_json::to_vec(&world).unwrap()).unwrap();
        assert_eq!(action(&engine, &restored), observed);
        if at != 23 {
            tick(&engine, &mut world);
        }
    }
    let before = world.characters[&actor()].clone();
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::Ui {
                request: GameplayUiRequest::ItemAction {
                    inventory_slot: 5,
                    expected_item: fixtures::id("item.bones.tutorial"),
                    expected_instance: None,
                    action: "bury".into(),
                },
            },
            &mut NoDraw,
        )
        .unwrap();
    assert!(world.characters[&actor()].runtime.pending_travel.is_none());
    assert_eq!(action(&engine, &world).animation.as_deref(), Some("827"));
    assert_eq!(world.characters[&actor()].tile, before.tile);
}

#[test]
fn real_wind_strike_remains_visible_for_its_source_clip_after_instant_launch() {
    let engine = source();
    let style = fixtures::id("style.magic.wind_strike");
    let (mut world, target) = combat_setup(&engine, &style);
    let character = world.characters.get_mut(&actor()).unwrap();
    character.runtime.combat.style = Some(fixtures::id("style.unarmed.punch"));
    for (slot, item) in [(0, "item.rune.air"), (1, "item.rune.mind")] {
        character.inventory.slots[slot] = Some(ItemStack {
            item: fixtures::id(item),
            quantity: Quantity::new(10).unwrap(),
            instance: None,
        });
    }
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::Cast {
                spell: "spell.wind_strike".into(),
                target: Some(target),
            },
            &mut engine_fixtures::v2::Hits(0),
        )
        .unwrap();
    let first = action(&engine, &world);
    assert_eq!(first.spell_id, Some(fixtures::id("spell.wind_strike")));
    assert_eq!(first.animation.as_deref(), Some("711"));
    tick(&engine, &mut world);
    assert_eq!(action(&engine, &world).id, first.id);
    assert_eq!(action(&engine, &world).cycle_started_at_tick, "0");
    tick(&engine, &mut world);
    assert_eq!(action(&engine, &world).id, first.id);
}

#[test]
fn food_overlay_does_not_replace_combat_state_or_retime_an_unexecuted_attack() {
    let engine = source();
    let style = fixtures::id("style.unarmed.punch");
    let (mut world, target) = combat_setup(&engine, &style);
    world.characters.get_mut(&actor()).unwrap().inventory.slots[0] = Some(ItemStack {
        item: fixtures::id("item.bread"),
        quantity: Quantity::new(1).unwrap(),
        instance: None,
    });
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &attack(&engine, &target),
            &mut engine_fixtures::v2::Hits(0),
        )
        .unwrap();
    let fight = action(&engine, &world);
    tick(&engine, &mut world);
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::Eat { inventory_slot: 0 },
            &mut NoDraw,
        )
        .unwrap();
    let overlay = action(&engine, &world);
    assert_ne!(overlay.id, fight.id);
    assert_eq!(overlay.animation.as_deref(), Some("12526"));
    assert_eq!(
        overlay.action_id.as_ref().unwrap().as_str(),
        "action.item.bread.eat"
    );
    assert!(matches!(
        world.characters[&actor()].activity,
        Activity::Fighting { .. }
    ));
    for _ in 0..3 {
        tick(&engine, &mut world);
    }
    let resumed = action(&engine, &world);
    assert_eq!(resumed.id, fight.id);
    assert_eq!(resumed.cycle_started_at_tick, fight.cycle_started_at_tick);
}

#[test]
fn selected_drink_overlay_preserves_the_executing_walk_and_independent_timers() {
    let engine = source();
    let mut world = world(&engine);
    let destination = world.characters[&actor()].tile.offset(2, 0).unwrap();
    world.characters.get_mut(&actor()).unwrap().inventory.slots[5] = Some(ItemStack {
        item: fixtures::id("item.energy_potion.one_dose"),
        quantity: Quantity::new(1).unwrap(),
        instance: None,
    });
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::Walk {
                destination,
                running: false,
            },
            &mut NoDraw,
        )
        .unwrap();
    tick(&engine, &mut world);
    let activity = world.characters[&actor()].activity.clone();
    let ready = world.characters[&actor()].runtime.combat.attack_ready;
    let food_ready = world.characters[&actor()].runtime.food_ready;
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::Ui {
                request: GameplayUiRequest::ItemAction {
                    inventory_slot: 5,
                    expected_item: fixtures::id("item.energy_potion.one_dose"),
                    expected_instance: None,
                    action: "drink".into(),
                },
            },
            &mut NoDraw,
        )
        .unwrap();
    assert_eq!(world.characters[&actor()].activity, activity);
    assert_eq!(
        world.characters[&actor()].runtime.combat.attack_ready,
        ready
    );
    assert_eq!(world.characters[&actor()].runtime.food_ready, food_ready);
    let observed = action(&engine, &world);
    assert_eq!(observed.animation.as_deref(), Some("829"));
    assert_eq!(
        observed.action_id.as_ref().unwrap().as_str(),
        "action.item.energy_potion.one_dose.drink"
    );
    tick(&engine, &mut world);
    assert_eq!(world.characters[&actor()].tile, destination);
    assert_eq!(action(&engine, &world).id, observed.id);
}

#[test]
fn source_milking_binds_2305_without_changing_the_click_recipe_deadline_or_output() {
    let engine = source();
    let mut world = world(&engine);
    let target: SpawnId = fixtures::id("spawn.dairy_cow.east.3253.3271.p0.t10.r2");
    let spawn = &engine.content().spawns[&target];
    let character = world.characters.get_mut(&actor()).unwrap();
    character.tile = spawn.tile.offset(-1, 0).unwrap();
    character.region = spawn.region.clone();
    character.inventory.slots[0] = Some(ItemStack {
        item: fixtures::id("item.bucket"),
        quantity: Quantity::new(1).unwrap(),
        instance: None,
    });
    let observed_target = WorldTarget::Spawn {
        spawn: target.clone(),
    };
    let bounds = engine
        .target_view(&world, &actor(), &observed_target)
        .unwrap()
        .expect("the actual source milking target is visible");
    let mut candidates = Vec::new();
    for offset in 0..i16::from(bounds.width) {
        candidates.extend(
            [
                bounds.tile.offset(offset, -1),
                bounds.tile.offset(offset, i16::from(bounds.height)),
            ]
            .into_iter()
            .flatten(),
        );
    }
    for offset in 0..i16::from(bounds.height) {
        candidates.extend(
            [
                bounds.tile.offset(-1, offset),
                bounds.tile.offset(i16::from(bounds.width), offset),
            ]
            .into_iter()
            .flatten(),
        );
    }
    let mut reachable = false;
    for tile in candidates {
        world.characters.get_mut(&actor()).unwrap().tile = tile;
        if engine
            .interaction_options(&world, &actor(), &observed_target)
            .unwrap()
            .iter()
            .any(|option| option.name == "Milk" && option.permission.allowed)
        {
            reachable = true;
            break;
        }
    }
    assert!(
        reachable,
        "the actual source milking fixture requires a permitted cardinal face"
    );
    engine
        .apply_intent(
            &mut world,
            &actor(),
            &GameIntent::Interact {
                target,
                action: "Milk".into(),
            },
            &mut NoDraw,
        )
        .unwrap();
    let observed = action(&engine, &world);
    assert_eq!(observed.recipe_id, Some(fixtures::id("recipe.cooks.milk")));
    assert_eq!(observed.animation.as_deref(), Some("2305"));
    assert_eq!(observed.next_action_tick.as_deref(), Some("3"));
    for _ in 0..3 {
        tick(&engine, &mut world);
    }
    assert_eq!(
        clubscape_simulation::inventory::count(
            &world.characters[&actor()].inventory,
            &engine.content().items,
            &fixtures::id("item.milk.bucket")
        )
        .unwrap(),
        1
    );
    let completed = action(&engine, &world);
    assert_eq!(completed.id, observed.id);
    assert_eq!(
        completed.cycle_started_at_tick, "0",
        "completion must not restart the original milking clip"
    );
}

#[test]
fn real_death_uses_its_source_life_phase_and_stops_at_actual_arrival() {
    let engine = source();
    let mut world = world(&engine);
    world.characters.get_mut(&actor()).unwrap().hitpoints = 0;
    tick(&engine, &mut world);
    let first = action(&engine, &world);
    assert_eq!(first.animation.as_deref(), Some("836"));
    let policy = engine
        .content()
        .mechanics
        .death
        .as_ref()
        .unwrap()
        .timing
        .require()
        .unwrap();
    for _ in 0..policy.dying_ticks + policy.respawn_ticks - 1 {
        tick(&engine, &mut world);
        let observed = action(&engine, &world);
        assert_eq!(observed.id, first.id);
        assert_eq!(observed.cycle_started_at_tick, first.cycle_started_at_tick);
        assert_eq!(observed.animation.as_deref(), Some("836"));
    }
    tick(&engine, &mut world);
    assert!(matches!(
        world.characters[&actor()].runtime.life,
        LifeState::Alive
    ));
    assert!(
        engine
            .actor_observer(&world, &actor())
            .unwrap()
            .action
            .is_none()
    );
}
