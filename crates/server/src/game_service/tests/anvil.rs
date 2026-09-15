use std::io::Read;

use clubscape_content::{CompiledContent, load_compiled};
use serde::Deserialize;

use super::*;

#[derive(Deserialize)]
struct Checkpoint {
    stage: StageId,
    target: SpawnId,
    tile: Tile,
    action: String,
    sequence: u64,
    operation: Uuid,
    tick: u64,
    interface: InterfaceId,
    source_interface: u32,
    source_object: u32,
    expected_stage: StageId,
    recipe: RecipeId,
    inventory: Vec<InventoryLine>,
    xp_tenths: BTreeMap<SkillId, u64>,
    all_other_skill_xp_tenths: u64,
}

#[derive(Deserialize)]
struct InventoryLine {
    slot: usize,
    item: ItemId,
    quantity: u32,
}

struct NoDraw;
impl clubscape_world_engine::RandomSource for NoDraw {
    fn draw_below(&mut self, _: u32) -> GameResult<u32> {
        panic!("this source control must not roll a gameplay outcome")
    }
}

fn source() -> (Arc<CompiledContent>, Arc<WorldEngine>, Arc<Checkpoint>) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let file = fs::File::open(root.join("content/m1/game-content.csc.gz")).unwrap();
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(file)
        .take(clubscape_content::MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() <= clubscape_content::MAX_INPUT_BYTES);
    let compiled = Arc::new(load_compiled(&bytes, ValidationMode::Runtime).unwrap());
    assert_eq!(
        compiled.definition().schema_version,
        4,
        "this regression must use actual UI4, not a v3 artifact"
    );
    assert!(compiled.definition().ui.is_some());
    let engine = Arc::new(WorldEngine::new(Arc::new(compiled.definition().clone())).unwrap());
    let checkpoint: Checkpoint = serde_json::from_slice(
        &fs::read(root.join("research/interface-contracts/anvil-checkpoint.json")).unwrap(),
    )
    .unwrap();
    let SpawnKind::Object { object } = &compiled.definition().spawns[&checkpoint.target].kind
    else {
        panic!()
    };
    assert_eq!(
        compiled.definition().objects[object].source_id,
        checkpoint.source_object
    );
    assert_eq!(
        compiled.definition().interfaces[&checkpoint.interface].source_ids,
        vec![checkpoint.source_interface]
    );
    (compiled, engine, Arc::new(checkpoint))
}

fn apply_checkpoint(engine: &WorldEngine, character: &mut CharacterState, checkpoint: &Checkpoint) {
    character.region = engine.content().spawns[&checkpoint.target].region.clone();
    character.tile = checkpoint.tile;
    character.tutorial_stage = checkpoint.stage.clone();
    character.inventory = Inventory::default();
    for line in &checkpoint.inventory {
        character.inventory.slots[line.slot] = Some(ItemStack {
            item: line.item.clone(),
            quantity: Quantity::new(line.quantity).unwrap(),
            instance: None,
        });
    }
    for (id, state) in &mut character.skills {
        state.xp_tenths = checkpoint
            .xp_tenths
            .get(id)
            .copied()
            .unwrap_or(checkpoint.all_other_skill_xp_tenths);
        state.current_level = clubscape_simulation::skills::level_for_xp(
            &engine.content().skills[id],
            state.xp_tenths,
        )
        .unwrap();
    }
    if !character.interfaces.contains(&checkpoint.interface) {
        character.interfaces.push(checkpoint.interface.clone());
    }
}

