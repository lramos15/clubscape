//! Synthetic wire/view fixtures; not a source gameplay or presentation journey.
use clubscape_protocol::{
    Account, ClientMessage, GAME_CAPABILITY, PROTOCOL_VERSION, ServerMessage,
    client_message::Command, game, server_message::Result as Outcome,
};
use clubscape_wasm::{
    Bridge,
    catalog::{DisplayCatalog, DisplayDefinition, ShopDefinition},
};
use prost::Message;
use serde_json::Value;

fn id(value: u32) -> String {
    format!("00000000-0000-4000-8000-{value:012x}")
}
fn response(bridge: &mut Bridge, request: u32, result: Outcome) -> Value {
    serde_json::from_str(
        &bridge
            .receive_for(
                &id(request),
                &ServerMessage {
                    protocol_version: PROTOCOL_VERSION,
                    request_id: id(request),
                    result: Some(result),
                }
                .encode_to_vec(),
            )
            .unwrap(),
    )
    .unwrap()
}
fn position() -> game::Tile {
    game::Tile {
        x: 3200,
        y: 3200,
        plane: 0,
    }
}
fn stack(item: &str, quantity: u32) -> game::Stack {
    game::Stack {
        item: item.into(),
        quantity,
        ..Default::default()
    }
}
fn snapshot(revision: u64) -> game::WorldSnapshot {
    game::WorldSnapshot {
        revision,
        tick: revision,
        character_revision: revision,
        full_snapshot: true,
        next_sequence: 1,
        player: Some(game::Player {
            actor_id: "actor.context_fixture".into(),
            display_name: "context_fixture".into(),
            region: "region.fixture".into(),
            tile: Some(position()),
            presence: Some(game::Presence {
                kind: game::PresenceKind::Connected as i32,
                connected: true,
                accepts_input: true,
                present_in_world: true,
            }),
            ..Default::default()
        }),
        ..Default::default()
    }
}
fn client() -> Bridge {
    let mut bridge = Bridge::default();
    bridge.prepare(&id(1), "hello", "{}").unwrap();
    response(
        &mut bridge,
        1,
        Outcome::Hello(clubscape_protocol::ServerHello {
            capabilities: vec![GAME_CAPABILITY.into()],
            gameplay_available: true,
            ..Default::default()
        }),
    );
    bridge
        .prepare(
            &id(2),
            "login",
            r#"{"loginName":"context_fixture","password":"only a protocol test password"}"#,
        )
        .unwrap();
    response(
        &mut bridge,
        2,
        Outcome::LoggedIn(clubscape_protocol::LoggedIn {
            account: Some(Account {
                account_id: id(90),
                login_name: "context_fixture".into(),
            }),
            session_token: "S".repeat(43),
            expires_at_unix_ms: 99,
        }),
    );
    bridge.prepare(&id(3), "join", "{}").unwrap();
    response(
        &mut bridge,
        3,
        Outcome::WorldJoined(game::WorldJoined {
            world_session_id: id(100),
            next_sequence: 1,
            content_revision: "context-fixture".into(),
            content_manifest_path: "/content/manifest.json".into(),
            snapshot: Some(snapshot(1)),
        }),
    );
    let mut catalog = DisplayCatalog {
        content_revision: "context-fixture".into(),
        ..Default::default()
    };
    for (id, source_id) in [
        ("item.fixture.coins", 1),
        ("item.fixture.tool", 2),
        ("item.fixture.other", 3),
    ] {
        catalog.items.insert(
            id.into(),
            DisplayDefinition {
                name: id.into(),
                source_id: Some(source_id),
                asset: None,
            },
        );
    }
    catalog.entities.insert(
        "object.fixture".into(),
        DisplayDefinition {
            name: "Fixture object".into(),
            source_id: Some(4),
            asset: None,
        },
    );
    catalog.shops.insert(
        "shop.fixture".into(),
        ShopDefinition {
            name: "Fixture shop".into(),
            currency: "item.fixture.coins".into(),
        },
    );
    bridge
        .set_catalog(&serde_json::to_string(&catalog).unwrap())
        .unwrap();
    bridge
}
fn poll(bridge: &mut Bridge, request: u32, snapshot: game::WorldSnapshot) -> Value {
    bridge.prepare(&id(request), "poll", "{}").unwrap();
    response(bridge, request, Outcome::WorldSnapshot(snapshot))
}
fn allowed() -> game::Permission {
    game::Permission {
        allowed: true,
        denial: None,
    }
}

