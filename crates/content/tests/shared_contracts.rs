mod common;

use clubscape_content::*;
use clubscape_game_types::*;
use common::*;
use serde_json::json;
use sha2::{Digest, Sha256};

type Mutation = fn(&mut GameContent);

fn rejects(mutate: Mutation, diagnostic: &str) {
    let mut content = fixture();
    mutate(&mut content);
    let error = compile_content(content, ValidationMode::TestFixture).unwrap_err();
    assert!(error.message.contains(diagnostic), "{error}");
}

#[test]
fn logical_interfaces_have_real_registry_lookups_and_reported_validation() {
    let content = compile_content(fixture(), ValidationMode::TestFixture).unwrap();
    assert_eq!(content.counts().interfaces, 2);
    assert_eq!(
        content
            .interface(&id("interface.test.inventory"))
            .unwrap()
            .source_ids,
        vec![1]
    );
    assert!(content.interface(&id("interface.test.missing")).is_none());
    assert!(
        !content
            .definition()
            .initial_state
            .interfaces
            .contains(&id("interface.test.bank"))
    );
    assert!(content.report().interface_definition_validation_performed);
    assert!(content.report().recipe_tool_reference_validation_performed);
    assert!(!content.report().presentation_verification_performed);
    let loaded = load_compiled(
        &encode_compiled(&content).unwrap(),
        ValidationMode::TestFixture,
    )
    .unwrap();
    assert_eq!(
        loaded.definition().interfaces,
        content.definition().interfaces
    );
    assert_eq!(
        loaded.interface(&id("interface.test.bank")),
        content.interface(&id("interface.test.bank"))
    );
}

#[test]
fn initial_guards_effects_and_event_filters_cannot_declare_missing_interfaces() {
    let cases: [(Mutation, &str); 7] = [
        (|content| content.interfaces.clear(), "interfaces"),
        (
            |content| {
                content.interfaces.remove(&id("interface.test.inventory"));
            },
            "undefined interface",
        ),
        (
            |content| {
                content
                    .initial_state
                    .interfaces
                    .push(id("interface.test.missing"))
            },
            "undefined interface",
        ),
        (
            |content| {
                tutorial_transition(content).guard = Guard::InterfaceUnlocked {
                    interface: id("interface.test.missing"),
                }
            },
            "undefined interface",
        ),
        (
            |content| {
                tutorial_transition(content)
                    .effects
                    .push(Effect::UnlockInterface {
                        interface: id("interface.test.missing"),
                    })
            },
            "undefined interface",
        ),
        (
            |content| {
                tutorial_transition(content)
                    .effects
                    .push(Effect::Conditional {
                        guard: Guard::Always,
                        effects: vec![Effect::UnlockInterface {
                            interface: id("interface.test.missing"),
                        }],
                    })
            },
            "undefined interface",
        ),
        (
            |content| {
                let transition = tutorial_transition(content);
                transition.event = "interface_opened".into();
                transition.target = Some("interface.test.missing".into());
            },
            "undefined interface",
        ),
    ];
    for (mutate, diagnostic) in cases {
        rejects(mutate, diagnostic);
    }
    rejects(
        |content| {
            tutorial_transition(content).guard = Guard::InterfaceUnlocked {
                interface: id("interface.test.bank"),
            }
        },
        "unreachable",
    );
    rejects(
        |content| {
            let transition = tutorial_transition(content);
            transition.event = "interface_opened".into();
            transition.target = Some("interface.test.bank".into());
            transition.effects.push(Effect::UnlockInterface {
                interface: id("interface.test.bank"),
            });
        },
        "unreachable",
    );
}

#[test]
fn interface_keys_widget_mappings_names_and_provenance_are_strict() {
    let cases: [(Mutation, &str); 6] = [
        (
            |content| {
                content
                    .interfaces
                    .get_mut(&id("interface.test.inventory"))
                    .unwrap()
                    .id = id("interface.test.other")
            },
            "map key differs",
        ),
        (
            |content| {
                content
                    .interfaces
                    .get_mut(&id("interface.test.inventory"))
                    .unwrap()
                    .name
                    .clear()
            },
            "name",
        ),
        (
            |content| {
                content
                    .interfaces
                    .get_mut(&id("interface.test.inventory"))
                    .unwrap()
                    .source
                    .clear()
            },
            "source",
        ),
        (
            |content| {
                content
                    .interfaces
                    .get_mut(&id("interface.test.inventory"))
                    .unwrap()
                    .source_ids
                    .push(1)
            },
            "duplicate",
        ),
        (
            |content| {
                content
                    .interfaces
                    .get_mut(&id("interface.test.bank"))
                    .unwrap()
                    .source_ids = vec![1]
            },
            "duplicate category-local source ID",
        ),
        (
            |content| {
                content
                    .interfaces
                    .get_mut(&id("interface.test.inventory"))
                    .unwrap()
                    .source[0]
                    .notes
                    .clear()
            },
            "notes",
        ),
    ];
    for (mutate, diagnostic) in cases {
        rejects(mutate, diagnostic);
    }
    let mut content = fixture();
    content
        .interfaces
        .get_mut(&id("interface.test.inventory"))
        .unwrap()
        .source_ids
        .clear();
    compile_content(content, ValidationMode::TestFixture).unwrap();
    let mut content = runtime_policy_fixture();
    content
        .interfaces
        .get_mut(&id("interface.test.inventory"))
        .unwrap()
        .source = sources();
    assert!(
        compile_content(content, ValidationMode::Runtime)
            .unwrap_err()
            .message
            .contains("TestFixture")
    );
}