fn request(checkpoint: &Checkpoint, sequence: u64, session: Uuid) -> (ClientMessage, GameIntent) {
    let message = ClientMessage {
        protocol_version: PROTOCOL_VERSION,
        request_id: checkpoint.operation.to_string(),
        command: Some(client_message::Command::WorldInput(game::WorldInput {
            world_session_id: session.to_string(),
            sequence,
            expected_character_revision: None,
            action: Some(game::world_input::Action::Interact(game::Interact {
                target: checkpoint.target.to_string(),
                action: checkpoint.action.clone(),
            })),
        })),
    };
    let decoded = ClientMessage::decode(message.encode_to_vec().as_slice()).unwrap();
    clubscape_protocol::validate_client_message(&decoded).unwrap();
    let Some(client_message::Command::WorldInput(input)) = &decoded.command else {
        panic!()
    };
    let intent = clubscape_protocol::game_intent(input).unwrap();
    assert!(
        matches!(intent, GameIntent::Interact { .. }),
        "no generic-interface or direct-production bypass"
    );
    (decoded, intent)
}

fn assert_open(
    compiled: &CompiledContent,
    engine: &WorldEngine,
    checkpoint: &Checkpoint,
    before: &CharacterState,
    world: &crate::game_storage::WorldSnapshot,
    actor: &ActorId,
    revision: u64,
) -> game::WorldSnapshot {
    let character = &world.state.characters[actor];
    assert_eq!(character.tutorial_stage, checkpoint.expected_stage);
    assert_eq!(
        character.inventory, before.inventory,
        "opening the menu neither consumes a bar nor grants a dagger"
    );
    assert_eq!(character.skills, before.skills);
    assert_eq!(character.runtime.entitlements, before.runtime.entitlements);
    assert!(
        matches!(character.activity, Activity::Idle),
        "recipe selection has not happened"
    );
    let snapshot =
        super::super::view::snapshot(compiled, engine, world, actor, revision, None).unwrap();
    let menu = snapshot.ui.as_ref().unwrap().production.as_ref().unwrap();
    assert_eq!(menu.interface, checkpoint.interface.as_str());
    assert_eq!(
        menu.target.as_ref().unwrap().target,
        Some(game::world_target::Target::Spawn(
            checkpoint.target.to_string()
        ))
    );
    assert!(
        menu.recipes
            .iter()
            .any(|recipe| recipe.recipe == checkpoint.recipe.as_str()
                && recipe.single.as_ref().unwrap().allowed
                && recipe.make_x.as_ref().unwrap().allowed)
    );
    snapshot
}

#[test]
fn actual_anvil_checkpoint_opens_source_312_and_advances_only_the_menu_hook_via_legacy_protocol() {
    let (compiled, engine, checkpoint) = source();
    let actor: ActorId = fixtures::id("actor.canonical.anvil");
    let mut world = engine.initial_world().unwrap();
    world.tick = checkpoint.tick;
    let mut character = engine
        .character_from_initial(actor.clone(), "Canonical anvil probe", BTreeMap::new())
        .unwrap();
    apply_checkpoint(&engine, &mut character, &checkpoint);
    character.last_action_tick = checkpoint.tick - 1;
    character.last_command_sequence = checkpoint.sequence - 1;
    world.characters.insert(actor.clone(), character);
    engine
        .apply_lifecycle(&mut world, &actor, LifecycleTransition::Join)
        .unwrap();
    let (_, intent) = request(&checkpoint, checkpoint.sequence, Uuid::new_v4());
    for denied in [
        "locked_interface",
        "wrong_stage",
        "out_of_reach",
        "generic_interface",
    ] {
        let mut rejected = world.clone();
        match denied {
            "locked_interface" => rejected
                .characters
                .get_mut(&actor)
                .unwrap()
                .interfaces
                .retain(|id| id != &checkpoint.interface),
            "wrong_stage" => {
                rejected.characters.get_mut(&actor).unwrap().tutorial_stage =
                    StageId::new("stage.tutorial.mining_hammer").unwrap()
            }
            "out_of_reach" => {
                rejected.characters.get_mut(&actor).unwrap().tile =
                    Tile::new(3070, 9498, 0).unwrap()
            }
            _ => {}
        }
        let before = rejected.clone();
        let action = if denied == "generic_interface" {
            GameIntent::OpenInterface {
                interface: checkpoint.interface.clone(),
            }
        } else {
            intent.clone()
        };
        assert!(
            engine
                .apply_intent(&mut rejected, &actor, &action, &mut NoDraw)
                .is_err(),
            "{denied}"
        );
        assert_eq!(
            rejected, before,
            "{denied} must not open a menu or consume progress"
        );
    }
    let before = world.characters[&actor].clone();
    let events = engine
        .apply_intent(&mut world, &actor, &intent, &mut NoDraw)
        .unwrap();
    assert!(events.iter().any(|event| matches!(&event.event, GameEvent::InterfaceOpened { interface } if interface == &checkpoint.interface)));
    assert!(events.iter().any(|event| matches!(&event.event, GameEvent::TutorialAdvanced { stage } if stage == &checkpoint.expected_stage)));
    let world = crate::game_storage::WorldSnapshot {
        world_id: Uuid::new_v4(),
        state: world,
        last_tick: None,
    };
    let snapshot = assert_open(&compiled, &engine, &checkpoint, &before, &world, &actor, 1);
    let decoded = game::WorldSnapshot::decode(snapshot.encode_to_vec().as_slice()).unwrap();
    assert_eq!(decoded, snapshot);
}