#[test]
fn dynamic_objects_resolve_only_canonical_metadata_and_preserve_public_source_state() {
    let mut bridge = client();
    let mut world = snapshot(2);
    world.dynamic_objects = vec![
        game::DynamicObject {
            id: "transform.fixture.9398".into(),
            definition_id: "object.fixture.other".into(),
            object_id: Some("object.fixture".into()),
            tile: Some(position()),
            instance: Some("instance.fixture".into()),
            state: Some("object_state.fixture.open".into()),
            door_open: Some(true),
            quarter_turns: 3,
            expires_at_tick: Some(u64::MAX),
        },
        game::DynamicObject {
            id: "dynamic_object.fixture.26185".into(),
            definition_id: "object.fixture".into(),
            tile: Some(position()),
            ..Default::default()
        },
    ];
    let value = poll(&mut bridge, 4, world);
    let objects = &value["world"]["dynamicObjects"];
    assert_eq!(objects[0]["id"], "transform.fixture.9398");
    assert_eq!(objects[0]["definitionId"], "object.fixture.other");
    assert_eq!(objects[0]["objectId"], "object.fixture");
    assert_eq!(
        objects[0]["sourceId"], 4,
        "not the id suffix or definition_id"
    );
    assert_eq!(objects[0]["tile"]["x"], 3200);
    assert_eq!(objects[0]["instance"], "instance.fixture");
    assert_eq!(objects[0]["state"], "object_state.fixture.open");
    assert_eq!(objects[0]["doorOpen"], true);
    assert_eq!(objects[0]["quarterTurns"], 3);
    assert_eq!(objects[0]["expiresAtTick"], u64::MAX.to_string());
    assert!(objects[1]["sourceId"].is_null());
    assert!(objects[1]["objectId"].is_null());
    assert!(objects[1]["doorOpen"].is_null());
    assert!(objects[1]["expiresAtTick"].is_null());
}

#[test]
fn unknown_dynamic_metadata_and_invalid_quarter_turns_fail_without_guessing() {
    for (object_id, quarter_turns) in [
        ("object.fixture.missing", 0),
        ("asset.source.osrs.cache2695.object.9398", 0),
        ("npc.fixture", 0),
        ("object.fixture", 4),
    ] {
        let mut bridge = client();
        let mut world = snapshot(2);
        world.dynamic_objects.push(game::DynamicObject {
            id: "transform.fixture".into(),
            object_id: Some(object_id.into()),
            tile: Some(position()),
            quarter_turns,
            ..Default::default()
        });
        bridge.prepare(&id(4), "poll", "{}").unwrap();
        let result = bridge.receive_for(
            &id(4),
            &ServerMessage {
                protocol_version: PROTOCOL_VERSION,
                request_id: id(4),
                result: Some(Outcome::WorldSnapshot(world)),
            }
            .encode_to_vec(),
        );
        assert!(result.is_err(), "{object_id}/{quarter_turns}");
    }
}

