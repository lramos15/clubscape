use clubscape_game_types::{GameIntent, GameplayUiRequest, ProductionMode};
use clubscape_protocol::{game, game_intent, ui_bank_revision, ui_request};
use game::gameplay_ui_request::Request as Ui;
use prost::Message;

fn message(request: Ui, bank: bool) -> game::GameplayUiRequest {
    game::GameplayUiRequest {
        expected_bank_revision: bank.then(|| "9007199254740993".into()),
        request: Some(request),
    }
}

#[test]
fn every_typed_ui_request_round_trips_with_exact_decimal_bank_preconditions() {
    let requests = vec![
        (Ui::Dismiss(game::UiIdentity { id: "ui.1".into() }), false),
        (
            Ui::DocumentPage(game::UiDocumentPage {
                id: "ui.1".into(),
                page: 1,
            }),
            false,
        ),
        (
            Ui::Production(game::UiProductionSelection {
                menu_id: "ui.2".into(),
                recipe: "recipe.test.bar".into(),
                quantity: 1,
                mode: game::ProductionMode::Single as i32,
            }),
            false,
        ),
        (
            Ui::Production(game::UiProductionSelection {
                menu_id: "ui.2".into(),
                recipe: "recipe.test.bar".into(),
                quantity: 1,
                mode: game::ProductionMode::MakeX as i32,
            }),
            false,
        ),
        (
            Ui::ItemAction(game::UiItemAction {
                inventory_slot: 27,
                expected_item: "item.test.potion".into(),
                expected_instance: Some("item_instance.test.selected".into()),
                action: "drink".into(),
            }),
            false,
        ),
        (Ui::SelectTab(game::UiTab { tab: 1 }), true),
        (Ui::CreateTab(game::UiIdentity { id: "5".into() }), true),
        (
            Ui::BankMove(game::UiBankMove {
                entry_id: "5".into(),
                before_entry_id: Some("4".into()),
                tab: 1,
            }),
            true,
        ),
        (Ui::CollapseTab(game::UiTab { tab: 1 }), true),
        (Ui::InsertMode(game::UiToggle { enabled: true }), true),
        (Ui::Placeholders(game::UiToggle { enabled: true }), true),
        (
            Ui::ReleasePlaceholder(game::UiIdentity { id: "4".into() }),
            true,
        ),
        (Ui::Placeholder(game::UiIdentity { id: "5".into() }), true),
        (Ui::DepositEquipment(game::Empty {}), true),
        (
            Ui::BankWithdraw(game::UiBankWithdrawal {
                entry_id: "5".into(),
                quantity: 2147483647,
                noted: true,
            }),
            true,
        ),
        (
            Ui::BankOptions(game::UiBankOptions {
                amount: 10,
                noted: true,
            }),
            true,
        ),
        (Ui::DeathPreview(game::Empty {}), false),
        (
            Ui::DiscardRecovery(game::Reclaim {
                death: "death.test.owned".into(),
                storage: game::RecoveryStorage::Grave as i32,
                items: vec!["recovery_item.test.owned".into()],
            }),
            false,
        ),
        (
            Ui::CofferOffer(game::UiCofferOffer {
                inventory_slot: 1,
                expected_item: "item.test.sacrifice".into(),
                expected_instance: None,
                quantity: 1,
            }),
            false,
        ),
        (
            Ui::Confirmation(game::UiConfirmation {
                id: "ui.3".into(),
                accept: false,
            }),
            false,
        ),
        (
            Ui::PublicChat(game::UiChat {
                channel: "public".into(),
                text: "Hello €".into(),
            }),
            false,
        ),
    ];
    for (request, bank) in requests {
        let wire = message(request, bank);
        let decoded = game::GameplayUiRequest::decode(wire.encode_to_vec().as_slice()).unwrap();
        assert_eq!(wire, decoded);
        let expected = ui_request(&decoded).unwrap();
        assert_eq!(expected.requires_bank_revision(), bank);
        assert_eq!(
            ui_bank_revision(&decoded).unwrap(),
            bank.then_some(9007199254740993)
        );
        let input = game::WorldInput {
            world_session_id: "11111111-1111-4111-8111-111111111111".into(),
            sequence: 1,
            expected_character_revision: None,
            action: Some(game::world_input::Action::Ui(decoded)),
        };
        assert_eq!(
            game_intent(&input).unwrap(),
            GameIntent::Ui { request: expected }
        );
    }
}

