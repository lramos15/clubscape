mod common;

use clubscape_content::{
    ValidationMode, compile_content, encode_compiled, load_compiled, read_content_json,
};
use clubscape_game_types::*;
use common::{id, item_on, sources};
use serde_json::json;

#[test]
fn absent_item_on_field_is_omitted_and_old_definitions_keep_exact_artifact_round_trips() {
    let old = common::fixture();
    let json = serde_json::to_value(&old).unwrap();
    assert!(
        json["recipes"]
            .as_object()
            .unwrap()
            .values()
            .all(|recipe| recipe.get("item_on_target").is_none())
    );
    let parsed = read_content_json(&serde_json::to_vec(&json).unwrap()).unwrap();
    assert_eq!(old, parsed);
    let first =
        encode_compiled(&compile_content(old, ValidationMode::TestFixture).unwrap()).unwrap();
    let second =
        encode_compiled(&compile_content(parsed, ValidationMode::TestFixture).unwrap()).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        encode_compiled(&load_compiled(&first, ValidationMode::TestFixture).unwrap()).unwrap(),
        first
    );
}

#[test]
fn explicit_rule_is_source_audited_and_does_not_create_an_interaction() {
    let content = item_on::content();
    let compiled = compile_content(content.clone(), ValidationMode::TestFixture).unwrap();
    let bytes = encode_compiled(&compiled).unwrap();
    let loaded = load_compiled(&bytes, ValidationMode::TestFixture).unwrap();
    assert_eq!(loaded.definition(), &content);
    assert!(
        loaded.definition().spawns[&id("spawn.test.furnace")]
            .interactions
            .is_empty()
    );
    let mut empty_source = content.clone();
    let SourceBinding::Bound { source, .. } = empty_source
        .recipes
        .get_mut(&id("recipe.test.bar"))
        .unwrap()
        .item_on_target
        .as_mut()
        .unwrap()
    else {
        panic!("bound target rule")
    };
    source.clear();
    assert!(compile_content(empty_source, ValidationMode::TestFixture).is_err());

    let mut invalid_source = content;
    let SourceBinding::Bound { source, .. } = invalid_source
        .recipes
        .get_mut(&id("recipe.test.bar"))
        .unwrap()
        .item_on_target
        .as_mut()
        .unwrap()
    else {
        panic!("bound target rule")
    };
    source[0].revision.clear();
    assert!(compile_content(invalid_source, ValidationMode::TestFixture).is_err());
}

#[test]
fn unresolved_empty_unknown_or_nonconversion_target_rules_are_rejected() {
    for mutation in 0..8 {
        let mut content = item_on::content();
        let recipe = content.recipes.get_mut(&id("recipe.test.bar")).unwrap();
        match mutation {
            0 => {
                recipe.item_on_target = Some(SourceBinding::Unresolved {
                    reason: "Explicit source gap".into(),
                    source: sources(),
                })
            }
            1 => recipe.target_objects.clear(),
            2 => recipe.target_objects = vec![id("object.test.missing")],
            3 => {
                let SourceBinding::Bound { value, .. } = recipe.item_on_target.as_mut().unwrap()
                else {
                    panic!("bound rule")
                };
                value.reach = 0;
            }
            4 => recipe.mechanics.as_mut().unwrap().cadence.menu_delay = item_on::bound(1),
            5 => {
                recipe.mechanics.as_mut().unwrap().cadence.single = SourceBinding::Unresolved {
                    reason: "No single timing".into(),
                    source: sources(),
                }
            }
            6 => {
                recipe.mechanics = None;
                recipe.ticks = Some(1);
            }
            7 => {
                recipe.mechanics.as_mut().unwrap().lifecycle = RecipeLifecycle::ConsumeOnly;
                recipe.outputs.clear();
            }
            _ => unreachable!(),
        }
        assert!(
            compile_content(content, ValidationMode::TestFixture).is_err(),
            "mutation{mutation}"
        );
    }
    let mut json = serde_json::to_value(item_on::content()).unwrap();
    json["recipes"]["recipe.test.bar"]["item_on_target"]["value"]["name"] = json!("Fill");
    assert!(read_content_json(&serde_json::to_vec(&json).unwrap()).is_err());
}

#[test]
fn mixed_menu_or_direct_production_is_rejected_even_when_menu_guard_is_false() {
    let baseline = common::fixture();
    let mut menu = baseline.spawns[&id("spawn.test.furnace")].interactions[0].clone();
    menu.guard = Guard::Not {
        guard: Box::new(Guard::Always),
    };
    let mut content = item_on::content();
    content
        .spawns
        .get_mut(&id("spawn.test.furnace"))
        .unwrap()
        .interactions
        .push(menu);
    assert!(compile_content(content, ValidationMode::TestFixture).is_err());
    for direct in [false, true] {
        let mut content = item_on::content();
        common::ui::projection(&mut content);
        if direct {
            let ui = content.ui.as_mut().unwrap();
            ui.production_interfaces.remove(&id("recipe.test.bar"));
            ui.direct_production.insert(id("recipe.test.bar"));
        }

        assert!(compile_content(content, ValidationMode::TestFixture).is_err());
    }
}

#[test]
fn item_on_guards_participate_in_iterative_depth_preflight_and_safe_rejection_drop() {
    let mut content = item_on::content();
    let mut guard = Guard::Always;
    for _ in 0..20_000 {
        guard = Guard::Not {
            guard: Box::new(guard),
        };
    }
    let SourceBinding::Bound { value, .. } = content
        .recipes
        .get_mut(&id("recipe.test.bar"))
        .unwrap()
        .item_on_target
        .as_mut()
        .unwrap()
    else {
        panic!("bound rule")
    };
    value.guard = guard;
    let error = compile_content(content, ValidationMode::TestFixture).unwrap_err();
    assert!(error.message.contains("depth"));
}
