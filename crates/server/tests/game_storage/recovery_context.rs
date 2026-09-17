use super::*;
use clubscape_game_types::GameplayUiRequest;
use clubscape_world_engine::{LifecycleTransition, WorldEngine};

#[path = "../../../world-engine/tests/support/recovery_context.rs"]
mod native;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn whole_context_recovery_is_one_durable_operation_across_records_restart_and_auth_renewal() {
    let database = TestDatabase::reset().await;
    let (engine, mut controlled) = native::setup(native::definition(), 1000);
    let first = native::add_record(
        &engine,
        &mut controlled,
        "one",
        vec![native::entry("one", "arrow", 7, 840)],
    );
    let second = native::add_record(
        &engine,
        &mut controlled,
        "two",
        vec![native::entry("two", "arrow", 7, 880)],
    );
    native::next(&engine, &mut controlled);
    let observed = native::view(&engine, &controlled);
    let request = GameplayUiRequest::RecoveryTakeAll {
        selection: observed.take_all.selection.clone(),
    };
    let mut expected = controlled.clone();
    native::apply(&engine, &mut expected, request.clone()).unwrap();
    let expected_character = expected.characters[&native::source::actor()].clone();
    let engine = Arc::new(engine);
    let world_id = Uuid::new_v4();
    database
        .store
        .initialize_world(world_id, engine.initial_world().unwrap())
        .await
        .unwrap();
    let lease = database
        .store
        .acquire_world_lease(world_id, MAX_WORLD_LEASE)
        .await
        .unwrap();
    let account = database.account("recovery_context").await;
    let create_engine = engine.clone();
    let character = database
        .store
        .create_character_with(
            &lease,
            account.authentication(),
            SourceCharacter {
                content_revision: engine.content().revision.clone(),
                initial_state: engine.content().initial_state.clone(),
                appearance: BTreeMap::new(),
            },
            move |character| {
                *character = create_engine.character_from_initial_at_tick(
                    character.actor_id.clone(),
                    character.display_name.clone(),
                    character.appearance.clone(),
                    character.last_action_tick,
                )?;
                Ok(())
            },
        )
        .await
        .unwrap();
    let actor = character.state.actor_id;
    let session = database
        .store
        .join_session(
            world_id,
            actor.clone(),
            account.authentication(),
            MAX_SESSION_LEASE,
        )
        .await
        .unwrap();
    let bootstrap_actor = actor.clone();
    let bootstrap_engine = engine.clone();
    database
        .store
        .commit_tick(&lease, 0, move |world| {
            // Explicit storage/component fixture, not an account journey or client state setter.
            for instance in controlled.runtime.instances.values_mut() {
                instance.owner = Some(bootstrap_actor.clone());
            }
            for record in controlled.runtime.deaths.values_mut() {
                record.owner = bootstrap_actor.clone();
            }
            world.runtime.instances = controlled.runtime.instances;
            world.runtime.deaths = controlled.runtime.deaths;
            let prepared = &controlled.characters[&native::source::actor()];
            let character = world.characters.get_mut(&bootstrap_actor).unwrap();
            character.region = prepared.region.clone();
            character.tile = prepared.tile;
            character.runtime.instance = prepared.runtime.instance.clone();
            character.runtime.death_coffer = prepared.runtime.death_coffer;
            bootstrap_engine.apply_lifecycle(world, &bootstrap_actor, LifecycleTransition::Join)?;
            bootstrap_engine
                .apply_intent(
                    world,
                    &bootstrap_actor,
                    &GameIntent::OpenDeathOffice,
                    &mut native::NoRandom,
                )
                .map(|events| events.into_iter().map(|event| event.event).collect())
        })
        .await
        .unwrap();
    let tick_engine = engine.clone();
    database
        .store
        .commit_tick(&lease, 1, move |world| {
            let context = tick_engine.tick_context(world)?;
            tick_engine
                .process_advanced_tick_with_context(world, &mut native::NoRandom, &context)
                .map(|events| events.into_iter().map(|event| event.event).collect())
        })
        .await
        .unwrap();
    let before = database.store.load_world(world_id).await.unwrap();
    let context = engine
        .ui_view(&before.state, &actor)
        .unwrap()
        .recovery
        .unwrap()
        .management
        .unwrap()
        .context
        .unwrap();
    assert_eq!(context.take_all.selection, observed.take_all.selection);
    assert_eq!(context.take_all.plan.unwrap().total_fee, "602");
    let mut stale_selection = observed.take_all.selection.clone();
    stale_selection.records[1].entries[0].quantity = Quantity::new(6).unwrap();
    let invalid_engine = engine.clone();
    let rejected = database
        .store
        .commit_command(
            &lease,
            &session.access(account.authentication()),
            GameCommand {
                operation_id: Uuid::new_v4(),
                sequence: 1,
                intent: GameIntent::Ui {
                    request: GameplayUiRequest::RecoveryTakeAll {
                        selection: stale_selection,
                    },
                },
            },
            move |world, actor, intent| {
                invalid_engine
                    .apply_intent(world, actor, intent, &mut native::NoRandom)
                    .map(|events| events.into_iter().map(|event| event.event).collect())
            },
        )
        .await
        .unwrap_err();
    assert_conflict(rejected);
    assert_eq!(database.journal_count().await, 0);
    assert_eq!(database.store.load_world(world_id).await.unwrap(), before);

