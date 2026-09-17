mod common;

use std::collections::BTreeSet;

use clubscape_content::*;
use clubscape_game_types::*;
use common::*;

type Mutation = fn(&mut GameContent);

fn rejects(mutate: Mutation, diagnostic: &str) {
    let mut content = fixture();
    mutate(&mut content);
    let error = compile_content(content, ValidationMode::TestFixture).unwrap_err();
    assert_eq!(error.code, GameErrorCode::InvalidContent);
    assert!(
        error.message.contains(diagnostic),
        "expected {diagnostic:?}: {}",
        error.message
    );
}

#[test]
fn compiles_real_synthetic_definitions_and_indexes_without_walkable_defaults() {
    let definition = fixture();
    let compiled = compile_content(definition.clone(), ValidationMode::TestFixture).unwrap();
    assert_eq!(compiled.definition(), &definition);
    assert_eq!(compiled.counts().collision_cells, 49);
    assert_eq!(compiled.counts().spawns, 5);
    assert!(compiled.collision(tile(1000, 1000)).unwrap().walkable);
    assert!(!compiled.collision(tile(1003, 1003)).unwrap().walkable);
    assert!(compiled.collision(tile(1100, 1100)).is_none());
    assert!(compiled.region_at(tile(1100, 1100)).is_none());
    assert_eq!(
        compiled.region_at(tile(1000, 1000)).unwrap().id,
        id("region.test.field")
    );
    let spawns: Vec<_> = compiled
        .spawns_at_tile(tile(1003, 1003))
        .map(|spawn| spawn.id.as_str())
        .collect();
    assert_eq!(spawns, vec!["spawn.test.ore", "spawn.test.rock"]);
    assert_eq!(compiled.spawns_at_tile(tile(1100, 1100)).count(), 0);
    assert!(compiled.spawn(&id("spawn.test.guide")).is_some());
    assert!(compiled.item(&id("item.test.pickaxe")).is_some());
    assert!(compiled.skill(&id("skill.test.mining")).is_some());
    assert!(compiled.object(&id("object.test.rock")).is_some());
    assert!(compiled.npc(&id("npc.test.guide")).is_some());
    assert!(compiled.recipe(&id("recipe.test.bar")).is_some());
    assert!(
        compiled
            .dialogue_node(&id("dialogue.test.guide"), "entry")
            .is_some()
    );
    assert!(
        compiled
            .dialogue_node(&id("dialogue.test.guide"), "missing")
            .is_none()
    );
    assert!(compiled.tutorial_stage(&id("stage.test.start")).is_some());
    assert!(compiled.quest(&id("quest.test.errand")).is_some());
    assert!(compiled.shop(&id("shop.test.general")).is_some());
    assert!(compiled.has_equipment_slot(&id("slot.test.weapon")));
    assert!(compiled.report().unassigned_asset_sites > 0);
    assert!(compiled.report().asset_manifest.is_none());
    assert!(!compiled.report().source_verification_performed);
    assert!(!compiled.report().presentation_verification_performed);
}

#[test]
fn default_runtime_policy_rejects_fixtures_but_preserves_documented_inference() {
    assert_eq!(ValidationMode::default(), ValidationMode::Runtime);
    assert!(compile_content(fixture(), ValidationMode::Runtime).is_err());
    let mut content = runtime_policy_fixture();
    let source = &mut content
        .items
        .get_mut(&id("item.test.pickaxe"))
        .unwrap()
        .source[0];
    source.status = EvidenceStatus::VerifiedReference;
    let source = &mut content
        .items
        .get_mut(&id("item.test.shield"))
        .unwrap()
        .source[0];
    source.status = EvidenceStatus::ApprovedAdaptation;
    let compiled = compile_content(content, ValidationMode::Runtime).unwrap();
    assert!(compiled.report().evidence.inference > 0);
    assert_eq!(compiled.report().evidence.verified_reference, 1);
    assert_eq!(compiled.report().evidence.approved_adaptation, 1);
    assert_eq!(compiled.report().evidence.test_fixture, 0);
    assert!(!compiled.report().source_verification_performed);
    assert!(!compiled.report().approval_verification_performed);
    let mut content = runtime_policy_fixture();
    content.initial_state.source = sources();
    let error = compile_content(content, ValidationMode::Runtime).unwrap_err();
    assert!(error.message.contains("TestFixture"));
}

