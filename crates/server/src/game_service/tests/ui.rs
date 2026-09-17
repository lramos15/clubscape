use super::*;
use game::gameplay_ui_request::Request as Ui;

pub(super) fn pack() -> Pack {
    let mut pack = Pack::ui();
    pack.definition.initial_state.tile = fixtures::tile(1004, 1002);
    pack.definition.initial_state.inventory.slots[5] =
        Some(fixtures::stack("item.test.potion_2", 1));
    pack.definition.initial_state.inventory.slots[6] = Some(fixtures::stack("item.test.food", 1));
    pack.definition.initial_state.inventory.slots[7] = Some(fixtures::stack("item.test.bones", 1));
    pack.definition.initial_state.bank.slots = vec![Some(fixtures::stack("item.test.ore", 2))];
    pack.definition.initial_state.run_energy = 1000;
    let bound = |value| SourceBinding::Bound {
        value,
        source: fixtures::sources(),
    };
    let recipe = pack
        .definition
        .recipes
        .get_mut(&fixtures::id("recipe.test.bar"))
        .unwrap();
    recipe.ticks = None;
    recipe.mechanics = Some(RecipeMechanics {
        method: fixtures::id("action.test.smith"),
        guard: Guard::Always,
        chance_skill: None,
        cadence: ActionCadence {
            single: bound(1),
            first: bound(3),
            repeat: bound(3),
            menu_delay: bound(0),
        },
        tool_ownership: OwnershipScope::InventoryAndEquipment,
        failed_xp: Vec::new(),
        success_effects: Vec::new(),
        failure_effects: Vec::new(),
        lifecycle: RecipeLifecycle::InventoryConversion,
    });
    pack.write();
    pack
}

fn item_action(slot: u32, item: &str, action: &str) -> Ui {
    Ui::ItemAction(game::UiItemAction {
        inventory_slot: slot,
        expected_item: item.into(),
        expected_instance: None,
        action: action.into(),
    })
}

fn chat(text: &str) -> Ui {
    Ui::PublicChat(game::UiChat {
        channel: "public".into(),
        text: text.into(),
    })
}

pub(super) fn recovery_pack() -> Pack {
    let mut pack = Pack::combat();
    fixtures::ui::projection(&mut pack.definition);
    pack.definition.initial_state.interfaces = pack.definition.interfaces.keys().cloned().collect();
    let slot = pack
        .definition
        .initial_state
        .inventory
        .slots
        .iter_mut()
        .find(|slot| slot.is_none())
        .unwrap();
    *slot = Some(fixtures::stack("item.test.bar", 1));
    for item in pack.definition.items.values_mut() {
        if item.weight.is_none() {
            item.weight = Some(SourceBinding::Bound {
                value: ItemWeight {
                    grams: 1,
                    inventory: WeightContribution::PerUnit,
                    equipment: WeightContribution::PerUnit,
                },
                source: fixtures::sources(),
            });
        }
    }
    pack.definition.revision = "source-ui-recovery-synthetic-v4".into();
    pack.write();
    pack
}

pub(super) async fn wait_for(
    endpoint: &Endpoint,
    account: &Account,
    joined: &game::WorldJoined,
    condition: impl Fn(&game::WorldSnapshot) -> bool,
) -> game::WorldSnapshot {
    timeout(WAIT, async {
        loop {
            let snapshot = endpoint.poll(account, joined, 0).await.snapshot();
            if condition(&snapshot) {
                return snapshot;
            }
            sleep(Duration::from_millis(60)).await;
        }
    })
    .await
    .expect("bounded real source-tick observation")
}

pub(super) async fn finish_disconnected_death(
    database: &Database,
    pack: &Pack,
    actor: &ActorId,
) -> DeathId {
    let store = GameStore::new(database.pool.clone());
    let lease = store
        .acquire_world_lease(pack.world_id, OWNER_LEASE)
        .await
        .unwrap();
    let engine = Arc::new(WorldEngine::new(Arc::new(pack.definition.clone())).unwrap());
    let mut world = store.load_world(pack.world_id).await.unwrap();
    for _ in 0..128 {
        if matches!(
            world.state.characters[actor].runtime.life,
            LifeState::FirstDeathOffice { .. }
        ) {
            break;
        }
        let engine = engine.clone();
        world = store
            .commit_live_tick(&lease, world.state.tick, Vec::new(), move |world, _| {
                let context = engine.tick_context(world)?;
                engine.process_advanced_tick_with_context(
                    world,
                    &mut engine_fixtures::v2::Hits(0),
                    &context,
                )
            })
            .await
            .unwrap()
            .snapshot;
    }
    assert!(matches!(
        world.state.characters[actor].runtime.life,
        LifeState::FirstDeathOffice { .. }
    ));
    let death = world.state.characters[actor]
        .runtime
        .active_death
        .clone()
        .unwrap();
    store.release_world_lease(&lease).await.unwrap();
    death
}

