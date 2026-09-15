//! Protocol/state fixtures only. These messages do not establish a game journey.
use clubscape_protocol::{
    Account, ClientMessage, GAME_CAPABILITY, PROTOCOL_VERSION, ServerMessage,
    client_message::Command, game, server_message::Result as Outcome,
};
use clubscape_wasm::{Bridge, catalog::*};
use prost::Message;
use serde_json::{Value, json};

fn id(value: u32) -> String {
    format!("00000000-0000-4000-8000-{value:012x}")
}
fn reply(
    bridge: &mut Bridge,
    request: u32,
    result: Outcome,
) -> Result<String, clubscape_wasm::BridgeError> {
    bridge.receive(
        &ServerMessage {
            protocol_version: PROTOCOL_VERSION,
            request_id: id(request),
            result: Some(result),
        }
        .encode_to_vec(),
    )
}
fn snapshot(revision: u64) -> game::WorldSnapshot {
    game::WorldSnapshot {
        revision,
        tick: revision,
        character_revision: revision,
        player: Some(game::Player {
            actor_id: "actor.fixture".into(),
            display_name: "protocol_fixture".into(),
            region: "region.fixture".into(),
            tile: Some(game::Tile {
                x: 3200,
                y: 3200,
                plane: 0,
            }),
            skills: vec![game::Skill {
                id: "skill.fixture".into(),
                xp_tenths: u64::MAX,
                base_level: 1,
                current_level: 1,
            }],
            bank: vec![game::ItemSlot {
                index: 0,
                stack: Some(game::Stack {
                    item: "private.bank.only".into(),
                    quantity: 999,
                    ..Default::default()
                }),
            }],
            settings: vec![game::SetSetting {
                setting: game::SettingKind::Run as i32,
                enabled: false,
            }],
            ..Default::default()
        }),
        full_snapshot: true,
        ..Default::default()
    }
}
fn authenticated() -> Bridge {
    let mut bridge = Bridge::default();
    bridge.prepare(&id(1), "hello", "{}").unwrap();
    reply(
        &mut bridge,
        1,
        Outcome::Hello(clubscape_protocol::ServerHello {
            capabilities: vec![GAME_CAPABILITY.into()],
            gameplay_available: true,
            ..Default::default()
        }),
    )
    .unwrap();
    bridge
        .prepare(
            &id(2),
            "login",
            r#"{"loginName":"protocol_fixture","password":"not a stored password"}"#,
        )
        .unwrap();
    reply(
        &mut bridge,
        2,
        Outcome::LoggedIn(clubscape_protocol::LoggedIn {
            account: Some(Account {
                account_id: id(99),
                login_name: "protocol_fixture".into(),
            }),
            session_token: "s".repeat(43),
            expires_at_unix_ms: 9999,
        }),
    )
    .unwrap();
    bridge
}
fn join(bridge: &mut Bridge, request: u32, next_sequence: u64, revision: u64) {
    bridge.prepare(&id(request), "join", "{}").unwrap();
    reply(
        bridge,
        request,
        Outcome::WorldJoined(game::WorldJoined {
            world_session_id: id(100 + request),
            next_sequence,
            content_revision: "protocol-fixture-v1".into(),
            content_manifest_path: "/content/manifest.json".into(),
            snapshot: Some(snapshot(revision)),
        }),
    )
    .unwrap();
}
fn joined() -> Bridge {
    let mut bridge = authenticated();
    join(&mut bridge, 3, 1, 9_007_199_254_740_993);
    let mut catalog = DisplayCatalog {
        content_revision: "protocol-fixture-v1".into(),
        ..Default::default()
    };
    catalog.skills.insert(
        "skill.fixture".into(),
        DisplayDefinition {
            name: "Fixture skill".into(),
            ..Default::default()
        },
    );
    bridge
        .set_catalog(&serde_json::to_string(&catalog).unwrap())
        .unwrap();
    bridge
}
fn poll(bridge: &mut Bridge, request: u32, snapshot: game::WorldSnapshot) -> String {
    bridge.prepare(&id(request), "poll", "{}").unwrap();
    reply(bridge, request, Outcome::WorldSnapshot(snapshot)).unwrap()
}
fn walk() -> &'static str {
    r#"{"kind":"walk","destination":{"x":3201,"y":3200,"plane":0},"running":false}"#
}

