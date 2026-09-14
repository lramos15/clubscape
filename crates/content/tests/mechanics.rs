mod common;

use std::collections::{BTreeMap, BTreeSet};

use clubscape_content::{
    ValidationMode, compile_content, encode_compiled, load_compiled, read_content_json,
};
use clubscape_game_types::*;
use common::*;

fn bound<T>(value: T) -> SourceBinding<T> {
    SourceBinding::Bound {
        value,
        source: sources(),
    }
}

fn missing<T>(reason: &str) -> SourceBinding<T> {
    SourceBinding::Unresolved {
        reason: reason.into(),
        source: sources(),
    }
}

fn levels() -> LevelDomain {
    LevelDomain {
        minimum: 1,
        maximum: 99,
        basis: SkillLevelBasis::Current,
    }
}

fn cadence(single: u32, first: u32, repeat: u32) -> ActionCadence {
    ActionCadence {
        single: bound(single),
        first: bound(first),
        repeat: bound(repeat),
        menu_delay: bound(0),
    }
}

fn location(x: u16, y: u16, instance: Option<InstanceTemplateId>) -> WorldLocation {
    WorldLocation {
        region: id("region.test.field"),
        tile: tile(x, y),
        instance,
    }
}

fn source_placement() -> SourceObjectPlacement {
    SourceObjectPlacement {
        shape: 10,
        quarter_turns: 0,
        layer: ObjectLayer::GameObject,
    }
}