#[test]
fn ui_fixture_compiles_and_projects_actual_declared_choices() {
    let pack = pack();
    let engine = &WorldEngine::new(Arc::new(pack.definition.clone())).unwrap();
    let actor: ActorId = fixtures::id("actor.ui.fixture");
    let mut world = engine.initial_world().unwrap();
    world.characters.insert(
        actor.clone(),
        engine
            .character_from_initial(actor.clone(), "Fixture", BTreeMap::new())
            .unwrap(),
    );
    engine
        .apply_lifecycle(&mut world, &actor, LifecycleTransition::Join)
        .unwrap();
    let before = world.clone();
    let view = engine.ui_view(&world, &actor).unwrap();
    assert_eq!(view.version, 1);
    assert_eq!(
        view.appearance.choices.keys().collect::<Vec<_>>(),
        vec!["body_type"]
    );
    assert_eq!(
        view.appearance.choices["body_type"]
            .iter()
            .map(|choice| choice.value)
            .collect::<Vec<_>>(),
        vec![0, 1]
    );
    assert!(!view.appearance.confirmed);
    assert_eq!(world, before);
}

#[test]
fn kept_on_death_projection_is_pure_and_equals_actual_source_retention() {
    let pack = recovery_pack();
    let engine = WorldEngine::new(Arc::new(pack.definition.clone())).unwrap();
    let actor: ActorId = fixtures::id("actor.ui.retention");
    let mut world = engine.initial_world().unwrap();
    world.characters.insert(
        actor.clone(),
        engine
            .character_from_initial(actor.clone(), "Fixture", BTreeMap::new())
            .unwrap(),
    );
    engine
        .apply_lifecycle(&mut world, &actor, LifecycleTransition::Join)
        .unwrap();
    engine
        .apply_intent(
            &mut world,
            &actor,
            &GameIntent::Ui {
                request: GameplayUiRequest::OpenDeathPreview,
            },
            &mut engine_fixtures::v2::Hits(0),
        )
        .unwrap();
    let before = world.clone();
    let preview = engine
        .ui_view(&world, &actor)
        .unwrap()
        .kept_on_death
        .unwrap();
    assert_eq!(world, before);
    assert!(preview.kept.is_empty());
    let expected: BTreeMap<_, _> = preview
        .lost
        .iter()
        .map(|item| (item.item.clone(), item.quantity))
        .collect();
    // An explicit synthetic zero-HP fixture drives the real death transition, not the query.
    world.characters.get_mut(&actor).unwrap().hitpoints = 0;
    let context = engine.tick_context(&world).unwrap();
    engine
        .tick_with_context(&mut world, &mut engine_fixtures::v2::Hits(0), &context)
        .unwrap();
    let death = &world.runtime.deaths[world.characters[&actor]
        .runtime
        .active_death
        .as_ref()
        .unwrap()];
    let actual: BTreeMap<_, _> = death
        .grave
        .iter()
        .flat_map(|grave| &grave.items)
        .chain(&death.office)
        .map(|item| (item.stack.item.clone(), item.stack.quantity.get()))
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(preview.full_grave_fee, "0");
    assert_eq!(preview.full_office_fee, "0");
}

