#![allow(dead_code)]

use std::collections::BTreeMap;

use clubscape_game_types::*;

pub fn id<T: TryFrom<String, Error = GameError>>(value: &str) -> T {
    T::try_from(value.to_owned()).unwrap()
}

pub fn tile(x: u16, y: u16) -> Tile {
    Tile::new(x, y, 0).unwrap()
}

pub fn stack(item: &str, quantity: u32) -> ItemStack {
    ItemStack {
        item: id(item),
        quantity: Quantity::new(quantity).unwrap(),
    }
}

pub fn sources() -> Vec<SourceRecord> {
    vec![SourceRecord {
        reference: "fixture:clubscape-content/strict-contract-tests".into(),
        revision: "synthetic-v1".into(),
        status: EvidenceStatus::TestFixture,
        notes: "Synthetic compiler test only; not OSRS data, source verification, or M1 acceptance evidence.".into(),
    }]
}

pub fn interface(name: &str, source_id: u32) -> InterfaceDefinition {
    InterfaceDefinition {
        id: id(&format!("interface.test.{name}")),
        name: format!("Synthetic {name} interface"),
        source_ids: vec![source_id],
        source: sources(),
    }
}

fn item(name: &str, source_id: u32, stackable: bool) -> ItemDefinition {
    ItemDefinition {
        id: id(&format!("item.test.{name}")),
        name: format!("Synthetic {name}"),
        source_id: Some(source_id),
        stackable,
        tradable: true,
        base_value: 1,
        equipment: None,
        noted_variant: None,
        unnoted_variant: None,
        healing: None,
        asset: Some(id(&format!("asset.test.{name}"))),
        source: sources(),
    }
}

fn equipment(primary: &str, occupied: &[&str], weapon: bool) -> EquipmentDefinition {
    EquipmentDefinition {
        slot: id(primary),
        occupied_slots: occupied.iter().map(|slot| id(slot)).collect(),
        requirements: vec![SkillRequirement {
            skill: id("skill.test.mining"),
            level: 1,
        }],
        bonuses: CombatBonuses::default(),
        attack_speed_ticks: weapon.then_some(4),
        attack_styles: if weapon {
            vec!["accurate".into()]
        } else {
            vec![]
        },
    }
}

pub fn chance(numerator: u32, denominator: u32) -> ChanceRule {
    ChanceRule {
        numerator_at_level_1: numerator,
        numerator_at_level_99: numerator,
        denominator,
    }
}

pub fn initial_quest_guard() -> Guard {
    Guard::QuestStage {
        quest: id("quest.test.errand"),
        stage: id("stage.test.quest_start"),
    }
}