#[test]
fn every_required_recipe_tool_is_defined_unnoted_unique_and_not_consumed() {
    let cases: [(Mutation, &str); 4] = [
        (
            |content| {
                content.recipes.values_mut().next().unwrap().tools = vec![id("item.test.missing")]
            },
            "undefined item",
        ),
        (
            |content| {
                content
                    .recipes
                    .values_mut()
                    .next()
                    .unwrap()
                    .tools
                    .push(id("item.test.pickaxe"))
            },
            "duplicate",
        ),
        (
            |content| {
                content.recipes.values_mut().next().unwrap().tools = vec![id("item.test.ore_note")]
            },
            "noted item",
        ),
        (
            |content| {
                content.recipes.values_mut().next().unwrap().tools = vec![id("item.test.ore")]
            },
            "must not be consumed",
        ),
    ];
    for (mutate, diagnostic) in cases {
        rejects(mutate, diagnostic);
    }
    let mut content = fixture();
    content.recipes.values_mut().next().unwrap().tools.clear();
    compile_content(content, ValidationMode::TestFixture).unwrap();
}

#[test]
fn compiler_preserves_tools_without_inventing_initial_grants_or_consumption() {
    for equipped in [false, true] {
        let mut definition = fixture();
        definition.initial_state.inventory.slots[0] = None;
        if equipped {
            definition
                .initial_state
                .equipment
                .insert(id("slot.test.weapon"), stack("item.test.pickaxe", 1));
        }
        let content = compile_content(definition, ValidationMode::TestFixture).unwrap();
        assert!(content.definition().initial_state.inventory.slots[0].is_none());
        let recipe = content.recipe(&id("recipe.test.bar")).unwrap();
        assert_eq!(recipe.tools, vec![id("item.test.pickaxe")]);
        assert_eq!(recipe.inputs, vec![stack("item.test.ore", 2)]);
        assert_eq!(recipe.outputs, vec![stack("item.test.bar", 1)]);
        let loaded = load_compiled(
            &encode_compiled(&content).unwrap(),
            ValidationMode::TestFixture,
        )
        .unwrap();
        assert_eq!(loaded.recipe(&recipe.id), Some(recipe));
    }
}

#[test]
fn required_tools_share_inventory_capacity_or_nonconflicting_equipment_slots() {
    rejects(
        |content| {
            let recipe = content.recipes.values_mut().next().unwrap();
            recipe.inputs = vec![stack("item.test.ore", 28)];
            recipe.tools = vec![id("item.test.food")];
        },
        "cannot be held",
    );
    rejects(
        |content| {
            let recipe = content.recipes.values_mut().next().unwrap();
            recipe.inputs = vec![stack("item.test.ore", 28)];
            recipe.tools = vec![id("item.test.two_handed"), id("item.test.shield")];
        },
        "cannot be held",
    );
    for (quantity, first_tool) in [(27, "item.test.two_handed"), (28, "item.test.pickaxe")] {
        let mut content = fixture();
        let recipe = content.recipes.values_mut().next().unwrap();
        recipe.inputs = vec![stack("item.test.ore", quantity)];
        recipe.tools = vec![id(first_tool), id("item.test.shield")];
        compile_content(content, ValidationMode::TestFixture).unwrap();
    }
}

#[test]
fn tool_placement_backtracks_instead_of_greedily_choosing_two_handed_equipment() {
    let mut content = fixture();
    let recipe = content.recipes.values_mut().next().unwrap();
    recipe.inputs = vec![stack("item.test.ore", 27)];
    recipe.tools = vec![
        id("item.test.two_handed"),
        id("item.test.pickaxe"),
        id("item.test.shield"),
    ];
    compile_content(content, ValidationMode::TestFixture).unwrap();
}