fn mechanic_fixture() -> GameContent {
    let mut content = fixture();
    for skill in content.skills.values_mut() {
        skill.xp_thresholds_tenths = (0..99).map(|level| level * 1000).collect();
        skill.maximum_xp_tenths = 1_000_000;
    }
    for item in content.items.values_mut() {
        item.weight = Some(bound(ItemWeight {
            grams: 100,
            inventory: WeightContribution::PerUnit,
            equipment: WeightContribution::PerUnit,
        }));
    }
    for slot in [
        "head", "cape", "neck", "body", "legs", "hands", "feet", "ring",
    ] {
        content
            .equipment_slots
            .push(id(&format!("slot.test.{slot}")));
    }
    content.initial_state.runtime.settings = CharacterSettings {
        run_enabled: Some(false),
        auto_retaliate: Some(true),
        death_auto_equip: Some(false),
        death_supply_piles: Some(false),
        experience: None,
        appearance_confirmed: false,
    };
    let flour = CounterDefinition {
        id: id("counter.test.flour"),
        scope: CounterScope::Character,
        value_type: CounterType::Integer {
            minimum: 0,
            maximum: 30,
        },
        initial: CounterValue::Integer(0),
        source_variable: Some(SourceVariable::Varbit { id: 10 }),
        source: sources(),
    };
    content
        .initial_state
        .runtime
        .counters
        .insert(flour.id.clone(), flour.initial);
    let hopper = CounterDefinition {
        id: id("counter.test.hopper"),
        scope: CounterScope::World,
        value_type: CounterType::Boolean,
        initial: CounterValue::Boolean(false),
        source_variable: None,
        source: sources(),
    };
    content.mechanics.world_members = Some(false);
    content.mechanics.counters = [flour, hopper]
        .into_iter()
        .map(|value| (value.id.clone(), value))
        .collect();
    let bank_grant = GrantDefinition {
        id: id("grant.test.bank"),
        target: ContainerKind::Bank,
        capacity: CapacityPolicy::Atomic,
        lines: vec![GrantLine {
            item: id("item.test.coins"),
            quantity: Quantity::new(25).unwrap(),
            mode: GrantMode::Add,
            ownership: OwnershipScope::Bank,
        }],
        entitlement: Some(id("entitlement.test.bank")),
        source: sources(),
    };
    let supply = GrantDefinition {
        id: id("grant.test.supply"),
        target: ContainerKind::Inventory,
        capacity: CapacityPolicy::OrderedPartial,
        lines: ["pickaxe", "shield"]
            .into_iter()
            .map(|name| GrantLine {
                item: id(&format!("item.test.{name}")),
                quantity: Quantity::new(1).unwrap(),
                mode: GrantMode::MissingOnly,
                ownership: OwnershipScope::InventoryAndEquipment,
            })
            .collect(),
        entitlement: Some(id("entitlement.test.supply")),
        source: sources(),
    };
    for grant in [bank_grant, supply] {
        let entitlement = EntitlementDefinition {
            id: grant.entitlement.clone().unwrap(),
            purpose: EntitlementPurpose::Grant {
                grant: grant.id.clone(),
            },
            source: sources(),
        };
        content
            .mechanics
            .entitlements
            .insert(entitlement.id.clone(), entitlement);
        content.mechanics.grants.insert(grant.id.clone(), grant);
    }
    content.mechanics.entitlements.insert(
        id("entitlement.test.quest"),
        EntitlementDefinition {
            id: id("entitlement.test.quest"),
            purpose: EntitlementPurpose::AtomicReward,
            source: sources(),
        },
    );
    content.mechanics.entitlements.insert(
        id("entitlement.test.departure"),
        EntitlementDefinition {
            id: id("entitlement.test.departure"),
            purpose: EntitlementPurpose::Reconciliation {
                reconciliation: id("reconciliation.test.departure"),
            },
            source: sources(),
        },
    );
    content.mechanics.reconciliations.insert(
        id("reconciliation.test.departure"),
        ReconciliationDefinition {
            id: id("reconciliation.test.departure"),
            entitlement: id("entitlement.test.departure"),
            policies: missing(
                "The source deposit/drop/withdraw departure matrix is not an observed policy.",
            ),
            source: sources(),
        },
    );
    let ground = GroundItemPolicy {
        id: id("ground_policy.test.owned"),
        public_after: bound(None),
        expires_after: bound(Some(100)),
        owner_can_take: true,
        source: sources(),
    };
    content
        .mechanics
        .ground_policies
        .insert(ground.id.clone(), ground);
    let fire = TemporaryObjectDefinition {
        id: id("temporary_object.test.fire"),
        object: id("object.test.furnace"),
        lifetime: bound(TickDuration::UniformInclusive {
            minimum: 100,
            maximum: 199,
        }),
        placement_guard: Guard::Always,
        interactions: vec![InteractionDefinition {
            name: "Cook".into(),
            reach: 1,
            guard: Guard::Always,
            action: InteractionAction::Production {
                recipes: vec![id("recipe.test.bar")],
            },
        }],
        owner_only_use: false,
        blocks_movement: false,
        blocks_projectiles: false,
        expired_items: vec![stack("item.test.ore", 1)],
        ground_policy: id("ground_policy.test.owned"),
        source: sources(),
    };
    content
        .mechanics
        .temporary_objects
        .insert(fire.id.clone(), fire);
    let closed = content.regions[&id("region.test.field")]
        .cells
        .iter()
        .find(|cell| cell.tile == tile(1003, 1003))
        .copied()
        .unwrap();
    let transform = ObjectTransformDefinition {
        id: id("transform.test.door"),
        scope: CounterScope::World,
        spawn: id("spawn.test.rock"),
        initial: id("object_state.test.closed"),
        states: BTreeMap::from([
            (
                id("object_state.test.closed"),
                ObjectTransformState {
                    object: Some(id("object.test.rock")),
                    tile: closed.tile,
                    door: Some(DoorPosition::Closed),
                    placement: source_placement(),
                    collision: vec![closed],
                },
            ),
            (
                id("object_state.test.open"),
                ObjectTransformState {
                    object: Some(id("object.test.furnace")),
                    tile: tile(1002, 1003),
                    door: Some(DoorPosition::Open),
                    placement: source_placement(),
                    collision: vec![CollisionCell {
                        walkable: true,
                        ..closed
                    }],
                },
            ),
        ]),
        source: sources(),
    };
    content
        .mechanics
        .object_transforms
        .insert(transform.id.clone(), transform);
    content
        .objects
        .get_mut(&id("object.test.furnace"))
        .unwrap()
        .morph = Some(SourceObjectMorph {
        counter: id("counter.test.flour"),
        variants: BTreeMap::from([(0, Some(id("object.test.rock")))]),
        fallback: Some(id("object.test.furnace")),
    });
    let InteractionAction::Gather { rule } = &mut content
        .spawns
        .get_mut(&id("spawn.test.rock"))
        .unwrap()
        .interactions[0]
        .action
    else {
        unreachable!()
    };
    rule.attempt_ticks = None;
    rule.respawn_ticks = None;
    rule.success = ChanceRule::source_skilling(100, 350, levels()).unwrap();
    rule.mechanics = Some(GatherMechanics {
        method: id("action.test.mining"),
        levels: levels(),
        cadence: cadence(8, 8, 8),
        tool_cadences: vec![],
        respawn: bound(TickDuration::UniformInclusive {
            minimum: 60,
            maximum: 100,
        }),
        alternatives: vec![],
        relocation: None,
    });
    let recipe = content.recipes.get_mut(&id("recipe.test.bar")).unwrap();
    recipe.ticks = None;
    recipe.success = ChanceRule::source_skilling(128, 512, levels()).unwrap();
    recipe.mechanics = Some(RecipeMechanics {
        method: id("action.test.smelting"),
        guard: Guard::Always,
        chance_skill: Some(id("skill.test.smithing")),
        cadence: cadence(6, 4, 5),
        tool_ownership: OwnershipScope::InventoryAndEquipment,
        failed_xp: vec![],
        success_effects: vec![],
        failure_effects: vec![],
        lifecycle: RecipeLifecycle::InventoryConversion,
    });
    content
        .spawns
        .get_mut(&id("spawn.test.guide"))
        .unwrap()
        .interactions
        .push(InteractionDefinition {
            name: "Present bank".into(),
            reach: 1,
            guard: Guard::Always,
            action: InteractionAction::OpenBank {
                interface: id("interface.test.bank"),
                before_open: vec![Effect::Grant {
                    grant: id("grant.test.bank"),
                }],
            },
        });
    content.mechanics.instances.insert(
        id("instance_template.test.office"),
        InstanceTemplateDefinition {
            id: id("instance_template.test.office"),
            chunk_size: 2,
            chunks: vec![InstanceChunkMapping {
                source_region: id("region.test.field"),
                source_origin: tile(1000, 1000),
                destination_region: id("region.test.field"),
                destination_origin: tile(1004, 1004),
                quarter_turns: 0,
            }],
            private_to_character: true,
            source: sources(),
        },
    );
    for name in ["new", "returning", "experienced"] {
        let experience = ExperienceDefinition {
            id: id(&format!("experience.test.{name}")),
            name: format!("Synthetic {name}"),
            selection_guard: Guard::Always,
            source: sources(),
        };
        content
            .mechanics
            .experiences
            .insert(experience.id.clone(), experience);
    }
    content.mechanics.appearance = Some(AppearanceDefinition {
        choices: BTreeMap::from([("colour".into(), BTreeSet::from([1, 2]))]),
        confirmation_guard: Guard::Always,
        source: sources(),
    });
    content.mechanics.travels.insert(
        id("travel.test.home"),
        TravelDefinition {
            id: id("travel.test.home"),
            guard: Guard::Always,
            destination: bound(TravelDestination::Experience {
                branches: content
                    .mechanics
                    .experiences
                    .keys()
                    .cloned()
                    .map(|id| (id, location(1000, 1000, None)))
                    .collect(),
            }),
            channel_ticks: bound(24),
            cooldown_ticks: bound(3000),
            cooldown_start: bound(CooldownStart::Completed),
            interruptions: BTreeSet::from([InterruptionCause::Movement, InterruptionCause::Combat]),
            completion_effects: vec![],
            source: sources(),
        },
    );
    let projectile = ProjectileDefinition {
        id: id("projectile.test.wind"),
        timing: missing("Launch/impact capture is explicitly unbound."),
        asset: None,
        source: sources(),
    };
    content
        .mechanics
        .projectiles
        .insert(projectile.id.clone(), projectile);
    let effective = EffectiveLevelFormula {
        skill: id("skill.test.mining"),
        basis: SkillLevelBasis::Current,
        style_bonus: 3,
        constant_bonus: 8,
        prayer_before_style: true,
    };
    for (name, method, attack_type) in [
        ("melee", AttackMethod::Melee, AttackType::Stab),
        ("ranged", AttackMethod::Ranged, AttackType::Ranged),
        ("magic", AttackMethod::Magic, AttackType::Magic),
    ] {
        let style = CombatStyleDefinition {
            id: id(&format!("style.test.{name}")),
            method,
            attack_type,
            attack: effective.clone(),
            defence: effective.clone(),
            accuracy: bound(AccuracyFormula::InclusiveOpposedRolls),
            negative_rolls: bound(NegativeRollPolicy::ClampToZero),
            maximum_hit: bound(MaximumHitFormula::Strength {
                level: effective.clone(),
                equipment_offset: 64,
                additive: 320,
                divisor: 640,
            }),
            damage: bound(DamagePolicy {
                successful_minimum: 1,
                cap_to_remaining_hitpoints: true,
            }),
            cycle_ticks: bound(4),
            reach: if method == AttackMethod::Melee { 1 } else { 7 },
            damage_xp: vec![DamageXp {
                skill: id("skill.test.hitpoints"),
                tenths_per_damage: Ratio {
                    numerator: 40,
                    denominator: 3,
                },
                rounding: IntegerRounding::Floor,
            }],
            projectile: (method != AttackMethod::Melee).then(|| id("projectile.test.wind")),
            source: sources(),
        };
        content
            .mechanics
            .combat_styles
            .insert(style.id.clone(), style);
    }
    let equipment = content
        .items
        .get_mut(&id("item.test.pickaxe"))
        .unwrap()
        .equipment
        .as_mut()
        .unwrap();
    equipment.attack_speed_ticks = None;
    equipment.attack_styles.clear();
    equipment.weapon = Some(WeaponDefinition {
        styles: vec![id("style.test.ranged")],
        ammunition: Some(AmmunitionRequirement {
            slot: id("slot.test.ammo"),
            compatible_items: vec![id("item.test.ammo")],
            per_attack: Quantity::new(1).unwrap(),
            break_chance: bound(Ratio {
                numerator: 1,
                denominator: 5,
            }),
            ground_policy: id("ground_policy.test.owned"),
        }),
    });
    content.mechanics.spells.insert(
        id("spell.test.wind"),
        SpellDefinition {
            id: id("spell.test.wind"),
            interface: id("interface.test.inventory"),
            requirements: vec![SkillRequirement {
                skill: id("skill.test.mining"),
                level: 1,
                basis: SkillLevelBasis::Base,
            }],
            guard: Guard::Always,
            runes: vec![stack("item.test.coins", 1)],
            launch_xp: vec![],
            action: SpellAction::Combat {
                style: id("style.test.magic"),
                projectile: id("projectile.test.wind"),
            },
            source: sources(),
        },
    );
    content.mechanics.spells.insert(
        id("spell.test.home"),
        SpellDefinition {
            id: id("spell.test.home"),
            interface: id("interface.test.inventory"),
            requirements: vec![],
            guard: Guard::Always,
            runes: vec![],
            launch_xp: vec![],
            action: SpellAction::Teleport {
                travel: id("travel.test.home"),
            },
            source: sources(),
        },
    );
    content.mechanics.prayers.insert(
        id("prayer.test.skin"),
        PrayerDefinition {
            id: id("prayer.test.skin"),
            interface: id("interface.test.inventory"),
            requirements: vec![],
            modifiers: vec![SkillModifier {
                skill: id("skill.test.mining"),
                multiplier: Ratio {
                    numerator: 105,
                    denominator: 100,
                },
                rounding: IntegerRounding::Floor,
            }],
            drain: bound(PrayerDrain {
                points_per_tick: Ratio {
                    numerator: 1,
                    denominator: 60,
                },
                bonus_offset: 30,
                bonus_divisor: 30,
            }),
            exclusive_with: vec![],
            source: sources(),
        },
    );
    content.mechanics.vitals = Some(VitalPolicy {
        hitpoints_skill: id("skill.test.hitpoints"),
        prayer_skill: id("skill.test.mining"),
        regeneration: vec![RegenerationPolicy {
            vital: Vital::Hitpoints,
            interval_ticks: bound(100),
            amount: 1,
            pauses: BTreeSet::from([ClockPause::Offline]),
            idle_after_milliseconds: None,
        }],
        level_up: bound(LevelUpVitalPolicy::IncreaseByBaseDifference),
        food_delay_ticks: bound(3),
        food_attack_delay_ticks: bound(3),
        source: sources(),
    });
    content.mechanics.run = Some(RunPolicy {
        agility: id("skill.test.mining"),
        levels: levels(),
        activation_minimum: 100,
        disable_on_exhaustion: true,
        drain: bound(RunDrainFormula {
            base: 60,
            weight_scale: 67,
            weight_minimum_grams: 0,
            weight_maximum_grams: 64_000,
            agility_scale: 300,
            floor_weight_term_before_agility: true,
            rounding: IntegerRounding::Floor,
        }),
        regeneration: bound(RunRegenerationFormula {
            skill_divisor: 10,
            additive_units: 15,
            pauses: BTreeSet::from([ClockPause::Offline, ClockPause::Running]),
        }),
        source: sources(),
    });
    let combat = content
        .npcs
        .get_mut(&id("npc.test.monster"))
        .unwrap()
        .combat
        .as_mut()
        .unwrap();
    combat.drops.clear();
    combat.respawn_ticks = None;
    combat.mechanics = Some(NpcCombatMechanics {
        attack_type: AttackType::Crush,
        attack_stat: NpcCombatStat::Attack,
        defence_stats: [
            AttackType::Stab,
            AttackType::Slash,
            AttackType::Crush,
            AttackType::Ranged,
            AttackType::Magic,
        ]
        .into_iter()
        .map(|attack| {
            (
                attack,
                if attack == AttackType::Magic {
                    NpcCombatStat::Magic
                } else {
                    NpcCombatStat::Defence
                },
            )
        })
        .collect(),
        effective_level_bonus: bound(9),
        retaliation: true,
        reach: 1,
        accuracy: bound(AccuracyFormula::InclusiveOpposedRolls),
        negative_rolls: bound(NegativeRollPolicy::ClampToZero),
        damage: bound(DamagePolicy {
            successful_minimum: 0,
            cap_to_remaining_hitpoints: true,
        }),
        respawn: missing("NPC respawn is not inferred from another variant."),
        credit: bound(KillCreditPolicy::MostDamageThenFirstContributor),
        loot: vec![
            LootPool::Guaranteed {
                items: vec![LootEntry {
                    item: id("item.test.ore"),
                    minimum: Quantity::new(1).unwrap(),
                    maximum: Quantity::new(1).unwrap(),
                }],
            },
            LootPool::Exclusive {
                total_weight: 128,
                entries: vec![
                    WeightedLoot {
                        weight: 64,
                        items: vec![LootEntry {
                            item: id("item.test.coins"),
                            minimum: Quantity::new(1).unwrap(),
                            maximum: Quantity::new(5).unwrap(),
                        }],
                    },
                    WeightedLoot {
                        weight: 64,
                        items: vec![],
                    },
                ],
            },
            LootPool::Conditional {
                guard: Guard::MembersWorld,
                pools: vec![LootPool::Independent {
                    chance: Ratio {
                        numerator: 1,
                        denominator: 20,
                    },
                    items: vec![LootEntry {
                        item: id("item.test.food"),
                        minimum: Quantity::new(1).unwrap(),
                        maximum: Quantity::new(1).unwrap(),
                    }],
                }],
            },
            LootPool::Unresolved {
                reason: "The Common energy-potion supplement has no bound probability.".into(),
                source: sources(),
            },
        ],
    });
    let price = |base, minimum, maximum, floor| StockPriceFormula {
        base_per_mille: base,
        change_per_stock: 30,
        minimum_per_mille: minimum,
        maximum_per_mille: maximum,
        minimum_price: floor,
        rounding: IntegerRounding::Floor,
    };
    let shop = content.shops.get_mut(&id("shop.test.general")).unwrap();
    shop.stock[0].buy_price = 1;
    shop.stock[0].sell_price = 0;
    shop.stock[0].mechanics = Some(ShopLineMechanics {
        pricing: ShopPricing::StockSensitive {
            buy: price(1300, 300, 6300, 1),
            sell: price(400, 100, 1400, 0),
            overstock: bound(OverstockPricing::LinearToClamp),
        },
        restock: StockRestockRule {
            interval_ticks: 5,
            amount: Quantity::new(1).unwrap(),
            phase: bound(RestockPhase::WorldEpoch),
        },
    });
    let provider = DeathValueProvider {
        id: id("value_provider.test.death"),
        method: DeathValueMethod::FixedSourceTable,
        revision: "synthetic-values-v1".into(),
        values: bound(content.items.keys().cloned().map(|id| (id, 500)).collect()),
        source: sources(),
    };
    content
        .mechanics
        .value_providers
        .insert(provider.id.clone(), provider);
    content.mechanics.death = Some(DeathPolicy {
        domain: DeathDomain::NormalUnsafeNonPvp,
        value_provider: id("value_provider.test.death"),
        retained_unskulled: 3,
        protect_item_extra: 1,
        ties: bound(RetentionTiePolicy::InventoryThenEquipment),
        respawn: bound(location(1000, 1000, None)),
        first_office: bound(location(
            1004,
            1004,
            Some(id("instance_template.test.office")),
        )),
        restoration: bound(DeathVitalRestoration {
            on_arrival: BTreeMap::from([(Vital::RunEnergy, VitalRestoration::ToBaseMaximum)]),
            on_first_office_exit: BTreeMap::new(),
        }),
        required_topics: BTreeSet::from([
            DeathTopic::Fees,
            DeathTopic::Timer,
            DeathTopic::KeptItems,
        ]),
        grave_active_ticks: 1500,
        grave_pauses: BTreeSet::from([
            ClockPause::Offline,
            ClockPause::Idle,
            ClockPause::GraveInterface,
            ClockPause::FirstDeathOffice,
        ]),
        idle_after_milliseconds: 10000,
        reclaim_range: 7,
        require_line_of_sight: true,
        grave_capacity: 120,
        office_capacity: 120,
        office_overflow: missing("Exact overflow eviction order is unbound."),
        grave_fee: bound(RecoveryFee::Bands {
            bands: vec![
                FeeBand {
                    minimum_value: 0,
                    fee: 0,
                },
                FeeBand {
                    minimum_value: 100000,
                    fee: 1000,
                },
                FeeBand {
                    minimum_value: 1000000,
                    fee: 10000,
                },
                FeeBand {
                    minimum_value: 10000000,
                    fee: 100000,
                },
            ],
            maximum_total: 500000,
        }),
        office_fee: bound(RecoveryFee::Percentage {
            free_below: 100000,
            rate: Ratio {
                numerator: 5,
                denominator: 100,
            },
            rounding: IntegerRounding::Floor,
        }),
        payment_order: vec![FeeSource::Coffer, FeeSource::Bank],
        currency: id("item.test.coins"),
        repeat: bound(RepeatDeathPolicy {
            keep_old_grave_location: true,
            refresh_timer_if_contents_change: true,
            old_unstackable_per_item_limit: 28,
            old_items_to_office: vec![id("item.test.ore")],
            supply_items: vec![id("item.test.food")],
            supply_ground_policy: id("ground_policy.test.owned"),
            source: sources(),
        }),
        source: sources(),
    });
    content
}

