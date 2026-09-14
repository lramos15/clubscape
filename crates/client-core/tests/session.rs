use clubscape_client_core::{ClientCore, ClientError, ClientEvent, Phase};
use clubscape_protocol::{
    Account, GAME_CAPABILITY, Hello, LoggedIn, Login, PROTOCOL_VERSION, ServerHello, ServerMessage,
    client_message::Command, game, server_message::Result as Outcome,
};

fn id(number: u32) -> String {
    format!("00000000-0000-4000-8000-{number:012x}")
}

fn response(number: u32, result: Outcome) -> ServerMessage {
    ServerMessage {
        protocol_version: PROTOCOL_VERSION,
        request_id: id(number),
        result: Some(result),
    }
}

fn snapshot(revision: u64) -> game::WorldSnapshot {
    game::WorldSnapshot {
        revision,
        character_revision: revision,
        tick: revision,
        player: Some(game::Player {
            actor_id: "actor.test_player".into(),
            display_name: "penguin".into(),
            tile: Some(game::Tile {
                x: 3200,
                y: 3200,
                plane: 0,
            }),
            run_energy: 10000,
            ..Default::default()
        }),
        full_snapshot: true,
        ..Default::default()
    }
}

fn authenticated(gameplay: bool) -> ClientCore {
    let mut core = ClientCore::default();
    core.prepare(&id(1), Command::Hello(Hello {})).unwrap();
    core.handle_response(response(
        1,
        Outcome::Hello(ServerHello {
            capabilities: if gameplay {
                vec![GAME_CAPABILITY.into()]
            } else {
                vec![]
            },
            gameplay_available: gameplay,
            ..Default::default()
        }),
    ))
    .unwrap();
    core.prepare(
        &id(2),
        Command::Login(Login {
            login_name: "penguin".into(),
            password: "private password input".into(),
        }),
    )
    .unwrap();
    core.handle_response(response(
        2,
        Outcome::LoggedIn(LoggedIn {
            account: Some(Account {
                account_id: id(50),
                login_name: "penguin".into(),
            }),
            session_token: "A".repeat(43),
            expires_at_unix_ms: 123,
        }),
    ))
    .unwrap();
    core
}

fn join(core: &mut ClientCore, request: u32, session: u32, next: u64, revision: u64) {
    core.prepare(&id(request), Command::JoinWorld(game::JoinWorld {}))
        .unwrap();
    core.handle_response(response(
        request,
        Outcome::WorldJoined(game::WorldJoined {
            world_session_id: id(session),
            next_sequence: next,
            content_revision: "source-test-only".into(),
            snapshot: Some(snapshot(revision)),
            ..Default::default()
        }),
    ))
    .unwrap();
}

fn joined() -> ClientCore {
    let mut core = authenticated(true);
    join(&mut core, 3, 100, 1, 10);
    core
}

fn walk() -> game::world_input::Action {
    game::world_input::Action::Walk(game::Walk {
        destination: Some(game::Tile {
            x: 3201,
            y: 3200,
            plane: 0,
        }),
        running: false,
    })
}

fn poll(
    core: &mut ClientCore,
    request: u32,
    state: game::WorldSnapshot,
) -> Result<Vec<ClientEvent>, ClientError> {
    core.prepare(
        &id(request),
        Command::PollWorld(game::PollWorld {
            world_session_id: id(100),
            after_revision: 0,
        }),
    )
    .unwrap();
    core.handle_response(response(request, Outcome::WorldSnapshot(state)))
}

#[test]
fn secrets_are_not_debugged_and_unadvertised_gameplay_is_unavailable() {
    let mut core = authenticated(false);
    let debug = format!("{core:?}");
    assert!(!debug.contains(&"A".repeat(43)));
    assert!(!debug.contains("private password input"));
    assert!(
        core.prepare(&id(3), Command::JoinWorld(game::JoinWorld {}))
            .is_err()
    );
    assert_eq!(core.phase(), &Phase::AccountReady);
}

#[test]
fn inputs_require_world_ownership_and_one_outstanding_sequence() {
    let mut core = joined();
    let message = core.submit_action(&id(4), walk()).unwrap();
    let Command::WorldInput(input) = message.command.unwrap() else {
        panic!("wrong command")
    };
    assert_eq!(input.sequence, 1);
    assert_eq!(input.world_session_id, id(100));
    assert!(core.submit_action(&id(5), walk()).is_err());
    assert!(core.retry_uncertain_input().is_err());
}

