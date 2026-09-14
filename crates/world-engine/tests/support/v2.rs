//! Explicitly synthetic bindings exercising mechanics-v2. Numeric examples cite the
//! independent journey oracle; geometry/dialogue/definitions are not product content.
#![allow(dead_code)]

use super::*;
use clubscape_world_engine::TickContext;
use std::collections::{BTreeSet, VecDeque};

pub fn bound<T>(value: T) -> SourceBinding<T> {
    SourceBinding::Bound {
        value,
        source: source(),
    }
}
pub fn unresolved<T>(reason: &str) -> SourceBinding<T> {
    SourceBinding::Unresolved {
        reason: reason.into(),
        source: source(),
    }
}
pub fn named_skill(name: &str) -> SkillId {
    SkillId::new(format!("skill.synthetic.{name}")).unwrap()
}
pub fn style(name: &str) -> CombatStyleId {
    CombatStyleId::new(format!("style.synthetic.{name}")).unwrap()
}
pub fn spell() -> SpellId {
    SpellId::new("spell.synthetic.wind").unwrap()
}
pub fn projectile() -> ProjectileId {
    ProjectileId::new("projectile.synthetic.shot").unwrap()
}
pub fn prayer() -> PrayerId {
    PrayerId::new("prayer.synthetic.skin").unwrap()
}
pub fn method(name: &str) -> ActionId {
    ActionId::new(format!("action.synthetic.{name}")).unwrap()
}
pub fn ground_policy() -> GroundPolicyId {
    GroundPolicyId::new("ground_policy.synthetic.owned").unwrap()
}
pub fn npc() -> NpcId {
    NpcId::new("npc.synthetic.target").unwrap()
}
pub fn counter(name: &str) -> CounterId {
    CounterId::new(format!("counter.synthetic.{name}")).unwrap()
}
pub fn grant_id(name: &str) -> GrantId {
    GrantId::new(format!("grant.synthetic.{name}")).unwrap()
}
pub fn entitlement(name: &str) -> EntitlementId {
    EntitlementId::new(format!("entitlement.synthetic.{name}")).unwrap()
}
pub fn travel(name: &str) -> TravelId {
    TravelId::new(format!("travel.synthetic.{name}")).unwrap()
}
pub fn template() -> InstanceTemplateId {
    InstanceTemplateId::new("instance_template.synthetic.office").unwrap()
}
pub fn fire() -> TemporaryObjectId {
    TemporaryObjectId::new("temporary_object.synthetic.fire").unwrap()
}
pub fn cadence(single: u32, first: u32, repeat: u32) -> ActionCadence {
    ActionCadence {
        single: bound(single),
        first: bound(first),
        repeat: bound(repeat),
        menu_delay: bound(0),
    }
}
pub fn levels() -> LevelDomain {
    LevelDomain {
        minimum: 1,
        maximum: 99,
        basis: SkillLevelBasis::Current,
    }
}

pub fn content() -> GameContent {
    let mut content = super::content();
    for name in [
        "attack", "strength", "defence", "ranged", "magic", "prayer", "agility",
    ] {
        let id = named_skill(name);
        let mut definition = content.skills[&skill()].clone();
        definition.id = id.clone();
        content.skills.insert(id.clone(), definition);
        content.initial_state.skills.insert(
            id,
            SkillState {
                xp_tenths: 0,
                current_level: 1,
            },
        );
    }
    content.mechanics.world_members = Some(false);
    content.initial_state.runtime.settings = CharacterSettings {
        run_enabled: Some(false),
        auto_retaliate: Some(true),
        death_auto_equip: Some(true),
        death_supply_piles: Some(false),
        experience: None,
        appearance_confirmed: false,
    };
    for definition in content.items.values_mut() {
        definition.weight = Some(bound(ItemWeight {
            grams: 0,
            inventory: WeightContribution::PerUnit,
            equipment: WeightContribution::PerUnit,
        }));
    }
    content.mechanics.vitals = Some(VitalPolicy {
        hitpoints_skill: hp_skill(),
        prayer_skill: named_skill("prayer"),
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
        source: source(),
    });
    content.mechanics.ground_policies.insert(
        ground_policy(),
        GroundItemPolicy {
            id: ground_policy(),
            public_after: bound(Some(100)),
            expires_after: bound(Some(200)),
            owner_can_take: true,
            source: source(),
        },
    );
    content
}