#[test]
fn source_records_require_identifiable_references_revisions_notes_and_no_duplicates() {
    for (mutate, diagnostic) in [
        (
            (|c: &mut GameContent| c.initial_state.source.clear()) as Mutation,
            "source",
        ),
        (|c| c.initial_state.source[0].notes.clear(), "notes"),
        (
            |c| c.initial_state.source[0].revision = "unknown".into(),
            "placeholder",
        ),
        (
            |c| c.initial_state.source[0].reference = "unidentified".into(),
            "reference",
        ),
        (
            |c| c.initial_state.source[0].reference = "tests/fixture".into(),
            "fixture:",
        ),
        (
            |c| {
                c.initial_state
                    .source
                    .push(c.initial_state.source[0].clone())
            },
            "duplicate source",
        ),
    ] {
        rejects(mutate, diagnostic);
    }
}

#[test]
fn requires_schema_identity_and_every_mandatory_category() {
    for (mutate, diagnostic) in [
        (
            (|c: &mut GameContent| c.schema_version = CONTENT_SCHEMA_VERSION + 1) as Mutation,
            "schema_version",
        ),
        (|c| c.revision.clear(), "revision"),
        (|c| c.baseline = "latest".into(), "baseline"),
        (|c| c.items.clear(), "items"),
        (|c| c.skills.clear(), "skills"),
        (|c| c.regions.clear(), "regions"),
        (|c| c.spawns.clear(), "spawns"),
        (|c| c.objects.clear(), "objects"),
        (|c| c.npcs.clear(), "npcs"),
        (|c| c.recipes.clear(), "recipes"),
        (|c| c.dialogues.clear(), "dialogues"),
        (|c| c.tutorial.clear(), "tutorial"),
        (|c| c.quests.clear(), "quests"),
        (|c| c.shops.clear(), "shops"),
        (|c| c.equipment_slots.clear(), "equipment_slots"),
    ] {
        rejects(mutate, diagnostic);
    }
}

#[test]
fn every_definition_map_key_must_equal_its_contained_id() {
    for mutate in [
        (|c: &mut GameContent| c.items.values_mut().next().unwrap().id = id("item.test.wrong"))
            as Mutation,
        |c| c.skills.values_mut().next().unwrap().id = id("skill.test.wrong"),
        |c| c.regions.values_mut().next().unwrap().id = id("region.test.wrong"),
        |c| c.spawns.values_mut().next().unwrap().id = id("spawn.test.wrong"),
        |c| c.objects.values_mut().next().unwrap().id = id("object.test.wrong"),
        |c| c.npcs.values_mut().next().unwrap().id = id("npc.test.wrong"),
        |c| c.recipes.values_mut().next().unwrap().id = id("recipe.test.wrong"),
        |c| c.dialogues.values_mut().next().unwrap().id = id("dialogue.test.wrong"),
        |c| c.tutorial.values_mut().next().unwrap().id = id("stage.test.wrong"),
        |c| c.quests.values_mut().next().unwrap().id = id("quest.test.wrong"),
        |c| c.shops.values_mut().next().unwrap().id = id("shop.test.wrong"),
    ] {
        rejects(mutate, "map key differs");
    }
}

#[test]
fn numeric_source_ids_are_unique_per_category_not_globally() {
    for mutate in [
        (|c: &mut GameContent| c.items.get_mut(&id("item.test.bar")).unwrap().source_id = Some(1))
            as Mutation,
        |c| {
            c.skills
                .get_mut(&id("skill.test.mining"))
                .unwrap()
                .source_id = 2
        },
        |c| {
            c.objects
                .get_mut(&id("object.test.furnace"))
                .unwrap()
                .source_id = 0
        },
        |c| c.npcs.get_mut(&id("npc.test.monster")).unwrap().source_id = 1,
    ] {
        rejects(mutate, "duplicate category-local source ID");
    }
    let mut content = fixture();
    content
        .items
        .get_mut(&id("item.test.bar"))
        .unwrap()
        .source_id = None;
    content
        .items
        .get_mut(&id("item.test.food"))
        .unwrap()
        .source_id = None;
    compile_content(content, ValidationMode::TestFixture).unwrap();
}

#[test]
fn notes_are_reciprocal_distinct_and_not_usable_as_food_or_equipment() {
    for mutate in [
        (|c: &mut GameContent| {
            c.items.get_mut(&id("item.test.ore")).unwrap().noted_variant =
                Some(id("item.test.missing"))
        }) as Mutation,
        |c| {
            c.items
                .get_mut(&id("item.test.ore_note"))
                .unwrap()
                .unnoted_variant = None
        },
        |c| {
            c.items
                .get_mut(&id("item.test.ore_note"))
                .unwrap()
                .stackable = false.into()
        },
        |c| c.items.get_mut(&id("item.test.ore_note")).unwrap().healing = Some(2),
        |c| c.items.get_mut(&id("item.test.ore")).unwrap().stackable = true.into(),
        |c| {
            c.items
                .get_mut(&id("item.test.ore"))
                .unwrap()
                .unnoted_variant = Some(id("item.test.ore"))
        },
    ] {
        let mut content = fixture();
        mutate(&mut content);
        assert!(compile_content(content, ValidationMode::TestFixture).is_err());
    }
}

