#[path = "support/recovery_context.rs"]
mod fixture;

use clubscape_game_types::{GameIntent, GameplayUiRequest};
use clubscape_protocol::{
    game, game_intent, recovery_context_from_wire, recovery_context_selection_to_wire,
    recovery_context_to_wire, ui_request,
};
use prost::Message;

fn request() -> game::GameplayUiRequest {
    game::GameplayUiRequest {
        expected_bank_revision: None,
        request: Some(game::gameplay_ui_request::Request::RecoveryTakeAll(
            game::UiRecoveryTakeAll {
                selection: Some(recovery_context_selection_to_wire(
                    fixture::context(42).take_all.selection,
                )),
            },
        )),
    }
}

#[test]
fn full_context_round_trips_exact_records_slots_and_large_decimal_quotes() {
    for fee in [42, 9_007_199_254_740_993] {
        let original = fixture::context(fee);
        original.validate_shape().unwrap();
        let wire = recovery_context_to_wire(original.clone());
        let decoded = game::UiRecoveryContext::decode(wire.encode_to_vec().as_slice()).unwrap();
        assert_eq!(decoded, wire);
        assert_eq!(recovery_context_from_wire(&decoded).unwrap(), original);
        let management = game::UiRecoveryManagement {
            context: Some(decoded),
            ..Default::default()
        };
        let bytes = management.encode_to_vec();
        assert_eq!(
            bytes[0], 0x2a,
            "UiRecoveryManagement.context is additive tag5"
        );
        assert_eq!(
            game::UiRecoveryManagement::decode(bytes.as_slice()).unwrap(),
            management
        );
    }
    assert!(
        game::UiRecoveryManagement::decode(&[][..])
            .unwrap()
            .context
            .is_none()
    );
}

#[test]
fn whole_context_request_has_tag_twenty_six_and_one_unchanged_intent() {
    let wire = request();
    let bytes = wire.encode_to_vec();
    assert_eq!(&bytes[..2], &[0xd2, 0x01]);
    let decoded = game::GameplayUiRequest::decode(bytes.as_slice()).unwrap();
    let expected = GameplayUiRequest::RecoveryTakeAll {
        selection: fixture::context(42).take_all.selection,
    };
    assert_eq!(ui_request(&decoded).unwrap(), expected);
    assert!(!expected.requires_bank_revision());
    let input = game::WorldInput {
        world_session_id: "00000000-0000-4000-8000-000000000010".into(),
        sequence: 1,
        expected_character_revision: None,
        action: Some(game::world_input::Action::Ui(decoded.clone())),
    };
    assert_eq!(
        game_intent(&input).unwrap(),
        GameIntent::Ui { request: expected }
    );
    let mut invalid = decoded;
    invalid.expected_bank_revision = Some("9007199254740993".into());
    assert!(ui_request(&invalid).is_err());
}

#[test]
fn selection_rejects_missing_context_zero_quantity_duplicate_ids_and_unknown_storage() {
    let original = request();
    for mutation in 0..6 {
        let mut request = original.clone();
        let Some(game::gameplay_ui_request::Request::RecoveryTakeAll(value)) =
            request.request.as_mut()
        else {
            panic!("whole-context request expected")
        };
        let selection = value.selection.as_mut().unwrap();
        match mutation {
            0 => selection.context = None,
            1 => selection.records[0].entries[0].quantity = 0,
            2 => selection.records[1].death = selection.records[0].death.clone(),
            3 => selection.records[1].entries[0].id = selection.records[0].entries[0].id.clone(),
            4 => selection.records[0].entries[0].current_storage = 0,
            5 => selection.records.clear(),
            _ => unreachable!(),
        }
        assert!(ui_request(&request).is_err(), "mutation {mutation}");
    }
}

#[test]
fn inconsistent_counts_display_totals_slots_and_combined_quotes_are_rejected() {
    let original = recovery_context_to_wire(fixture::context(42));
    for mutation in 0..9 {
        let mut wire = original.clone();
        match mutation {
            0 => wire.version = 2,
            1 => wire.counts.as_mut().unwrap().capacity = 0,
            2 => wire.counts.as_mut().unwrap().stored = 2,
            3 => wire.counts.as_mut().unwrap().native_item_types = Some(2),
            4 => wire.slots[1].slot = 0,
            5 => wire.slots[0].selected_type_caption = None,
            6 => {
                let Some(game::ui_recovery_type_caption::Caption::Source(caption)) = wire.slots[0]
                    .selected_type_caption
                    .as_mut()
                    .unwrap()
                    .caption
                    .as_mut()
                else {
                    panic!("source caption expected")
                };
                caption.quantity = "7".into();
            }
            7 => {
                wire.take_all
                    .as_mut()
                    .unwrap()
                    .plan
                    .as_mut()
                    .unwrap()
                    .total_fee = "588".into()
            }
            8 => {
                wire.take_all
                    .as_mut()
                    .unwrap()
                    .selection
                    .as_mut()
                    .unwrap()
                    .records[1]
                    .entries[0]
                    .quantity = 6
            }
            _ => unreachable!(),
        }
        assert!(
            recovery_context_from_wire(&wire).is_err(),
            "mutation {mutation}"
        );
    }
}
