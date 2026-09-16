use std::io::Read;

use clubscape_content::{CompiledContent, load_compiled, read_content_json};

use super::super::readiness::{Profile, Readiness};
use super::*;

const RECIPE: &str = "recipe.water.bucket";
const SINK: &str = "spawn.water_source.3205.3215.p0.t10.r0";

struct WaterPair {
    old: Arc<CompiledContent>,
    target: Arc<CompiledContent>,
    from: String,
    to: String,
}

struct NoDraw;

impl clubscape_world_engine::RandomSource for NoDraw {
    fn draw_below(&mut self, _: u32) -> GameResult<u32> {
        panic!("this input or migration must not draw a gameplay outcome")
    }
}

fn gzip(path: &str) -> Vec<u8> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(fs::File::open(root.join(path)).unwrap())
        .take(clubscape_content::MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() <= clubscape_content::MAX_INPUT_BYTES);
    bytes
}

fn source_pair(profile: &str) -> WaterPair {
    let (from, to, target_path, motion_entry) = match profile {
        "legacy5e" => (
            "5e0aa8a28851752ae0b8b0a08c635c6c8d2979f509f3a3e564ee74e2abfc8e6f",
            "b2a1be20a0e6c3e1968f6f7610198ce38212f196c005539b4ac5013d10ebc650",
            "content/m1/legacy5e-water/game-content.csc.gz",
            false,
        ),
        "current5b" => (
            "5b3ba5f108ed3fec8f8b5f7f49b429c059e21b6f192ec99a0569616e08330059",
            "adb24c14f76181a76e2874e97be3e2743a4c475d3ff89a7720b70ae6f6981bbc",
            "content/m1/game-content.csc.gz",
            true,
        ),
        _ => panic!("unknown exact migration profile"),
    };
    let old = Arc::new(
        compile_content(
            read_content_json(&gzip(&format!(
                "research/water-fill/inputs/{profile}-source.json.gz"
            )))
            .unwrap(),
            ValidationMode::Runtime,
        )
        .unwrap(),
    );
    assert_eq!(sha256(&encode_compiled(&old).unwrap()), from);
    let bytes = gzip(target_path);
    assert_eq!(sha256(&bytes), to);
    let target = Arc::new(load_compiled(&bytes, ValidationMode::Runtime).unwrap());
    let old_value = serde_json::to_value(old.definition()).unwrap();
    let mut unchanged = serde_json::to_value(target.definition()).unwrap();
    assert_ne!(unchanged["revision"], old_value["revision"]);
    unchanged["revision"] = old_value["revision"].clone();
    assert!(
        unchanged["recipes"]
            .as_object_mut()
            .unwrap()
            .remove(RECIPE)
            .is_some()
    );
    assert!(old_value["recipes"].get(RECIPE).is_none());
    if motion_entry {
        assert!(
            unchanged
                .pointer_mut("/ui/actor_animations/recipes")
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(RECIPE)
                .is_some()
        );
    }
    assert_eq!(
        unchanged, old_value,
        "only the admitted source delta is allowed"
    );
    WaterPair {
        old,
        target,
        from: from.into(),
        to: to.into(),
    }
}

fn readiness(content: &CompiledContent) -> Readiness {
    Readiness::check(
        content,
        Some(&Profile {
            id: "ordinary_normal_f2p".into(),
            excluded_items: BTreeSet::from([
                fixtures::id("item.ensouled_goblin_head"),
                fixtures::id("item.milk.bottomless_bucket"),
            ]),
        }),
    )
    .unwrap()
}

fn engine(content: &CompiledContent) -> Arc<WorldEngine> {
    Arc::new(WorldEngine::new(Arc::new(content.definition().clone())).unwrap())
}

fn item(content: &GameContent, source_id: u32) -> ItemId {
    let matches: Vec<_> = content
        .items
        .values()
        .filter(|item| item.source_id == Some(source_id))
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "the fixture needs an unambiguous source item"
    );
    matches[0].id.clone()
}

fn controlled_character(engine: &WorldEngine, character: &mut CharacterState) {
    // Controlled source fixture, not a restored account or a played tutorial/quest.
    let derived = engine
        .character_from_initial_at_tick(
            character.actor_id.clone(),
            character.display_name.clone(),
            character.appearance.clone(),
            character.last_action_tick,
        )
        .unwrap();
    character.runtime = derived.runtime;
    character.region = engine.content().spawns[&fixtures::id(SINK)].region.clone();
    character.tile = Tile::new(3205, 3214, 0).unwrap();
    character.tutorial_stage = fixtures::id("stage.tutorial.mainland");
    character.inventory = Inventory::default();
    for (slot, source_id) in [(0, 1925), (1, 1933)] {
        character.inventory.slots[slot] = Some(ItemStack {
            item: item(engine.content(), source_id),
            quantity: Quantity::new(1).unwrap(),
            instance: None,
        });
    }
}