fn compile(content: GameContent) -> clubscape_content::CompiledContent {
    compile_content(content, ValidationMode::TestFixture).unwrap()
}

fn reject(mutate: impl FnOnce(&mut GameContent), diagnostic: &str) {
    let mut content = mechanic_fixture();
    mutate(&mut content);
    let error = compile_content(content, ValidationMode::TestFixture).unwrap_err();
    assert!(
        error.message.contains(diagnostic),
        "{error}; expected {diagnostic}"
    );
}

fn character(content: &GameContent) -> CharacterState {
    let initial = &content.initial_state;
    CharacterState {
        schema_version: GAME_SCHEMA_VERSION,
        actor_id: id("actor.test.player"),
        display_name: "Synthetic player".into(),
        appearance: BTreeMap::new(),
        region: initial.region.clone(),
        tile: initial.tile,
        inventory: initial.inventory.clone(),
        equipment: initial.equipment.clone(),
        bank: initial.bank.clone(),
        skills: initial.skills.clone(),
        hitpoints: initial.hitpoints,
        prayer_points: initial.prayer_points,
        run_energy: initial.run_energy,
        quest_points: initial.quest_points,
        tutorial_stage: initial.tutorial_stage.clone(),
        quests: initial.quests.clone(),
        flags: initial.flags.clone(),
        interfaces: initial.interfaces.clone(),
        activity: Activity::Idle,
        dialogue: None,
        last_action_tick: 7,
        last_command_sequence: 23,
        runtime: CharacterRuntime::from_initial(content),
    }
}