#[test]
fn equipment_occupancy_is_explicit_and_two_handed_conflicts_are_rejected() {
    for (mutate, diagnostic) in [
        (
            (|c: &mut GameContent| c.equipment_slots.push(id("slot.test.weapon"))) as Mutation,
            "duplicate",
        ),
        (
            |c| {
                c.items
                    .get_mut(&id("item.test.pickaxe"))
                    .unwrap()
                    .equipment
                    .as_mut()
                    .unwrap()
                    .occupied_slots
                    .clear()
            },
            "must not be empty",
        ),
        (
            |c| {
                c.items
                    .get_mut(&id("item.test.pickaxe"))
                    .unwrap()
                    .equipment
                    .as_mut()
                    .unwrap()
                    .occupied_slots = vec![id("slot.test.offhand")]
            },
            "primary slot",
        ),
        (
            |c| {
                c.items
                    .get_mut(&id("item.test.pickaxe"))
                    .unwrap()
                    .equipment
                    .as_mut()
                    .unwrap()
                    .occupied_slots
                    .push(id("slot.test.unknown"))
            },
            "undefined equipment slot",
        ),
        (
            |c| {
                c.items
                    .get_mut(&id("item.test.pickaxe"))
                    .unwrap()
                    .equipment
                    .as_mut()
                    .unwrap()
                    .attack_speed_ticks = Some(0)
            },
            "attack styles",
        ),
        (
            |c| {
                c.initial_state
                    .equipment
                    .insert(id("slot.test.weapon"), stack("item.test.two_handed", 1));
                c.initial_state
                    .equipment
                    .insert(id("slot.test.offhand"), stack("item.test.shield", 1));
            },
            "occupied-slot conflict",
        ),
        (
            |c| {
                c.initial_state
                    .equipment
                    .insert(id("slot.test.offhand"), stack("item.test.pickaxe", 1));
            },
            "primary slot",
        ),
        (
            |c| {
                c.initial_state
                    .equipment
                    .insert(id("slot.test.weapon"), stack("item.test.pickaxe", 2));
            },
            "quantity must be one",
        ),
        (
            |c| {
                c.items
                    .get_mut(&id("item.test.ammo"))
                    .unwrap()
                    .equipment
                    .as_mut()
                    .unwrap()
                    .requirements[0]
                    .level = 2
            },
            "requirement is not met",
        ),
    ] {
        rejects(mutate, diagnostic);
    }
}

#[test]
fn inventory_bank_and_initial_state_invariants_do_not_use_defaults() {
    for (mutate, diagnostic) in [
        (
            (|c: &mut GameContent| {
                c.initial_state.inventory.slots[0] = Some(stack("item.test.missing", 1))
            }) as Mutation,
            "undefined item",
        ),
        (
            |c| c.initial_state.inventory.slots[0] = Some(stack("item.test.ore", 2)),
            "separate slots",
        ),
        (
            |c| c.initial_state.inventory.slots[5] = Some(stack("item.test.coins", 1)),
            "multiple inventory slots",
        ),
        (|c| c.initial_state.bank.capacity = 0, "bank capacity"),
        (
            |c| {
                c.initial_state.bank.capacity = 1;
                c.initial_state.bank.slots = vec![None, None];
            },
            "bank capacity",
        ),
        (
            |c| c.initial_state.bank.slots = vec![Some(stack("item.test.ore_note", 3))],
            "unnoted",
        ),
        (
            |c| {
                c.initial_state.bank.slots = vec![
                    Some(stack("item.test.ore", 1)),
                    Some(stack("item.test.ore", 2)),
                ]
            },
            "unique",
        ),
        (
            |c| {
                c.initial_state.skills.remove(&id("skill.test.mining"));
            },
            "explicit initial state",
        ),
        (
            |c| {
                c.initial_state.quests.remove(&id("quest.test.errand"));
            },
            "explicit initial state",
        ),
        (
            |c| c.initial_state.tutorial_stage = id("stage.test.unknown"),
            "undefined tutorial",
        ),
        (
            |c| {
                c.initial_state
                    .quests
                    .get_mut(&id("quest.test.errand"))
                    .unwrap()
                    .stage = id("stage.test.unknown")
            },
            "undefined stage",
        ),
        (
            |c| {
                c.initial_state
                    .quests
                    .get_mut(&id("quest.test.errand"))
                    .unwrap()
                    .stage = id("stage.test.quest_done")
            },
            "advanced or completed",
        ),
        (|c| c.initial_state.quest_points = 1, "pre-award"),
        (|c| c.initial_state.hitpoints = 0, "alive"),
        (
            |c| {
                c.initial_state
                    .interfaces
                    .push(id("interface.test.inventory"))
            },
            "duplicate",
        ),
    ] {
        rejects(mutate, diagnostic);
    }
    let mut content = fixture();
    content.initial_state.bank.slots = vec![Some(stack("item.test.ore", 100)), None];
    compile_content(content, ValidationMode::TestFixture).unwrap();
}