fn expected_migration(before: &WorldSnapshot, target: &CompiledContent) -> WorldSnapshot {
    let mut expected = before.clone();
    expected.state.content_revision = target.definition().revision.clone();
    expected.state.revision += 1;
    expected
}

async fn migrate(
    store: &GameStore,
    lease: &WorldLease,
    pair: &WaterPair,
) -> Result<WorldSnapshot, crate::game_storage::GameStorageError> {
    let engine = engine(&pair.target);
    let readiness = readiness(&pair.target);
    store
        .migrate_ui_content(
            lease,
            pair.from.clone(),
            pair.to.clone(),
            pair.target.definition().revision.clone(),
            move |world| {
                engine.migrate_ui_state(world)?;
                readiness.validate_world(world)
            },
        )
        .await
}

async fn journals(database: &Database, world: Uuid) -> (Vec<String>, Vec<String>) {
    let commands = sqlx::query_scalar(
        "SELECT committed_result::text FROM processed_game_commands
         WHERE world_id = $1 ORDER BY operation_id",
    )
    .bind(world)
    .fetch_all(&database.pool)
    .await
    .unwrap();
    let lifecycle = sqlx::query_scalar(
        "SELECT committed_result::text FROM game_lifecycle_commands
         WHERE world_id = $1 ORDER BY operation_id",
    )
    .bind(world)
    .fetch_all(&database.pool)
    .await
    .unwrap();
    (commands, lifecycle)
}

async fn audit_count(database: &Database, world: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM game_content_migrations WHERE world_id = $1")
        .bind(world)
        .fetch_one(&database.pool)
        .await
        .unwrap()
}

#[test]
fn exact_water_source_pairs_preserve_existing_runtime_and_metadata() {
    for profile in ["legacy5e", "current5b"] {
        let pair = source_pair(profile);
        let old = engine(&pair.old);
        let target = engine(&pair.target);
        let actor: ActorId = fixtures::id("actor.water.migration_fixture");
        let mut world = old.initial_world().unwrap();
        let mut character = old
            .character_from_initial(actor.clone(), "Water fixture", BTreeMap::new())
            .unwrap();
        controlled_character(&old, &mut character);
        world.characters.insert(actor.clone(), character);
        old.apply_lifecycle(&mut world, &actor, LifecycleTransition::Join)
            .unwrap();
        world.validate_runtime(pair.old.definition()).unwrap();
        readiness(&pair.old).validate_world(&world).unwrap();
        let before = world.clone();
        world.content_revision = pair.target.definition().revision.clone();
        target.migrate_ui_state(&mut world).unwrap();
        readiness(&pair.target).validate_world(&world).unwrap();
        let migrated = world.clone();
        world.content_revision = before.content_revision.clone();
        assert_eq!(
            world, before,
            "existing UI/audio/gameplay must not be rewritten"
        );
        world = serde_json::from_slice(&serde_json::to_vec(&migrated).unwrap()).unwrap();
        target.migrate_ui_state(&mut world).unwrap();
        assert_eq!(world, migrated);
    }
}