#[test]
fn public_state_keeps_u64_exact_and_excludes_auth_lease_and_closed_bank() {
    let bridge = joined();
    let text = bridge.state().unwrap();
    let state: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(state["world"]["revision"], "9007199254740993");
    assert_eq!(state["world"]["tick"], "9007199254740993");
    assert_eq!(
        state["world"]["player"]["skills"][0]["xpTenths"],
        u64::MAX.to_string()
    );
    assert_eq!(
        state["world"]["player"]["inventory"]
            .as_array()
            .unwrap()
            .len(),
        28
    );
    assert!(state["world"]["bank"].is_null());
    assert!(!text.contains("private.bank.only"));
    assert!(!text.contains("not a stored password"));
    assert!(!text.contains(&"s".repeat(43)));
    assert!(!text.contains(&id(103)));
}

#[test]
fn every_representable_shared_intent_round_trips_through_generated_wire() {
    let cases = vec![
        json!({"kind":"walk","destination":{"x":3201,"y":3200,"plane":0},"running":true}),
        json!({"kind":"interact","target":"spawn.fixture","action":"Talk-to"}),
        json!({"kind":"interact_with","target":{"kind":"temporary_object","object":"dynamic_object.fixture"},"action":"Use"}),
        json!({"kind":"select_dialogue","speaker":"spawn.fixture","choice":"choice.fixture"}),
        json!({"kind":"open_interface","interface":"interface.fixture"}),
        json!({"kind":"close_interface"}),
        json!({"kind":"equip","inventory_slot":0}),
        json!({"kind":"unequip","slot":"slot.fixture"}),
        json!({"kind":"drop","inventory_slot":0,"quantity":1}),
        json!({"kind":"take_ground_item","ground_item_id":"ground.fixture"}),
        json!({"kind":"use_item","inventory_slot":0,"target":{"kind":"inventory","slot":1}}),
        json!({"kind":"use_item","inventory_slot":0,"target":{"kind":"world","spawn":"spawn.fixture"}}),
        json!({"kind":"use_item","inventory_slot":0,"target":{"kind":"temporary_object","object":"dynamic_object.fixture"}}),
        json!({"kind":"use_item","inventory_slot":0,"target":{"kind":"ground","ground_item_id":"ground.fixture"}}),
        json!({"kind":"move_inventory","from":0,"to":1}),
        json!({"kind":"eat","inventory_slot":0}),
        json!({"kind":"produce","recipe":"recipe.fixture","target":"spawn.fixture","quantity":1}),
        json!({"kind":"produce_at","recipe":"recipe.fixture","target":{"kind":"spawn","spawn":"spawn.fixture"},"quantity":1}),
        json!({"kind":"bank_deposit","banker":"spawn.fixture","inventory_slot":0,"quantity":1}),
        json!({"kind":"bank_withdraw","banker":"spawn.fixture","bank_slot":0,"quantity":1,"noted":false}),
        json!({"kind":"shop_sell","shop":"shop.fixture","inventory_slot":0,"quantity":1}),
        json!({"kind":"set_combat_style","style":"style.fixture"}),
        json!({"kind":"cast","spell":"spell.fixture","target":null}),
        json!({"kind":"set_prayer","prayer":"prayer.fixture","enabled":true}),
        json!({"kind":"set_setting","setting":{"setting":"run","enabled":true}}),
        json!({"kind":"set_setting","setting":{"setting":"auto_retaliate","enabled":false}}),
        json!({"kind":"set_setting","setting":{"setting":"death_auto_equip","enabled":true}}),
        json!({"kind":"set_setting","setting":{"setting":"death_supply_piles","enabled":false}}),
        json!({"kind":"confirm_appearance","appearance":{"body":1}}),
        json!({"kind":"select_experience","experience":"experience.fixture"}),
        json!({"kind":"reclaim","death":"death.fixture","storage":"grave","items":["recovery_item.fixture"]}),
        json!({"kind":"cancel_activity"}),
        json!({"kind":"request_logout"}),
        json!({"kind":"produce_selected","recipe":"recipe.fixture","target":null,"quantity":1,"mode":"single"}),
        json!({"kind":"produce_selected","recipe":"recipe.fixture","target":null,"quantity":1,"mode":"make_x"}),
        json!({"kind":"open_grave","death":"death.fixture"}),
        json!({"kind":"open_death_office"}),
    ];
    for case in cases {
        let mut bridge = joined();
        let wire = bridge
            .submit(&id(4), &case.to_string())
            .unwrap_or_else(|error| panic!("{case}: {error:?}"));
        let message = ClientMessage::decode(wire.as_slice()).unwrap();
        let Some(Command::WorldInput(input)) = message.command else {
            panic!("wrong wire command")
        };
        assert_eq!(input.sequence, 1);
        assert_eq!(
            input.expected_character_revision,
            Some(9_007_199_254_740_993)
        );
        let expected: clubscape_game_types::GameIntent = serde_json::from_value(case).unwrap();
        assert_eq!(clubscape_protocol::game_intent(&input).unwrap(), expected);
    }
}

