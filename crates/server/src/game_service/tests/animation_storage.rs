use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn source_channel_animation_survives_repository_reload_and_duplicate_receipts_do_not_restart_it()
 {
    let database = Database::reset().await;
    let accounts = Live::start(Config::new(&database.url, "127.0.0.1:0", None).unwrap()).await;
    let account = accounts.endpoint.account("animation_history").await;
    accounts.stop().await.unwrap();
    let engine = animation_authority::source();
    let world_id = Uuid::new_v4();
    let store = GameStore::new(database.pool.clone());
    store
        .initialize_world(world_id, engine.initial_world().unwrap())
        .await
        .unwrap();
    let lease = store
        .acquire_world_lease(world_id, OWNER_LEASE)
        .await
        .unwrap();
    let authentication = AuthTokenDigest::from_token(&account.token).unwrap();
    let create_engine = engine.clone();
    let created = store
        .create_character_with(
            &lease,
            authentication,
            SourceCharacter {
                content_revision: engine.content().revision.clone(),
                initial_state: engine.content().initial_state.clone(),
                appearance: BTreeMap::new(),
            },
            move |character| {
                let derived = create_engine.character_from_initial_at_tick(
                    character.actor_id.clone(),
                    character.display_name.clone(),
                    character.appearance.clone(),
                    character.last_action_tick,
                )?;
                character.runtime = derived.runtime;
                character.tutorial_stage = fixtures::id("stage.tutorial.teleport_channel");
                character.runtime.settings.experience = Some(fixtures::id("experience.brand_new"));
                character.runtime.counters.insert(
                    fixtures::id("counter.tutorial.departure_authorized"),
                    CounterValue::Boolean(true),
                );
                character.interfaces = create_engine.content().interfaces.keys().cloned().collect();
                for slot in [5, 6] {
                    character.inventory.slots[slot] = Some(ItemStack {
                        item: fixtures::id("item.bones.tutorial"),
                        quantity: Quantity::new(1)?,
                        instance: None,
                    });
                }
                Ok(())
            },
        )
        .await
        .unwrap();
    let actor = created.state.actor_id.clone();
    let lifecycle_engine = engine.clone();
    let joining_actor = actor.clone();
    store
        .control_world(&lease, move |world| {
            lifecycle_engine.apply_lifecycle(world, &joining_actor, LifecycleTransition::Join)
        })
        .await
        .unwrap();
    let session = store
        .join_session(world_id, actor.clone(), authentication, PLAYER_LEASE)
        .await
        .unwrap();
    let access = session.access(authentication);
    let command = GameCommand {
        operation_id: Uuid::new_v4(),
        sequence: 1,
        intent: GameIntent::Cast {
            spell: "spell.lumbridge_home_teleport".into(),
            target: None,
        },
    };
    let cast_engine = engine.clone();
    let first = store
        .commit_routed_command(
            &lease,
            &access,
            command.clone(),
            None,
            move |world, actor, intent| {
                cast_engine.apply_intent(world, actor, intent, &mut engine_fixtures::v2::Hits(0))
            },
        )
        .await
        .unwrap();
    let initial = engine
        .actor_observer(&first.snapshot.state, &actor)
        .unwrap()
        .action
        .unwrap();
    assert_eq!(initial.animation.as_deref(), Some("4847"));
    let mut current = first.snapshot;
    for _ in 0..6 {
        let tick_engine = engine.clone();
        current = store
            .commit_routed_tick(
                &lease,
                current.state.tick,
                vec![access.clone()],
                move |world| {
                    let context = tick_engine.tick_context(world)?;
                    tick_engine.process_advanced_tick_with_context(
                        world,
                        &mut engine_fixtures::v2::Hits(0),
                        &context,
                    )
                },
            )
            .await
            .unwrap()
            .snapshot;
    }
    store.release_world_lease(&lease).await.unwrap();
    let restarted = GameStore::new(database.pool.clone());
    let lease = restarted
        .acquire_world_lease(world_id, OWNER_LEASE)
        .await
        .unwrap();
    let loaded = restarted.load_world(world_id).await.unwrap();
    let phase = engine
        .actor_observer(&loaded.state, &actor)
        .unwrap()
        .action
        .unwrap();
    assert_eq!(phase.id, initial.id);
    assert_eq!(phase.started_at_tick, "0");
    assert_eq!(phase.cycle_started_at_tick, "6");
    assert_eq!(phase.animation.as_deref(), Some("4850"));
    assert!(
        restarted
            .commit_routed_command(&lease, &access, command, None, |_, _, _| panic!(
                "duplicate animation action must not execute again"
            ))
            .await
            .unwrap()
            .commit
            .duplicate
    );
    let after = restarted.load_world(world_id).await.unwrap();
    assert_eq!(
        engine
            .actor_observer(&after.state, &actor)
            .unwrap()
            .action
            .unwrap(),
        phase
    );
    let bury_engine = engine.clone();
    let burial = restarted
        .commit_routed_command(
            &lease,
            &access,
            GameCommand {
                operation_id: Uuid::new_v4(),
                sequence: 2,
                intent: GameIntent::Ui {
                    request: GameplayUiRequest::ItemAction {
                        inventory_slot: 5,
                        expected_item: fixtures::id("item.bones.tutorial"),
                        expected_instance: None,
                        action: "bury".into(),
                    },
                },
            },
            None,
            move |world, actor, intent| {
                bury_engine.apply_intent(world, actor, intent, &mut engine_fixtures::v2::Hits(0))
            },
        )
        .await
        .unwrap();
    let action = engine
        .actor_observer(&burial.snapshot.state, &actor)
        .unwrap()
        .action
        .unwrap();
    assert_ne!(action.id, phase.id);
    assert_eq!(action.animation.as_deref(), Some("827"));
    assert!(
        burial.snapshot.state.characters[&actor]
            .runtime
            .pending_travel
            .is_none()
    );
    assert!(burial.snapshot.state.characters[&actor].inventory.slots[6].is_some());
    restarted.release_world_lease(&lease).await.unwrap();
    database.pool.close().await;
}