#[test]
fn xp_tables_require_safe_ordered_thresholds_and_consistent_initial_levels() {
    for (mutate, diagnostic) in [
        (
            (|c: &mut GameContent| {
                c.skills
                    .get_mut(&id("skill.test.mining"))
                    .unwrap()
                    .xp_thresholds_tenths = vec![1, 2]
            }) as Mutation,
            "thresholds",
        ),
        (
            |c| {
                c.skills
                    .get_mut(&id("skill.test.mining"))
                    .unwrap()
                    .xp_thresholds_tenths = vec![0, 2, 2]
            },
            "thresholds",
        ),
        (
            |c| {
                c.skills
                    .get_mut(&id("skill.test.mining"))
                    .unwrap()
                    .xp_thresholds_tenths = vec![0, 2, 1]
            },
            "thresholds",
        ),
        (
            |c| {
                c.skills
                    .get_mut(&id("skill.test.mining"))
                    .unwrap()
                    .maximum_xp_tenths = 0
            },
            "thresholds",
        ),
        (
            |c| {
                c.skills
                    .get_mut(&id("skill.test.mining"))
                    .unwrap()
                    .maximum_xp_tenths = 100
            },
            "thresholds",
        ),
        (
            |c| {
                c.initial_state
                    .skills
                    .get_mut(&id("skill.test.mining"))
                    .unwrap()
                    .xp_tenths = 10_001
            },
            "initial XP",
        ),
        (
            |c| {
                c.initial_state
                    .skills
                    .get_mut(&id("skill.test.mining"))
                    .unwrap()
                    .current_level = 2
            },
            "current level",
        ),
        (|c| gather(c).required_level = 0, "invalid level"),
        (|c| gather(c).required_level = 4, "invalid level"),
        (|c| gather(c).xp_tenths = 10_001, "XP award"),
        (
            |c| {
                c.tutorial
                    .get_mut(&id("stage.test.learn"))
                    .unwrap()
                    .xp_caps_tenths
                    .insert(id("skill.test.mining"), 10_001);
            },
            "XP cap",
        ),
        (
            |c| {
                c.tutorial
                    .get_mut(&id("stage.test.learn"))
                    .unwrap()
                    .xp_stop_levels
                    .insert(id("skill.test.mining"), 0);
            },
            "invalid level",
        ),
    ] {
        rejects(mutate, diagnostic);
    }
    let mut content = fixture();
    let stage = content.tutorial.get_mut(&id("stage.test.learn")).unwrap();
    stage.xp_stop_levels.insert(id("skill.test.mining"), 2);
    stage.xp_caps_tenths.insert(id("skill.test.mining"), 1_500);
    let compiled = compile_content(content, ValidationMode::TestFixture).unwrap();
    assert_eq!(
        compiled
            .tutorial_stage(&id("stage.test.learn"))
            .unwrap()
            .xp_caps_tenths[&id("skill.test.mining")],
        1_500
    );
}

#[test]
fn collision_cells_and_spawn_ownership_never_silently_overwrite() {
    rejects(
        |c| {
            let region = c.regions.values_mut().next().unwrap();
            region.cells.push(region.cells[0]);
        },
        "duplicate collision",
    );
    rejects(
        |c| {
            let mut region = c.regions.values().next().unwrap().clone();
            region.id = id("region.test.overlap");
            c.regions.insert(region.id.clone(), region);
        },
        "duplicate collision",
    );
    rejects(
        |c| c.regions.values_mut().next().unwrap().min = tile(1010, 1000),
        "ordered",
    );
    rejects(
        |c| c.regions.values_mut().next().unwrap().cells[0].tile = tile(1100, 1100),
        "outside",
    );
    rejects(
        |c| c.regions.values_mut().next().unwrap().cells.clear(),
        "empty",
    );
    rejects(
        |c| {
            c.regions
                .values_mut()
                .next()
                .unwrap()
                .source_map_squares
                .push(42)
        },
        "duplicate",
    );
    rejects(|c| c.initial_state.tile = tile(1003, 1003), "not walkable");
    rejects(
        |c| c.initial_state.tile = tile(1100, 1100),
        "no explicit collision",
    );
    rejects(
        |c| c.initial_state.region = id("region.test.unknown"),
        "undefined region",
    );
    rejects(
        |c| c.spawns.get_mut(&id("spawn.test.guide")).unwrap().tile = tile(1003, 1003),
        "not walkable",
    );
    rejects(
        |c| c.spawns.get_mut(&id("spawn.test.rock")).unwrap().facing = 8,
        "facing",
    );
    rejects(
        |c| {
            let mut spawn = c.spawns[&id("spawn.test.rock")].clone();
            spawn.id = id("spawn.test.duplicate");
            c.spawns.insert(spawn.id.clone(), spawn);
        },
        "duplicate placement",
    );
}