#[test]
fn all_mechanics_round_trip_strict_json_and_versioned_compiler_without_acceptance_claims() {
    let content = mechanic_fixture();
    assert_eq!(content.equipment_slots.len(), 11);
    let json = serde_json::to_vec(&content).unwrap();
    assert_eq!(read_content_json(&json).unwrap(), content);
    let compiled = compile(content);
    let bytes = encode_compiled(&compiled).unwrap();
    let loaded = load_compiled(&bytes, ValidationMode::TestFixture).unwrap();
    assert_eq!(loaded.definition(), compiled.definition());
    assert!(!loaded.report().source_verification_performed);
    assert!(!loaded.report().presentation_verification_performed);
    assert_eq!(loaded.report().unresolved_bindings.len(), 5);
    assert_eq!(loaded.definition().schema_version, CONTENT_SCHEMA_VERSION);
    character(loaded.definition())
        .validate_runtime(loaded.definition())
        .unwrap();
}

#[test]
fn new_source_fields_are_required_and_unknown_nested_fields_are_rejected() {
    let value = serde_json::to_value(mechanic_fixture()).unwrap();
    for (path, field) in [
        ("", "mechanics"),
        ("/initial_state", "runtime"),
        ("/initial_state/runtime/settings", "run_enabled"),
        ("/items/item.test.ore", "weight"),
    ] {
        let mut changed = value.clone();
        changed
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(
            read_content_json(&serde_json::to_vec(&changed).unwrap()).is_err(),
            "{path}/{field}"
        );
    }
    let mut changed = value;
    changed["mechanics"]["run"]["drain"]["value"]["quiet_default"] = serde_json::json!(true);
    assert!(read_content_json(&serde_json::to_vec(&changed).unwrap()).is_err());
}

