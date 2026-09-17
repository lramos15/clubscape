use super::*;

#[test]
fn source_animation_projection_uses_exact_method_recipe_and_explicit_binding_not_neighbours() {
    let mut content = fixtures::fixture();
    let sources: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../research/interface-contracts/actor-observer-bindings.json"
    )))
    .unwrap();
    content.baseline = sources["baseline"].as_str().unwrap().into();
    let action = |method: &str, recipe: Option<&str>, animation: Option<&str>| ActorObserverView {
        running: false,
        movement_tick: None,
        action: Some(ActorActionView {
            version: 1,
            id: "observed.test.1".into(),
            activity: "producing".into(),
            action_id: Some(fixtures::id(method)),
            target: None,
            recipe_id: recipe.map(fixtures::id),
            style_id: None,
            spell_id: None,
            animation: animation.map(str::to_owned),
            started_at_tick: "9007199254740993".into(),
            cycle_started_at_tick: "9007199254740993".into(),
            next_action_tick: Some("9007199254740994".into()),
            observed_at_tick: "9007199254740993".into(),
        }),
    };
    for (method, recipe, expected) in [
        ("action.fishing.shrimps", None, "621"),
        ("action.mining.copper", None, "625"),
        (
            "action.cooking.shrimps",
            Some("recipe.cooking.shrimps.fire"),
            "897",
        ),
        (
            "action.cooking.shrimps",
            Some("recipe.cooking.shrimps.range"),
            "896",
        ),
        (
            "action.smithing.bronze_dagger",
            Some("recipe.smithing.bronze_dagger"),
            "898",
        ),
    ] {
        let (animation, observed) =
            super::super::observer::project(&content, action(method, recipe, None)).unwrap();
        assert_eq!(animation, expected);
        assert_eq!(
            observed.action.as_ref().unwrap().animation.as_deref(),
            Some(expected)
        );
        assert_eq!(
            super::super::observer::action(observed.action.unwrap()).started_at_tick,
            "9007199254740993"
        );
    }
    let (explicit, _) = super::super::observer::project(
        &content,
        action(
            "action.fishing.shrimps",
            None,
            Some("asset.source.explicit"),
        ),
    )
    .unwrap();
    assert_eq!(explicit, "asset.source.explicit");
    let (unknown, observed) = super::super::observer::project(
        &content,
        action("action.cooking.dough", Some("recipe.cooking.dough"), None),
    )
    .unwrap();
    assert_eq!(unknown, "recipe.cooking.dough");
    assert!(
        observed.action.unwrap().animation.is_none(),
        "never choose firemaking for inventory dough"
    );
    for (running, expected) in [(false, "819"), (true, "824")] {
        let (animation, _) = super::super::observer::project(
            &content,
            ActorObserverView {
                running,
                movement_tick: Some("4".into()),
                action: None,
            },
        )
        .unwrap();
        assert_eq!(animation, expected);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn real_socket_observers_preserve_actual_run_movement_and_private_action_correlation() {
    let database = Database::reset().await;
    let mut pack = Pack::ui();
    let mut movement = engine_fixtures::v2::content();
    engine_fixtures::v2::with_run(&mut movement);
    let mut run = movement.mechanics.run.unwrap();
    run.agility = fixtures::id("skill.test.mining");
    run.levels.maximum = u16::try_from(
        pack.definition.skills[&run.agility]
            .xp_thresholds_tenths
            .len(),
    )
    .unwrap();
    pack.definition.mechanics.run = Some(run);
    pack.definition.initial_state.run_energy = 100;
    pack.definition.initial_state.runtime.settings.run_enabled = Some(true);
    let InteractionAction::Gather { rule } = &mut pack
        .definition
        .spawns
        .get_mut(&fixtures::id("spawn.test.rock"))
        .unwrap()
        .interactions[0]
        .action
    else {
        panic!()
    };
    rule.animation = Some(fixtures::id("asset.test.mining_animation"));
    let mut serialized = serde_json::to_value(&pack.definition).unwrap();
    normalize_fixture_sources(&mut serialized);
    pack.definition = serde_json::from_value(serialized).unwrap();
    pack.write();
    let service = Live::start(pack.config(&database)).await;
    let account = service.endpoint.account("observer_actor").await;
    let actor = service.endpoint.create(&account).await;
    let joined = service.endpoint.join(&account).await;
    let watcher = service.endpoint.account("observer_watcher").await;
    service.endpoint.create(&watcher).await;
    let watching = service.endpoint.join(&watcher).await;
    assert_eq!(
        joined
            .snapshot
            .as_ref()
            .unwrap()
            .player
            .as_ref()
            .unwrap()
            .running,
        Some(false)
    );
    service
        .endpoint
        .input(
            &account,
            &joined,
            1,
            Uuid::new_v4(),
            game::world_input::Action::Walk(game::Walk {
                destination: Some(game::Tile {
                    x: 1002,
                    y: 1005,
                    plane: 0,
                }),
                running: false,
            }),
        )
        .await
        .action();
    let (moved, visible) = timeout(WAIT, async {
        loop {
            let moved = service.endpoint.poll(&account, &joined, 0).await.snapshot();
            if moved.player.as_ref().unwrap().movement_tick.is_some() {
                let visible = service
                    .endpoint
                    .poll(&watcher, &watching, 0)
                    .await
                    .snapshot();
                break (moved, visible);
            }
            sleep(Duration::from_millis(40)).await;
        }
    })
    .await
    .unwrap();
    let player = moved.player.as_ref().unwrap();
    assert_eq!(player.running, Some(true));
    assert_eq!(
        player.activity, "idle",
        "the last run step is observed after the path finishes"
    );
    let other = visible
        .entities
        .iter()
        .find(|entity| entity.id == actor)
        .unwrap();
    if visible.tick == moved.tick {
        assert_eq!(other.running, Some(true));
    }
    assert!(
        other.equipment.is_empty(),
        "observer metadata must not introduce private inventory/equipment payloads"
    );
    let still = timeout(WAIT, async {
        loop {
            let view = service.endpoint.poll(&account, &joined, 0).await.snapshot();
            if view.tick > moved.tick {
                break view;
            }
            sleep(Duration::from_millis(40)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(still.player.as_ref().unwrap().running, Some(false));
    service
        .endpoint
        .input(
            &account,
            &joined,
            2,
            Uuid::new_v4(),
            game::world_input::Action::Walk(game::Walk {
                destination: Some(game::Tile {
                    x: 1002,
                    y: 1003,
                    plane: 0,
                }),
                running: false,
            }),
        )
        .await
        .action();
    timeout(WAIT, async {
        loop {
            let view = service.endpoint.poll(&account, &joined, 0).await.snapshot();
            if view.player.as_ref().unwrap().tile.as_ref().unwrap().x == 1002
                && view.player.as_ref().unwrap().tile.as_ref().unwrap().y == 1003
            {
                break;
            }
            sleep(Duration::from_millis(40)).await;
        }
    })
    .await
    .unwrap();
    service
        .endpoint
        .input(&account, &joined, 3, Uuid::new_v4(), mine())
        .await
        .action();
    let active = service.endpoint.poll(&account, &joined, 0).await.snapshot();
    let observed = active
        .player
        .as_ref()
        .unwrap()
        .action
        .as_ref()
        .unwrap()
        .clone();
    assert_eq!(
        observed.target.as_ref().unwrap().target,
        Some(game::world_target::Target::Spawn("spawn.test.rock".into()))
    );
    assert!(!active.player.as_ref().unwrap().animation.is_empty());
    let again = service
        .endpoint
        .poll(&account, &joined, active.revision)
        .await
        .snapshot();
    assert_eq!(
        again.player.as_ref().unwrap().action.as_ref().unwrap().id,
        observed.id
    );
    let rejoined = service.endpoint.join(&account).await;
    assert_eq!(
        rejoined
            .snapshot
            .as_ref()
            .unwrap()
            .player
            .as_ref()
            .unwrap()
            .action
            .as_ref()
            .unwrap()
            .id,
        observed.id,
        "same live-owner reconnect does not restart an already-correlated action"
    );
    let stored = database.world(pack.world_id).await;
    assert!(
        stored.state.characters[&ActorId::new(actor).unwrap()]
            .runtime
            .observation
            .as_ref()
            .unwrap()
            .action
            .is_some()
    );
    service.stop().await.unwrap();
    database.pool.close().await;
}