    let wire = clubscape_protocol::game::WorldInput {
        world_session_id: session.session_id.to_string(),
        sequence: 1,
        expected_character_revision: None,
        action: Some(clubscape_protocol::game::world_input::Action::Ui(
            clubscape_protocol::game::GameplayUiRequest {
                expected_bank_revision: None,
                request: Some(
                    clubscape_protocol::game::gameplay_ui_request::Request::RecoveryTakeAll(
                        clubscape_protocol::game::UiRecoveryTakeAll {
                            selection: Some(
                                clubscape_protocol::recovery_context_selection_to_wire(
                                    observed.take_all.selection.clone(),
                                ),
                            ),
                        },
                    ),
                ),
            },
        )),
    };
    let operation = GameCommand {
        operation_id: Uuid::new_v4(),
        sequence: 1,
        intent: clubscape_protocol::game_intent(&wire).unwrap(),
    };
    assert_eq!(operation.intent, GameIntent::Ui { request });
    let apply_engine = engine.clone();
    let committed = database
        .store
        .commit_command(
            &lease,
            &session.access(account.authentication()),
            operation.clone(),
            move |world, actor, intent| {
                apply_engine
                    .apply_intent(world, actor, intent, &mut native::NoRandom)
                    .map(|events| events.into_iter().map(|event| event.event).collect())
            },
        )
        .await
        .unwrap();
    assert!(!committed.duplicate);
    assert_eq!(
        committed.receipt.character.inventory,
        expected_character.inventory
    );
    assert_eq!(committed.receipt.character.bank, expected_character.bank);
    assert_eq!(committed.receipt.character.runtime.death_coffer, 398);
    let receipts: Vec<_> = committed
        .receipt
        .events
        .iter()
        .filter_map(|event| match event {
            GameEvent::RecoveryCompleted { death, fee, .. } => Some((death.clone(), *fee)),
            _ => None,
        })
        .collect();
    assert_eq!(receipts, vec![(first.clone(), 294), (second.clone(), 308)]);
    assert_eq!(database.journal_count().await, 1);
    let durable = database.store.load_world(world_id).await.unwrap();
    for death in [&first, &second] {
        assert!(durable.state.runtime.deaths[death].office.is_empty());
        assert_eq!(durable.state.runtime.deaths[death].reclaimed.len(), 1);
    }
    database
        .store
        .leave_session(&session.access(account.authentication()))
        .await
        .unwrap();
    let renewal = database
        .service
        .login(&account.name, &account.password)
        .await;
    let auth = AuthTokenDigest::from_token(&renewal.session_token).unwrap();
    let restarted = GameStore::new(open_pool(database.options.clone(), 4).await);
    let renewed_session = restarted
        .join_session(world_id, actor.clone(), auth, MAX_SESSION_LEASE)
        .await
        .unwrap();
    assert_ne!(renewed_session.session_id, session.session_id);
    let reloaded = restarted.load_world(world_id).await.unwrap();
    let restarted_engine = WorldEngine::new(Arc::new(engine.content().clone())).unwrap();
    let empty = restarted_engine
        .ui_view(&reloaded.state, &actor)
        .unwrap()
        .recovery
        .unwrap()
        .management
        .unwrap()
        .context
        .unwrap();
    assert!(empty.slots.is_empty() && !empty.take_all.permission.allowed);
    assert_conflict(
        restarted
            .commit_command(
                &lease,
                &renewed_session.access(auth),
                operation.clone(),
                |_, _, _| panic!("A different repository cannot reuse the old world lease"),
            )
            .await
            .unwrap_err(),
    );
    database.store.release_world_lease(&lease).await.unwrap();
    let replacement = restarted
        .acquire_world_lease(world_id, MAX_WORLD_LEASE)
        .await
        .unwrap();
    assert!(replacement.fence > lease.fence);
    let duplicate = restarted
        .commit_command(
            &replacement,
            &renewed_session.access(auth),
            operation,
            |_, _, _| {
                panic!("Committed multi-record recovery must not rerun after restart or renewal")
            },
        )
        .await
        .unwrap();
    assert!(duplicate.duplicate);
    assert_eq!(duplicate.receipt, committed.receipt);
    assert_eq!(restarted.load_world(world_id).await.unwrap(), durable);
    assert_eq!(database.journal_count().await, 1);
    restarted.release_world_lease(&replacement).await.unwrap();
    restarted.close().await.unwrap();
    database.stop().await;
}