#[test]
fn overlapping_bounds_and_shared_source_map_squares_have_explicit_cell_owners() {
    let mut content = fixture();
    let original = content.regions.values_mut().next().unwrap();
    let cell = original.cells.pop().unwrap();
    let mut other = original.clone();
    other.id = id("region.test.other");
    other.cells = vec![cell];
    content.regions.insert(other.id.clone(), other);
    let compiled = compile_content(content, ValidationMode::TestFixture).unwrap();
    assert_eq!(
        compiled.region_at(cell.tile).unwrap().id,
        id("region.test.other")
    );
}

#[test]
fn recipes_tools_spawns_npcs_shops_and_probabilities_are_checked() {
    for (mutate, diagnostic) in [
        (
            (|c: &mut GameContent| gather(c).attempt_ticks = Some(0)) as Mutation,
            "durations",
        ),
        (|c| gather(c).respawn_ticks = Some(0), "durations"),
        (|c| gather(c).success.denominator = 0, "chance domain"),
        (
            |c| gather(c).depletion.numerator_at_level_99 = 4,
            "chance domain",
        ),
        (|c| gather(c).success = chance(0, 1), "impossible"),
        (
            |c| gather(c).tools = vec![id("item.test.missing")],
            "undefined item",
        ),
        (
            |c| gather(c).tools = vec![id("item.test.ore_note")],
            "noted item",
        ),
        (
            |c| gather(c).tools.push(id("item.test.pickaxe")),
            "duplicate",
        ),
        (|c| gather(c).output = stack("item.test.ore", 29), "28-slot"),
        (
            |c| c.recipes.values_mut().next().unwrap().ticks = Some(0),
            "duration",
        ),
        (
            |c| c.recipes.values_mut().next().unwrap().inputs.clear(),
            "empty",
        ),
        (
            |c| c.recipes.values_mut().next().unwrap().outputs.clear(),
            "empty",
        ),
        (
            |c| c.recipes.values_mut().next().unwrap().outputs[0].item = id("item.test.missing"),
            "undefined item",
        ),
        (
            |c| {
                c.recipes.values_mut().next().unwrap().target_objects =
                    vec![id("object.test.missing")]
            },
            "undefined target object",
        ),
        (
            |c| {
                c.recipes.values_mut().next().unwrap().target_objects = vec![id("object.test.rock")]
            },
            "does not allow",
        ),
        (
            |c| c.objects.values_mut().next().unwrap().size_x = 0,
            "dimensions",
        ),
        (
            |c| {
                c.npcs
                    .get_mut(&id("npc.test.monster"))
                    .unwrap()
                    .combat
                    .as_mut()
                    .unwrap()
                    .hitpoints = 0
            },
            "hitpoints",
        ),
        (
            |c| {
                c.npcs
                    .get_mut(&id("npc.test.monster"))
                    .unwrap()
                    .combat
                    .as_mut()
                    .unwrap()
                    .attack_speed_ticks = 0
            },
            "duration",
        ),
        (
            |c| {
                c.npcs
                    .get_mut(&id("npc.test.monster"))
                    .unwrap()
                    .combat
                    .as_mut()
                    .unwrap()
                    .drops[0]
                    .minimum_quantity = 0
            },
            "drop quantity",
        ),
        (
            |c| {
                c.npcs
                    .get_mut(&id("npc.test.monster"))
                    .unwrap()
                    .combat
                    .as_mut()
                    .unwrap()
                    .drops[0]
                    .maximum_quantity = MAX_STACK_QUANTITY + 1
            },
            "drop quantity",
        ),
        (
            |c| {
                c.npcs
                    .get_mut(&id("npc.test.monster"))
                    .unwrap()
                    .combat
                    .as_mut()
                    .unwrap()
                    .drops[0]
                    .numerator = 3
            },
            "probability",
        ),
        (
            |c| c.shops.values_mut().next().unwrap().currency = id("item.test.ore"),
            "currency",
        ),
        (
            |c| c.shops.values_mut().next().unwrap().stock[0].restock_ticks = 0,
            "restock",
        ),
        (
            |c| c.shops.values_mut().next().unwrap().stock[0].base_stock = MAX_STACK_QUANTITY + 1,
            "stock",
        ),
        (
            |c| c.shops.values_mut().next().unwrap().stock[0].sell_price = 5,
            "price",
        ),
        (
            |c| c.shops.values_mut().next().unwrap().stock[0].item = id("item.test.missing"),
            "undefined item",
        ),
        (
            |c| {
                c.spawns.get_mut(&id("spawn.test.rock")).unwrap().kind = SpawnKind::Object {
                    object: id("object.test.missing"),
                }
            },
            "undefined object",
        ),
        (
            |c| {
                c.spawns.get_mut(&id("spawn.test.monster")).unwrap().kind = SpawnKind::Npc {
                    npc: id("npc.test.missing"),
                }
            },
            "undefined NPC",
        ),
        (
            |c| {
                c.spawns.get_mut(&id("spawn.test.monster")).unwrap().kind = SpawnKind::Npc {
                    npc: id("npc.test.guide"),
                }
            },
            "combat definition",
        ),
    ] {
        rejects(mutate, diagnostic);
    }
}