#[test]
fn stale_updates_do_not_undo_state_or_repeat_events() {
    let mut core = joined();
    let mut newer = snapshot(12);
    newer.events.push(game::Event {
        event_id: "world.12.0".into(),
        kind: "message".into(),
        ..Default::default()
    });
    let first = poll(&mut core, 4, newer.clone()).unwrap();
    assert_eq!(
        first
            .iter()
            .filter(|event| matches!(event, ClientEvent::Gameplay(_)))
            .count(),
        1
    );
    assert!(poll(&mut core, 5, snapshot(11)).unwrap().is_empty());
    let replay = poll(&mut core, 6, newer).unwrap();
    assert!(
        !replay
            .iter()
            .any(|event| matches!(event, ClientEvent::Gameplay(_)))
    );
    assert_eq!(core.snapshot().unwrap().revision, 12);
}

#[test]
fn reconnect_recovers_committed_operation_without_replaying_its_reward() {
    let mut core = joined();
    core.submit_action(&id(4), walk()).unwrap();
    core.transport_lost();
    assert_eq!(core.phase(), &Phase::Reconnecting);
    join(&mut core, 5, 101, 2, 11);
    assert_eq!(core.next_sequence(), Some(2));
    assert!(core.retry_uncertain_input().is_err());
    assert_eq!(core.phase(), &Phase::InWorld);
}

#[test]
fn uncertain_retry_retains_operation_identity_and_intent_with_renewed_session() {
    let mut core = joined();
    let original = core.submit_action(&id(4), walk()).unwrap();
    core.transport_lost();
    join(&mut core, 5, 101, 1, 10);
    let retried = core.retry_uncertain_input().unwrap();
    assert_eq!(retried.request_id, original.request_id);
    let Command::WorldInput(retry) = retried.command.unwrap() else {
        panic!("wrong command")
    };
    let Command::WorldInput(original) = original.command.unwrap() else {
        panic!("wrong command")
    };
    assert_eq!(retry.sequence, original.sequence);
    assert_eq!(retry.action, original.action);
    assert_eq!(retry.world_session_id, id(101));
    assert!(core.retry_uncertain_input().is_err());
}

#[test]
fn malformed_event_batch_is_atomic_and_does_not_poison_deduplication() {
    let mut core = joined();
    let mut invalid = snapshot(11);
    invalid.events.push(game::Event {
        event_id: "world.11.0".into(),
        ..Default::default()
    });
    invalid.events.push(game::Event::default());
    assert!(poll(&mut core, 4, invalid.clone()).is_err());
    assert_eq!(core.snapshot().unwrap().revision, 10);
    invalid.events.pop();
    let events = core
        .handle_response(response(4, Outcome::WorldSnapshot(invalid)))
        .unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, ClientEvent::Gameplay(_)))
    );
}

#[test]
fn delta_entities_preserve_unchanged_entities_and_apply_removals() {
    let mut core = joined();
    let mut initial = snapshot(11);
    initial.entities = vec![
        game::Entity {
            id: "spawn.first".into(),
            name: "first".into(),
            ..Default::default()
        },
        game::Entity {
            id: "spawn.second".into(),
            name: "second".into(),
            ..Default::default()
        },
    ];
    poll(&mut core, 4, initial).unwrap();
    let mut delta = snapshot(12);
    delta.full_snapshot = false;
    delta.entities = vec![game::Entity {
        id: "spawn.third".into(),
        ..Default::default()
    }];
    delta.removed_entities.push("spawn.first".into());
    poll(&mut core, 5, delta).unwrap();
    let ids: Vec<_> = core
        .snapshot()
        .unwrap()
        .entities
        .iter()
        .map(|entity| entity.id.as_str())
        .collect();
    assert_eq!(ids, ["spawn.second", "spawn.third"]);
}

#[test]
fn mismatched_acknowledgement_does_not_consume_the_pending_sequence() {
    let mut core = joined();
    core.submit_action(&id(4), walk()).unwrap();
    assert!(
        core.handle_response(response(
            4,
            Outcome::ActionResult(game::ActionResult {
                sequence: 99,
                operation_id: id(4),
                snapshot: Some(snapshot(11)),
                duplicate: false,
            })
        ))
        .is_err()
    );
    assert_eq!(core.next_sequence(), Some(1));
    core.handle_response(response(
        4,
        Outcome::ActionResult(game::ActionResult {
            sequence: 1,
            operation_id: id(4),
            snapshot: Some(snapshot(11)),
            duplicate: false,
        }),
    ))
    .unwrap();
    assert_eq!(core.next_sequence(), Some(2));
}

#[test]
fn malformed_protocol_and_inventory_state_fail_without_losing_last_good_state() {
    let mut core = joined();
    assert!(core.decode_response(&[0xff, 0xff]).is_err());
    let mut state = snapshot(11);
    state
        .player
        .as_mut()
        .unwrap()
        .inventory
        .push(game::ItemSlot {
            index: 28,
            stack: Some(game::Stack {
                item: "item.test".into(),
                quantity: 1,
            }),
        });
    assert!(poll(&mut core, 4, state).is_err());
    assert_eq!(core.snapshot().unwrap().revision, 10);
}