#[test]
fn initial_run_energy_is_bounded_in_shared_units_without_implicit_conversion() {
    rejects(
        |content| content.initial_state.run_energy = MAX_RUN_ENERGY + 1,
        "hundredths",
    );
    for energy in [0, 100, 3_456, MAX_RUN_ENERGY] {
        let mut content = fixture();
        content.initial_state.run_energy = energy;
        let compiled = compile_content(content, ValidationMode::TestFixture).unwrap();
        let loaded = load_compiled(
            &encode_compiled(&compiled).unwrap(),
            ValidationMode::TestFixture,
        )
        .unwrap();
        assert_eq!(loaded.definition().initial_state.run_energy, energy);
    }
}

#[test]
fn shared_serde_defaults_do_not_hide_missing_registry_or_tool_declarations() {
    let mut missing_registry = serde_json::to_value(fixture()).unwrap();
    missing_registry
        .as_object_mut()
        .unwrap()
        .remove("interfaces");
    let error = read_content_json(&serde_json::to_vec(&missing_registry).unwrap()).unwrap_err();
    assert!(error.message.contains("missing field `interfaces`"));
    let mut empty_registry = serde_json::to_value(fixture()).unwrap();
    empty_registry["interfaces"] = json!({});
    let parsed = read_content_json(&serde_json::to_vec(&empty_registry).unwrap()).unwrap();
    assert!(
        compile_content(parsed, ValidationMode::TestFixture)
            .unwrap_err()
            .message
            .contains("interfaces")
    );
    let mut missing_tools = serde_json::to_value(fixture()).unwrap();
    missing_tools["recipes"]["recipe.test.bar"]
        .as_object_mut()
        .unwrap()
        .remove("tools");
    let error = read_content_json(&serde_json::to_vec(&missing_tools).unwrap()).unwrap_err();
    assert!(error.message.contains("missing field `tools`"));
    let mut unknown_field = serde_json::to_value(fixture()).unwrap();
    unknown_field["interfaces"]["interface.test.inventory"]["unknown_control"] = json!(true);
    assert!(
        read_content_json(&serde_json::to_vec(&unknown_field).unwrap())
            .unwrap_err()
            .message
            .contains("unknown field")
    );
}

#[test]
fn loader_revalidates_the_new_contract_even_with_valid_envelope_checksums() {
    let base =
        encode_compiled(&compile_content(fixture(), ValidationMode::TestFixture).unwrap()).unwrap();
    let cases: [(Mutation, &str); 3] = [
        (|content| content.interfaces.clear(), "interfaces"),
        (
            |content| {
                content.recipes.values_mut().next().unwrap().tools = vec![id("item.test.missing")]
            },
            "undefined item",
        ),
        (
            |content| content.initial_state.run_energy = MAX_RUN_ENERGY + 1,
            "run_energy",
        ),
    ];
    for (mutate, diagnostic) in cases {
        let mut content = fixture();
        mutate(&mut content);
        let payload = rmp_serde::to_vec_named(&serde_json::to_value(&content).unwrap()).unwrap();
        let mut bytes = base[..ARTIFACT_HEADER_BYTES].to_vec();
        bytes[12..20].copy_from_slice(&(payload.len() as u64).to_le_bytes());
        bytes[20..52].copy_from_slice(&Sha256::digest(&payload));
        bytes.extend_from_slice(&payload);
        let error = load_compiled(&bytes, ValidationMode::TestFixture).unwrap_err();
        assert!(error.message.contains(diagnostic), "{error}");
    }
}