#[test]
fn source_chance_domains_and_new_cadences_do_not_accept_legacy_placeholders() {
    let content = mechanic_fixture();
    let InteractionAction::Gather { rule } =
        &content.spawns[&id("spawn.test.rock")].interactions[0].action
    else {
        unreachable!()
    };
    assert_eq!(rule.success.numerator_at_level_99, 351);
    assert_eq!(rule.success.numerator(1).unwrap(), 101);
    compile(content);
    reject(
        |content| {
            let InteractionAction::Gather { rule } = &mut content
                .spawns
                .get_mut(&id("spawn.test.rock"))
                .unwrap()
                .interactions[0]
                .action
            else {
                unreachable!()
            };
            rule.attempt_ticks = Some(8);
        },
        "legacy fixed",
    );
    reject(
        |content| {
            content
                .recipes
                .values_mut()
                .next()
                .unwrap()
                .mechanics
                .as_mut()
                .unwrap()
                .chance_skill = None
        },
        "chance skill",
    );
    reject(
        |content| {
            content
                .recipes
                .values_mut()
                .next()
                .unwrap()
                .mechanics
                .as_mut()
                .unwrap()
                .cadence
                .first = bound(0)
        },
        "cadence",
    );
    reject(
        |content| {
            content
                .recipes
                .values_mut()
                .next()
                .unwrap()
                .mechanics
                .as_mut()
                .unwrap()
                .cadence
                .first = SourceBinding::Bound {
                value: 4,
                source: vec![],
            }
        },
        "empty",
    );
}

#[test]
fn counter_initialization_scope_type_bounds_and_graph_dependencies_are_checked() {
    reject(
        |content| content.initial_state.runtime.counters.clear(),
        "every character counter",
    );
    reject(
        |content| {
            content
                .initial_state
                .runtime
                .counters
                .insert(id("counter.test.flour"), CounterValue::Integer(31));
        },
        "scope/value",
    );
    reject(
        |content| {
            content
                .mechanics
                .counters
                .get_mut(&id("counter.test.flour"))
                .unwrap()
                .initial = CounterValue::Boolean(false);
        },
        "type or bounds",
    );
    reject(
        |content| {
            content
                .tutorial
                .get_mut(&id("stage.test.start"))
                .unwrap()
                .transitions[0]
                .guard = Guard::Counter {
                counter: id("counter.test.flour"),
                predicate: CounterPredicate::Equals {
                    value: CounterValue::Integer(1),
                },
            };
        },
        "unreachable",
    );
}

#[test]
fn stationary_water_and_scenery_anchors_preserve_clipping_but_mobile_footprints_do_not_relax() {
    let mut content = mechanic_fixture();
    let mut fishing = content.npcs[&id("npc.test.guide")].clone();
    fishing.id = id("npc.test.resource");
    fishing.source_id = 77;
    fishing.navigation = NpcNavigation::Stationary {
        anchor: StationaryAnchor::NonWalkingResource {
            access_tiles: vec![tile(1002, 1003)],
        },
    };
    let mut spawn = content.spawns[&id("spawn.test.rock")].clone();
    spawn.id = id("spawn.test.resource");
    spawn.kind = SpawnKind::Npc {
        npc: fishing.id.clone(),
    };
    content.npcs.insert(fishing.id.clone(), fishing);
    content.spawns.insert(spawn.id.clone(), spawn);
    let compiled = compile(content.clone());
    assert!(!compiled.collision(tile(1003, 1003)).unwrap().walkable);
    content
        .npcs
        .get_mut(&id("npc.test.resource"))
        .unwrap()
        .navigation = NpcNavigation::Mobile {
        wander_radius: 0,
        step_ticks: bound(1),
        clip: NpcClipPolicy::MovementAndActors,
    };
    assert!(
        compile_content(content.clone(), ValidationMode::TestFixture)
            .unwrap_err()
            .message
            .contains("not walkable")
    );
    content
        .npcs
        .get_mut(&id("npc.test.resource"))
        .unwrap()
        .navigation = NpcNavigation::Stationary {
        anchor: StationaryAnchor::SceneryBound {
            object: id("object.test.rock"),
            access_tiles: vec![tile(1002, 1003)],
        },
    };
    compile(content.clone());
    content
        .npcs
        .get_mut(&id("npc.test.resource"))
        .unwrap()
        .navigation = NpcNavigation::Stationary {
        anchor: StationaryAnchor::SceneryBound {
            object: id("object.test.furnace"),
            access_tiles: vec![tile(1002, 1003)],
        },
    };
    assert!(
        compile_content(content, ValidationMode::TestFixture)
            .unwrap_err()
            .message
            .contains("matching occupied")
    );
}

#[test]
fn grants_and_pre_bank_presentation_have_typed_reciprocal_once_only_ledgers() {
    reject(
        |content| {
            content
                .mechanics
                .grants
                .get_mut(&id("grant.test.bank"))
                .unwrap()
                .target = ContainerKind::Inventory
        },
        "bank-targeted",
    );
    reject(
        |content| {
            content
                .mechanics
                .entitlements
                .get_mut(&id("entitlement.test.bank"))
                .unwrap()
                .purpose = EntitlementPurpose::AtomicReward
        },
        "purpose/owner",
    );
    let content = mechanic_fixture();
    let mut actor = character(&content);
    actor.runtime.entitlements.insert(
        id("entitlement.test.supply"),
        EntitlementState::Grant {
            delivered: BTreeMap::new(),
            satisfied: BTreeSet::from([id("item.test.pickaxe")]),
            complete: false,
        },
    );
    actor.validate_runtime(&content).unwrap();
    let before = actor.runtime.clone();
    actor.runtime.entitlements.insert(
        id("entitlement.test.supply"),
        EntitlementState::Grant {
            delivered: BTreeMap::from([(id("item.test.shield"), 1)]),
            satisfied: BTreeSet::from([id("item.test.pickaxe"), id("item.test.shield")]),
            complete: true,
        },
    );
    actor.validate_runtime(&content).unwrap();
    before.validate_ledger_successor(&actor.runtime).unwrap();
    assert!(actor.runtime.validate_ledger_successor(&before).is_err());
    actor.runtime.entitlements.clear();
    assert!(before.validate_ledger_successor(&actor.runtime).is_err());
}

