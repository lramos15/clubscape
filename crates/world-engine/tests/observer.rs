mod support;

use clubscape_game_types::*;
use support::{v2 as v, *};

#[test]
fn actual_final_run_step_survives_idle_activity_and_energy_exhaustion() {
    let mut content = v::content();
    v::with_run(&mut content);
    content.initial_state.run_energy = 100;
    content.initial_state.runtime.settings.run_enabled = Some(true);
    let (engine, mut world) = setup(content);
    apply(
        &engine,
        &mut world,
        GameIntent::Walk {
            destination: tile(14, 10, 0),
            running: false,
        },
    );
    assert!(
        !engine.actor_observer(&world, &actor()).unwrap().running,
        "queued intent is not an executed move"
    );
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert!(engine.actor_observer(&world, &actor()).unwrap().running);
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert_eq!(state(&world).tile, tile(14, 10, 0));
    assert_eq!(state(&world).run_energy, 0);
    assert_eq!(state(&world).runtime.settings.run_enabled, Some(false));
    assert!(matches!(state(&world).activity, Activity::Idle));
    let view = engine.actor_observer(&world, &actor()).unwrap();
    assert!(view.running);
    assert_eq!(view.movement_tick, Some("2".into()));
    let before = world.clone();
    assert_eq!(engine.actor_observer(&world, &actor()).unwrap(), view);
    assert_eq!(world, before);
    world = serde_json::from_slice(&serde_json::to_vec(&world).unwrap()).unwrap();
    assert!(engine.actor_observer(&world, &actor()).unwrap().running);
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert!(!engine.actor_observer(&world, &actor()).unwrap().running);
    assert!(
        engine
            .actor_observer(&world, &actor())
            .unwrap()
            .movement_tick
            .is_none()
    );
}

#[test]
fn single_step_and_checkbox_only_state_never_claim_running() {
    let mut content = v::content();
    v::with_run(&mut content);
    content.initial_state.runtime.settings.run_enabled = Some(true);
    let (engine, mut world) = setup(content);
    assert!(!engine.actor_observer(&world, &actor()).unwrap().running);
    apply(
        &engine,
        &mut world,
        GameIntent::Walk {
            destination: tile(11, 10, 0),
            running: true,
        },
    );
    v::tick(&engine, &mut world, &mut NeverDraw);
    let view = engine.actor_observer(&world, &actor()).unwrap();
    assert!(!view.running);
    assert_eq!(view.movement_tick, Some("1".into()));
    assert_eq!(state(&world).runtime.settings.run_enabled, Some(true));
}

#[test]
fn gathering_observer_uses_exact_saved_target_and_keeps_instance_and_cycle_correlation() {
    let mut content = v::content();
    v::typed_gather(&mut content);
    gather_rule(&mut content).animation =
        Some(AssetId::new("asset.synthetic.exact_mining").unwrap());
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, interact("rock"));
    let first = engine
        .actor_observer(&world, &actor())
        .unwrap()
        .action
        .unwrap();
    assert_eq!(first.action_id, Some(v::method("mining")));
    assert_eq!(
        first.target,
        Some(WorldTarget::Spawn {
            spawn: spawn("rock")
        })
    );
    assert_eq!(
        first.animation.as_deref(),
        Some("asset.synthetic.exact_mining")
    );
    assert_eq!(first.started_at_tick, "0");
    v::tick(&engine, &mut world, &mut NeverDraw);
    let before = world.clone();
    let second = engine
        .actor_observer(&world, &actor())
        .unwrap()
        .action
        .unwrap();
    assert_eq!(second.id, first.id);
    assert_eq!(second.cycle_started_at_tick, "0");
    assert_eq!(world, before);
    world = serde_json::from_slice(&serde_json::to_vec(&world).unwrap()).unwrap();
    assert_eq!(
        engine
            .actor_observer(&world, &actor())
            .unwrap()
            .action
            .unwrap()
            .id,
        first.id
    );
    apply(&engine, &mut world, GameIntent::CancelActivity);
    assert!(
        engine
            .actor_observer(&world, &actor())
            .unwrap()
            .action
            .is_none()
    );
}

#[test]
fn completed_production_has_exact_recipe_and_no_fabricated_neighbour_target() {
    let mut content = v::content();
    v::typed_recipe(&mut content, "bronze", v::cadence(1, 3, 5));
    give_initial(&mut content, &[stack("ore", 2), stack("tin", 2)]);
    let (engine, mut world) = setup(content);
    apply(&engine, &mut world, produce("bronze", Some("furnace"), 2));
    let first = engine
        .actor_observer(&world, &actor())
        .unwrap()
        .action
        .unwrap();
    assert_eq!(first.recipe_id, Some(recipe("bronze")));
    assert_eq!(
        first.target,
        Some(WorldTarget::Spawn {
            spawn: spawn("furnace")
        })
    );
    v::tick_n(&engine, &mut world, 3, &mut NeverDraw);
    let next = engine
        .actor_observer(&world, &actor())
        .unwrap()
        .action
        .unwrap();
    assert_eq!(next.id, first.id);
    assert_eq!(next.cycle_started_at_tick, "3");
    assert_eq!(next.next_action_tick.as_deref(), Some("8"));
    v::tick_n(&engine, &mut world, 5, &mut NeverDraw);
    assert!(matches!(state(&world).activity, Activity::Idle));
    let completed = engine
        .actor_observer(&world, &actor())
        .unwrap()
        .action
        .unwrap();
    assert_eq!(completed.id, first.id);
    assert_eq!(
        completed.cycle_started_at_tick, "3",
        "completion does not restart the last source action cycle"
    );
    v::tick(&engine, &mut world, &mut NeverDraw);
    assert!(
        engine
            .actor_observer(&world, &actor())
            .unwrap()
            .action
            .is_none()
    );
}

#[test]
fn malformed_future_observation_is_not_a_successful_default() {
    let (engine, mut world) = setup(v::content());
    world
        .characters
        .get_mut(&actor())
        .unwrap()
        .runtime
        .observation = Some(ActorObservation {
        movement: Some(MovementObservation {
            tick: 1,
            from: tile(10, 10, 0),
            to: tile(11, 10, 0),
            instance: None,
            running: true,
        }),
        ..ActorObservation::default()
    });
    assert!(engine.actor_observer(&world, &actor()).is_err());
}

#[test]
fn scene_observer_uses_actual_template_and_preserves_opaque_identity_and_private_ownership() {
    let mut content = v::content();
    v::with_death(&mut content);
    let (engine, mut world) = setup(content);
    let ordinary = engine.scene_view(&world, &actor()).unwrap();
    assert!(ordinary.instance.is_none() && ordinary.instance_template.is_none());
    let instance = InstanceId::new("instance.opaque.live_session").unwrap();
    world.runtime.instances.insert(
        instance.clone(),
        InstanceState {
            template: v::template(),
            owner: Some(actor()),
            counters: Default::default(),
            entities: Default::default(),
            object_states: Default::default(),
        },
    );
    world.characters.get_mut(&actor()).unwrap().runtime.instance = Some(instance.clone());
    let before = world.clone();
    let scene = engine.scene_view(&world, &actor()).unwrap();
    assert_eq!(scene.instance, Some(instance.clone()));
    assert_eq!(scene.instance_template, Some(v::template()));
    assert_eq!(world, before);
    world.runtime.instances.get_mut(&instance).unwrap().owner =
        Some(ActorId::new("actor.other").unwrap());
    assert_eq!(
        engine.scene_view(&world, &actor()).unwrap_err().code,
        GameErrorCode::NotOwned
    );
}
