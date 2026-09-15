use super::*;

struct NoDraw;
impl clubscape_world_engine::RandomSource for NoDraw {
    fn draw_below(&mut self, _: u32) -> GameResult<u32> {
        panic!("audio authority must not draw gameplay RNG")
    }
}

fn counter() -> CounterId {
    fixtures::id("counter.test.audio_visit")
}

fn audio_pack() -> Pack {
    let mut pack = Pack::ui();
    let mut movement = engine_fixtures::v2::content();
    engine_fixtures::v2::with_run(&mut movement);
    let mut run = movement.mechanics.run.unwrap();
    run.agility = fixtures::id("skill.test.mining");
    run.levels.maximum = pack.definition.skills[&run.agility]
        .xp_thresholds_tenths
        .len() as u16;
    pack.definition.mechanics.run = Some(run);
    pack.definition.initial_state.runtime.settings.run_enabled = Some(false);
    pack.definition.mechanics.counters.insert(
        counter(),
        CounterDefinition {
            id: counter(),
            scope: CounterScope::Character,
            value_type: CounterType::Boolean,
            initial: CounterValue::Boolean(false),
            source_variable: None,
            source: fixtures::sources(),
        },
    );
    pack.definition
        .initial_state
        .runtime
        .counters
        .insert(counter(), CounterValue::Boolean(false));
    pack.definition
        .spawns
        .get_mut(&fixtures::id("spawn.test.guide"))
        .unwrap()
        .interactions
        .push(InteractionDefinition {
            name: "Remember".into(),
            reach: 4,
            guard: Guard::Always,
            action: InteractionAction::Effects {
                effects: vec![Effect::SetCounter {
                    counter: counter(),
                    value: CounterValue::Boolean(true),
                }],
            },
        });
    let area = |x, y| MusicAreaDefinition {
        plane: 0,
        polygons: vec![vec![
            [x * 2 - 1, y * 2 - 1],
            [x * 2 + 1, y * 2 - 1],
            [x * 2 + 1, y * 2 + 1],
            [x * 2 - 1, y * 2 + 1],
        ]],
        source: fixtures::sources(),
    };
    let predicate = |value| Guard::Counter {
        counter: counter(),
        predicate: CounterPredicate::Equals {
            value: CounterValue::Boolean(value),
        },
    };
    pack.definition.ui.as_mut().unwrap().audio_authority = Some(AudioAuthorityDefinition {
        version: 1,
        profile: "source_audio.synthetic".into(),
        areas: BTreeMap::from([
            ("strip".into(), area(1002, 1004)),
            ("east".into(), area(1004, 1003)),
        ]),
        tracks: [
            (62, true, None, None),
            (144, false, Some("strip"), Some(counter())),
            (76, false, Some("east"), None),
        ]
        .into_iter()
        .map(|(group, automatic, area, conserved)| {
            (
                group,
                MusicTrackDefinition {
                    group,
                    name: format!("Synthetic source {group}"),
                    automatic,
                    area: area.map(str::to_owned),
                    conserved,
                    source: fixtures::sources(),
                },
            )
        })
        .collect(),
        varps: BTreeMap::from([(
            491,
            NativeVarpDefinition {
                binding: "source_varp.synthetic_actual_fact".into(),
                fields: vec![NativeVarpField {
                    lsb: 2,
                    width: 1,
                    cases: vec![
                        NativeVarpCase {
                            guard: predicate(false),
                            value: 0,
                        },
                        NativeVarpCase {
                            guard: predicate(true),
                            value: 1,
                        },
                    ],
                }],
                source: fixtures::sources(),
            },
        )]),
        source: fixtures::sources(),
    });
    let mut serialized = serde_json::to_value(&pack.definition).unwrap();
    normalize_fixture_sources(&mut serialized);
    pack.definition = serde_json::from_value(serialized).unwrap();
    pack.write();
    pack
}