pub fn fixture() -> GameContent {
    let mut items: BTreeMap<_, _> = [
        item("coins", 1, true),
        item("pickaxe", 2, false),
        item("shield", 3, false),
        item("two_handed", 4, false),
        item("ore", 5, false),
        item("ore_note", 6, true),
        item("bar", 7, false),
        item("food", 8, false),
        item("ammo", 9, true),
    ]
    .into_iter()
    .map(|item| (item.id.clone(), item))
    .collect();
    items.get_mut(&id("item.test.pickaxe")).unwrap().equipment =
        Some(equipment("slot.test.weapon", &["slot.test.weapon"], true));
    items.get_mut(&id("item.test.shield")).unwrap().equipment = Some(equipment(
        "slot.test.offhand",
        &["slot.test.offhand"],
        false,
    ));
    items
        .get_mut(&id("item.test.two_handed"))
        .unwrap()
        .equipment = Some(equipment(
        "slot.test.weapon",
        &["slot.test.weapon", "slot.test.offhand"],
        true,
    ));
    items.get_mut(&id("item.test.ammo")).unwrap().equipment =
        Some(equipment("slot.test.ammo", &["slot.test.ammo"], false));
    items.get_mut(&id("item.test.ore")).unwrap().noted_variant = Some(id("item.test.ore_note"));
    items
        .get_mut(&id("item.test.ore_note"))
        .unwrap()
        .unnoted_variant = Some(id("item.test.ore"));
    items.get_mut(&id("item.test.food")).unwrap().healing = Some(1);
    let skills: BTreeMap<_, _> = ["mining", "smithing", "hitpoints"]
        .into_iter()
        .enumerate()
        .map(|(number, name)| {
            let skill = SkillDefinition {
                id: id(&format!("skill.test.{name}")),
                name: format!("Synthetic {name}"),
                source_id: number as u16,
                xp_thresholds_tenths: vec![0, 1_000, 3_000],
                maximum_xp_tenths: 10_000,
                source: sources(),
            };
            (skill.id.clone(), skill)
        })
        .collect();
    let region = RegionDefinition {
        id: id("region.test.field"),
        name: "Synthetic validation field".into(),
        min: tile(1000, 1000),
        max: tile(1006, 1006),
        cells: (1000..=1006)
            .flat_map(|x| {
                (1000..=1006).map(move |y| CollisionCell {
                    tile: tile(x, y),
                    height: 0,
                    walkable: (x, y) != (1003, 1003),
                    blocked_movement: 0,
                    blocked_sight: 0,
                })
            })
            .collect(),
        source_map_squares: vec![42],
        scene_asset: None,
        source: sources(),
    };
    let objects: BTreeMap<_, _> = ["rock", "furnace"]
        .into_iter()
        .enumerate()
        .map(|(number, name)| {
            let object = ObjectDefinition {
                id: id(&format!("object.test.{name}")),
                name: format!("Synthetic {name}"),
                source_id: number as u32,
                size_x: 1,
                size_y: 1,
                asset: None,
                source: sources(),
            };
            (object.id.clone(), object)
        })
        .collect();
    let guide = NpcDefinition {
        id: id("npc.test.guide"),
        name: "Synthetic guide".into(),
        source_id: 1,
        size: 1,
        combat: None,
        asset: None,
        source: sources(),
    };
    let monster = NpcDefinition {
        id: id("npc.test.monster"),
        name: "Synthetic monster".into(),
        source_id: 2,
        size: 1,
        combat: Some(NpcCombatDefinition {
            hitpoints: 5,
            attack: 1,
            strength: 1,
            defence: 1,
            ranged: 0,
            magic: 0,
            attack_speed_ticks: 4,
            max_hit: 1,
            bonuses: CombatBonuses::default(),
            respawn_ticks: 5,
            aggressive: false,
            drops: vec![DropDefinition {
                item: id("item.test.coins"),
                minimum_quantity: 1,
                maximum_quantity: 3,
                numerator: 1,
                denominator: 2,
            }],
        }),
        asset: None,
        source: sources(),
    };
    let recipe = RecipeDefinition {
        id: id("recipe.test.bar"),
        name: "Synthetic ore processing".into(),
        inputs: vec![stack("item.test.ore", 2)],
        outputs: vec![stack("item.test.bar", 1)],
        failed_outputs: vec![],
        tools: vec![id("item.test.pickaxe")],
        requirements: vec![SkillRequirement {
            skill: id("skill.test.smithing"),
            level: 1,
        }],
        xp: vec![XpReward {
            skill: id("skill.test.smithing"),
            amount_tenths: 200,
        }],
        ticks: 3,
        success: chance(1, 1),
        target_objects: vec![id("object.test.furnace")],
        source: sources(),
    };
    let dialogue = DialogueDefinition {
        id: id("dialogue.test.guide"),
        nodes: vec![
            DialogueNode {
                id: "entry".into(),
                text: "Synthetic repeatable conversation.".into(),
                guard: Guard::Always,
                choices: vec![
                    DialogueChoice {
                        id: "ask".into(),
                        text: "Ask again.".into(),
                        guard: Guard::Always,
                        effects: vec![Effect::Message {
                            text: "Synthetic information.".into(),
                        }],
                        next_node: Some("more".into()),
                    },
                    DialogueChoice {
                        id: "leave".into(),
                        text: "Leave.".into(),
                        guard: Guard::Always,
                        effects: vec![],
                        next_node: None,
                    },
                ],
            },
            DialogueNode {
                id: "more".into(),
                text: "This intentional loop has no one-time reward.".into(),
                guard: Guard::Always,
                choices: vec![DialogueChoice {
                    id: "again".into(),
                    text: "Return.".into(),
                    guard: Guard::Always,
                    effects: vec![],
                    next_node: Some("entry".into()),
                }],
            },
        ],
        entry_nodes: vec!["entry".into()],
        source: sources(),
    };
    let shop = ShopDefinition {
        id: id("shop.test.general"),
        name: "Synthetic shop".into(),
        currency: id("item.test.coins"),
        stock: vec![ShopItem {
            item: id("item.test.ore"),
            base_stock: 10,
            restock_ticks: 5,
            buy_price: 4,
            sell_price: 2,
        }],
        accepts_general_items: true,
        source: sources(),
    };
    let spawn = |name: &str, tile: Tile, kind: SpawnKind, interactions| SpawnDefinition {
        id: id(&format!("spawn.test.{name}")),
        region: region.id.clone(),
        tile,
        facing: 0,
        kind,
        interactions,
        source: sources(),
    };
    let action = |name: &str, action| InteractionDefinition {
        name: name.into(),
        reach: 1,
        guard: Guard::Always,
        action,
    };
    let spawns: BTreeMap<_, _> = [
        spawn(
            "rock",
            tile(1003, 1003),
            SpawnKind::Object {
                object: id("object.test.rock"),
            },
            vec![action(
                "Mine",
                InteractionAction::Gather {
                    rule: GatherRule {
                        skill: id("skill.test.mining"),
                        required_level: 1,
                        tools: vec![id("item.test.pickaxe")],
                        output: stack("item.test.ore", 1),
                        xp_tenths: 100,
                        attempt_ticks: 2,
                        success: chance(1, 2),
                        depletion: chance(1, 3),
                        respawn_ticks: 4,
                        animation: None,
                        sound: None,
                    },
                },
            )],
        ),
        spawn(
            "guide",
            tile(1002, 1001),
            SpawnKind::Npc {
                npc: guide.id.clone(),
            },
            vec![
                action(
                    "Talk",
                    InteractionAction::Dialogue {
                        dialogue: dialogue.id.clone(),
                    },
                ),
                action("Bank", InteractionAction::Bank),
                action(
                    "Shop",
                    InteractionAction::Shop {
                        shop: shop.id.clone(),
                    },
                ),
            ],
        ),
        spawn(
            "monster",
            tile(1004, 1002),
            SpawnKind::Npc {
                npc: monster.id.clone(),
            },
            vec![action("Attack", InteractionAction::Attack)],
        ),
        spawn(
            "furnace",
            tile(1004, 1003),
            SpawnKind::Object {
                object: id("object.test.furnace"),
            },
            vec![action(
                "Smelt",
                InteractionAction::Production {
                    recipes: vec![recipe.id.clone()],
                },
            )],
        ),
        spawn(
            "ore",
            tile(1003, 1003),
            SpawnKind::Item {
                stack: stack("item.test.ore", 1),
                respawn_ticks: 10,
            },
            vec![],
        ),
    ]
    .into_iter()
    .map(|spawn| (spawn.id.clone(), spawn))
    .collect();
    let tutorial = ["start", "learn", "done"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            let transitions = match index {
                0 => vec![ProgressTransition {
                    event: "interacted".into(),
                    target: Some("spawn.test.rock".into()),
                    guard: Guard::Always,
                    effects: vec![Effect::SetTutorialStage {
                        stage: id("stage.test.learn"),
                    }],
                }],
                1 => vec![ProgressTransition {
                    event: "gathered".into(),
                    target: Some("spawn.test.rock".into()),
                    guard: Guard::Always,
                    effects: vec![Effect::SetTutorialStage {
                        stage: id("stage.test.done"),
                    }],
                }],
                _ => vec![],
            };
            let stage = TutorialStageDefinition {
                id: id(&format!("stage.test.{name}")),
                instruction: format!("Synthetic {name} stage."),
                allowed_actions: vec!["walk".into(), "interact".into(), "select_dialogue".into()],
                xp_caps_tenths: BTreeMap::new(),
                xp_stop_levels: BTreeMap::new(),
                nonfatal_combat: true,
                transitions,
                source: sources(),
            };
            (stage.id.clone(), stage)
        })
        .collect();
    let quest = QuestDefinition {
        id: id("quest.test.errand"),
        name: "Synthetic errand".into(),
        initial_stage: id("stage.test.quest_start"),
        completed_stage: id("stage.test.quest_done"),
        journal: BTreeMap::from([
            (
                id("stage.test.quest_start"),
                "Synthetic initial quest state.".into(),
            ),
            (
                id("stage.test.quest_done"),
                "Synthetic completed quest state.".into(),
            ),
        ]),
        transitions: vec![ProgressTransition {
            event: "dialogue_selected".into(),
            target: Some("spawn.test.guide".into()),
            guard: Guard::All {
                guards: vec![
                    initial_quest_guard(),
                    Guard::HasItems {
                        items: vec![stack("item.test.bar", 1)],
                    },
                ],
            },
            effects: vec![
                Effect::TakeItems {
                    items: vec![stack("item.test.bar", 1)],
                },
                Effect::GiveItems {
                    items: vec![stack("item.test.coins", 20)],
                },
                Effect::AwardXp {
                    rewards: vec![XpReward {
                        skill: id("skill.test.smithing"),
                        amount_tenths: 200,
                    }],
                },
                Effect::AddQuestPoints { amount: 1 },
                Effect::SetQuestStage {
                    quest: id("quest.test.errand"),
                    stage: id("stage.test.quest_done"),
                },
            ],
        }],
        source: sources(),
    };
    let mut inventory = Inventory::default();
    for (index, stack) in [
        stack("item.test.pickaxe", 1),
        stack("item.test.coins", 100),
        stack("item.test.ore", 1),
        stack("item.test.ore", 1),
        stack("item.test.bar", 1),
    ]
    .into_iter()
    .enumerate()
    {
        inventory.slots[index] = Some(stack);
    }
    let initial_state = InitialStateDefinition {
        region: region.id.clone(),
        tile: tile(1000, 1000),
        inventory,
        equipment: BTreeMap::from([(id("slot.test.ammo"), stack("item.test.ammo", 20))]),
        bank: Bank {
            capacity: 32,
            slots: vec![],
        },
        skills: skills
            .keys()
            .map(|id| {
                (
                    id.clone(),
                    SkillState {
                        xp_tenths: 0,
                        current_level: 1,
                    },
                )
            })
            .collect(),
        hitpoints: 1,
        prayer_points: 0,
        run_energy: MAX_RUN_ENERGY,
        tutorial_stage: id("stage.test.start"),
        quest_points: 0,
        quests: BTreeMap::from([(
            quest.id.clone(),
            QuestState {
                stage: quest.initial_stage.clone(),
                flags: BTreeMap::new(),
            },
        )]),
        flags: BTreeMap::from([("test_seen".into(), 0)]),
        interfaces: vec![id("interface.test.inventory")],
        source: sources(),
    };
    GameContent {
        schema_version: GAME_SCHEMA_VERSION,
        revision: "synthetic-v1".into(),
        baseline: "fixture:strict-content-v1".into(),
        items,
        skills,
        regions: BTreeMap::from([(region.id.clone(), region)]),
        spawns,
        objects,
        npcs: BTreeMap::from([(guide.id.clone(), guide), (monster.id.clone(), monster)]),
        recipes: BTreeMap::from([(recipe.id.clone(), recipe)]),
        dialogues: BTreeMap::from([(dialogue.id.clone(), dialogue)]),
        tutorial,
        quests: BTreeMap::from([(quest.id.clone(), quest)]),
        shops: BTreeMap::from([(shop.id.clone(), shop)]),
        interfaces: [interface("inventory", 1), interface("bank", 2)]
            .into_iter()
            .map(|interface| (interface.id.clone(), interface))
            .collect(),
        equipment_slots: vec![
            id("slot.test.weapon"),
            id("slot.test.offhand"),
            id("slot.test.ammo"),
        ],
        initial_state,
    }
}

