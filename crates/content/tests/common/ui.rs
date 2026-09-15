use super::*;

pub fn enable(content: &mut GameContent) {
    projection(content);
    let sources = sources();
    let bound = |value| SourceBinding::Bound {
        value,
        source: sources.clone(),
    };
    let mut potion = content.items[&id("item.test.food")].clone();
    for (offset, name) in ["potion_2", "potion_1", "vial", "bones"]
        .into_iter()
        .enumerate()
    {
        potion.id = id(&format!("item.test.{name}"));
        potion.name = format!("Synthetic {name}");
        potion.source_id = Some(9600 + offset as u32);
        potion.healing = None;
        content.items.insert(potion.id.clone(), potion.clone());
    }
    for item in content.items.values_mut() {
        item.weight = Some(SourceBinding::Bound {
            value: ItemWeight {
                grams: 1,
                inventory: WeightContribution::PerUnit,
                equipment: WeightContribution::PerUnit,
            },
            source: sources.clone(),
        });
    }
    let mut actions = BTreeMap::new();
    for (old, new) in [("potion_2", "potion_1"), ("potion_1", "vial")] {
        actions.insert(
            id(&format!("item.test.{old}")),
            vec![
                ItemUiDefinition {
                    id: "drink".into(),
                    label: "Drink".into(),
                    guard: Guard::Always,
                    action: ItemUiAction::Drink {
                        replacement: id(&format!("item.test.{new}")),
                        cooldown: id("action.test.drink"),
                        delay_ticks: bound(3),
                        restore: BTreeMap::from([(
                            Vital::RunEnergy,
                            VitalRestoration::Amount { amount: 1500 },
                        )]),
                        skills: Vec::new(),
                    },
                    source: sources.clone(),
                },
                ItemUiDefinition {
                    id: "empty".into(),
                    label: "Empty".into(),
                    guard: Guard::Always,
                    action: ItemUiAction::Empty {
                        replacement: id("item.test.vial"),
                    },
                    source: sources.clone(),
                },
            ],
        );
    }
    let mut burial = content.recipes[&id("recipe.test.bar")].clone();
    burial.id = id("recipe.test.bury");
    burial.name = "Synthetic source burial".into();
    burial.inputs = vec![stack("item.test.bones", 1)];
    burial.outputs.clear();
    burial.tools.clear();
    burial.target_objects.clear();
    burial.xp[0].amount_tenths = 45;
    burial.ticks = None;
    burial.mechanics = Some(RecipeMechanics {
        method: id("action.test.bury"),
        guard: Guard::Always,
        chance_skill: None,
        cadence: ActionCadence {
            single: bound(2),
            first: bound(2),
            repeat: bound(2),
            menu_delay: bound(0),
        },
        tool_ownership: OwnershipScope::InventoryAndEquipment,
        failed_xp: Vec::new(),
        success_effects: Vec::new(),
        failure_effects: Vec::new(),
        lifecycle: RecipeLifecycle::ConsumeOnly,
    });
    content.recipes.insert(burial.id.clone(), burial);
    actions.insert(
        id("item.test.bones"),
        vec![ItemUiDefinition {
            id: "bury".into(),
            label: "Bury".into(),
            guard: Guard::Always,
            action: ItemUiAction::ConsumeRecipe {
                recipe: id("recipe.test.bury"),
            },
            source: sources.clone(),
        }],
    );
    actions.insert(
        id("item.test.food"),
        vec![ItemUiDefinition {
            id: "read".into(),
            label: "Read".into(),
            guard: Guard::Always,
            action: ItemUiAction::Read {
                interface: id("interface.test.ui_book"),
                title: "Synthetic source text".into(),
                pages: vec![
                    "Synthetic first page.".into(),
                    "Synthetic second page.".into(),
                ],
                map_asset: None,
                native_map: false,
            },
            source: sources.clone(),
        }],
    );
    let quest_id: QuestId = id("quest.test.errand");
    let reward_id: EntitlementId = id("entitlement.test.ui_reward");
    let mut rewards = BTreeMap::new();
    if let Some(quest) = content.quests.get_mut(&quest_id) {
        let original = std::mem::take(&mut quest.transitions[0].effects);
        quest.transitions[0].effects = vec![Effect::Once {
            entitlement: reward_id.clone(),
            effects: original,
        }];
        content.mechanics.entitlements.insert(
            reward_id.clone(),
            EntitlementDefinition {
                id: reward_id.clone(),
                purpose: EntitlementPurpose::AtomicReward,
                source: sources.clone(),
            },
        );
        rewards.insert(
            quest_id,
            QuestUiDefinition {
                entitlement: reward_id,
                interface: id("interface.test.ui_level_up"),
                title: "Synthetic source quest reward".into(),
                lines: vec!["One source quest point and the declared test rewards.".into()],
                items: vec![stack("item.test.coins", 20)],
                xp: vec![XpReward {
                    skill: id("skill.test.smithing"),
                    amount_tenths: 200,
                }],
                quest_points: 1,
                source: sources.clone(),
            },
        );
    }
    let ui = content.ui.as_mut().unwrap();
    ui.item_actions = actions;
    ui.quest_rewards = rewards;
    ui.production_interfaces = content
        .recipes
        .keys()
        .map(|recipe| (recipe.clone(), id("interface.test.ui_production")))
        .collect();
}