fn setup(pack: &Pack) -> (WorldEngine, WorldState, ActorId) {
    let engine = WorldEngine::new(Arc::new(pack.definition.clone())).unwrap();
    let actor: ActorId = fixtures::id("actor.audio.owner");
    let mut world = engine.initial_world().unwrap();
    world.characters.insert(
        actor.clone(),
        engine
            .character_from_initial(actor.clone(), "Owner", BTreeMap::new())
            .unwrap(),
    );
    engine
        .apply_lifecycle(&mut world, &actor, LifecycleTransition::Join)
        .unwrap();
    (engine, world, actor)
}

fn step(engine: &WorldEngine, world: &mut WorldState) {
    let context = engine.tick_context(world).unwrap();
    engine
        .tick_with_context(world, &mut NoDraw, &context)
        .unwrap();
}

fn remember() -> GameIntent {
    GameIntent::Interact {
        target: fixtures::id("spawn.test.guide"),
        action: "Remember".into(),
    }
}

#[test]
fn fresh_source_authority_is_private_immutable_and_not_all_unlocked() {
    let pack = audio_pack();
    let (engine, mut world, actor) = setup(&pack);
    let before = world.clone();
    let view = engine.audio_authority_view(&world, &actor).unwrap();
    assert_eq!(view.music.unlocked_groups, vec![62]);
    assert_eq!(view.music.history, MusicHistoryStatus::FromCreation);
    assert!(view.music.complete);
    assert!(!view.music.tracks.iter().any(|track| track.group == 0));
    assert!(
        view.music
            .tracks
            .iter()
            .filter(|track| track.group != 62)
            .all(|track| track.status == MusicUnlockStatus::Locked)
    );
    assert_eq!(view.varps[0].value, Some(0));
    assert_eq!(view.varps[0].known_bits, 4);
    assert_eq!(world, before);
    engine
        .apply_intent(&mut world, &actor, &remember(), &mut NoDraw)
        .unwrap();
    let changed = engine.audio_authority_view(&world, &actor).unwrap();
    assert_eq!(changed.music.unlocked_groups, vec![62, 144]);
    assert_eq!(changed.varps[0].value, Some(4));
    let other: ActorId = fixtures::id("actor.audio.other");
    world.characters.insert(
        other.clone(),
        engine
            .character_from_initial(other.clone(), "Other", BTreeMap::new())
            .unwrap(),
    );
    assert_eq!(
        engine
            .audio_authority_view(&world, &other)
            .unwrap()
            .music
            .unlocked_groups,
        vec![62]
    );
    assert_eq!(
        engine.audio_authority_view(&world, &other).unwrap().varps[0].value,
        Some(0)
    );
}

#[test]
fn a_queued_run_does_not_unlock_but_its_intermediate_actual_step_does() {
    let pack = audio_pack();
    let (engine, mut world, actor) = setup(&pack);
    engine
        .apply_intent(
            &mut world,
            &actor,
            &GameIntent::Walk {
                destination: fixtures::tile(1002, 1005),
                running: true,
            },
            &mut NoDraw,
        )
        .unwrap();
    assert_eq!(
        engine
            .audio_authority_view(&world, &actor)
            .unwrap()
            .music
            .unlocked_groups,
        vec![62]
    );
    step(&engine, &mut world);
    assert_eq!(world.characters[&actor].tile, fixtures::tile(1002, 1005));
    let history = engine.audio_authority_view(&world, &actor).unwrap().music;
    assert_eq!(history.unlocked_groups, vec![62, 144]);
    assert_eq!(
        history
            .tracks
            .iter()
            .find(|track| track.group == 144)
            .unwrap()
            .confirmed_at_tick
            .as_deref(),
        Some("1")
    );
    world = serde_json::from_slice(&serde_json::to_vec(&world).unwrap()).unwrap();
    engine
        .apply_lifecycle(&mut world, &actor, LifecycleTransition::Rejoin)
        .unwrap();
    step(&engine, &mut world);
    assert_eq!(
        engine.audio_authority_view(&world, &actor).unwrap().music,
        history
    );
}