#[test]
fn event_facts_distinguish_real_success_failure_cast_resolution_and_requests() {
    let success = EventCondition::Production {
        recipe: id("recipe.test.bar"),
        method: id("action.test.smelting"),
        facility: Some(id("spawn.test.furnace")),
        outcome: ProductionOutcome::Success,
        output: Some(id("item.test.bar")),
    };
    let failed = GameEvent::ProductionResolved {
        recipe: id("recipe.test.bar"),
        method: id("action.test.smelting"),
        facility: Some(WorldTarget::Spawn {
            spawn: id("spawn.test.furnace"),
        }),
        outcome: ProductionOutcome::Failure,
        outputs: vec![],
    };
    assert!(!success.matches(&failed));
    assert!(!success.matches(&GameEvent::Produced {
        recipe: id("recipe.test.bar"),
        outputs: vec![stack("item.test.bar", 1)]
    }));
    let cast = EventCondition::Spell {
        spell: id("spell.test.wind"),
        target: Some(id("spawn.test.monster")),
        outcomes: BTreeSet::from([SpellOutcome::Hit, SpellOutcome::Splash]),
    };
    assert!(cast.matches(&GameEvent::SpellResolved {
        spell: id("spell.test.wind"),
        target: id("spawn.test.monster"),
        outcome: SpellOutcome::Splash,
        damage: 0,
        tile: tile(1004, 1002)
    }));
    assert!(!cast.matches(&GameEvent::Hit {
        target: id("spawn.test.monster"),
        damage: 1,
        style: "magic".into()
    }));
    reject(
        |content| {
            content
                .tutorial
                .get_mut(&id("stage.test.start"))
                .unwrap()
                .transitions[0]
                .guard = Guard::Event { condition: success };
        },
        "different authoritative event",
    );
}

#[test]
fn loot_pool_weights_rune_ammo_prayer_and_binding_references_are_checked() {
    reject(
        |content| {
            content
                .npcs
                .get_mut(&id("npc.test.monster"))
                .unwrap()
                .combat
                .as_mut()
                .unwrap()
                .mechanics
                .as_mut()
                .unwrap()
                .loot[1] = LootPool::Exclusive {
                total_weight: 128,
                entries: vec![WeightedLoot {
                    weight: 127,
                    items: vec![],
                }],
            };
        },
        "sum",
    );
    reject(
        |content| {
            content
                .items
                .get_mut(&id("item.test.pickaxe"))
                .unwrap()
                .equipment
                .as_mut()
                .unwrap()
                .weapon
                .as_mut()
                .unwrap()
                .ammunition
                .as_mut()
                .unwrap()
                .compatible_items = vec![id("item.test.ore")];
        },
        "ammunition",
    );
    reject(
        |content| {
            content
                .mechanics
                .spells
                .get_mut(&id("spell.test.wind"))
                .unwrap()
                .runes = vec![stack("item.test.ore", 1)]
        },
        "rune costs",
    );
    reject(
        |content| {
            content
                .mechanics
                .prayers
                .get_mut(&id("prayer.test.skin"))
                .unwrap()
                .drain = bound(PrayerDrain {
                points_per_tick: Ratio {
                    numerator: 1,
                    denominator: 0,
                },
                bonus_offset: 30,
                bonus_divisor: 30,
            });
        },
        "denominator",
    );
    reject(
        |content| {
            content
                .mechanics
                .combat_styles
                .get_mut(&id("style.test.magic"))
                .unwrap()
                .projectile = Some(id("projectile.test.missing"))
        },
        "projectile",
    );
}

#[test]
fn death_value_snapshot_instance_topics_and_overflow_are_not_invented_defaults() {
    reject(
        |content| {
            content
                .mechanics
                .death
                .as_mut()
                .unwrap()
                .required_topics
                .remove(&DeathTopic::Fees);
        },
        "all topics",
    );
    reject(
        |content| {
            content.mechanics.death.as_mut().unwrap().first_office =
                bound(location(1004, 1004, None))
        },
        "instance mapping",
    );
    reject(
        |content| {
            content.mechanics.death.as_mut().unwrap().value_provider =
                id("value_provider.test.missing")
        },
        "valuation provider",
    );
    reject(
        |content| {
            content
                .mechanics
                .instances
                .get_mut(&id("instance_template.test.office"))
                .unwrap()
                .chunks[0]
                .quarter_turns = 4
        },
        "rotation",
    );
    let content = mechanic_fixture();
    assert_eq!(
        content
            .mechanics
            .death
            .as_ref()
            .unwrap()
            .office_overflow
            .require()
            .unwrap_err()
            .code,
        GameErrorCode::Unavailable
    );
    assert_eq!(
        content
            .mechanics
            .reconciliations
            .values()
            .next()
            .unwrap()
            .policies
            .require()
            .unwrap_err()
            .code,
        GameErrorCode::Unavailable
    );
}

#[test]
fn legacy_metadata_migration_preserves_deadlines_pending_operations_and_acknowledged_state() {
    let content = fixture();
    let mut actor = character(&content);
    actor.flags.insert("__world_engine.command_seen".into(), 1);
    actor.flags.insert("__world_engine.food_ready".into(), 19);
    actor.flags.insert("__world_engine.attack_ready".into(), 21);
    actor
        .flags
        .insert("__world_engine.gather_interaction".into(), 1);
    actor.activity = Activity::Gathering {
        target: id("spawn.test.rock"),
        next_tick: 44,
    };
    let before = actor.clone();
    actor.migrate_engine_metadata(&content).unwrap();
    assert_eq!(actor.inventory, before.inventory);
    assert_eq!(actor.skills, before.skills);
    assert_eq!(actor.quests, before.quests);
    assert_eq!(actor.activity, before.activity);
    assert_eq!(actor.last_command_sequence, 23);
    assert_eq!(actor.runtime.food_ready, 19);
    assert_eq!(actor.runtime.combat.attack_ready, 21);
    assert!(!actor.flags.contains_key("__world_engine.command_seen"));
    assert!(
        matches!(actor.runtime.engine, EngineMetadata::Typed { ref schedule } if schedule.command_seen && schedule.gather_interaction == Some(1))
    );
    actor.validate_runtime(&content).unwrap();
    let once = actor.clone();
    actor.migrate_engine_metadata(&content).unwrap();
    assert_eq!(actor, once);
    let mut bad = before;
    bad.flags.insert("__world_engine.unknown_pending".into(), 1);
    let unchanged = bad.clone();
    assert!(bad.migrate_engine_metadata(&content).is_err());
    assert_eq!(bad, unchanged);
}

#[test]
fn legacy_json_defaults_are_explicit_legacy_state_not_reset_source_settings() {
    let content = fixture();
    let expected = character(&content);
    let mut json = serde_json::to_value(&expected).unwrap();
    json.as_object_mut().unwrap().remove("runtime");
    let restored: CharacterState = serde_json::from_value(json).unwrap();
    assert_eq!(restored.inventory, expected.inventory);
    assert_eq!(restored.skills, expected.skills);
    assert_eq!(
        restored.last_command_sequence,
        expected.last_command_sequence
    );
    assert!(matches!(restored.runtime.engine, EngineMetadata::Legacy));
    assert!(matches!(restored.runtime.life, LifeState::Legacy));
    assert_eq!(restored.runtime.settings.run_enabled, None);
}