pub fn runtime_policy_fixture() -> GameContent {
    let mut content = fixture();
    content.baseline = "osrs:synthetic-policy-test-build-1".into();
    let records = |sources: &mut Vec<SourceRecord>| {
        for source in sources {
            source.reference = "tests/fixtures/structural-provenance-policy".into();
            source.status = EvidenceStatus::Inference;
            source.notes =
                "Synthetic test of runtime provenance policy, not an actual source observation."
                    .into();
        }
    };
    macro_rules! replace {
        ($($field:ident),+ $(,)?) => {$(
            for definition in content.$field.values_mut() { records(&mut definition.source); }
        )+};
    }
    replace!(
        items, skills, regions, spawns, objects, npcs, recipes, dialogues, tutorial, quests, shops,
        interfaces
    );
    records(&mut content.initial_state.source);
    content
}

pub fn gather(content: &mut GameContent) -> &mut GatherRule {
    let InteractionAction::Gather { rule } = &mut content
        .spawns
        .get_mut(&id("spawn.test.rock"))
        .unwrap()
        .interactions[0]
        .action
    else {
        panic!("fixture gather")
    };
    rule
}

pub fn quest_transition(content: &mut GameContent) -> &mut ProgressTransition {
    &mut content
        .quests
        .get_mut(&id("quest.test.errand"))
        .unwrap()
        .transitions[0]
}

pub fn tutorial_transition(content: &mut GameContent) -> &mut ProgressTransition {
    &mut content
        .tutorial
        .get_mut(&id("stage.test.start"))
        .unwrap()
        .transitions[0]
}