#[test]
fn all_guard_and_effect_references_are_validated_recursively() {
    for (mutate, diagnostic) in [
        (
            (|c: &mut GameContent| {
                tutorial_transition(c).guard = Guard::Flag {
                    name: "missing".into(),
                    equals: 0,
                }
            }) as Mutation,
            "explicit initial value",
        ),
        (
            |c| {
                tutorial_transition(c).guard = Guard::Equipped {
                    item: id("item.test.ore"),
                }
            },
            "cannot be equipped",
        ),
        (
            |c| {
                tutorial_transition(c).guard = Guard::SkillAtLeast {
                    requirement: SkillRequirement {
                        skill: id("skill.test.missing"),
                        level: 1,
                        basis: SkillLevelBasis::Current,
                    },
                }
            },
            "undefined skill",
        ),
        (
            |c| {
                tutorial_transition(c).guard = Guard::InterfaceUnlocked {
                    interface: id("interface.test.missing"),
                }
            },
            "undefined interface",
        ),
        (
            |c| {
                tutorial_transition(c).guard = Guard::Within {
                    tile: tile(1100, 1100),
                    distance: 1,
                }
            },
            "collision cell",
        ),
        (
            |c| tutorial_transition(c).guard = Guard::Any { guards: vec![] },
            "empty",
        ),
        (
            |c| {
                tutorial_transition(c).effects.push(Effect::Travel {
                    region: id("region.test.field"),
                    tile: tile(1003, 1003),
                })
            },
            "not walkable",
        ),
        (
            |c| {
                tutorial_transition(c).effects.push(Effect::Conditional {
                    guard: Guard::Always,
                    effects: vec![Effect::SetQuestStage {
                        quest: id("quest.test.errand"),
                        stage: id("stage.test.missing"),
                    }],
                })
            },
            "undefined stage",
        ),
    ] {
        rejects(mutate, diagnostic);
    }
    let mut content = fixture();
    tutorial_transition(&mut content)
        .effects
        .push(Effect::UnlockInterface {
            interface: id("interface.test.bank"),
        });
    content
        .tutorial
        .get_mut(&id("stage.test.learn"))
        .unwrap()
        .transitions[0]
        .guard = Guard::InterfaceUnlocked {
        interface: id("interface.test.bank"),
    };
    compile_content(content, ValidationMode::TestFixture).unwrap();
}

#[test]
fn guard_effect_depth_and_total_nodes_are_bounded_before_recursive_work() {
    rejects(
        |c| {
            let mut guard = Guard::Always;
            for _ in 0..33 {
                guard = Guard::Not {
                    guard: Box::new(guard),
                };
            }
            tutorial_transition(c).guard = guard;
        },
        "depth",
    );
    rejects(
        |c| {
            tutorial_transition(c).guard = Guard::All {
                guards: vec![Guard::Always; 100_001],
            }
        },
        "node budget",
    );
    rejects(
        |c| {
            let mut effect = Effect::Message {
                text: "Synthetic depth test.".into(),
            };
            for _ in 0..33 {
                effect = Effect::Conditional {
                    guard: Guard::Always,
                    effects: vec![effect],
                };
            }
            tutorial_transition(c).effects.push(effect);
        },
        "depth",
    );
}

#[test]
fn rejecting_extremely_deep_owned_rules_does_not_overflow_the_drop_stack() {
    rejects(
        |c| {
            let mut guard = Guard::Always;
            for _ in 0..20_000 {
                guard = Guard::Not {
                    guard: Box::new(guard),
                };
            }
            tutorial_transition(c).guard = guard;
        },
        "depth",
    );
    rejects(
        |c| {
            let mut effect = Effect::Message {
                text: "Synthetic deep rejection.".into(),
            };
            for _ in 0..20_000 {
                effect = Effect::Conditional {
                    guard: Guard::Always,
                    effects: vec![effect],
                };
            }
            tutorial_transition(c).effects = vec![effect];
        },
        "depth",
    );
}