#[test]
fn recursive_new_effects_and_loot_cannot_bypass_preflight_or_overflow_drop_stack() {
    let mut content = mechanic_fixture();
    let mut pool = LootPool::Guaranteed {
        items: vec![LootEntry {
            item: id("item.test.ore"),
            minimum: Quantity::new(1).unwrap(),
            maximum: Quantity::new(1).unwrap(),
        }],
    };
    for _ in 0..20_000 {
        pool = LootPool::Conditional {
            guard: Guard::Always,
            pools: vec![pool],
        };
    }
    content
        .npcs
        .get_mut(&id("npc.test.monster"))
        .unwrap()
        .combat
        .as_mut()
        .unwrap()
        .mechanics
        .as_mut()
        .unwrap()
        .loot = vec![pool];
    assert!(
        compile_content(content, ValidationMode::TestFixture)
            .unwrap_err()
            .message
            .contains("nesting")
    );
}

#[test]
fn charged_buckets_preserve_instance_identity_and_mode_two_is_not_coerced() {
    let mut content = mechanic_fixture();
    let charges = ChargeDefinition {
        kind: id("charge.test.milk"),
        maximum: 100,
        empty_variant: id("item.test.empty_bucket"),
        charged_variant: id("item.test.milk_bucket"),
        trade_with_charges: false,
        source: sources(),
    };
    for (name, number) in [("empty_bucket", 90), ("milk_bucket", 91)] {
        let mut item = content.items[&id("item.test.food")].clone();
        item.id = id(&format!("item.test.{name}"));
        item.source_id = Some(number);
        item.healing = None;
        item.charges = Some(charges.clone());
        let SourceBinding::Bound { value, .. } = &mut content
            .mechanics
            .value_providers
            .get_mut(&id("value_provider.test.death"))
            .unwrap()
            .values
        else {
            unreachable!()
        };
        value.insert(item.id.clone(), 500);
        content.items.insert(item.id.clone(), item);
    }
    compile(content.clone());
    let mut actor = character(&content);
    actor.inventory.slots[5] = Some(ItemStack {
        item: id("item.test.milk_bucket"),
        quantity: Quantity::new(1).unwrap(),
        instance: Some(Box::new(ItemInstance {
            id: id("item_instance.test.bucket"),
            charges: Some(ItemCharges {
                kind: id("charge.test.milk"),
                remaining: 5,
            }),
            origin: None,
        })),
    });
    actor.validate_runtime(&content).unwrap();
    let json = serde_json::to_vec(&actor).unwrap();
    assert_eq!(
        serde_json::from_slice::<CharacterState>(&json).unwrap(),
        actor
    );
    actor.inventory.slots[6] = actor.inventory.slots[5].clone();
    assert!(
        actor
            .validate_runtime(&content)
            .unwrap_err()
            .message
            .contains("multiple")
    );
    actor.inventory.slots[6] = None;
    actor.inventory.slots[5]
        .as_mut()
        .unwrap()
        .instance
        .as_mut()
        .unwrap()
        .charges
        .as_mut()
        .unwrap()
        .remaining = 0;
    assert!(actor.validate_runtime(&content).is_err());
    actor.inventory.slots[5].as_mut().unwrap().item = id("item.test.empty_bucket");
    actor.validate_runtime(&content).unwrap();
    content
        .items
        .get_mut(&id("item.test.food"))
        .unwrap()
        .stackable = Stackability::Conditional {
        source_mode: 2,
        rule: missing("Selected mode-2 behavior is not yet source-bound."),
    };
    let compiled = compile(content);
    assert!(matches!(
        compiled.item(&id("item.test.food")).unwrap().stackable,
        Stackability::Conditional { source_mode: 2, .. }
    ));
    assert!(
        compiled
            .report()
            .unresolved_bindings
            .iter()
            .any(|path| path.contains("stackable.rule"))
    );
}