pub fn with_run(content: &mut GameContent) {
    content.mechanics.run = Some(RunPolicy {
        agility: named_skill("agility"),
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
            pauses: BTreeSet::from([ClockPause::Offline]),
        }),
        source: source(),
    });
}

pub fn typed_recipe(content: &mut GameContent, name: &str, time: ActionCadence) {
    let recipe = content.recipes.get_mut(&recipe(name)).unwrap();
    recipe.ticks = None;
    recipe.mechanics = Some(RecipeMechanics {
        method: method(name),
        guard: Guard::Always,
        chance_skill: Some(skill()),
        cadence: time,
        tool_ownership: OwnershipScope::InventoryAndEquipment,
        failed_xp: vec![],
        success_effects: vec![],
        failure_effects: vec![],
        lifecycle: RecipeLifecycle::InventoryConversion,
    });
}

pub fn typed_gather(content: &mut GameContent) {
    let rule = gather_rule(content);
    rule.attempt_ticks = None;
    rule.respawn_ticks = None;
    rule.mechanics = Some(GatherMechanics {
        method: method("mining"),
        levels: levels(),
        cadence: cadence(8, 8, 8),
        tool_cadences: vec![ToolCadence {
            tool: item("pick"),
            location: OwnershipScope::InventoryAndEquipment,
            cadence: cadence(8, 8, 8),
        }],
        respawn: bound(TickDuration::UniformInclusive {
            minimum: 4,
            maximum: 8,
        }),
        alternatives: vec![],
        relocation: None,
    });
}

pub fn formula(skill: &str, bonus: i16) -> EffectiveLevelFormula {
    EffectiveLevelFormula {
        skill: named_skill(skill),
        basis: SkillLevelBasis::Current,
        style_bonus: bonus,
        constant_bonus: 8,
        prayer_before_style: true,
    }
}