#[test]
fn bank_context_gates_private_slots_and_preserves_actual_permissions() {
    let mut bridge = client();
    let mut world = snapshot(2);
    world.bank_context = Some(game::BankContext {
        banker: "spawn.fixture.banker".into(),
        interface: Some("interface.bank".into()),
        deposit: Some(game::Permission {
            allowed: false,
            denial: Some(game::RuleDenial {
                code: game::RuleErrorCode::RequirementNotMet as i32,
                message: "Source deposit lock.".into(),
            }),
        }),
        withdraw: Some(allowed()),
    });
    let player = world.player.as_mut().unwrap();
    player.bank_open = true;
    player.bank_capacity = 4;
    player.bank = vec![game::ItemSlot {
        index: 2,
        stack: Some(stack("item.fixture.coins", 25)),
    }];
    let value = poll(&mut bridge, 4, world);
    assert_eq!(value["world"]["bank"]["capacity"], 4);
    assert_eq!(value["world"]["bank"]["slots"][2]["item"]["quantity"], 25);
    assert_eq!(value["world"]["bank"]["deposit"]["allowed"], false);
    assert_eq!(
        value["world"]["bank"]["deposit"]["denial"]["message"],
        "Source deposit lock."
    );
    let mut closed = snapshot(3);
    closed.player.as_mut().unwrap().bank = vec![game::ItemSlot {
        index: 0,
        stack: Some(stack("item.private.unprojected", 1)),
    }];
    let closed = poll(&mut bridge, 5, closed);
    assert!(closed["world"]["bank"].is_null());
    assert!(!closed.to_string().contains("item.private.unprojected"));
}

#[test]
fn shop_rows_keep_item_ids_stock_zero_and_server_prices_without_inventing_stacks() {
    let mut bridge = client();
    let mut world = snapshot(2);
    world.shop = Some(game::ShopView {
        shop: "shop.fixture".into(),
        interface: Some("interface.shop".into()),
        currency: "item.fixture.coins".into(),
        lines: vec![game::ShopLine {
            index: 7,
            item: "item.fixture.tool".into(),
            stock: 0,
            buy_price: 17,
            sell_price: 3,
        }],
    });
    let value = poll(&mut bridge, 4, world);
    let shop = &value["world"]["shop"];
    assert_eq!(shop["name"], "Fixture shop");
    assert_eq!(shop["rows"][0]["itemId"], "item.fixture.tool");
    assert_eq!(shop["rows"][0]["item"]["id"], "item.fixture.tool");
    assert_eq!(shop["rows"][0]["item"]["quantity"], 0);
    assert_eq!(shop["rows"][0]["stock"], 0);
    assert_eq!(shop["rows"][0]["buyPrice"], 17);
}

#[test]
fn quote_requests_do_not_consume_sequences_and_stale_item_selections_are_not_prices() {
    let mut bridge = client();
    let request = r#"{"kind":"shop_buy","shop":"shop.fixture","itemIndex":0,"expected_item":"item.fixture.tool","quantity":2}"#;
    let wire = bridge.prepare(&id(4), "quote", request).unwrap();
    let Some(Command::PollWorld(poll)) = ClientMessage::decode(wire.as_slice()).unwrap().command
    else {
        panic!()
    };
    assert_eq!(poll.after_revision, 1);
    let selected = poll.quote.unwrap();
    let Some(game::quote_request::Request::ShopBuy(buy)) = &selected.request else {
        panic!()
    };
    assert_eq!(buy.expected_item.as_deref(), Some("item.fixture.tool"));
    assert!(matches!(
        clubscape_protocol::quote_request(&selected).unwrap(),
        clubscape_protocol::ReadOnlyQuote::ShopBuy {
            expected_item: Some(_),
            ..
        }
    ));
    let mut world = snapshot(2);
    world.quote = Some(game::Quote {
        result: Some(game::quote::Result::Shop(game::ShopQuote {
            shop: "shop.fixture".into(),
            item: "item.fixture.other".into(),
            requested: 2,
            quantity: 2,
            currency: "item.fixture.coins".into(),
            total_price: 80,
            stock_after: 0,
            partial_reason: None,
        })),
    });
    let mismatched = response(&mut bridge, 4, Outcome::WorldSnapshot(world.clone()));
    assert!(mismatched["quote"].is_null());
    assert!(mismatched["quoteError"].is_string());
    assert_eq!(mismatched["world"]["revision"], "2");
    assert_eq!(mismatched["nextSequence"], "1");
    bridge.prepare(&id(5), "quote", request).unwrap();
    let Some(game::quote::Result::Shop(quote)) = world.quote.as_mut().unwrap().result.as_mut()
    else {
        panic!()
    };
    quote.item = "item.fixture.tool".into();
    quote.quantity = 1;
    quote.total_price = 17;
    quote.partial_reason = Some(game::RuleDenial {
        code: game::RuleErrorCode::InsufficientItems as i32,
        message: "Source partial quote.".into(),
    });
    let matched = response(&mut bridge, 5, Outcome::WorldSnapshot(world));
    assert_eq!(matched["quote"]["itemId"], "item.fixture.tool");
    assert_eq!(matched["quote"]["quantity"], 1);
    assert_eq!(matched["quote"]["totalPrice"], 17);
    assert_eq!(
        matched["quote"]["partialReason"]["message"],
        "Source partial quote."
    );
    assert_eq!(matched["nextSequence"], "1");
    assert!(
        serde_json::from_str::<Value>(&bridge.state().unwrap()).unwrap()["quote"].is_null(),
        "Quotes are correlated replies, not sticky prices."
    );
}