#[test]
fn complete_event_contract_and_scoped_permissions_validate_registry_targets() {
    let content = mechanic_fixture();
    let cases = [
        ("appearance_confirmed", None),
        ("experience_selected", Some("experience.test.new")),
        ("interface_closed", Some("interface.test.inventory")),
        ("interface_presented", Some("interface.test.bank")),
        ("setting_changed", None),
        ("inspected", Some("spawn.test.guide")),
        ("production_resolved", Some("recipe.test.bar")),
        ("combat_resolved", Some("spawn.test.monster")),
        ("npc_killed", Some("spawn.test.monster")),
        ("spell_resolved", Some("spell.test.wind")),
        ("teleport", Some("travel.test.home")),
        ("item_transferred", None),
        ("food_eaten", Some("item.test.food")),
        ("prayer_changed", Some("prayer.test.skin")),
        (
            "temporary_object_created",
            Some("temporary_object.test.fire"),
        ),
        ("object_transformed", Some("transform.test.door")),
        ("counter_changed", Some("counter.test.flour")),
        ("death_occurred", None),
        ("death_topic_completed", None),
        ("recovery_completed", None),
        ("grave_expired", None),
    ];
    for (event, target) in cases {
        let mut content = content.clone();
        let transition = &mut content
            .tutorial
            .get_mut(&id("stage.test.start"))
            .unwrap()
            .transitions[0];
        transition.event = event.into();
        transition.target = target.map(str::to_owned);
        compile(content);
    }
    let mut content = content;
    content
        .tutorial
        .get_mut(&id("stage.test.start"))
        .unwrap()
        .allowed_actions = [
        "*",
        "gather:spawn.test.rock",
        "interact:spawn.test.guide:Present bank",
        "produce:recipe.test.bar",
        "cast:spell.test.wind",
        "prayer:prayer.test.skin",
        "set_setting",
        "select_experience",
        "reclaim",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    compile(content);
}

#[test]
fn once_wrappers_participate_in_graph_rewards_and_cannot_erase_idempotency() {
    let mut content = mechanic_fixture();
    let quest = content.quests.get_mut(&id("quest.test.errand")).unwrap();
    let effects = std::mem::take(&mut quest.transitions[0].effects);
    quest.transitions[0].effects = vec![Effect::Once {
        entitlement: id("entitlement.test.quest"),
        effects,
    }];
    compile(content.clone());
    content
        .quests
        .get_mut(&id("quest.test.errand"))
        .unwrap()
        .transitions[0]
        .effects
        .push(Effect::GiveItems {
            items: vec![stack("item.test.coins", 1)],
        });
    assert!(compile_content(content, ValidationMode::TestFixture).is_err());
}

#[test]
fn temporary_facilities_and_ground_item_intents_are_not_fake_static_spawns() {
    let content = mechanic_fixture();
    let target = WorldTarget::TemporaryObject {
        object: id("dynamic_object.test.fire"),
    };
    let intent = GameIntent::ProduceAt {
        recipe: id("recipe.test.bar"),
        target: Some(target.clone()),
        quantity: Quantity::new(1).unwrap(),
    };
    assert_eq!(
        serde_json::from_slice::<GameIntent>(&serde_json::to_vec(&intent).unwrap()).unwrap(),
        intent
    );
    let event = GameEvent::ProductionResolved {
        recipe: id("recipe.test.bar"),
        method: id("action.test.smelting"),
        facility: Some(target),
        outcome: ProductionOutcome::Success,
        outputs: vec![stack("item.test.bar", 1)],
    };
    let mut condition = EventCondition::Production {
        recipe: id("recipe.test.bar"),
        method: id("action.test.smelting"),
        facility: None,
        outcome: ProductionOutcome::Success,
        output: Some(id("item.test.bar")),
    };
    assert!(condition.matches(&event));
    let EventCondition::Production { facility, .. } = &mut condition else {
        unreachable!()
    };
    *facility = Some(id("spawn.test.furnace"));
    assert!(!condition.matches(&event));
    assert_eq!(
        content.mechanics.temporary_objects[&id("temporary_object.test.fire")]
            .interactions
            .len(),
        1
    );
    compile(content);
    reject(
        |content| {
            content
                .mechanics
                .temporary_objects
                .get_mut(&id("temporary_object.test.fire"))
                .unwrap()
                .interactions[0]
                .action = InteractionAction::Production {
                recipes: vec![id("recipe.test.missing")],
            };
        },
        "temporary production facility",
    );
}

#[test]
fn scripted_noncombat_anchor_and_npc_morph_need_source_evidence_and_references() {
    let mut content = mechanic_fixture();
    let npc = content.npcs.get_mut(&id("npc.test.guide")).unwrap();
    npc.navigation = NpcNavigation::Stationary {
        anchor: StationaryAnchor::ScriptedActor {
            access_tiles: vec![tile(1002, 1003)],
            source: sources(),
        },
    };
    npc.morph = Some(SourceNpcMorph {
        counter: id("counter.test.flour"),
        variants: BTreeMap::from([(0, None)]),
        fallback: Some(id("npc.test.guide")),
    });
    content
        .spawns
        .get_mut(&id("spawn.test.guide"))
        .unwrap()
        .tile = tile(1003, 1003);
    let compiled = compile(content.clone());
    assert!(!compiled.collision(tile(1003, 1003)).unwrap().walkable);
    let NpcNavigation::Stationary {
        anchor: StationaryAnchor::ScriptedActor { source, .. },
    } = &mut content
        .npcs
        .get_mut(&id("npc.test.guide"))
        .unwrap()
        .navigation
    else {
        unreachable!()
    };
    source.clear();
    assert!(compile_content(content, ValidationMode::TestFixture).is_err());
}

#[test]
fn world_runtime_checks_scoped_counters_lifetimes_instances_and_grave_ownership() {
    let content = mechanic_fixture();
    let actor = character(&content);
    let owner = actor.actor_id.clone();
    let mut world = WorldState {
        schema_version: GAME_SCHEMA_VERSION,
        content_revision: content.revision.clone(),
        tick: 50,
        revision: 1,
        characters: BTreeMap::from([(owner.clone(), actor)]),
        entities: BTreeMap::new(),
        shops: BTreeMap::new(),
        ground_items: vec![],
        runtime: WorldRuntime::from_initial(&content),
    };
    world.validate_runtime(&content).unwrap();
    world
        .runtime
        .counters
        .insert(id("counter.test.hopper"), CounterValue::Integer(1));
    assert!(world.validate_runtime(&content).is_err());
    world
        .runtime
        .counters
        .insert(id("counter.test.hopper"), CounterValue::Boolean(true));
    let at = RuntimeLocation {
        region: id("region.test.field"),
        tile: tile(1000, 1000),
        instance: None,
    };
    world.runtime.temporary_objects.insert(
        id("dynamic_object.test.fire"),
        DynamicObject {
            definition: id("temporary_object.test.fire"),
            owner: owner.clone(),
            location: at.clone(),
            created_at_tick: 50,
            expires_at_tick: 150,
        },
    );
    world.validate_runtime(&content).unwrap();
    world
        .runtime
        .temporary_objects
        .values_mut()
        .next()
        .unwrap()
        .expires_at_tick = 149;
    assert!(world.validate_runtime(&content).is_err());
    world
        .runtime
        .temporary_objects
        .values_mut()
        .next()
        .unwrap()
        .expires_at_tick = 150;
    let lost = RecoveryItem {
        id: id("recovery_item.test.ore"),
        stack: stack("item.test.ore", 1),
        layout: ItemLayout::Inventory { slot: 3 },
        effective_unit_value: 500,
        fee_paid: 0,
    };
    world.runtime.deaths.insert(
        id("death.test.first"),
        DeathRecord {
            owner: owner.clone(),
            occurred_at_tick: 40,
            origin: at.clone(),
            respawn: at.clone(),
            value_provider: id("value_provider.test.death"),
            value_revision: "synthetic-values-v1".into(),
            retained: vec![],
            grave: Some(GraveState {
                location: at,
                active_ticks_remaining: 1500,
                clock_started: false,
                paused: BTreeSet::from([ClockPause::FirstDeathOffice]),
                items: vec![lost],
            }),
            office: vec![],
            reclaimed: BTreeSet::new(),
        },
    );
    world
        .characters
        .get_mut(&owner)
        .unwrap()
        .runtime
        .active_death = Some(id("death.test.first"));
    world.validate_runtime(&content).unwrap();
    world
        .runtime
        .deaths
        .values_mut()
        .next()
        .unwrap()
        .reclaimed
        .insert(id("recovery_item.test.ore"));
    assert!(world.validate_runtime(&content).is_err());
    world
        .runtime
        .deaths
        .values_mut()
        .next()
        .unwrap()
        .reclaimed
        .clear();
    world.runtime.instances.insert(
        id("instance.test.office"),
        InstanceState {
            template: id("instance_template.test.office"),
            owner: Some(owner.clone()),
            counters: BTreeMap::new(),
            entities: BTreeMap::new(),
            object_states: BTreeMap::new(),
        },
    );
    let actor = world.characters.get_mut(&owner).unwrap();
    actor.runtime.instance = Some(id("instance.test.office"));
    actor.tile = tile(1004, 1004);
    world.validate_runtime(&content).unwrap();
    world.runtime.instances.values_mut().next().unwrap().owner = Some(id("actor.test.other"));
    assert!(world.validate_runtime(&content).is_err());
}
