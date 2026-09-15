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
        panic!("opening a source smithing menu must not roll production")
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