#[test]
fn explicit_legacy_migration_recovers_only_conserved_facts_and_keeps_unknown_history() {
    let pack = audio_pack();
    let mut old = pack.definition.clone();
    old.ui.as_mut().unwrap().audio_authority = None;
    old.revision = "synthetic-audio-legacy".into();
    let legacy = WorldEngine::new(Arc::new(old)).unwrap();
    let actor: ActorId = fixtures::id("actor.audio.legacy");
    let mut world = legacy.initial_world().unwrap();
    world.characters.insert(
        actor.clone(),
        legacy
            .character_from_initial(actor.clone(), "Legacy", BTreeMap::new())
            .unwrap(),
    );
    legacy
        .apply_lifecycle(&mut world, &actor, LifecycleTransition::Join)
        .unwrap();
    legacy
        .apply_intent(&mut world, &actor, &remember(), &mut NoDraw)
        .unwrap();
    step(&legacy, &mut world);
    let before = world.characters[&actor].clone();
    let engine = WorldEngine::new(Arc::new(pack.definition.clone())).unwrap();
    world.content_revision = pack.definition.revision.clone();
    assert!(engine.audio_authority_view(&world, &actor).is_err());
    engine.migrate_ui_state(&mut world).unwrap();
    let view = engine.audio_authority_view(&world, &actor).unwrap();
    assert_eq!(view.music.history, MusicHistoryStatus::LegacyUntracked);
    assert!(!view.music.complete);
    assert_eq!(view.music.unlocked_groups, vec![62, 144]);
    assert_eq!(
        view.music
            .tracks
            .iter()
            .find(|track| track.group == 76)
            .unwrap()
            .status,
        MusicUnlockStatus::Unknown
    );
    assert_eq!(view.music.tracked_from_tick, "1");
    assert_eq!(world.characters[&actor].runtime.ui, before.runtime.ui);
    assert_eq!(world.characters[&actor].inventory, before.inventory);
    assert_eq!(world.characters[&actor].skills, before.skills);
    let migrated = world.clone();
    engine.migrate_ui_state(&mut world).unwrap();
    assert_eq!(world, migrated);
}

#[test]
fn history_removal_or_rewriting_is_corruption_not_a_new_default() {
    let pack = audio_pack();
    let (engine, mut world, actor) = setup(&pack);
    let original = world.characters[&actor].runtime.clone();
    world
        .characters
        .get_mut(&actor)
        .unwrap()
        .runtime
        .audio_authority = None;
    assert!(engine.audio_authority_view(&world, &actor).is_err());
    assert!(
        original
            .validate_ledger_successor(&world.characters[&actor].runtime)
            .is_err()
    );
    world.characters.get_mut(&actor).unwrap().runtime = original.clone();
    let audio = world
        .characters
        .get_mut(&actor)
        .unwrap()
        .runtime
        .audio_authority
        .as_mut()
        .unwrap();
    audio.unlocks.get_mut(&62).unwrap().rule = "music.62.faked".into();
    assert!(engine.audio_authority_view(&world, &actor).is_err());
    assert!(
        original
            .validate_ledger_successor(&world.characters[&actor].runtime)
            .is_err()
    );
}

#[test]
fn source_variable_cases_fail_explicitly_instead_of_choosing_a_first_or_zero_default() {
    let pack = audio_pack();
    let (_, world, actor) = setup(&pack);
    for overlap in [false, true] {
        let mut definition = pack.definition.clone();
        let field = &mut definition
            .ui
            .as_mut()
            .unwrap()
            .audio_authority
            .as_mut()
            .unwrap()
            .varps
            .get_mut(&491)
            .unwrap()
            .fields[0];
        field.cases = if overlap {
            vec![
                NativeVarpCase {
                    guard: Guard::Always,
                    value: 0,
                },
                NativeVarpCase {
                    guard: Guard::Always,
                    value: 1,
                },
            ]
        } else {
            vec![NativeVarpCase {
                guard: Guard::Not {
                    guard: Box::new(Guard::Always),
                },
                value: 0,
            }]
        };
        let engine = WorldEngine::new(Arc::new(definition)).unwrap();
        if overlap {
            assert_eq!(
                engine
                    .audio_authority_view(&world, &actor)
                    .unwrap_err()
                    .code,
                GameErrorCode::InvalidContent
            );
        } else {
            let view = engine.audio_authority_view(&world, &actor).unwrap();
            assert!(view.varps[0].value.is_none() && view.varps[0].unavailable_reason.is_some());
            assert_eq!(view.varps[0].known_bits, 0);
        }
    }
}