#[test]
fn compiler_event_bindings_match_every_canonical_game_event_identity() {
    let events = [
        (
            GameEvent::Moved {
                tile: tile(1001, 1000),
            },
            "moved",
            None,
        ),
        (
            GameEvent::Interacted {
                target: id("spawn.test.rock"),
                action: "Mine".into(),
            },
            "interacted",
            Some("spawn.test.rock"),
        ),
        (
            GameEvent::DialogueSelected {
                speaker: id("spawn.test.guide"),
                choice: "ask".into(),
            },
            "dialogue_selected",
            Some("spawn.test.guide"),
        ),
        (
            GameEvent::InterfaceOpened {
                interface: id("interface.test.inventory"),
            },
            "interface_opened",
            Some("interface.test.inventory"),
        ),
        (
            GameEvent::Gathered {
                target: id("spawn.test.rock"),
                stack: stack("item.test.ore", 1),
            },
            "gathered",
            Some("spawn.test.rock"),
        ),
        (
            GameEvent::Produced {
                recipe: id("recipe.test.bar"),
                outputs: vec![stack("item.test.bar", 1)],
            },
            "produced",
            Some("recipe.test.bar"),
        ),
        (
            GameEvent::Equipped {
                slot: id("slot.test.weapon"),
                stack: stack("item.test.two_handed", 1),
            },
            "equipped",
            Some("slot.test.weapon"),
        ),
        (
            GameEvent::XpGained {
                skill: id("skill.test.mining"),
                amount_tenths: 100,
            },
            "xp_gained",
            Some("skill.test.mining"),
        ),
        (
            GameEvent::Hit {
                target: id("spawn.test.monster"),
                damage: 1,
                style: "accurate".into(),
            },
            "hit",
            Some("spawn.test.monster"),
        ),
        (
            GameEvent::Defeated {
                target: id("spawn.test.monster"),
                style: "accurate".into(),
            },
            "defeated",
            Some("spawn.test.monster"),
        ),
        (GameEvent::Died, "died", None),
        (GameEvent::Recovered, "recovered", None),
        (
            GameEvent::TutorialAdvanced {
                stage: id("stage.test.learn"),
            },
            "tutorial_advanced",
            Some("stage.test.learn"),
        ),
        (
            GameEvent::QuestAdvanced {
                quest: id("quest.test.errand"),
                stage: id("stage.test.quest_done"),
            },
            "quest_advanced",
            Some("quest.test.errand"),
        ),
        (
            GameEvent::Message {
                text: "Synthetic information.".into(),
            },
            "message",
            None,
        ),
        (
            GameEvent::Sound {
                asset: "asset.test.sound".into(),
            },
            "sound",
            Some("asset.test.sound"),
        ),
        (
            GameEvent::Animation {
                target: "actor.test.player".into(),
                animation: "asset.test.animation".into(),
            },
            "animation",
            Some("actor.test.player"),
        ),
    ];
    for (event, kind, target) in events {
        assert_eq!(event.kind(), kind);
        assert_eq!(event.primary_target(), target);
        assert_eq!(serde_json::to_value(&event).unwrap()["kind"], kind);
        let mut content = fixture();
        gather(&mut content).sound = Some(id("asset.test.sound"));
        gather(&mut content).animation = Some(id("asset.test.animation"));
        let transition = if kind == "tutorial_advanced" {
            &mut content
                .tutorial
                .get_mut(&id("stage.test.learn"))
                .unwrap()
                .transitions[0]
        } else {
            tutorial_transition(&mut content)
        };
        transition.event = event.kind().into();
        transition.target = event.primary_target().map(str::to_owned);
        compile_content(content, ValidationMode::TestFixture)
            .unwrap_or_else(|error| panic!("{kind}: {error}"));
    }
}

#[test]
fn message_sound_and_animation_filters_are_checked_without_claiming_playback() {
    for (kind, target, diagnostic) in [
        ("message", "spawn.test.guide", "no string target"),
        ("sound", "item.test.ore", "asset ID"),
        ("animation", "spawn.test.missing", "undefined event target"),
        ("animation", "not_a_stable_actor", "actor ID"),
    ] {
        let mut content = fixture();
        let transition = tutorial_transition(&mut content);
        transition.event = kind.into();
        transition.target = Some(target.into());
        assert!(
            compile_content(content, ValidationMode::TestFixture)
                .unwrap_err()
                .message
                .contains(diagnostic)
        );
    }
    let mut content = fixture();
    gather(&mut content).sound = Some(id("asset.test.sound"));
    let transition = tutorial_transition(&mut content);
    transition.event = "sound".into();
    transition.target = Some("asset.test.sound".into());
    let mut compiled = compile_content(content, ValidationMode::TestFixture).unwrap();
    assert!(
        compiled
            .referenced_assets()
            .contains(&id("asset.test.sound"))
    );
    assert!(!compiled.report().presentation_verification_performed);
    let mut assets = compiled.referenced_assets().clone();
    assets.remove(&id("asset.test.sound"));
    assert!(
        compiled
            .check_asset_manifest(&AssetManifest {
                identity: "synthetic-manifest-v1".into(),
                assets
            })
            .is_err()
    );
}

#[test]
fn message_and_media_dependencies_require_rooted_declared_producers() {
    let mut content = fixture();
    content.dialogues.values_mut().next().unwrap().nodes[0].choices[0]
        .effects
        .clear();
    let transition = tutorial_transition(&mut content);
    transition.event = "message".into();
    transition.target = None;
    transition.effects.push(Effect::Message {
        text: "Synthetic unrooted loop.".into(),
    });
    assert!(
        compile_content(content, ValidationMode::TestFixture)
            .unwrap_err()
            .message
            .contains("unreachable")
    );
    for event in ["sound", "animation"] {
        let mut content = fixture();
        tutorial_transition(&mut content).event = event.into();
        tutorial_transition(&mut content).target = None;
        assert!(
            compile_content(content, ValidationMode::TestFixture)
                .unwrap_err()
                .message
                .contains("unreachable")
        );
    }
}
