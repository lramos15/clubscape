use clubscape_game_types::{CharacterSetting, GameIntent, ItemTarget, WorldTarget};
use clubscape_protocol::{game, game_intent};
use prost::Message;

fn input(action: game::world_input::Action) -> game::WorldInput {
    game::WorldInput {
        world_session_id: "00000000-0000-4000-8000-000000000001".into(),
        sequence: 1,
        action: Some(action),
        ..Default::default()
    }
}

#[test]
fn dynamic_targets_and_settings_are_typed_intents_not_outcomes() {
    let action = input(game::world_input::Action::InteractWith(
        game::InteractWith {
            target: Some(game::WorldTarget {
                target: Some(game::world_target::Target::TemporaryObject(
                    "dynamic_object.fire_1".into(),
                )),
            }),
            action: "Cook".into(),
        },
    ));
    let decoded = game::WorldInput::decode(action.encode_to_vec().as_slice()).unwrap();
    assert!(matches!(
        game_intent(&decoded).unwrap(),
        GameIntent::InteractWith {
            target: WorldTarget::TemporaryObject { .. },
            ..
        }
    ));
    let setting = input(game::world_input::Action::SetSetting(game::SetSetting {
        setting: game::SettingKind::Run as i32,
        enabled: true,
    }));
    assert_eq!(
        game_intent(&setting).unwrap(),
        GameIntent::SetSetting {
            setting: CharacterSetting::Run(true)
        }
    );
    let item = input(game::world_input::Action::UseItem(game::UseItem {
        inventory_slot: 0,
        target: Some(game::use_item::Target::GroundItem("ground.log_1".into())),
    }));
    assert!(matches!(
        game_intent(&item).unwrap(),
        GameIntent::UseItem {
            target: ItemTarget::Ground { .. },
            ..
        }
    ));
}

#[test]
fn recovery_and_settings_reject_unknown_or_duplicated_selectors() {
    for setting in [0, 999, -1] {
        assert!(
            game_intent(&input(game::world_input::Action::SetSetting(
                game::SetSetting {
                    setting,
                    enabled: true,
                }
            )))
            .is_err()
        );
    }
    let mut reclaim = game::Reclaim {
        death: "death.owned".into(),
        storage: game::RecoveryStorage::Grave as i32,
        items: vec!["recovery_item.first".into()],
    };
    assert!(game_intent(&input(game::world_input::Action::Reclaim(reclaim.clone()))).is_ok());
    reclaim.items.push("recovery_item.first".into());
    assert!(game_intent(&input(game::world_input::Action::Reclaim(reclaim.clone()))).is_err());
    reclaim.items.clear();
    assert!(game_intent(&input(game::world_input::Action::Reclaim(reclaim))).is_err());
}

#[test]
fn appearance_cannot_smuggle_paths_or_unbounded_options() {
    let mut appearance = game::ConfirmAppearance::default();
    appearance.appearance.insert("body".into(), 0xabcdef);
    assert!(
        game_intent(&input(game::world_input::Action::ConfirmAppearance(
            appearance.clone()
        )))
        .is_ok()
    );
    appearance.appearance.insert("../source".into(), 1);
    assert!(
        game_intent(&input(game::world_input::Action::ConfirmAppearance(
            appearance
        )))
        .is_err()
    );
}