#[test]
fn all_authorized_recovery_panels_layouts_and_u64_fees_survive_without_rounding() {
    let mut bridge = client();
    let mut world = snapshot(2);
    world.recovery = Some(game::RecoveryContext {
        views: vec![
            game::RecoveryView {
                death: "death.fixture.first".into(),
                storage: game::RecoveryStorage::Grave as i32,
                interface: Some("interface.grave".into()),
                active_ticks_remaining: Some(9),
                entries: vec![game::RecoveryEntry {
                    id: "recovery_item.fixture.first".into(),
                    stack: Some(stack("item.fixture.tool", 1)),
                    full_entry_fee: u64::MAX,
                    current_storage: game::RecoveryStorage::Grave as i32,
                    layout: Some(game::ItemLayout {
                        location: Some(game::item_layout::Location::InventorySlot(3)),
                    }),
                }],
            },
            game::RecoveryView {
                death: "death.fixture.second".into(),
                storage: game::RecoveryStorage::DeathOffice as i32,
                entries: vec![game::RecoveryEntry {
                    id: "recovery_item.fixture.second".into(),
                    stack: Some(stack("item.fixture.other", 2)),
                    full_entry_fee: 0,
                    current_storage: game::RecoveryStorage::DeathOffice as i32,
                    layout: Some(game::ItemLayout {
                        location: Some(game::item_layout::Location::EquipmentSlot(
                            "slot.weapon".into(),
                        )),
                    }),
                }],
                ..Default::default()
            },
        ],
    });
    let value = poll(&mut bridge, 4, world);
    assert!(
        value["world"]["recovery"].is_null(),
        "The legacy one-panel field cannot hide another authorized view."
    );
    let views = value["world"]["recoveryContext"]["views"]
        .as_array()
        .unwrap();
    assert_eq!(views.len(), 2);
    assert_eq!(views[0]["items"][0]["fullEntryFee"], u64::MAX.to_string());
    assert!(views[0]["items"][0]["cost"].is_null());
    assert_eq!(views[1]["items"][0]["fullEntryFee"], "0");
    assert_eq!(views[1]["items"][0]["layout"]["slot"], "slot.weapon");
    bridge.prepare(&id(5), "quote", r#"{"kind":"recovery","death":"death.fixture.first","storage":"grave","items":["recovery_item.fixture.first"]}"#).unwrap();
    let mut world = snapshot(3);
    world.quote = Some(game::Quote {
        result: Some(game::quote::Result::Recovery(game::RecoveryQuote {
            death: "death.fixture.first".into(),
            storage: game::RecoveryStorage::Grave as i32,
            selected: vec!["recovery_item.fixture.first".into()],
            full_selection_fee: u64::MAX,
        })),
    });
    let value = response(&mut bridge, 5, Outcome::WorldSnapshot(world));
    assert_eq!(value["quote"]["fullSelectionFee"], u64::MAX.to_string());
}

#[test]
fn authoritative_options_presence_and_temporary_targets_are_not_reconstructed() {
    let mut bridge = client();
    let mut world = snapshot(2);
    world.entities.push(game::Entity {
        id: "dynamic_object.fixture".into(),
        definition_id: "object.fixture".into(),
        kind: game::EntityKind::Object as i32,
        tile: Some(position()),
        actions_evaluated: true,
        actions: vec!["Examine".into()],
        asset: Some("asset.fixture.object".into()),
        width: 2,
        height: 1,
        interaction_options: vec![
            game::InteractionOption {
                name: "Use".into(),
                permission: Some(game::Permission {
                    allowed: false,
                    denial: Some(game::RuleDenial {
                        code: game::RuleErrorCode::OutOfReach as i32,
                        message: "Actual source reach denial.".into(),
                    }),
                }),
            },
            game::InteractionOption {
                name: "Examine".into(),
                permission: Some(allowed()),
            },
        ],
        ..Default::default()
    });
    let value = poll(&mut bridge, 4, world);
    assert_eq!(value["world"]["entities"][0]["kind"], "temporary_object");
    assert_eq!(
        value["world"]["entities"][0]["actions"][0]["reason"],
        "Actual source reach denial."
    );
    assert_eq!(value["world"]["entities"][0]["actions"][1]["allowed"], true);
    let mut offline = snapshot(3);
    offline.player.as_mut().unwrap().presence = Some(game::Presence {
        kind: game::PresenceKind::Offline as i32,
        connected: false,
        accepts_input: false,
        present_in_world: false,
    });
    let value = poll(&mut bridge, 5, offline);
    assert_eq!(value["world"]["player"]["presence"]["kind"], "offline");
    assert!(
        bridge
            .submit(&id(6), r#"{"kind":"cancel_activity"}"#)
            .is_err()
    );
    assert_eq!(
        serde_json::from_str::<Value>(&bridge.state().unwrap()).unwrap()["nextSequence"],
        "1"
    );
}

#[test]
fn unknown_lifecycle_outcomes_keep_the_original_uuid_and_lease_without_rejoining() {
    let mut bridge = client();
    let original_leave = bridge.prepare(&id(4), "leave", "{}").unwrap();
    assert!(bridge.retry_lifecycle().is_err());
    bridge.transport_lost();
    assert!(bridge.prepare(&id(5), "join", "{}").is_err());
    assert_eq!(bridge.retry_lifecycle().unwrap().unwrap(), original_leave);
    response(&mut bridge, 4, Outcome::WorldLeft(game::WorldLeft {}));
    assert!(bridge.retry_lifecycle().unwrap().is_none());
    let original_logout = bridge.prepare(&id(5), "logout", "{}").unwrap();
    let unavailable = ServerMessage {
        protocol_version: PROTOCOL_VERSION,
        request_id: id(5),
        result: Some(Outcome::Error(clubscape_protocol::Error {
            code: clubscape_protocol::ErrorCode::Unavailable as i32,
            message: "Lifecycle outcome is unknown.".into(),
            error_id: id(99),
            retry_after_seconds: 1,
        })),
    }
    .encode_to_vec();
    assert!(bridge.receive_for(&id(5), &unavailable).is_err());
    assert_eq!(bridge.retry_lifecycle().unwrap().unwrap(), original_logout);
    let final_state = response(
        &mut bridge,
        5,
        Outcome::LoggedOut(clubscape_protocol::LoggedOut {}),
    );
    assert_eq!(final_state["authenticated"], false);
    assert!(bridge.authorization().is_none());
}