pub fn with_combat(content: &mut GameContent) {
    for (name, method_kind, attack_type, attack_skill, bonus, speed, reach, xp_skill) in [
        (
            "accurate",
            AttackMethod::Melee,
            AttackType::Stab,
            "attack",
            3,
            4,
            1,
            "attack",
        ),
        (
            "ranged",
            AttackMethod::Ranged,
            AttackType::Ranged,
            "ranged",
            3,
            4,
            7,
            "ranged",
        ),
        (
            "rapid",
            AttackMethod::Ranged,
            AttackType::Ranged,
            "ranged",
            0,
            3,
            7,
            "ranged",
        ),
        (
            "longrange",
            AttackMethod::Ranged,
            AttackType::Ranged,
            "ranged",
            0,
            4,
            9,
            "ranged",
        ),
        (
            "magic",
            AttackMethod::Magic,
            AttackType::Magic,
            "magic",
            0,
            5,
            10,
            "magic",
        ),
    ] {
        let maximum_hit = if method_kind == AttackMethod::Magic {
            MaximumHitFormula::LevelTable {
                skill: named_skill("magic"),
                basis: SkillLevelBasis::Current,
                hits: BTreeMap::from([(1, 2), (5, 4), (9, 6), (13, 8)]),
            }
        } else {
            MaximumHitFormula::Strength {
                level: formula(
                    if method_kind == AttackMethod::Ranged {
                        "ranged"
                    } else {
                        "strength"
                    },
                    if method_kind == AttackMethod::Ranged {
                        bonus
                    } else {
                        0
                    },
                ),
                equipment_offset: 64,
                additive: 320,
                divisor: 640,
            }
        };
        let mut xp = vec![
            DamageXp {
                skill: named_skill(xp_skill),
                tenths_per_damage: Ratio {
                    numerator: if method_kind == AttackMethod::Magic || name == "longrange" {
                        20
                    } else {
                        40
                    },
                    denominator: 1,
                },
                rounding: IntegerRounding::Floor,
            },
            DamageXp {
                skill: hp_skill(),
                tenths_per_damage: Ratio {
                    numerator: 40,
                    denominator: 3,
                },
                rounding: IntegerRounding::Floor,
            },
        ];
        if name == "longrange" {
            xp.push(DamageXp {
                skill: named_skill("defence"),
                tenths_per_damage: Ratio {
                    numerator: 20,
                    denominator: 1,
                },
                rounding: IntegerRounding::Floor,
            });
        }
        content.mechanics.combat_styles.insert(
            style(name),
            CombatStyleDefinition {
                id: style(name),
                method: method_kind,
                attack_type,
                attack: formula(attack_skill, bonus),
                defence: formula("defence", 0),
                accuracy: bound(AccuracyFormula::InclusiveOpposedRolls),
                negative_rolls: bound(NegativeRollPolicy::ClampToZero),
                maximum_hit: bound(maximum_hit),
                damage: bound(DamagePolicy {
                    successful_minimum: 1,
                    cap_to_remaining_hitpoints: true,
                }),
                cycle_ticks: bound(speed),
                reach,
                damage_xp: xp,
                projectile: (method_kind != AttackMethod::Melee).then(projectile),
                source: source(),
            },
        );
    }
    let dagger = content
        .items
        .get_mut(&item("dagger"))
        .unwrap()
        .equipment
        .as_mut()
        .unwrap();
    dagger.attack_speed_ticks = None;
    dagger.attack_styles.clear();
    dagger.weapon = Some(WeaponDefinition {
        styles: vec![style("accurate")],
        ammunition: None,
    });
    dagger.bonuses.attack.insert(AttackType::Stab, 4);
    dagger.bonuses.melee_strength = 5;
    let bow = content
        .items
        .get_mut(&item("pick"))
        .unwrap()
        .equipment
        .as_mut()
        .unwrap();
    bow.attack_speed_ticks = None;
    bow.attack_styles.clear();
    bow.weapon = Some(WeaponDefinition {
        styles: vec![style("ranged"), style("rapid"), style("longrange")],
        ammunition: Some(AmmunitionRequirement {
            slot: slot("ammo"),
            compatible_items: vec![item("arrow")],
            per_attack: quantity(1),
            break_chance: bound(Ratio {
                numerator: 1,
                denominator: 5,
            }),
            ground_policy: ground_policy(),
        }),
    });
    content
        .items
        .get_mut(&item("arrow"))
        .unwrap()
        .equipment
        .as_mut()
        .unwrap()
        .bonuses
        .ranged_strength = 7;
    content.mechanics.projectiles.insert(
        projectile(),
        ProjectileDefinition {
            id: projectile(),
            timing: bound(ProjectileTiming {
                launch_delay_ticks: 0,
                base_flight_ticks: 2,
                ticks_per_tile: Ratio {
                    numerator: 0,
                    denominator: 1,
                },
                rounding: IntegerRounding::Floor,
                damage_on_launch: false,
                recheck_target_on_impact: true,
            }),
            asset: None,
            source: source(),
        },
    );
    content.mechanics.spells.insert(
        spell(),
        SpellDefinition {
            id: spell(),
            interface: interface(),
            requirements: vec![SkillRequirement {
                skill: named_skill("magic"),
                level: 1,
                basis: SkillLevelBasis::Current,
            }],
            guard: Guard::Always,
            runes: vec![stack("rune", 1), stack("coins", 1)],
            launch_xp: vec![XpReward {
                skill: named_skill("magic"),
                amount_tenths: 55,
            }],
            action: SpellAction::Combat {
                style: style("magic"),
                projectile: projectile(),
            },
            source: source(),
        },
    );
    content.npcs.insert(
        npc(),
        NpcDefinition {
            id: npc(),
            name: "Synthetic numeric goblin vector".into(),
            source_id: 0,
            size: 1,
            navigation: NpcNavigation::Mobile {
                wander_radius: 0,
                step_ticks: bound(1),
                clip: NpcClipPolicy::MovementAndActors,
            },
            morph: None,
            combat: Some(NpcCombatDefinition {
                hitpoints: 5,
                attack: 1,
                strength: 1,
                defence: 1,
                ranged: 1,
                magic: 1,
                attack_speed_ticks: 4,
                max_hit: 1,
                bonuses: CombatBonuses {
                    attack: BTreeMap::from([(AttackType::Crush, -21)]),
                    defence: [
                        AttackType::Stab,
                        AttackType::Slash,
                        AttackType::Crush,
                        AttackType::Ranged,
                        AttackType::Magic,
                    ]
                    .into_iter()
                    .map(|t| (t, -15))
                    .collect(),
                    melee_strength: -15,
                    ..CombatBonuses::default()
                },
                respawn_ticks: None,
                aggressive: false,
                drops: vec![],
                mechanics: Some(NpcCombatMechanics {
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
                    .map(|t| {
                        (
                            t,
                            if t == AttackType::Magic {
                                NpcCombatStat::Magic
                            } else {
                                NpcCombatStat::Defence
                            },
                        )
                    })
                    .collect(),
                    effective_level_bonus: bound(9),
                    retaliation: false,
                    reach: 1,
                    accuracy: bound(AccuracyFormula::InclusiveOpposedRolls),
                    negative_rolls: bound(NegativeRollPolicy::ClampToZero),
                    damage: bound(DamagePolicy {
                        successful_minimum: 0,
                        cap_to_remaining_hitpoints: true,
                    }),
                    respawn: bound(TickDuration::Fixed { ticks: 35 }),
                    credit: bound(KillCreditPolicy::MostDamageThenFirstContributor),
                    loot: vec![],
                }),
            }),
            asset: None,
            source: source(),
        },
    );
    content.spawns.insert(
        spawn("enemy"),
        SpawnDefinition {
            id: spawn("enemy"),
            region: content.initial_state.region.clone(),
            tile: tile(11, 10, 0),
            facing: 0,
            placement: None,
            kind: SpawnKind::Npc { npc: npc() },
            interactions: vec![InteractionDefinition {
                name: "use".into(),
                reach: 1,
                guard: Guard::Always,
                action: InteractionAction::Attack,
            }],
            source: source(),
        },
    );
}