async fn verify_durable_upgrade(profile: &str) {
    let pair = source_pair(profile);
    let old = engine(&pair.old);
    let target = engine(&pair.target);
    let database = Database::reset().await;
    let accounts = Live::start(Config::new(&database.url, "127.0.0.1:0", None).unwrap()).await;
    let account = accounts.endpoint.account(&format!("water_{profile}")).await;
    accounts.stop().await.unwrap();
    let authentication = AuthTokenDigest::from_token(&account.token).unwrap();
    let world_id = Uuid::new_v4();
    let store = GameStore::new(database.pool.clone());
    store
        .initialize_world(world_id, old.initial_world().unwrap())
        .await
        .unwrap();
    let lease = store
        .acquire_world_lease(world_id, OWNER_LEASE)
        .await
        .unwrap();
    let key = store.runtime_key(&lease, pair.from.clone()).await.unwrap();
    let create_engine = old.clone();
    let created = store
        .create_character_with(
            &lease,
            authentication,
            SourceCharacter {
                content_revision: pair.old.definition().revision.clone(),
                initial_state: pair.old.definition().initial_state.clone(),
                appearance: BTreeMap::new(),
            },
            move |character| {
                controlled_character(&create_engine, character);
                Ok(())
            },
        )
        .await
        .unwrap();
    let actor = created.state.actor_id.clone();
    let join_engine = old.clone();
    let joined = store
        .apply_session_lifecycle(
            &lease,
            authentication,
            Uuid::new_v4(),
            LiveSessionAction::Join,
            PLAYER_LEASE,
            move |world, actor, transition| join_engine.apply_lifecycle(world, actor, transition),
        )
        .await
        .unwrap();
    let access = joined.receipt.session.unwrap().access(authentication);
    let original = GameCommand {
        operation_id: Uuid::new_v4(),
        sequence: 1,
        intent: GameIntent::CancelActivity,
    };
    let input_engine = old.clone();
    let acknowledged = store
        .commit_routed_command(
            &lease,
            &access,
            original.clone(),
            None,
            move |world, actor, intent| {
                input_engine.apply_intent(world, actor, intent, &mut NoDraw)
            },
        )
        .await
        .unwrap();
    assert!(!acknowledged.commit.duplicate);
    let before = store.load_world(world_id).await.unwrap();
    let character_before = store
        .load_character(world_id, authentication)
        .await
        .unwrap()
        .unwrap();
    let history = journals(&database, world_id).await;
    assert_eq!((history.0.len(), history.1.len()), (1, 1));
    let foreign = GameStore::new(database.pool.clone());
    assert!(
        foreign
            .acquire_world_lease(world_id, OWNER_LEASE)
            .await
            .is_err()
    );
    assert!(migrate(&foreign, &lease, &pair).await.is_err());
    assert!(
        store
            .migrate_ui_content(
                &lease,
                "0".repeat(64),
                pair.to.clone(),
                pair.target.definition().revision.clone(),
                |_| panic!("wrong source pin must not reach the callback"),
            )
            .await
            .is_err()
    );
    for change_gameplay in [false, true] {
        let migrate_engine = target.clone();
        let owner = actor.clone();
        assert!(
            store
                .migrate_ui_content(
                    &lease,
                    pair.from.clone(),
                    pair.to.clone(),
                    pair.target.definition().revision.clone(),
                    move |world| {
                        migrate_engine.migrate_ui_state(world)?;
                        if change_gameplay {
                            world.characters.get_mut(&owner).unwrap().quest_points += 1;
                            Ok(())
                        } else {
                            Err(GameError::new(
                                GameErrorCode::InvalidInput,
                                "Controlled migration callback failure",
                            ))
                        }
                    },
                )
                .await
                .is_err()
        );
        assert_eq!(store.load_world(world_id).await.unwrap(), before);
        assert_eq!(audit_count(&database, world_id).await, 0);
        assert!(journals(&database, world_id).await == history);
        assert!(store.runtime_key(&lease, pair.from.clone()).await.unwrap() == key);
    }
    let migrated = migrate(&store, &lease, &pair).await.unwrap();
    assert_eq!(migrated, expected_migration(&before, &pair.target));
    let character_after = store
        .load_character(world_id, authentication)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(character_after.state, character_before.state);
    assert_eq!(character_after.revision, character_before.revision + 1);
    assert_eq!(character_after.last_sequence, 1);
    assert_eq!(audit_count(&database, world_id).await, 1);
    assert!(journals(&database, world_id).await == history);
    assert!(store.runtime_key(&lease, pair.to.clone()).await.unwrap() == key);
    assert_eq!(migrate(&store, &lease, &pair).await.unwrap(), migrated);
    assert!(
        store
            .migrate_ui_content(
                &lease,
                "f".repeat(64),
                pair.to.clone(),
                pair.target.definition().revision.clone(),
                |_| panic!("an idempotent retry must verify its original audit"),
            )
            .await
            .is_err()
    );
    store.release_world_lease(&lease).await.unwrap();
    let restarted = GameStore::new(database.pool.clone());
    assert!(migrate(&restarted, &lease, &pair).await.is_err());
    let replacement = restarted
        .acquire_world_lease(world_id, OWNER_LEASE)
        .await
        .unwrap();
    assert!(replacement.fence > lease.fence);
    assert!(migrate(&store, &lease, &pair).await.is_err());
    assert_eq!(
        migrate(&restarted, &replacement, &pair).await.unwrap(),
        migrated
    );
    assert_eq!(audit_count(&database, world_id).await, 1);
    let replay = restarted
        .commit_routed_command(&replacement, &access, original.clone(), None, |_, _, _| {
            panic!("an old acknowledged input must not run under new content")
        })
        .await
        .unwrap();
    assert!(replay.commit.duplicate);
    assert_eq!(replay.commit.receipt, acknowledged.commit.receipt);
    assert_eq!(replay.snapshot, migrated);
    let tick_engine = target.clone();
    let advanced = restarted
        .commit_routed_tick(
            &replacement,
            migrated.state.tick,
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
        .unwrap();
    let before_water = advanced.snapshot.state.characters[&actor].clone();
    let water = GameCommand {
        operation_id: Uuid::new_v4(),
        sequence: 2,
        intent: GameIntent::UseItem {
            inventory_slot: 0,
            target: ItemTarget::World {
                spawn: fixtures::id(SINK),
            },
        },
    };
    let use_engine = target.clone();
    let pending = restarted
        .commit_routed_command(
            &replacement,
            &access,
            water.clone(),
            None,
            move |world, actor, intent| use_engine.apply_intent(world, actor, intent, &mut NoDraw),
        )
        .await
        .unwrap();
    assert!(!pending.commit.duplicate);
    assert_eq!(
        pending.snapshot.state.characters[&actor].inventory,
        before_water.inventory
    );
    restarted.release_world_lease(&replacement).await.unwrap();
    let resumed = GameStore::new(database.pool.clone());
    let resumed_lease = resumed
        .acquire_world_lease(world_id, OWNER_LEASE)
        .await
        .unwrap();
    assert!(resumed_lease.fence > replacement.fence);
    let loaded = resumed.load_world(world_id).await.unwrap();
    assert_eq!(loaded, pending.snapshot);
    loaded
        .state
        .validate_runtime(pair.target.definition())
        .unwrap();
    readiness(&pair.target)
        .validate_world(&loaded.state)
        .unwrap();
    let resolve_engine = target.clone();
    let resolved = resumed
        .commit_routed_tick(
            &resumed_lease,
            loaded.state.tick,
            vec![access.clone()],
            move |world| {
                let context = resolve_engine.tick_context(world)?;
                resolve_engine.process_advanced_tick_with_context(
                    world,
                    &mut engine_fixtures::v2::Hits(0),
                    &context,
                )
            },
        )
        .await
        .unwrap();
    assert_eq!(resolved.snapshot.state.tick, loaded.state.tick + 1);
    assert_eq!(
        resolved
            .commit
            .receipt
            .routed_events
            .iter()
            .filter(|event| event.actor_id == actor
                && matches!(&event.event, GameEvent::ProductionResolved { recipe, .. }
                    if recipe.as_str() == RECIPE))
            .count(),
        1
    );
    let after_water = &resolved.snapshot.state.characters[&actor];
    let mut expected_inventory = before_water.inventory.clone();
    expected_inventory.slots[0] = Some(ItemStack {
        item: item(target.content(), 1929),
        quantity: Quantity::new(1).unwrap(),
        instance: None,
    });
    assert_eq!(after_water.inventory, expected_inventory);
    assert_eq!(after_water.skills, before_water.skills);
    assert_eq!(after_water.quests, before_water.quests);
    assert_eq!(after_water.quest_points, before_water.quest_points);
    assert_eq!(
        after_water.runtime.entitlements,
        before_water.runtime.entitlements
    );
    assert!(matches!(after_water.activity, Activity::Idle));
    let tick_replay = resumed
        .commit_routed_tick(
            &resumed_lease,
            loaded.state.tick,
            vec![access.clone()],
            |_| panic!("a committed water resolution must not run twice"),
        )
        .await
        .unwrap();
    assert!(tick_replay.commit.duplicate);
    assert_eq!(tick_replay.snapshot, resolved.snapshot);
    for (command, receipt) in [
        (water.clone(), pending.commit.receipt),
        (original, acknowledged.commit.receipt),
    ] {
        let repeated = resumed
            .commit_routed_command(&resumed_lease, &access, command, None, |_, _, _| {
                panic!("known receipts cannot restart or rewind production")
            })
            .await
            .unwrap();
        assert!(repeated.commit.duplicate);
        assert_eq!(repeated.commit.receipt, receipt);
        assert_eq!(repeated.snapshot, resolved.snapshot);
    }
    let repeat_engine = target.clone();
    assert!(
        resumed
            .commit_routed_command(
                &resumed_lease,
                &access,
                GameCommand {
                    operation_id: Uuid::new_v4(),
                    sequence: 3,
                    intent: water.intent,
                },
                None,
                move |world, actor, intent| {
                    repeat_engine.apply_intent(world, actor, intent, &mut NoDraw)
                },
            )
            .await
            .is_err()
    );
    assert_eq!(
        resumed.load_world(world_id).await.unwrap(),
        resolved.snapshot
    );
    assert!(resumed.runtime_key(&resumed_lease, pair.to).await.unwrap() == key);
    resumed.release_world_lease(&resumed_lease).await.unwrap();
    database.pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn legacy_water_migration_preserves_fences_journals_and_resolves_once_after_restart() {
    verify_durable_upgrade("legacy5e").await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn current_water_migration_preserves_fences_journals_and_resolves_once_after_restart() {
    verify_durable_upgrade("current5b").await;
}
