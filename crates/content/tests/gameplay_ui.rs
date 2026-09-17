mod common;

use clubscape_content::{ValidationMode, compile_content};
use clubscape_game_types::*;

fn content() -> GameContent {
    let mut content = common::fixture();
    common::ui::enable(&mut content);
    content
}

#[test]
fn ui_source_profile_is_explicit_versioned_and_compiler_checked() {
    let content = content();
    let compiled = compile_content(content.clone(), ValidationMode::TestFixture).unwrap();
    assert_eq!(compiled.definition().schema_version, 4);
    assert_eq!(compiled.definition().ui.as_ref().unwrap().version, 1);
    assert!(compile_content(content, ValidationMode::Runtime).is_err());
}

#[test]
fn ui_cannot_invent_quest_rewards_or_omit_source_interface_classification() {
    for change in 0..5 {
        let mut content = content();
        let definition = content.ui.as_mut().unwrap();
        match change {
            0 => {
                definition
                    .quest_rewards
                    .values_mut()
                    .next()
                    .unwrap()
                    .quest_points += 1
            }
            1 => definition.quest_rewards.values_mut().next().unwrap().xp[0].amount_tenths += 1,
            2 => {
                definition
                    .stage_interfaces
                    .values_mut()
                    .next()
                    .unwrap()
                    .pop();
            }
            3 => {
                definition
                    .ability_names
                    .insert("spell.unbound".into(), "Missing source".into());
            }
            4 => {
                definition
                    .direct_production
                    .insert(common::id("recipe.unbound"));
            }
            _ => unreachable!(),
        }
        assert!(
            compile_content(content, ValidationMode::TestFixture).is_err(),
            "source mutation {change} must fail"
        );
    }
}

#[test]
fn selected_consumption_and_native_data_bounds_are_real_requirements() {
    for change in 0..6 {
        let mut content = content();
        match change {
            0 => content
                .recipes
                .get_mut(&common::id("recipe.test.bury"))
                .unwrap()
                .outputs
                .push(common::stack("item.test.coins", 1)),
            1 => {
                content
                    .recipes
                    .get_mut(&common::id("recipe.test.bury"))
                    .unwrap()
                    .success = ChanceRule::constant(1, 2)
            }
            2 => content
                .recipes
                .get_mut(&common::id("recipe.test.bury"))
                .unwrap()
                .target_objects
                .push(common::id("object.test.furnace")),
            3 => content.ui.as_mut().unwrap().chat.maximum_bytes = 0,
            4 => content.ui.as_mut().unwrap().bank.maximum_tabs = 10,
            5 => {
                let action = &mut content
                    .ui
                    .as_mut()
                    .unwrap()
                    .item_actions
                    .get_mut(&common::id("item.test.food"))
                    .unwrap()[0]
                    .action;
                let ItemUiAction::Read { pages, .. } = action else {
                    panic!()
                };
                *pages = vec!["x".repeat(16384); 3];
            }
            _ => unreachable!(),
        }
        assert!(
            compile_content(content, ValidationMode::TestFixture).is_err(),
            "source mutation {change} must fail"
        );
    }
}