#[test]
fn canonical_first_death_exposes_the_actual_private_template_and_four_existing_chunks() {
    let (_, engine, _) = source();
    let actor: ActorId = fixtures::id("actor.canonical.scene");
    let other: ActorId = fixtures::id("actor.canonical.other");
    let policy = engine.content().mechanics.death.as_ref().unwrap();
    let location = policy.respawn.require().unwrap();
    let mut character = engine
        .character_from_initial(actor.clone(), "Scene", BTreeMap::new())
        .unwrap();
    character.region = location.region.clone();
    character.tile = location.tile;
    character.tutorial_stage = fixtures::id("stage.tutorial.mainland");
    for slot in &mut character.inventory.slots[..4] {
        *slot = Some(ItemStack {
            item: fixtures::id("item.bones"),
            quantity: Quantity::new(1).unwrap(),
            instance: None,
        });
    }
    let mut world = engine.initial_world().unwrap();
    world.characters.insert(actor.clone(), character);
    world.characters.insert(
        other.clone(),
        engine
            .character_from_initial(other.clone(), "Other", BTreeMap::new())
            .unwrap(),
    );
    for id in [&actor, &other] {
        engine
            .apply_lifecycle(&mut world, id, LifecycleTransition::Join)
            .unwrap();
    }
    world.characters.get_mut(&actor).unwrap().hitpoints = 0;
    let timing = policy.timing.require().unwrap();
    for _ in 0..=timing.dying_ticks + timing.respawn_ticks + 2 {
        let context = engine.tick_context(&world).unwrap();
        engine
            .tick_with_context(&mut world, &mut engine_fixtures::v2::Hits(0), &context)
            .unwrap();
        if matches!(
            world.characters[&actor].runtime.life,
            LifeState::FirstDeathOffice { .. }
        ) {
            break;
        }
    }
    assert!(matches!(
        world.characters[&actor].runtime.life,
        LifeState::FirstDeathOffice { .. }
    ));
    let before = world.clone();
    let scene = engine.scene_view(&world, &actor).unwrap();
    assert_eq!(scene.instance, world.characters[&actor].runtime.instance);
    assert_eq!(
        scene.instance_template,
        Some(fixtures::id("instance_template.death.office"))
    );
    let id = scene.instance.as_ref().unwrap();
    assert_eq!(world.runtime.instances[id].owner.as_ref(), Some(&actor));
    let template = &engine.content().mechanics.instances[scene.instance_template.as_ref().unwrap()];
    assert_eq!(template.chunk_size, 8);
    assert!(template.private_to_character);
    assert_eq!(template.chunks.len(), 4);
    let origins: BTreeSet<_> = template
        .chunks
        .iter()
        .map(|chunk| {
            assert_eq!(chunk.source_origin, chunk.destination_origin);
            assert_eq!(chunk.source_region, chunk.destination_region);
            assert_eq!(chunk.quarter_turns, 0);
            (
                chunk.source_origin.x(),
                chunk.source_origin.y(),
                chunk.source_origin.plane(),
            )
        })
        .collect();
    assert_eq!(
        origins,
        BTreeSet::from([
            (3168, 5720, 0),
            (3168, 5728, 0),
            (3176, 5720, 0),
            (3176, 5728, 0)
        ])
    );
    assert!(
        engine
            .scene_view(&world, &other)
            .unwrap()
            .instance_template
            .is_none()
    );
    assert_eq!(world, before);
}