#[test]
fn atomic_effect_batches_cannot_overflow_quantities_or_xp() {
    rejects(
        |c| {
            tutorial_transition(c).effects.extend([
                Effect::GiveItems {
                    items: vec![stack("item.test.coins", MAX_STACK_QUANTITY)],
                },
                Effect::GiveItems {
                    items: vec![stack("item.test.coins", 1)],
                },
            ]);
        },
        "quantity limit",
    );
    rejects(
        |c| {
            tutorial_transition(c).effects.extend([
                Effect::AwardXp {
                    rewards: vec![XpReward {
                        skill: id("skill.test.mining"),
                        amount_tenths: 6_000,
                    }],
                },
                Effect::AwardXp {
                    rewards: vec![XpReward {
                        skill: id("skill.test.mining"),
                        amount_tenths: 6_000,
                    }],
                },
            ]);
        },
        "maximum XP",
    );
    rejects(
        |c| {
            c.skills
                .get_mut(&id("skill.test.mining"))
                .unwrap()
                .maximum_xp_tenths = u64::MAX;
            tutorial_transition(c).effects.extend([
                Effect::AwardXp {
                    rewards: vec![XpReward {
                        skill: id("skill.test.mining"),
                        amount_tenths: u64::MAX,
                    }],
                },
                Effect::AwardXp {
                    rewards: vec![XpReward {
                        skill: id("skill.test.mining"),
                        amount_tenths: 1,
                    }],
                },
            ]);
        },
        "overflows u64",
    );
}

#[test]
fn repeatable_dialogue_loops_are_allowed_but_dangling_duplicate_and_unreachable_nodes_fail() {
    for (mutate, diagnostic) in [
        (
            (|c: &mut GameContent| {
                c.dialogues.values_mut().next().unwrap().entry_nodes = vec!["missing".into()]
            }) as Mutation,
            "undefined entry",
        ),
        (
            |c| {
                c.dialogues.values_mut().next().unwrap().nodes[0].choices[0].next_node =
                    Some("missing".into())
            },
            "undefined next",
        ),
        (
            |c| {
                let d = c.dialogues.values_mut().next().unwrap();
                d.nodes.push(d.nodes[0].clone());
            },
            "duplicate dialogue",
        ),
        (
            |c| {
                let n = &mut c.dialogues.values_mut().next().unwrap().nodes[0];
                n.choices.push(n.choices[0].clone());
            },
            "duplicate",
        ),
        (
            |c| {
                let d = c.dialogues.values_mut().next().unwrap();
                let mut n = d.nodes[1].clone();
                n.id = "orphan".into();
                d.nodes.push(n);
            },
            "unreachable",
        ),
        (
            |c| {
                c.dialogues.values_mut().next().unwrap().nodes[0].guard = Guard::Not {
                    guard: Box::new(Guard::Always),
                }
            },
            "unreachable",
        ),
    ] {
        rejects(mutate, diagnostic);
    }
}

#[test]
fn dialogue_effects_may_enable_a_different_guard_at_the_next_node() {
    let mut content = fixture();
    let dialogue = content.dialogues.values_mut().next().unwrap();
    dialogue.nodes[0].guard = Guard::TutorialStage {
        stage: id("stage.test.start"),
    };
    dialogue.nodes[0].choices[0]
        .effects
        .push(Effect::SetTutorialStage {
            stage: id("stage.test.learn"),
        });
    dialogue.nodes[1].guard = Guard::TutorialStage {
        stage: id("stage.test.learn"),
    };
    compile_content(content, ValidationMode::TestFixture).unwrap();
}

#[test]
fn conditional_stage_writes_cannot_create_ambiguous_last_write_wins() {
    rejects(
        |c| {
            tutorial_transition(c).effects.push(Effect::Conditional {
                guard: Guard::Always,
                effects: vec![Effect::SetTutorialStage {
                    stage: id("stage.test.done"),
                }],
            });
        },
        "multiple unconditional or conditional stage writes",
    );
}