#[test]
fn ui_bounds_reject_noncanonical_revisions_slots_modes_and_empty_selections() {
    for revision in ["", "01", "+1", "-1", "1.0", "9223372036854775808"] {
        let mut wire = message(Ui::SelectTab(game::UiTab { tab: 0 }), true);
        wire.expected_bank_revision = Some(revision.into());
        assert!(ui_request(&wire).is_err());
    }
    assert!(ui_request(&message(Ui::SelectTab(game::UiTab { tab: 0 }), false)).is_err());
    assert!(ui_request(&message(Ui::DeathPreview(game::Empty {}), true)).is_err());
    for entry in ["0", "01", "ui.1", "9223372036854775808"] {
        assert!(
            ui_request(&message(
                Ui::CreateTab(game::UiIdentity { id: entry.into() }),
                true
            ))
            .is_err()
        );
    }
    for slot in [28, 256, u32::MAX] {
        assert!(
            ui_request(&message(
                Ui::ItemAction(game::UiItemAction {
                    inventory_slot: slot,
                    expected_item: "item.test.bones".into(),
                    expected_instance: None,
                    action: "bury".into(),
                }),
                false
            ))
            .is_err()
        );
    }
    for (quantity, mode) in [
        (0, game::ProductionMode::Single as i32),
        (2, game::ProductionMode::Single as i32),
        (1, 0),
        (u32::MAX, game::ProductionMode::MakeX as i32),
    ] {
        assert!(
            ui_request(&message(
                Ui::Production(game::UiProductionSelection {
                    menu_id: "ui.2".into(),
                    recipe: "recipe.test.bar".into(),
                    quantity,
                    mode,
                }),
                false
            ))
            .is_err()
        );
    }
    assert!(ui_request(&message(Ui::SelectTab(game::UiTab { tab: 10 }), true)).is_err());
    assert!(
        ui_request(&message(
            Ui::DiscardRecovery(game::Reclaim {
                death: "death.test.owned".into(),
                storage: game::RecoveryStorage::DeathOffice as i32,
                items: Vec::new(),
            }),
            false
        ))
        .is_err()
    );
    assert!(
        ui_request(&message(
            Ui::PublicChat(game::UiChat {
                channel: "private".into(),
                text: "not a public channel".into()
            }),
            false
        ))
        .is_err()
    );
}

#[test]
fn production_mode_and_old_snapshot_absence_are_not_success_shaped_defaults() {
    for (wire, expected) in [
        (game::ProductionMode::Single, ProductionMode::Single),
        (game::ProductionMode::MakeX, ProductionMode::MakeX),
    ] {
        let request = ui_request(&message(
            Ui::Production(game::UiProductionSelection {
                menu_id: "ui.1".into(),
                recipe: "recipe.test.bar".into(),
                quantity: 1,
                mode: wire as i32,
            }),
            false,
        ))
        .unwrap();
        assert!(
            matches!(request, GameplayUiRequest::ProductionSelect { mode, .. } if mode == expected)
        );
    }

    let old = game::WorldSnapshot {
        revision: 9,
        tick: 4,
        ..Default::default()
    };
    assert!(
        game::WorldSnapshot::decode(old.encode_to_vec().as_slice())
            .unwrap()
            .ui
            .is_none()
    );
    assert_eq!(clubscape_protocol::GAMEPLAY_UI_CAPABILITY, "game.ui.v1");
    let view = game::UiXp {
        skill: "skill.test.example".into(),
        amount_tenths: u64::MAX.to_string(),
    };
    assert_eq!(
        game::UiXp::decode(view.encode_to_vec().as_slice())
            .unwrap()
            .amount_tenths,
        u64::MAX.to_string()
    );
}

#[test]
fn inventory_production_transports_an_absent_world_target_not_an_empty_fake_target() {
    let menu = game::UiProduction {
        id: "ui.7".into(),
        interface: "interface.cooking".into(),
        target: None,
        recipes: vec![game::UiProductionChoice {
            recipe: "recipe.cooking.dough".into(),
            ..Default::default()
        }],
    };
    let decoded = game::UiProduction::decode(menu.encode_to_vec().as_slice()).unwrap();
    assert_eq!(decoded, menu);
    assert!(decoded.target.is_none());
}

#[test]
fn observer_motion_action_and_dynamic_objects_keep_optional_and_wide_clock_semantics() {
    let old = game::Player::default();
    assert!(old.running.is_none() && old.movement_tick.is_none() && old.action.is_none());
    let current = game::WorldSnapshot {
        player: Some(game::Player {
            running: Some(true),
            movement_tick: Some("9007199254740993".into()),
            action: Some(game::ActorAction {
                version: 1,
                id: "actor_action.test.7".into(),
                activity: "producing".into(),
                recipe_id: Some("recipe.cooking.dough".into()),
                target: None,
                started_at_tick: "9007199254740993".into(),
                cycle_started_at_tick: "9007199254740993".into(),
                observed_at_tick: "9007199254740993".into(),
                ..Default::default()
            }),
            ..Default::default()
        }),
        dynamic_objects: vec![game::DynamicObject {
            id: "transform.source.door".into(),
            object_id: Some("object.source.door".into()),
            state: Some("object_state.open".into()),
            door_open: Some(true),
            quarter_turns: 1,
            instance: Some("instance.owned.1".into()),
            ..Default::default()
        }],
        ..Default::default()
    };
    let decoded = game::WorldSnapshot::decode(current.encode_to_vec().as_slice()).unwrap();
    assert_eq!(decoded, current);
    assert!(decoded.player.unwrap().action.unwrap().target.is_none());
    assert_eq!(
        decoded.dynamic_objects[0].object_id.as_deref(),
        Some("object.source.door")
    );
}