fn authority(snapshot: &game::WorldSnapshot) -> &game::AudioAuthority {
    snapshot.audio_authority.as_ref().unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn real_audio_authority_uses_creation_tick_and_preserves_committed_history_across_restart() {
    let database = Database::reset().await;
    let pack = audio_pack();
    let service = Live::start(pack.config(&database)).await;
    let account = service.endpoint.account("audio_owner").await;
    let actor = ActorId::new(service.endpoint.create(&account).await).unwrap();
    let created = database.world(pack.world_id).await.state.characters[&actor].clone();
    assert_eq!(
        created
            .runtime
            .audio_authority
            .as_ref()
            .unwrap()
            .tracked_from_tick,
        created.last_action_tick
    );
    let joined = service.endpoint.join(&account).await;
    assert_eq!(
        authority(joined.snapshot.as_ref().unwrap())
            .music
            .as_ref()
            .unwrap()
            .unlocked_groups,
        vec![62]
    );
    let other = service.endpoint.account("audio_other").await;
    service.endpoint.create(&other).await;
    let other_joined = service.endpoint.join(&other).await;
    let operation = Uuid::new_v4();
    let walk = game::world_input::Action::Walk(game::Walk {
        destination: Some(game::Tile {
            x: 1002,
            y: 1005,
            plane: 0,
        }),
        running: true,
    });
    service
        .endpoint
        .input(&account, &joined, 1, operation, walk.clone())
        .await
        .action();
    let arrived = ui::wait_for(&service.endpoint, &account, &joined, |snapshot| {
        snapshot.player.as_ref().unwrap().tile.as_ref().unwrap().y == 1005
    })
    .await;
    assert_eq!(
        authority(&arrived).music.as_ref().unwrap().unlocked_groups,
        vec![62, 144]
    );
    let saved = authority(&arrived).music.clone();
    assert!(
        service
            .endpoint
            .input(&account, &joined, 1, operation, walk.clone())
            .await
            .action()
            .duplicate
    );
    assert_eq!(
        authority(&service.endpoint.poll(&account, &joined, 0).await.snapshot()).music,
        saved
    );
    service
        .endpoint
        .input(
            &account,
            &joined,
            2,
            Uuid::new_v4(),
            game::world_input::Action::Interact(game::Interact {
                target: "spawn.test.guide".into(),
                action: "Remember".into(),
            }),
        )
        .await
        .action();
    assert_eq!(
        authority(&service.endpoint.poll(&account, &joined, 0).await.snapshot()).varps[0].value,
        Some(4)
    );
    let private = service
        .endpoint
        .poll(&other, &other_joined, 0)
        .await
        .snapshot();
    assert_eq!(
        authority(&private).music.as_ref().unwrap().unlocked_groups,
        vec![62]
    );
    assert_eq!(authority(&private).varps[0].value, Some(0));
    assert_eq!(service.endpoint.create(&account).await, actor.to_string());
    crate::store::logout(
        &database.pool,
        &crate::crypto::token_digest(&account.token).unwrap(),
    )
    .await
    .unwrap();
    service
        .endpoint
        .input(&account, &joined, 1, operation, walk.clone())
        .await
        .error(StatusCode::UNAUTHORIZED);
    service.stop().await.unwrap();
    let restarted = Live::start(pack.config(&database)).await;
    let account = restarted.endpoint.relogin(&account).await;
    let rejoined = restarted.endpoint.join(&account).await;
    assert_eq!(rejoined.next_sequence, 3);
    assert_eq!(authority(rejoined.snapshot.as_ref().unwrap()).music, saved);
    assert_eq!(
        authority(rejoined.snapshot.as_ref().unwrap()).varps[0].value,
        Some(4)
    );
    assert!(
        restarted
            .endpoint
            .input(&account, &rejoined, 1, operation, walk)
            .await
            .action()
            .duplicate
    );
    restarted.stop().await.unwrap();
    database.pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn explicit_audio_migration_retains_old_facts_receipts_ui_and_previously_tracked_history() {
    let database = Database::reset().await;
    let mut pack = audio_pack();
    let audio = pack
        .definition
        .ui
        .as_mut()
        .unwrap()
        .audio_authority
        .take()
        .unwrap();
    pack.definition.revision = "source-audio-legacy-v4".into();
    pack.write();
    let from_hash = sha256(&fs::read(pack.root.join("world.csc")).unwrap());
    let service = Live::start(pack.config(&database)).await;
    let account = service.endpoint.account("audio_legacy").await;
    let actor = ActorId::new(service.endpoint.create(&account).await).unwrap();
    let joined = service.endpoint.join(&account).await;
    assert!(joined.snapshot.as_ref().unwrap().audio_authority.is_none());
    let operation = Uuid::new_v4();
    let remembered = game::world_input::Action::Interact(game::Interact {
        target: "spawn.test.guide".into(),
        action: "Remember".into(),
    });
    service
        .endpoint
        .input(&account, &joined, 1, operation, remembered.clone())
        .await
        .action();
    pack.definition.ui.as_mut().unwrap().audio_authority = Some(audio);
    pack.definition.revision = "source-audio-tracked-v4".into();
    pack.write();
    assert!(
        crate::migrate_game_ui(pack.config(&database), from_hash.clone())
            .await
            .is_err()
    );
    service.stop().await.unwrap();
    let before = database.world(pack.world_id).await;
    assert!(
        Service::bind(pack.config(&database)).await.is_err(),
        "startup cannot silently repin the legacy world"
    );
    crate::migrate_game_ui(pack.config(&database), from_hash.clone())
        .await
        .unwrap();
    let after = database.world(pack.world_id).await;
    let history = after.state.characters[&actor]
        .runtime
        .audio_authority
        .clone()
        .unwrap();
    assert_eq!(history.history, MusicHistoryStatus::LegacyUntracked);
    assert_eq!(history.tracked_from_tick, before.state.tick);
    assert_eq!(
        history.unlocks.keys().copied().collect::<Vec<_>>(),
        vec![62, 144]
    );
    let mut preserved = after.state.clone();
    preserved.content_revision = before.state.content_revision.clone();
    preserved.revision = before.state.revision;
    preserved.runtime.audio_authority_version = None;
    for character in preserved.characters.values_mut() {
        character.runtime.audio_authority = None;
    }
    assert_eq!(
        preserved, before.state,
        "music migration cannot rewrite gameplay, UI history or clocks"
    );
    crate::migrate_game_ui(pack.config(&database), from_hash)
        .await
        .unwrap();
    assert_eq!(database.world(pack.world_id).await.state, after.state);
    let restarted = Live::start(pack.config(&database)).await;
    let rejoined = restarted.endpoint.join(&account).await;
    let projected = authority(rejoined.snapshot.as_ref().unwrap())
        .music
        .as_ref()
        .unwrap();
    assert_eq!(
        projected.history,
        game::MusicHistoryStatus::LegacyUntracked as i32
    );
    assert!(!projected.complete);
    assert_eq!(
        projected
            .tracks
            .iter()
            .find(|track| track.group == 76)
            .unwrap()
            .status,
        game::MusicUnlockStatus::Unknown as i32
    );
    assert!(
        restarted
            .endpoint
            .input(&account, &rejoined, 1, operation, remembered)
            .await
            .action()
            .duplicate
    );
    restarted.stop().await.unwrap();
    let from_hash = sha256(&fs::read(pack.root.join("world.csc")).unwrap());
    pack.definition.revision = "source-audio-compatible-v4".into();
    pack.write();
    crate::migrate_game_ui(pack.config(&database), from_hash)
        .await
        .unwrap();
    assert_eq!(
        database.world(pack.world_id).await.state.characters[&actor]
            .runtime
            .audio_authority
            .as_ref(),
        Some(&history)
    );
    database.pool.close().await;
}