#[test]
fn stage_graphs_reject_dangling_targets_constant_false_paths_and_closed_progression_loops() {
    for (mutate, diagnostic) in [
        (
            (|c: &mut GameContent| tutorial_transition(c).event = "invented_event".into())
                as Mutation,
            "unsupported",
        ),
        (
            |c| tutorial_transition(c).target = Some("spawn.test.missing".into()),
            "undefined event target",
        ),
        (
            |c| {
                tutorial_transition(c).event = "moved".into();
            },
            "no string target",
        ),
        (
            |c| {
                tutorial_transition(c).guard = Guard::Not {
                    guard: Box::new(Guard::Always),
                }
            },
            "unreachable",
        ),
        (
            |c| {
                tutorial_transition(c).guard = Guard::All {
                    guards: vec![
                        Guard::TutorialStage {
                            stage: id("stage.test.start"),
                        },
                        Guard::TutorialStage {
                            stage: id("stage.test.learn"),
                        },
                    ],
                }
            },
            "unreachable",
        ),
        (
            |c| {
                tutorial_transition(c).guard = Guard::All {
                    guards: vec![
                        Guard::Flag {
                            name: "test_seen".into(),
                            equals: 0,
                        },
                        Guard::Flag {
                            name: "test_seen".into(),
                            equals: 1,
                        },
                    ],
                }
            },
            "unreachable",
        ),
        (
            |c| {
                tutorial_transition(c).effects = vec![Effect::SetTutorialStage {
                    stage: id("stage.test.missing"),
                }]
            },
            "undefined tutorial",
        ),
        (
            |c| {
                c.quests.values_mut().next().unwrap().journal.insert(
                    id("stage.test.orphan"),
                    "Synthetic unreachable state.".into(),
                );
            },
            "unreachable",
        ),
        (
            |c| {
                c.tutorial
                    .get_mut(&id("stage.test.start"))
                    .unwrap()
                    .allowed_actions
                    .push("advance_stage".into())
            },
            "unknown allowed action",
        ),
    ] {
        rejects(mutate, diagnostic);
    }
    rejects(
        |c| {
            c.tutorial.remove(&id("stage.test.done"));
            c.tutorial
                .get_mut(&id("stage.test.learn"))
                .unwrap()
                .transitions[0]
                .effects = vec![Effect::SetTutorialStage {
                stage: id("stage.test.start"),
            }];
        },
        "closed progression loop",
    );
}

#[test]
fn quest_rewards_must_advance_atomically_and_exclude_their_destination() {
    rejects(|c| quest_transition(c).guard = Guard::Always, "repeatable");
    rejects(
        |c| {
            quest_transition(c)
                .effects
                .retain(|effect| !matches!(effect, Effect::SetQuestStage { .. }));
        },
        "destination",
    );
    rejects(
        |c| {
            let transition = quest_transition(c);
            let stage = transition.effects.pop().unwrap();
            transition.effects.push(Effect::Conditional {
                guard: initial_quest_guard(),
                effects: vec![stage],
            });
        },
        "destination",
    );
    rejects(
        |c| {
            tutorial_transition(c)
                .effects
                .push(Effect::AddQuestPoints { amount: 1 })
        },
        "destination",
    );
    rejects(
        |c| {
            quest_transition(c).effects.push(Effect::SetQuestStage {
                quest: id("quest.test.errand"),
                stage: id("stage.test.quest_done"),
            });
        },
        "multiple unconditional",
    );
    rejects(
        |c| {
            c.quests
                .values_mut()
                .next()
                .unwrap()
                .transitions
                .push(ProgressTransition {
                    event: "interacted".into(),
                    target: None,
                    guard: Guard::Always,
                    effects: vec![Effect::SetQuestStage {
                        quest: id("quest.test.errand"),
                        stage: id("stage.test.quest_start"),
                    }],
                });
        },
        "can be reset",
    );
    let mut content = fixture();
    let transition = quest_transition(&mut content);
    let guard = std::mem::replace(&mut transition.guard, Guard::Always);
    let effects = std::mem::take(&mut transition.effects);
    transition.effects = vec![Effect::Conditional { guard, effects }];
    compile_content(content, ValidationMode::TestFixture).unwrap();
}

#[test]
fn optional_asset_manifest_checks_membership_without_claiming_presentation() {
    let mut compiled = compile_content(fixture(), ValidationMode::TestFixture).unwrap();
    let missing = AssetManifest {
        identity: "synthetic-manifest-v1".into(),
        assets: BTreeSet::new(),
    };
    assert!(
        compiled
            .check_asset_manifest(&missing)
            .unwrap_err()
            .message
            .contains("absent")
    );
    assert!(compiled.report().asset_manifest.is_none());
    let complete = AssetManifest {
        identity: "synthetic-manifest-v1".into(),
        assets: compiled.referenced_assets().clone(),
    };
    compiled.check_asset_manifest(&complete).unwrap();
    assert_eq!(
        compiled.report().asset_manifest.as_deref(),
        Some("synthetic-manifest-v1")
    );
    assert!(!compiled.report().presentation_verification_performed);
    compile_content_with_manifest(fixture(), ValidationMode::TestFixture, &complete).unwrap();
}