#[test]
fn shop_purchase_identity_is_retained_without_guessing_the_pending_wire_contract() {
    let mut bridge = joined();
    let input = r#"{"kind":"shop_buy","shop":"shop.fixture","item_index":0,"quantity":1}"#;
    assert!(bridge.submit(&id(4), input).is_err());
    let error = bridge
        .submit_selected(&id(4), input, "item.fixture")
        .unwrap_err();
    let error = serde_json::to_value(error).unwrap();
    assert_eq!(error["kind"], "unsupported");
    assert!(error["message"].as_str().unwrap().contains("item.fixture"));
    assert_eq!(
        serde_json::from_str::<Value>(&bridge.state().unwrap()).unwrap()["uncertainInput"],
        false
    );
    assert!(
        bridge.submit(&id(4), walk()).is_ok(),
        "Refused purchases cannot allocate a sequence."
    );
}

#[test]
fn malformed_inputs_and_creation_options_do_not_enter_the_pending_queue() {
    let mut bridge = joined();
    for input in [
        r#"{"kind":"drop","inventory_slot":28,"quantity":1}"#,
        r#"{"kind":"drop","inventory_slot":0,"quantity":0}"#,
        r#"{"kind":"walk","destination":{"x":1.5,"y":3200,"plane":0},"running":false}"#,
        r#"{"kind":"walk","destination":{"x":3200,"y":3200,"plane":4},"running":false}"#,
        r#"{"kind":"set_setting","setting":{"setting":"invented","enabled":true}}"#,
        r#"{"kind":"produce_selected","recipe":"recipe.fixture","target":null,"quantity":2,"mode":"single"}"#,
    ] {
        assert!(bridge.submit(&id(4), input).is_err());
    }
    assert!(
        bridge
            .prepare(&id(5), "create_character", r#"{"body":1}"#)
            .is_err()
    );
    assert!(bridge.submit(&id(4), walk()).is_ok());
}

#[test]
fn reconnect_retries_only_the_same_uncertain_operation_or_proves_its_commit() {
    for committed in [false, true] {
        let mut bridge = joined();
        let original =
            ClientMessage::decode(bridge.submit(&id(4), walk()).unwrap().as_slice()).unwrap();
        bridge.transport_lost();
        join(
            &mut bridge,
            5,
            if committed { 2 } else { 1 },
            9_007_199_254_740_993,
        );
        let retry = bridge.retry().unwrap();
        if committed {
            assert!(retry.is_none());
        } else {
            let retry = ClientMessage::decode(retry.unwrap().as_slice()).unwrap();
            assert_eq!(retry.request_id, original.request_id);
            let (Some(Command::WorldInput(original)), Some(Command::WorldInput(retry))) =
                (original.command, retry.command)
            else {
                panic!()
            };
            assert_eq!(original.action, retry.action);
            assert_eq!(original.sequence, retry.sequence);
            assert_ne!(original.world_session_id, retry.world_session_id);
        }
    }
}

#[test]
fn stale_or_duplicate_replies_do_not_regress_the_view_or_replay_audio() {
    let mut bridge = joined();
    let revision = 9_007_199_254_740_994;
    let mut state = snapshot(revision);
    state.events.push(game::Event {
        event_id: "event.fixture.1".into(),
        kind: "sound".into(),
        sound_asset: "asset.fixture.sound".into(),
        ..Default::default()
    });
    let first: Value = serde_json::from_str(&poll(&mut bridge, 4, state.clone())).unwrap();
    assert_eq!(first["events"].as_array().unwrap().len(), 1);
    let repeat: Value = serde_json::from_str(
        &reply(&mut bridge, 4, Outcome::WorldSnapshot(state.clone())).unwrap(),
    )
    .unwrap();
    assert!(repeat["events"].as_array().unwrap().is_empty());
    let stale: Value = serde_json::from_str(&poll(&mut bridge, 5, snapshot(revision - 1))).unwrap();
    assert_eq!(stale["world"]["revision"], revision.to_string());
    let replay: Value = serde_json::from_str(&poll(&mut bridge, 6, state)).unwrap();
    assert!(replay["events"].as_array().unwrap().is_empty());
}

#[test]
fn failures_keep_error_ids_and_authoritative_permissions_explicit() {
    let mut bridge = joined();
    assert!(bridge.receive(&[0xff, 0x00]).is_err());
    let request = bridge.submit(&id(4), walk()).unwrap();
    let error = reply(
        &mut bridge,
        4,
        Outcome::Error(clubscape_protocol::Error {
            code: clubscape_protocol::ErrorCode::Unavailable as i32,
            message: "The source lifecycle API is not available.".into(),
            error_id: id(60),
            retry_after_seconds: 3,
        }),
    )
    .unwrap_err();
    let error = serde_json::to_value(error).unwrap();
    assert_eq!(error["errorId"], id(60));
    assert_eq!(error["retryAfterSeconds"], 3);
    assert!(
        serde_json::from_str::<Value>(&bridge.state().unwrap()).unwrap()["uncertainInput"]
            .as_bool()
            .unwrap()
    );
    assert!(ClientMessage::decode(request.as_slice()).is_ok());
}

#[test]
fn authorization_revocation_clears_public_and_private_session_state() {
    let mut bridge = joined();
    bridge.prepare(&id(4), "poll", "{}").unwrap();
    assert!(
        reply(
            &mut bridge,
            4,
            Outcome::Error(clubscape_protocol::Error {
                code: clubscape_protocol::ErrorCode::Unauthenticated as i32,
                error_id: id(60),
                message: "Session expired.".into(),
                ..Default::default()
            })
        )
        .is_err()
    );
    let state: Value = serde_json::from_str(&bridge.state().unwrap()).unwrap();
    assert_eq!(state["authenticated"], false);
    assert!(state["world"].is_null());
    assert!(bridge.authorization().is_none());
}

#[test]
fn a_world_cannot_redirect_the_browser_or_replace_its_source_identity() {
    for path in [
        "https://elsewhere.example/content.json",
        "//elsewhere.example/content.json",
        "/content/../private.json",
        "/content/%2e.json",
        "/content/file.json?token=secret",
    ] {
        let mut bridge = authenticated();
        bridge.prepare(&id(3), "join", "{}").unwrap();
        assert!(
            reply(
                &mut bridge,
                3,
                Outcome::WorldJoined(game::WorldJoined {
                    world_session_id: id(103),
                    next_sequence: 1,
                    content_revision: "protocol-fixture-v1".into(),
                    content_manifest_path: path.into(),
                    snapshot: Some(snapshot(1)),
                })
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<Value>(&bridge.state().unwrap()).unwrap()["world"].is_null()
        );
    }
}

#[test]
fn a_duplicate_http_reply_cannot_acknowledge_a_different_pending_operation() {
    let mut bridge = joined();
    bridge.submit(&id(4), walk()).unwrap();
    let old_reply = ServerMessage {
        protocol_version: PROTOCOL_VERSION,
        request_id: id(3),
        result: Some(Outcome::WorldSnapshot(snapshot(9_007_199_254_740_994))),
    }
    .encode_to_vec();
    assert!(bridge.receive_for(&id(4), &old_reply).is_err());
    let state: Value = serde_json::from_str(&bridge.state().unwrap()).unwrap();
    assert_eq!(state["nextSequence"], "1");
    assert_eq!(state["uncertainInput"], true);
    assert_eq!(state["world"]["revision"], "9007199254740993");
}

#[test]
fn malformed_acknowledgements_keep_the_uncertain_operation_recoverable() {
    let mut bridge = joined();
    bridge.submit(&id(4), walk()).unwrap();
    assert!(
        reply(
            &mut bridge,
            4,
            Outcome::ActionResult(game::ActionResult {
                sequence: 77,
                operation_id: id(4),
                snapshot: Some(snapshot(9_007_199_254_740_994)),
                duplicate: false,
            })
        )
        .is_err()
    );
    bridge.transport_lost();
    join(&mut bridge, 5, 1, 9_007_199_254_740_993);
    let retry = ClientMessage::decode(bridge.retry().unwrap().unwrap().as_slice()).unwrap();
    assert_eq!(retry.request_id, id(4));
}

#[test]
fn source_models_are_not_fabricated_icons_and_unevaluated_permissions_stay_unavailable() {
    let mut bridge = joined();
    let mut catalog = DisplayCatalog {
        content_revision: "protocol-fixture-v1".into(),
        ..Default::default()
    };
    catalog
        .skills
        .insert("skill.fixture".into(), DisplayDefinition::default());
    catalog.items.insert(
        "item.fixture".into(),
        DisplayDefinition {
            name: "Fixture item".into(),
            source_id: Some(1),
            asset: Some("asset.fixture.model".into()),
        },
    );
    catalog.entities.insert(
        "npc.fixture".into(),
        DisplayDefinition {
            name: "Fixture NPC".into(),
            source_id: Some(2),
            asset: Some("asset.fixture.npc".into()),
        },
    );
    bridge
        .set_catalog(&serde_json::to_string(&catalog).unwrap())
        .unwrap();
    let mut value = snapshot(9_007_199_254_740_994);
    value
        .player
        .as_mut()
        .unwrap()
        .inventory
        .push(game::ItemSlot {
            index: 0,
            stack: Some(game::Stack {
                item: "item.fixture".into(),
                quantity: 1,
                ..Default::default()
            }),
        });
    value.entities.push(game::Entity {
        id: "spawn.fixture".into(),
        definition_id: "npc.fixture".into(),
        kind: game::EntityKind::Npc as i32,
        tile: Some(game::Tile {
            x: 3200,
            y: 3200,
            plane: 0,
        }),
        actions: vec!["Talk-to".into()],
        actions_evaluated: false,
        ..Default::default()
    });
    value.ground_items.push(game::GroundItem {
        id: "ground.fixture".into(),
        tile: Some(game::Tile {
            x: 3200,
            y: 3200,
            plane: 0,
        }),
        stack: Some(game::Stack {
            item: "item.fixture".into(),
            quantity: 1,
            ..Default::default()
        }),
        can_take: true,
        permissions_evaluated: false,
        ..Default::default()
    });
    let state: Value = serde_json::from_str(&poll(&mut bridge, 4, value)).unwrap();
    assert!(state["world"]["player"]["inventory"][0]["item"]["iconAsset"].is_null());
    assert_eq!(
        state["world"]["entities"][0]["actions"][0]["allowed"],
        false
    );
    assert_eq!(state["world"]["groundItems"][0]["canTake"], false);
    assert!(
        state["world"]["unavailableViews"]
            .as_array()
            .unwrap()
            .iter()
            .any(|view| view["view"] == "inventory_actions")
    );
}