pub fn with_prayer(content: &mut GameContent) {
    content.mechanics.prayers.insert(
        prayer(),
        PrayerDefinition {
            id: prayer(),
            interface: interface(),
            requirements: vec![SkillRequirement {
                skill: named_skill("prayer"),
                level: 1,
                basis: SkillLevelBasis::Current,
            }],
            modifiers: vec![SkillModifier {
                skill: named_skill("defence"),
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
                bonus_offset: 60,
                bonus_divisor: 60,
            }),
            exclusive_with: vec![],
            source: source(),
        },
    );
}

pub fn with_death(content: &mut GameContent) {
    let provider = ValueProviderId::new("value_provider.synthetic.fixed").unwrap();
    content.mechanics.value_providers.insert(
        provider.clone(),
        DeathValueProvider {
            id: provider.clone(),
            method: DeathValueMethod::FixedSourceTable,
            revision: "synthetic-fixed-values".into(),
            values: bound(
                content
                    .items
                    .keys()
                    .map(|id| (id.clone(), if id == &item("dagger") { 26 } else { 1 }))
                    .collect(),
            ),
            source: source(),
        },
    );
    content.mechanics.instances.insert(
        template(),
        InstanceTemplateDefinition {
            id: template(),
            chunk_size: 8,
            chunks: vec![InstanceChunkMapping {
                source_region: content.initial_state.region.clone(),
                source_origin: tile(10, 10, 0),
                destination_region: RegionId::new("region.synthetic.floor_1").unwrap(),
                destination_origin: tile(10, 10, 1),
                quarter_turns: 0,
            }],
            private_to_character: true,
            source: source(),
        },
    );
    content.mechanics.death = Some(DeathPolicy {
        domain: DeathDomain::NormalUnsafeNonPvp,
        value_provider: provider,
        retained_unskulled: 3,
        protect_item_extra: 1,
        ties: bound(RetentionTiePolicy::InventoryThenEquipment),
        respawn: bound(WorldLocation {
            region: content.initial_state.region.clone(),
            tile: tile(10, 10, 0),
            instance: None,
        }),
        first_office: bound(WorldLocation {
            region: RegionId::new("region.synthetic.floor_1").unwrap(),
            tile: tile(10, 10, 1),
            instance: Some(template()),
        }),
        restoration: bound(DeathVitalRestoration {
            on_arrival: BTreeMap::from([
                (Vital::Hitpoints, VitalRestoration::ToBaseMaximum),
                (Vital::Prayer, VitalRestoration::ToBaseMaximum),
                (Vital::RunEnergy, VitalRestoration::ToBaseMaximum),
            ]),
            on_first_office_exit: BTreeMap::from([(
                Vital::RunEnergy,
                VitalRestoration::ToBaseMaximum,
            )]),
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
        idle_after_milliseconds: 10_000,
        reclaim_range: 7,
        require_line_of_sight: true,
        grave_capacity: 120,
        office_capacity: 120,
        office_overflow: bound(RecoveryOverflow::RejectTransfer),
        grave_fee: bound(RecoveryFee::Bands {
            bands: vec![
                FeeBand {
                    minimum_value: 100_000,
                    fee: 1000,
                },
                FeeBand {
                    minimum_value: 1_000_000,
                    fee: 10_000,
                },
                FeeBand {
                    minimum_value: 10_000_000,
                    fee: 100_000,
                },
            ],
            maximum_total: 500_000,
        }),
        office_fee: bound(RecoveryFee::Percentage {
            free_below: 100_000,
            rate: Ratio {
                numerator: 5,
                denominator: 100,
            },
            rounding: IntegerRounding::Floor,
        }),
        payment_order: vec![FeeSource::Coffer, FeeSource::Bank],
        currency: item("coins"),
        repeat: bound(RepeatDeathPolicy {
            keep_old_grave_location: true,
            refresh_timer_if_contents_change: true,
            old_unstackable_per_item_limit: 28,
            old_items_to_office: vec![item("ore")],
            supply_items: vec![item("cooked")],
            supply_ground_policy: ground_policy(),
            source: source(),
        }),
        source: source(),
    });
    content.mechanics.travels.insert(
        travel("exit"),
        TravelDefinition {
            id: travel("exit"),
            guard: Guard::All {
                guards: vec![
                    Guard::Life {
                        phase: LifePhase::FirstDeathOffice,
                    },
                    Guard::DeathTopics {
                        topics: BTreeSet::from([
                            DeathTopic::Fees,
                            DeathTopic::Timer,
                            DeathTopic::KeptItems,
                        ]),
                    },
                ],
            },
            destination: bound(TravelDestination::PreviousRespawn),
            channel_ticks: bound(0),
            cooldown_ticks: bound(0),
            cooldown_start: bound(CooldownStart::Completed),
            interruptions: BTreeSet::from([
                InterruptionCause::Movement,
                InterruptionCause::AnotherAction,
                InterruptionCause::Combat,
            ]),
            completion_effects: vec![],
            source: source(),
        },
    );
    add_object(
        content,
        "portal",
        InteractionAction::TravelVia {
            travel: travel("exit"),
        },
    );
    add_dialogue(content);
    content.dialogues.get_mut(&dialogue()).unwrap().nodes[0].choices = [
        ("fees", DeathTopic::Fees),
        ("timer", DeathTopic::Timer),
        ("kept", DeathTopic::KeptItems),
    ]
    .into_iter()
    .map(|(id, topic)| DialogueChoice {
        id: id.into(),
        text: "Synthetic Death topic".into(),
        guard: Guard::Always,
        effects: vec![Effect::CompleteDeathTopic { topic }],
        next_node: Some("entry".into()),
    })
    .collect();
}

pub fn with_fire(content: &mut GameContent) {
    let mut recipe = content.recipes[&recipe("dough")].clone();
    recipe.id = super::recipe("fire");
    recipe.inputs = vec![stack("ore", 1)];
    recipe.outputs = vec![];
    recipe.failed_outputs = vec![];
    recipe.tools = vec![item("hammer")];
    recipe.ticks = None;
    recipe.xp = vec![XpReward {
        skill: skill(),
        amount_tenths: 400,
    }];
    recipe.success = ChanceRule::source_skilling(64, 512, levels()).unwrap();
    recipe.mechanics = Some(RecipeMechanics {
        method: method("fire"),
        guard: Guard::Always,
        chance_skill: Some(skill()),
        cadence: cadence(4, 4, 4),
        tool_ownership: OwnershipScope::Inventory,
        failed_xp: vec![],
        success_effects: vec![],
        failure_effects: vec![],
        lifecycle: RecipeLifecycle::Firemaking {
            ground_input: item("ore"),
            fire: fire(),
            step_priority: vec![
                Direction::West,
                Direction::East,
                Direction::South,
                Direction::North,
            ],
            retain_ground_input_on_failure: true,
        },
    });
    content.recipes.insert(recipe.id.clone(), recipe);
    content.mechanics.temporary_objects.insert(
        fire(),
        TemporaryObjectDefinition {
            id: fire(),
            object: object("range"),
            lifetime: bound(TickDuration::UniformInclusive {
                minimum: 5,
                maximum: 8,
            }),
            placement_guard: Guard::Always,
            interactions: vec![InteractionDefinition {
                name: "cook".into(),
                reach: 1,
                guard: Guard::Always,
                action: InteractionAction::Production {
                    recipes: vec![super::recipe("cook")],
                },
            }],
            owner_only_use: false,
            blocks_movement: false,
            blocks_projectiles: false,
            expired_items: vec![stack("pebble", 1)],
            ground_policy: ground_policy(),
            source: source(),
        },
    );
    typed_recipe(content, "cook", cadence(1, 1, 1));
}

pub fn tick(
    engine: &WorldEngine,
    world: &mut WorldState,
    rng: &mut impl RandomSource,
) -> Vec<ActorEvent> {
    engine
        .tick_with_context(world, rng, &TickContext::all_active(world))
        .unwrap()
}

pub fn tick_n(
    engine: &WorldEngine,
    world: &mut WorldState,
    n: usize,
    rng: &mut impl RandomSource,
) -> Vec<ActorEvent> {
    (0..n).flat_map(|_| tick(engine, world, rng)).collect()
}

pub fn armed(content: &mut GameContent, ranged: bool) {
    content.initial_state.equipment.insert(
        slot("weapon"),
        stack(if ranged { "pick" } else { "dagger" }, 1),
    );
    if ranged {
        content.initial_state.inventory.slots[0] = None;
        content
            .initial_state
            .equipment
            .insert(slot("ammo"), stack("arrow", 50));
    }
}

pub fn select_style(engine: &WorldEngine, world: &mut WorldState, name: &str) {
    apply(
        engine,
        world,
        GameIntent::SetCombatStyle {
            style: style(name).to_string(),
        },
    );
    tick(engine, world, &mut NeverDraw);
}

pub struct Hits(pub u32);
impl RandomSource for Hits {
    fn draw_below(&mut self, upper: u32) -> GameResult<u32> {
        let value = if self.0 % 3 == 1 { 0 } else { upper - 1 };
        self.0 += 1;
        Ok(value)
    }
}
pub struct Rolls(pub VecDeque<u32>);
impl Rolls {
    pub fn new(values: &[u32]) -> Self {
        Self(values.iter().copied().collect())
    }
}
impl RandomSource for Rolls {
    fn draw_below(&mut self, upper: u32) -> GameResult<u32> {
        let value = self.0.pop_front().expect("unexpected engine RNG call");
        assert!(value < upper, "{value} is not below {upper}");
        Ok(value)
    }
}