#[test]
fn spell_availability_uses_actual_combat_and_transport_planners_and_native_style_order() {
    use engine_fixtures::v2 as v;
    let mut pack = recovery_pack();
    v::armed(&mut pack.definition, true);
    pack.definition.initial_state.inventory.slots[4] = Some(engine_fixtures::stack("rune", 50));
    pack.definition.initial_state.inventory.slots[5] = Some(engine_fixtures::stack("coins", 50));
    let travel_id = v::travel("ui_spell");
    let mut travel = pack.definition.mechanics.travels[&v::travel("exit")].clone();
    travel.id = travel_id.clone();
    travel.guard = Guard::Always;
    travel.destination = SourceBinding::Bound {
        value: TravelDestination::Fixed {
            location: WorldLocation {
                region: pack.definition.initial_state.region.clone(),
                tile: pack.definition.initial_state.tile,
                instance: None,
            },
        },
        source: fixtures::sources(),
    };
    pack.definition
        .mechanics
        .travels
        .insert(travel_id.clone(), travel);
    let spell_id: SpellId = fixtures::id("spell.test.ui_teleport");
    let mut spell = pack.definition.mechanics.spells[&v::spell()].clone();
    spell.id = spell_id.clone();
    spell.action = SpellAction::Teleport {
        travel: travel_id.clone(),
    };
    pack.definition
        .mechanics
        .spells
        .insert(spell_id.clone(), spell);
    pack.definition
        .ui
        .as_mut()
        .unwrap()
        .ability_names
        .insert(spell_id.to_string(), "Synthetic source teleport".into());
    pack.write();
    let engine = WorldEngine::new(Arc::new(pack.definition.clone())).unwrap();
    let mut world = engine.initial_world().unwrap();
    let actor: ActorId = fixtures::id("actor.ui.abilities");
    world.characters.insert(
        actor.clone(),
        engine
            .character_from_initial(actor.clone(), "Fixture", BTreeMap::new())
            .unwrap(),
    );
    engine
        .apply_lifecycle(&mut world, &actor, LifecycleTransition::Join)
        .unwrap();
    let view = engine.ui_view(&world, &actor).unwrap();
    assert_eq!(
        view.combat_styles
            .iter()
            .map(|style| style.id.clone())
            .collect::<Vec<_>>(),
        vec![
            v::style("ranged").to_string(),
            v::style("rapid").to_string(),
            v::style("longrange").to_string()
        ]
    );
    assert!(view.spells.iter().all(|spell| spell.permission.allowed));
    world
        .characters
        .get_mut(&actor)
        .unwrap()
        .runtime
        .combat
        .spell_ready = 10;
    world
        .characters
        .get_mut(&actor)
        .unwrap()
        .runtime
        .travel_cooldowns
        .insert(travel_id, 10);
    let before = world.clone();
    let view = engine.ui_view(&world, &actor).unwrap();
    assert!(view.spells.iter().all(
        |spell| !spell.permission.allowed && spell.permission.code == Some(GameErrorCode::Busy)
    ));
    assert_eq!(world, before);
    for (id, target) in [
        (v::spell(), Some(engine_fixtures::spawn("enemy"))),
        (spell_id, None),
    ] {
        assert_eq!(
            engine
                .apply_intent(
                    &mut world,
                    &actor,
                    &GameIntent::Cast {
                        spell: id.to_string(),
                        target
                    },
                    &mut v::Hits(0)
                )
                .unwrap_err()
                .code,
            GameErrorCode::Busy
        );
        assert_eq!(world, before);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn authenticated_ui_controls_commit_once_and_preserve_bank_identity_across_restart() {
    let database = Database::reset().await;
    let pack = pack();
    let service = Live::start(pack.config(&database)).await;
    let endpoint = &service.endpoint;
    let hello = endpoint
        .call(
            client_message::Command::Hello(Hello {}),
            None,
            Uuid::new_v4(),
        )
        .await;
    let Some(server_message::Result::Hello(hello)) = hello.message.result else {
        panic!()
    };
    assert!(
        hello
            .capabilities
            .iter()
            .any(|capability| capability == clubscape_protocol::GAMEPLAY_UI_CAPABILITY)
    );
    let account = endpoint.account("ui_controls").await;
    let actor = endpoint.create(&account).await;
    let joined = endpoint.join(&account).await;
    assert_eq!(
        joined
            .snapshot
            .as_ref()
            .unwrap()
            .ui
            .as_ref()
            .unwrap()
            .version,
        1
    );
    assert!(
        joined
            .snapshot
            .as_ref()
            .unwrap()
            .ui
            .as_ref()
            .unwrap()
            .bank
            .is_none()
    );
    let menu = endpoint
        .input(
            &account,
            &joined,
            1,
            Uuid::new_v4(),
            game::world_input::Action::Interact(game::Interact {
                target: "spawn.test.furnace".into(),
                action: "Smelt".into(),
            }),
        )
        .await
        .action()
        .snapshot
        .unwrap()
        .ui
        .unwrap()
        .production
        .unwrap();
    assert_eq!(
        menu.recipes[0].outputs[0].stack.as_ref().unwrap().item,
        "item.test.bar"
    );
    endpoint
        .ui(
            &account,
            &joined,
            2,
            Uuid::new_v4(),
            None,
            Ui::Production(game::UiProductionSelection {
                menu_id: "ui.stale".into(),
                recipe: "recipe.test.bar".into(),
                quantity: 1,
                mode: game::ProductionMode::Single as i32,
            }),
        )
        .await
        .error(StatusCode::CONFLICT);
    let production = Ui::ProductionAll(game::UiProductionAll {
        menu_id: menu.id,
        recipe: "recipe.test.bar".into(),
    });
    let operation = Uuid::new_v4();
    let accepted = endpoint
        .ui(&account, &joined, 2, operation, None, production.clone())
        .await
        .action();
    assert!(!accepted.duplicate);
    assert!(
        endpoint
            .ui(&account, &joined, 2, operation, None, production)
            .await
            .action()
            .duplicate
    );
    let done = wait_for(endpoint, &account, &joined, |snapshot| {
        snapshot
            .player
            .as_ref()
            .unwrap()
            .skills
            .iter()
            .any(|skill| skill.id == "skill.test.smithing" && skill.xp_tenths == 200)
    })
    .await;
    assert!(done.tick > accepted.snapshot.unwrap().tick);
    let dose_operation = Uuid::new_v4();
    let dose = item_action(5, "item.test.potion_2", "drink");
    let result = endpoint
        .ui(&account, &joined, 3, dose_operation, None, dose.clone())
        .await
        .action()
        .snapshot
        .unwrap();
    assert_eq!(result.player.as_ref().unwrap().run_energy, 2500);
    assert_eq!(
        result
            .player
            .as_ref()
            .unwrap()
            .inventory
            .iter()
            .find(|slot| slot.index == 5)
            .unwrap()
            .stack
            .as_ref()
            .unwrap()
            .item,
        "item.test.potion_1"
    );
    endpoint
        .ui(
            &account,
            &joined,
            3,
            dose_operation,
            None,
            item_action(5, "item.test.potion_2", "empty"),
        )
        .await
        .error(StatusCode::CONFLICT);
    let bank_open = endpoint
        .input(
            &account,
            &joined,
            4,
            Uuid::new_v4(),
            game::world_input::Action::Interact(game::Interact {
                target: "spawn.test.guide".into(),
                action: "Bank".into(),
            }),
        )
        .await
        .action()
        .snapshot
        .unwrap();
    let entry = bank_open
        .ui
        .as_ref()
        .unwrap()
        .bank
        .as_ref()
        .unwrap()
        .entries[0]
        .id
        .clone();
    endpoint
        .ui(
            &account,
            &joined,
            5,
            Uuid::new_v4(),
            None,
            Ui::InsertMode(game::UiToggle { enabled: true }),
        )
        .await
        .error(StatusCode::BAD_REQUEST);
    let operation = Uuid::new_v4();
    let revision = Some(
        bank_open
            .ui
            .as_ref()
            .unwrap()
            .bank
            .as_ref()
            .unwrap()
            .revision
            .parse()
            .unwrap(),
    );
    let (first, second) = tokio::join!(
        endpoint.ui(
            &account,
            &joined,
            5,
            operation,
            revision,
            Ui::InsertMode(game::UiToggle { enabled: true })
        ),
        endpoint.ui(
            &account,
            &joined,
            5,
            operation,
            revision,
            Ui::InsertMode(game::UiToggle { enabled: true })
        ),
    );
    let (first, second) = (first.action(), second.action());
    assert_ne!(first.duplicate, second.duplicate);
    let snapshot = second.snapshot.unwrap();
    assert!(
        snapshot
            .ui
            .as_ref()
            .unwrap()
            .bank
            .as_ref()
            .unwrap()
            .insert_mode
    );
    endpoint
        .ui(
            &account,
            &joined,
            6,
            Uuid::new_v4(),
            revision,
            Ui::SelectTab(game::UiTab { tab: 0 }),
        )
        .await
        .error(StatusCode::CONFLICT);
    let placeholder = endpoint
        .ui(
            &account,
            &joined,
            6,
            Uuid::new_v4(),
            Some(
                snapshot
                    .ui
                    .as_ref()
                    .unwrap()
                    .bank
                    .as_ref()
                    .unwrap()
                    .revision
                    .parse()
                    .unwrap(),
            ),
            Ui::Placeholder(game::UiIdentity { id: entry.clone() }),
        )
        .await
        .action()
        .snapshot
        .unwrap();
    let row = &placeholder
        .ui
        .as_ref()
        .unwrap()
        .bank
        .as_ref()
        .unwrap()
        .entries[0];
    assert_eq!(row.id, entry);
    assert!(row.placeholder && row.value.is_none());
    let saved = database.world(pack.world_id).await;
    let character = &saved.state.characters[&ActorId::new(&actor).unwrap()];
    assert!(character.bank.slots[0].is_none());
    assert_eq!(
        character.runtime.ui.as_ref().unwrap().bank.entries[0]
            .id
            .to_string(),
        entry
    );
    assert_eq!(
        endpoint.create(&account).await,
        actor,
        "creation retry must not overwrite acknowledged source progress"
    );
    service.stop().await.unwrap();
    let restarted = Live::start(pack.config(&database)).await;
    let rejoined = restarted.endpoint.join(&account).await;
    assert_eq!(rejoined.next_sequence, 7);
    assert!(
        rejoined
            .snapshot
            .as_ref()
            .unwrap()
            .ui
            .as_ref()
            .unwrap()
            .bank
            .is_none(),
        "transient container access closes at shutdown"
    );
    let retry = restarted
        .endpoint
        .ui(&account, &rejoined, 3, dose_operation, None, dose)
        .await
        .action();
    assert!(retry.duplicate);
    assert_eq!(
        retry
            .snapshot
            .as_ref()
            .unwrap()
            .player
            .as_ref()
            .unwrap()
            .run_energy,
        2500
    );
    let loaded = database.world(pack.world_id).await;
    let character = &loaded.state.characters[&ActorId::new(actor).unwrap()];
    assert_eq!(
        character.runtime.ui.as_ref().unwrap().bank.entries[0]
            .id
            .to_string(),
        entry
    );
    assert!(character.runtime.ui.as_ref().unwrap().bank.entries[0].placeholder);
    assert_eq!(
        character.skills[&fixtures::id("skill.test.smithing")].xp_tenths,
        200
    );
    restarted.stop().await.unwrap();
    database.pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn public_chat_is_authenticated_routed_bounded_replay_safe_and_private_views_stay_owned() {
    let database = Database::reset().await;
    let mut pack = pack();
    pack.definition.initial_state.tile = fixtures::tile(1002, 1003);
    pack.definition.ui.as_mut().unwrap().chat.radius = 1;
    pack.write();
    let service = Live::start(pack.config(&database)).await;
    let endpoint = &service.endpoint;
    let owner = endpoint.account("ui_chat_owner").await;
    let owner_actor = endpoint.create(&owner).await;
    let joined = endpoint.join(&owner).await;
    let near = endpoint.account("ui_chat_near").await;
    endpoint.create(&near).await;
    let nearby = endpoint.join(&near).await;
    let far = endpoint.account("ui_chat_far").await;
    endpoint.create(&far).await;
    let distant = endpoint.join(&far).await;
    endpoint
        .input(
            &far,
            &distant,
            1,
            Uuid::new_v4(),
            game::world_input::Action::Walk(game::Walk {
                destination: Some(game::Tile {
                    x: 1004,
                    y: 1003,
                    plane: 0,
                }),
                running: false,
            }),
        )
        .await
        .action();
    wait_for(endpoint, &far, &distant, |snapshot| {
        snapshot.player.as_ref().unwrap().tile.as_ref().unwrap().x == 1004
    })
    .await;
    for text in ["<col=ff0000>spoof", "invalid\nline", "invalid 🐧"] {
        endpoint
            .ui(&owner, &joined, 1, Uuid::new_v4(), None, chat(text))
            .await
            .error(StatusCode::BAD_REQUEST);
    }
    endpoint
        .ui(
            &owner,
            &nearby,
            1,
            Uuid::new_v4(),
            None,
            chat("wrong owner"),
        )
        .await
        .error(StatusCode::CONFLICT);
    let operation = Uuid::new_v4();
    let first = endpoint
        .ui(
            &owner,
            &joined,
            1,
            operation,
            None,
            chat("red:wave:Hello €"),
        )
        .await
        .action()
        .snapshot
        .unwrap();
    let line = &first
        .ui
        .as_ref()
        .unwrap()
        .public_chat
        .as_ref()
        .unwrap()
        .messages[0];
    assert_eq!(line.sender, owner.name);
    assert_eq!(line.actor, owner_actor);
    assert_eq!((line.colour, line.effect), (1, 1));
    let id = line.id.clone();
    assert!(
        endpoint
            .ui(
                &owner,
                &joined,
                1,
                operation,
                None,
                chat("red:wave:Hello €")
            )
            .await
            .action()
            .duplicate
    );
    let received = endpoint.poll(&near, &nearby, 0).await.snapshot();
    assert_eq!(
        received
            .ui
            .as_ref()
            .unwrap()
            .public_chat
            .as_ref()
            .unwrap()
            .messages
            .iter()
            .filter(|line| line.id == id)
            .count(),
        1
    );
    assert!(
        received
            .events
            .iter()
            .any(|event| event.public_chat.as_ref().is_some_and(|line| line.id == id))
    );
    let outside = endpoint.poll(&far, &distant, 0).await.snapshot();
    assert!(
        outside
            .ui
            .as_ref()
            .unwrap()
            .public_chat
            .as_ref()
            .unwrap()
            .messages
            .is_empty()
    );
    assert!(
        outside
            .events
            .iter()
            .all(|event| event.public_chat.is_none())
    );
    endpoint
        .ui(
            &owner,
            &joined,
            2,
            Uuid::new_v4(),
            None,
            item_action(6, "item.test.food", "read"),
        )
        .await
        .action();
    let private = endpoint.poll(&near, &nearby, 0).await.snapshot();
    assert!(private.ui.as_ref().unwrap().document.is_none());
    assert!(private.ui.as_ref().unwrap().bank.is_none());
    assert!(private.ui.as_ref().unwrap().recovery.is_none());
    crate::store::logout(
        &database.pool,
        &crate::crypto::token_digest(&owner.token).unwrap(),
    )
    .await
    .unwrap();
    endpoint
        .ui(&owner, &joined, 3, Uuid::new_v4(), None, chat("revoked"))
        .await
        .error(StatusCode::UNAUTHORIZED);
    let stored = database.world(pack.world_id).await;
    assert_eq!(
        stored.state.characters[&ActorId::new(owner_actor).unwrap()].last_command_sequence,
        2
    );
    service.stop().await.unwrap();
    database.pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn explicit_ui_content_migration_preserves_owned_state_and_old_command_receipts() {
    let database = Database::reset().await;
    let mut pack = Pack::new();
    let from_hash = sha256(&fs::read(pack.root.join("world.csc")).unwrap());
    let service = Live::start(pack.config(&database)).await;
    let account = service.endpoint.account("ui_migration").await;
    let actor = service.endpoint.create(&account).await;
    let joined = service.endpoint.join(&account).await;
    let operation = Uuid::new_v4();
    service
        .endpoint
        .input(&account, &joined, 1, operation, practice())
        .await
        .action();
    fixtures::ui::enable(&mut pack.definition);
    pack.definition.revision = "explicit-ui-content-migration-v4".into();
    pack.write();
    assert!(
        crate::migrate_game_ui(pack.config(&database), from_hash.clone())
            .await
            .is_err(),
        "an active owner cannot be taken over by a content migration"
    );
    service.stop().await.unwrap();
    let before = database.world(pack.world_id).await;
    assert!(
        crate::migrate_game_ui(pack.config(&database), "0".repeat(64))
            .await
            .is_err()
    );
    assert_eq!(database.world(pack.world_id).await.state, before.state);
    crate::migrate_game_ui(pack.config(&database), from_hash.clone())
        .await
        .unwrap();
    let after = database.world(pack.world_id).await;
    let owner = ActorId::new(&actor).unwrap();
    let mut preserved = after.state.characters[&owner].clone();
    let ui = preserved.runtime.ui.take().unwrap();
    assert!(
        ui.rewards.is_empty() && ui.chat_messages.is_empty(),
        "migration never synthesizes or replays old presentation history"
    );
    assert_eq!(preserved, before.state.characters[&owner]);
    assert_eq!(after.state.tick, before.state.tick);
    assert_eq!(after.state.revision, before.state.revision + 1);
    crate::migrate_game_ui(pack.config(&database), from_hash)
        .await
        .unwrap();
    assert_eq!(
        database.world(pack.world_id).await.state,
        after.state,
        "the same migration is idempotent"
    );
    assert!(
        crate::migrate_game_ui(pack.config(&database), "f".repeat(64))
            .await
            .is_err(),
        "an already-migrated target still requires the actual prior audit pin"
    );
    assert_eq!(database.world(pack.world_id).await.state, after.state);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM game_content_migrations WHERE world_id = $1")
            .bind(pack.world_id)
            .fetch_one(&database.pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    let restarted = Live::start(pack.config(&database)).await;
    let rejoined = restarted.endpoint.join(&account).await;
    assert_eq!(rejoined.next_sequence, 2);
    assert_eq!(
        rejoined
            .snapshot
            .as_ref()
            .unwrap()
            .ui
            .as_ref()
            .unwrap()
            .version,
        1
    );
    assert!(
        restarted
            .endpoint
            .input(&account, &rejoined, 1, operation, practice())
            .await
            .action()
            .duplicate
    );
    let result = database.world(pack.world_id).await;
    assert_eq!(
        result.state.characters[&owner].inventory,
        before.state.characters[&owner].inventory
    );
    assert_eq!(
        result.state.characters[&owner].skills,
        before.state.characters[&owner].skills
    );
    restarted.stop().await.unwrap();
    database.pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn committed_quest_scroll_and_real_level_presentation_survive_restart_without_regranting() {
    let database = Database::reset().await;
    let mut pack = pack();
    pack.definition
        .initial_state
        .skills
        .get_mut(&fixtures::id("skill.test.smithing"))
        .unwrap()
        .xp_tenths = 900;
    pack.write();
    let service = Live::start(pack.config(&database)).await;
    let account = service.endpoint.account("ui_reward").await;
    let actor = service.endpoint.create(&account).await;
    let joined = service.endpoint.join(&account).await;
    service
        .endpoint
        .input(
            &account,
            &joined,
            1,
            Uuid::new_v4(),
            game::world_input::Action::Interact(game::Interact {
                target: "spawn.test.guide".into(),
                action: "Talk".into(),
            }),
        )
        .await
        .action();
    let reward_operation = Uuid::new_v4();
    let intent = game::world_input::Action::DialogueChoice(game::DialogueChoice {
        speaker: "spawn.test.guide".into(),
        choice: "ask".into(),
    });
    let awarded = service
        .endpoint
        .input(&account, &joined, 2, reward_operation, intent.clone())
        .await
        .action()
        .snapshot
        .unwrap();
    let reward = awarded.ui.as_ref().unwrap().reward.as_ref().unwrap();
    assert_eq!(reward.kind, "quest");
    assert_eq!(reward.quest_points, 1);
    assert_eq!(reward.xp[0].amount_tenths, "200");
    let reward_id = reward.id.clone();
    service.stop().await.unwrap();
    let restarted = Live::start(pack.config(&database)).await;
    let rejoined = restarted.endpoint.join(&account).await;
    assert_eq!(
        rejoined
            .snapshot
            .as_ref()
            .unwrap()
            .ui
            .as_ref()
            .unwrap()
            .reward
            .as_ref()
            .unwrap()
            .id,
        reward_id
    );
    assert!(
        rejoined.snapshot.as_ref().unwrap().events.is_empty(),
        "reconnection does not replay obsolete reward/audio events"
    );
    assert!(
        restarted
            .endpoint
            .input(&account, &rejoined, 2, reward_operation, intent)
            .await
            .action()
            .duplicate
    );
    let dismiss_operation = Uuid::new_v4();
    let dismiss = Ui::Dismiss(game::UiIdentity { id: reward_id });
    let level = restarted
        .endpoint
        .ui(
            &account,
            &rejoined,
            3,
            dismiss_operation,
            None,
            dismiss.clone(),
        )
        .await
        .action()
        .snapshot
        .unwrap();
    let level = level.ui.unwrap().reward.unwrap();
    assert_eq!(level.kind, "level_up");
    assert_eq!(
        level.level,
        Some(2),
        "the actual pre-trained level, never hard-coded product level4"
    );
    assert!(
        restarted
            .endpoint
            .ui(&account, &rejoined, 3, dismiss_operation, None, dismiss)
            .await
            .action()
            .duplicate
    );
    let finished = restarted
        .endpoint
        .ui(
            &account,
            &rejoined,
            4,
            Uuid::new_v4(),
            None,
            Ui::Dismiss(game::UiIdentity { id: level.id }),
        )
        .await
        .action()
        .snapshot
        .unwrap();
    assert!(finished.ui.unwrap().reward.is_none());
    let stored = database.world(pack.world_id).await;
    let character = &stored.state.characters[&ActorId::new(actor).unwrap()];
    assert_eq!(character.quest_points, 1);
    assert_eq!(
        character.skills[&fixtures::id("skill.test.smithing")].xp_tenths,
        1100
    );
    assert!(
        character
            .runtime
            .entitlements
            .contains_key(&fixtures::id("entitlement.test.ui_reward"))
    );
    restarted.stop().await.unwrap();
    database.pool.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn real_death_owned_reclaim_coffer_confirmation_and_discard_are_durable_private_controls() {
    let database = Database::reset().await;
    let pack = recovery_pack();
    let service = Live::start(pack.config(&database)).await;
    let account = service.endpoint.account("ui_recovery").await;
    let actor = ActorId::new(service.endpoint.create(&account).await).unwrap();
    let joined = service.endpoint.join(&account).await;
    let other = service.endpoint.account("ui_recovery_other").await;
    service.endpoint.create(&other).await;
    service.endpoint.join(&other).await;
    let preview = service
        .endpoint
        .ui(
            &account,
            &joined,
            1,
            Uuid::new_v4(),
            None,
            Ui::DeathPreview(game::Empty {}),
        )
        .await
        .action()
        .snapshot
        .unwrap();
    assert_eq!(
        preview.ui.unwrap().kept_on_death.unwrap().scope,
        "normal_unsafe_non_pvp"
    );
    service
        .endpoint
        .input(
            &account,
            &joined,
            2,
            Uuid::new_v4(),
            game::world_input::Action::CloseInterface(game::Empty {}),
        )
        .await
        .action();
    service
        .endpoint
        .input(
            &account,
            &joined,
            3,
            Uuid::new_v4(),
            game::world_input::Action::Interact(game::Interact {
                target: engine_fixtures::spawn("enemy").to_string(),
                action: "use".into(),
            }),
        )
        .await
        .action();
    crate::store::logout(
        &database.pool,
        &crate::crypto::token_digest(&account.token).unwrap(),
    )
    .await
    .unwrap();
    service.stop().await.unwrap();
    let death = finish_disconnected_death(&database, &pack, &actor).await;
    let restarted = Live::start(pack.config(&database)).await;
    let account = restarted.endpoint.relogin(&account).await;
    let joined = restarted.endpoint.join(&account).await;
    let other_joined = restarted.endpoint.join(&other).await;
    let mut sequence = joined.next_sequence;
    let mut snapshot = joined.snapshot.clone().unwrap();
    while let Some(reward) = &snapshot.ui.as_ref().unwrap().reward {
        snapshot = restarted
            .endpoint
            .ui(
                &account,
                &joined,
                sequence,
                Uuid::new_v4(),
                None,
                Ui::Dismiss(game::UiIdentity {
                    id: reward.id.clone(),
                }),
            )
            .await
            .action()
            .snapshot
            .unwrap();
        sequence += 1;
    }
    snapshot = restarted
        .endpoint
        .input(
            &account,
            &joined,
            sequence,
            Uuid::new_v4(),
            game::world_input::Action::OpenDeathOffice(game::Empty {}),
        )
        .await
        .action()
        .snapshot
        .unwrap();
    sequence += 1;
    let panel = &snapshot.recovery.as_ref().unwrap().views[0];
    let selected = panel
        .entries
        .iter()
        .find(|entry| entry.stack.as_ref().unwrap().item == "item.test.bar")
        .unwrap()
        .id
        .clone();
    snapshot = restarted
        .endpoint
        .input(
            &account,
            &joined,
            sequence,
            Uuid::new_v4(),
            game::world_input::Action::Reclaim(game::Reclaim {
                death: death.to_string(),
                storage: game::RecoveryStorage::DeathOffice as i32,
                items: vec![selected],
            }),
        )
        .await
        .action()
        .snapshot
        .unwrap();
    sequence += 1;
    let slot = snapshot
        .player
        .as_ref()
        .unwrap()
        .inventory
        .iter()
        .find(|slot| {
            slot.stack
                .as_ref()
                .is_some_and(|stack| stack.item == "item.test.bar")
        })
        .unwrap()
        .index;
    let offer = restarted
        .endpoint
        .ui(
            &account,
            &joined,
            sequence,
            Uuid::new_v4(),
            None,
            Ui::CofferOffer(game::UiCofferOffer {
                inventory_slot: slot,
                expected_item: "item.test.bar".into(),
                expected_instance: None,
                quantity: 1,
            }),
        )
        .await
        .action()
        .snapshot
        .unwrap()
        .ui
        .unwrap()
        .confirmation
        .unwrap();
    sequence += 1;
    assert_eq!(offer.credit.as_deref(), Some("10500"));
    let operation = Uuid::new_v4();
    let confirm = Ui::Confirmation(game::UiConfirmation {
        id: offer.id.clone(),
        accept: true,
    });
    snapshot = restarted
        .endpoint
        .ui(
            &account,
            &joined,
            sequence,
            operation,
            None,
            confirm.clone(),
        )
        .await
        .action()
        .snapshot
        .unwrap();
    assert!(
        restarted
            .endpoint
            .ui(&account, &joined, sequence, operation, None, confirm)
            .await
            .action()
            .duplicate
    );
    sequence += 1;
    assert_eq!(
        snapshot
            .ui
            .as_ref()
            .unwrap()
            .recovery
            .as_ref()
            .unwrap()
            .coffer_balance,
        "10500"
    );
    restarted
        .endpoint
        .ui(
            &account,
            &joined,
            sequence,
            Uuid::new_v4(),
            None,
            Ui::Confirmation(game::UiConfirmation {
                id: offer.id,
                accept: true,
            }),
        )
        .await
        .error(StatusCode::CONFLICT);
    let discard = snapshot.recovery.as_ref().unwrap().views[0].entries[0]
        .id
        .clone();
    let request = Ui::DiscardRecovery(game::Reclaim {
        death: death.to_string(),
        storage: game::RecoveryStorage::DeathOffice as i32,
        items: vec![discard.clone()],
    });
    restarted
        .endpoint
        .ui(
            &other,
            &other_joined,
            other_joined.next_sequence,
            Uuid::new_v4(),
            None,
            request.clone(),
        )
        .await
        .error(StatusCode::CONFLICT);
    let confirmation = restarted
        .endpoint
        .ui(&account, &joined, sequence, Uuid::new_v4(), None, request)
        .await
        .action()
        .snapshot
        .unwrap()
        .ui
        .unwrap()
        .confirmation
        .unwrap();
    sequence += 1;
    let operation = Uuid::new_v4();
    let confirm = Ui::Confirmation(game::UiConfirmation {
        id: confirmation.id,
        accept: true,
    });
    restarted
        .endpoint
        .ui(
            &account,
            &joined,
            sequence,
            operation,
            None,
            confirm.clone(),
        )
        .await
        .action();
    assert!(
        restarted
            .endpoint
            .ui(&account, &joined, sequence, operation, None, confirm)
            .await
            .action()
            .duplicate
    );
    let private = restarted
        .endpoint
        .poll(&other, &other_joined, 0)
        .await
        .snapshot();
    assert!(private.recovery.is_none() && private.ui.as_ref().unwrap().recovery.is_none());
    assert!(private.ui.as_ref().unwrap().confirmation.is_none());
    let stored = database.world(pack.world_id).await;
    assert!(
        stored.state.runtime.deaths[&death]
            .discarded
            .contains(&RecoveryItemId::new(&discard).unwrap())
    );
    assert_eq!(stored.state.characters[&actor].runtime.death_coffer, 10500);
    restarted.stop().await.unwrap();
    let again = Live::start(pack.config(&database)).await;
    let rejoined = again.endpoint.join(&account).await;
    assert_eq!(rejoined.next_sequence, sequence + 1);
    let stored = database.world(pack.world_id).await;
    assert_eq!(stored.state.characters[&actor].runtime.death_coffer, 10500);
    assert!(
        stored.state.runtime.deaths[&death]
            .discarded
            .contains(&RecoveryItemId::new(discard).unwrap())
    );
    again.stop().await.unwrap();
    database.pool.close().await;
}