pub fn projection(content: &mut GameContent) {
    let sources = sources();
    for (offset, name) in ["production", "level_up", "book", "equipment_stats"]
        .into_iter()
        .enumerate()
    {
        let id: InterfaceId = id(&format!("interface.test.ui_{name}"));
        content.interfaces.insert(
            id.clone(),
            InterfaceDefinition {
                id,
                name: format!("Synthetic UI {name}"),
                access: InterfaceAccess::Contextual,
                source_ids: vec![9500 + offset as u32],
                source: sources.clone(),
            },
        );
    }
    let stage_interfaces = content
        .tutorial
        .keys()
        .map(|stage| {
            (
                stage.clone(),
                content
                    .interfaces
                    .keys()
                    .map(|interface| InterfaceUiRule {
                        interface: interface.clone(),
                        visibility: if content.interfaces[interface].access
                            == InterfaceAccess::Contextual
                        {
                            UiVisibility::Hidden
                        } else {
                            UiVisibility::Enabled
                        },
                        highlighted: false,
                        guard: Guard::Always,
                        unavailable_reason: None,
                        source: sources.clone(),
                    })
                    .collect(),
            )
        })
        .collect();
    let weapon_style_names = content
        .items
        .iter()
        .filter_map(|(id, item)| {
            item.equipment
                .as_ref()
                .and_then(|equipment| equipment.weapon.as_ref())
                .map(|weapon| {
                    (
                        id.clone(),
                        weapon
                            .styles
                            .iter()
                            .map(|id| (id.clone(), format!("Synthetic {id}")))
                            .collect(),
                    )
                })
        })
        .collect();
    let unarmed_style_names = content
        .mechanics
        .player_combat
        .as_ref()
        .and_then(|policy| policy.unarmed.require().ok())
        .map(|weapon| {
            weapon
                .styles
                .iter()
                .map(|id| (id.clone(), format!("Synthetic {id}")))
                .collect()
        })
        .unwrap_or_default();
    content.ui = Some(GameplayUiDefinition {
        version: 1,
        production_interfaces: content
            .recipes
            .keys()
            .map(|recipe| (recipe.clone(), id("interface.test.ui_production")))
            .collect(),
        direct_production: std::collections::BTreeSet::new(),
        quest_rewards: BTreeMap::new(),
        level_up: LevelUpUiDefinition {
            interface: id("interface.test.ui_level_up"),
            title: "Synthetic {skill} level".into(),
            line: "Source level {level}.".into(),
            source: sources.clone(),
        },
        stage_interfaces,
        stage_overlays: content
            .tutorial
            .keys()
            .map(|id| (id.clone(), None))
            .collect(),
        equipment_stats_interface: id("interface.test.ui_equipment_stats"),
        death_preview_interface: id("interface.test.ui_level_up"),
        ability_names: content
            .mechanics
            .prayers
            .keys()
            .map(|id| (id.to_string(), format!("Synthetic {id}")))
            .chain(
                content
                    .mechanics
                    .spells
                    .keys()
                    .map(|id| (id.to_string(), format!("Synthetic {id}"))),
            )
            .collect(),
        weapon_style_names,
        unarmed_style_names,
        item_actions: BTreeMap::new(),
        bank: BankUiDefinition {
            maximum_tabs: 9,
            initial_insert: false,
            initial_placeholders: false,
            unavailable_containers: Vec::new(),
            source: sources.clone(),
        },
        coffer: SourceBinding::Bound {
            value: CofferUiDefinition {
                eligible_items: content
                    .items
                    .keys()
                    .filter(|item| item.as_str() == "item.test.bar")
                    .cloned()
                    .collect(),
                exchange_values: content
                    .items
                    .keys()
                    .filter(|item| item.as_str() == "item.test.bar")
                    .map(|item| (item.clone(), 10000))
                    .collect(),
                minimum_value: 10000,
                credit: Ratio {
                    numerator: 105,
                    denominator: 100,
                },
                maximum_balance: 2147483647,
                source: sources.clone(),
            },
            source: sources.clone(),
        },
        chat: ChatUiDefinition {
            guard: Guard::Always,
            maximum_bytes: 80,
            radius: 15,
            messages_per_window: 5,
            window_ticks: 8,
            source: sources.clone(),
        },
        appearance_base: SourceBinding::Bound {
            value: PenguinBaseUiView {
                asset: id("asset.test.penguin"),
                source_npc: 0,
                adaptation: "synthetic only; not product".into(),
            },
            source: sources.clone(),
        },
        source: sources,
    });
}