#[test]
fn canonical_earned_level_uses_real_chat_payload_and_exact_native_popup_association() {
    let (_, engine, _) = source();
    let content = engine.content();
    assert_eq!(
        content.interfaces[&fixtures::id("interface.level_up")].source_ids,
        vec![233]
    );
    assert_eq!(
        content.interfaces[&fixtures::id("interface.level_up_notification")].source_ids,
        vec![660]
    );
    let native: serde_json::Value = serde_json::from_slice(
        &fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../research/interface-contracts/level-up-native.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        native["groups"]["LevelupDisplay"]["widgets"]["TEXT1"]["definition"]["fontId"],
        497
    );
    assert_eq!(
        native["groups"]["NotificationDisplay"]["symbols"]["TITLE_TEXT"],
        43253764
    );
    let actor: ActorId = fixtures::id("actor.canonical.level");
    let skill: SkillId = fixtures::id("skill.prayer");
    let threshold = content.skills[&skill].xp_thresholds_tenths[1];
    let mut character = engine
        .character_from_initial(actor.clone(), "Level", BTreeMap::new())
        .unwrap();
    character.tutorial_stage = fixtures::id("stage.tutorial.mainland");
    character.skills.get_mut(&skill).unwrap().xp_tenths = threshold - 45;
    character.inventory.slots[0] = Some(ItemStack {
        item: fixtures::id("item.bones"),
        quantity: Quantity::new(1).unwrap(),
        instance: None,
    });
    let mut world = engine.initial_world().unwrap();
    world.characters.insert(actor.clone(), character);
    engine
        .apply_lifecycle(&mut world, &actor, LifecycleTransition::Join)
        .unwrap();
    let before = world.characters[&actor].clone();
    engine
        .apply_intent(
            &mut world,
            &actor,
            &GameIntent::Ui {
                request: GameplayUiRequest::ItemAction {
                    inventory_slot: 0,
                    expected_item: fixtures::id("item.bones"),
                    expected_instance: None,
                    action: "bury".into(),
                },
            },
            &mut NoDraw,
        )
        .unwrap();
    for _ in 0..2 {
        let context = engine.tick_context(&world).unwrap();
        engine
            .tick_with_context(&mut world, &mut engine_fixtures::v2::Hits(0), &context)
            .unwrap();
    }
    let view = engine.ui_view(&world, &actor).unwrap();
    let reward = view.reward.as_ref().unwrap();
    assert_eq!(reward.kind, RewardUiKind::LevelUp);
    assert_eq!(reward.interface.as_str(), "interface.level_up");
    assert_eq!(reward.skill.as_ref(), Some(&skill));
    assert_eq!(reward.level, Some(2));
    assert!(reward.title.contains("Prayer") && reward.lines[0].contains('2'));
    assert!(reward.quest.is_none() && reward.items.is_empty() && reward.xp.is_empty());
    assert_eq!(world.characters[&actor].skills[&skill].xp_tenths, threshold);
    assert_eq!(world.characters[&actor].quest_points, before.quest_points);
    assert_eq!(
        world.characters[&actor].runtime.entitlements,
        before.runtime.entitlements
    );
    let wire = super::super::ui_wire::view(view).unwrap();
    assert_eq!(wire.reward.unwrap().interface, "interface.level_up");
    let restored: WorldState =
        serde_json::from_slice(&serde_json::to_vec(&world).unwrap()).unwrap();
    assert_eq!(
        engine.ui_view(&restored, &actor).unwrap().reward,
        engine.ui_view(&world, &actor).unwrap().reward
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires isolated PostgreSQL; run just test-integration"]
async fn canonical_anvil_legacy_command_and_public_menu_receipt_commit_once_in_real_postgresql() {
    let database = Database::reset().await;
    let accounts = Live::start(Config::new(&database.url, "127.0.0.1:0", None).unwrap()).await;
    let account = accounts.endpoint.account("canonical_anvil").await;
    accounts.stop().await.unwrap();
    let (compiled, engine, checkpoint) = source();
    let store = GameStore::new(database.pool.clone());
    let world_id = Uuid::new_v4();
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
    let state = checkpoint.clone();
    let created = store
        .create_character_with(
            &lease,
            authentication,
            SourceCharacter {
                content_revision: compiled.definition().revision.clone(),
                initial_state: compiled.definition().initial_state.clone(),
                appearance: BTreeMap::new(),
            },
            move |character| {
                let derived = create_engine.character_from_initial(
                    character.actor_id.clone(),
                    character.display_name.clone(),
                    character.appearance.clone(),
                )?;
                character.runtime = derived.runtime;
                apply_checkpoint(&create_engine, character, &state);
                Ok(())
            },
        )
        .await
        .unwrap();
    let actor = created.state.actor_id.clone();
    let lifecycle_engine = engine.clone();
    store
        .control_world(&lease, move |world| {
            lifecycle_engine.apply_lifecycle(
                world,
                &created.state.actor_id,
                LifecycleTransition::Join,
            )
        })
        .await
        .unwrap();
    let session = store
        .join_session(world_id, actor.clone(), authentication, PLAYER_LEASE)
        .await
        .unwrap();
    let access = session.access(authentication);
    let (message, intent) = request(&checkpoint, 1, session.session_id);
    let before = store
        .load_character(world_id, authentication)
        .await
        .unwrap()
        .unwrap()
        .state;
    let command = GameCommand {
        operation_id: checkpoint.operation,
        sequence: 1,
        intent,
    };
    let commit_engine = engine.clone();
    let committed = store
        .commit_routed_command(
            &lease,
            &access,
            command.clone(),
            None,
            move |world, actor, intent| {
                commit_engine.apply_intent(world, actor, intent, &mut NoDraw)
            },
        )
        .await
        .unwrap();
    assert!(!committed.commit.duplicate);
    assert!(committed.commit.receipt.routed_events.iter().any(|event|
        matches!(&event.event, GameEvent::InterfaceOpened { interface } if interface == &checkpoint.interface)));
    let mut snapshot = assert_open(
        &compiled,
        &engine,
        &checkpoint,
        &before,
        &committed.snapshot,
        &actor,
        committed.character_revision,
    );
    snapshot.events = committed
        .commit
        .receipt
        .routed_events
        .iter()
        .map(super::super::view::event)
        .collect();
    let response = ServerMessage {
        protocol_version: PROTOCOL_VERSION,
        request_id: message.request_id,
        result: Some(server_message::Result::ActionResult(game::ActionResult {
            sequence: 1,
            operation_id: checkpoint.operation.to_string(),
            duplicate: false,
            snapshot: Some(snapshot),
        })),
    };
    let decoded = ServerMessage::decode(response.encode_to_vec().as_slice()).unwrap();
    assert_eq!(decoded, response);
    assert!(
        store
            .commit_routed_command(&lease, &access, command, None, |_, _, _| panic!(
                "committed menu cannot rerun"
            ))
            .await
            .unwrap()
            .commit
            .duplicate
    );
    let restored = store
        .load_character(world_id, authentication)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored.state.tutorial_stage, checkpoint.expected_stage);
    assert_eq!(restored.state.inventory, before.inventory);
    assert_eq!(restored.last_sequence, 1);
    store.release_world_lease(&lease).await.unwrap();
    database.pool.close().await;
}
