use clubscape_game_types::*;
use clubscape_simulation::{inventory, skills};

use crate::{
    TickContext, WorldEngine, invalid_content, invalid_state, runtime, source_math, unavailable,
    unknown,
};

impl WorldEngine {
    pub(crate) fn maximum_vital(
        &self,
        character: &CharacterState,
        vital: Vital,
    ) -> GameResult<u16> {
        if vital == Vital::RunEnergy {
            return Ok(MAX_RUN_ENERGY);
        }
        let policy = self
            .content
            .mechanics
            .vitals
            .as_ref()
            .ok_or_else(|| unavailable("Source vital skills are not bound."))?;
        self.level(
            character,
            if vital == Vital::Hitpoints {
                &policy.hitpoints_skill
            } else {
                &policy.prayer_skill
            },
            SkillLevelBasis::Base,
        )
    }

    pub(crate) fn restore_vital(
        &self,
        character: &mut CharacterState,
        vital: Vital,
        restoration: &VitalRestoration,
    ) -> GameResult<()> {
        let maximum = self.maximum_vital(character, vital)?;
        let current = match vital {
            Vital::Hitpoints => &mut character.hitpoints,
            Vital::Prayer => &mut character.prayer_points,
            Vital::RunEnergy => &mut character.run_energy,
        };
        *current = match restoration {
            VitalRestoration::ToBaseMaximum => maximum,
            VitalRestoration::Amount { amount } => {
                (*current).max(current.saturating_add(*amount).min(maximum))
            }
            VitalRestoration::Set { amount } if *amount <= maximum => *amount,
            VitalRestoration::Set { .. } => {
                return Err(invalid_content(
                    "Vital restoration exceeds its source maximum.",
                ));
            }
        };
        Ok(())
    }

    pub(crate) fn award_xp(
        &self,
        character: &mut CharacterState,
        rewards: &[XpReward],
    ) -> GameResult<Vec<GameEvent>> {
        let before = character.skills.clone();
        let prior_hitpoints = character.hitpoints;
        let prior_prayer = character.prayer_points;
        let events = skills::award_character_xp(
            character,
            &self.content,
            rewards,
            skills::CurrentLevelPolicy::AddBaseLevelGains,
        )?;
        if let Some(policy) = &self.content.mechanics.vitals {
            for (skill, vital) in [
                (&policy.hitpoints_skill, Vital::Hitpoints),
                (&policy.prayer_skill, Vital::Prayer),
            ] {
                if !rewards.iter().any(|reward| &reward.skill == skill) {
                    continue;
                }
                let definition = self
                    .content
                    .skills
                    .get(skill)
                    .ok_or_else(|| unknown("Unknown vital skill."))?;
                let prior = before
                    .get(skill)
                    .ok_or_else(|| invalid_state("Missing prior vital skill."))?;
                let old = skills::level_for_xp(definition, prior.xp_tenths)?;
                let new = self.level(character, skill, SkillLevelBasis::Base)?;
                if new > old {
                    match policy.level_up.require()? {
                        LevelUpVitalPolicy::PreserveCurrent => {}
                        LevelUpVitalPolicy::IncreaseByBaseDifference => self.restore_vital(
                            character,
                            vital,
                            &VitalRestoration::Amount { amount: new - old },
                        )?,
                        LevelUpVitalPolicy::RestoreToBase => {
                            self.restore_vital(character, vital, &VitalRestoration::ToBaseMaximum)?
                        }
                        LevelUpVitalPolicy::RaiseIfAtOldBaseOtherwisePreserve => {
                            let old_current = if vital == Vital::Hitpoints {
                                prior_hitpoints
                            } else {
                                prior_prayer
                            };
                            let current = if old_current == old { new } else { old_current };
                            match vital {
                                Vital::Hitpoints => character.hitpoints = current,
                                Vital::Prayer => character.prayer_points = current,
                                Vital::RunEnergy => unreachable!(),
                            }
                            let state = character
                                .skills
                                .get_mut(skill)
                                .ok_or_else(|| invalid_state("Missing awarded vital skill."))?;
                            state.current_level = if prior.current_level == old {
                                new
                            } else {
                                prior.current_level
                            };
                        }
                    }
                }
            }
        }
        Ok(events)
    }

    pub(crate) fn eat(
        &self,
        tick: u64,
        character: &mut CharacterState,
        slot: usize,
    ) -> GameResult<Vec<GameEvent>> {
        if tick < character.runtime.food_ready {
            return Err(GameError::new(
                GameErrorCode::Busy,
                "Food cooldown is not ready.",
            ));
        }
        let stack = inventory::stack_at(&character.inventory, slot)?.clone();
        let healing = self
            .content
            .items
            .get(&stack.item)
            .ok_or_else(|| unknown("Unknown food."))?
            .healing
            .filter(|hp| *hp > 0)
            .ok_or_else(|| invalid_state("This item is not edible food."))?;
        let (maximum, food_delay, attack_delay) =
            if let Some(policy) = &self.content.mechanics.vitals {
                (
                    self.maximum_vital(character, Vital::Hitpoints)?,
                    u64::from(*policy.food_delay_ticks.require()?),
                    u64::from(*policy.food_attack_delay_ticks.require()?),
                )
            } else {
                // Explicit compatibility for legacy ordinary-food content; new content binds VitalPolicy.
                (
                    self.level(
                        character,
                        &SkillId::new("skill.hitpoints")?,
                        SkillLevelBasis::Base,
                    )?,
                    3,
                    3,
                )
            };
        let before = character.hitpoints;
        inventory::remove_from_slot(
            &mut character.inventory,
            &self.content.items,
            slot,
            Quantity::new(1)?,
        )?;
        character.hitpoints = before.max(before.saturating_add(healing).min(maximum));
        character.runtime.food_ready = runtime::deadline(tick, food_delay)?;
        character.runtime.combat.attack_ready = runtime::deadline(
            character.runtime.combat.attack_ready.max(tick),
            attack_delay,
        )?;
        character.runtime.combat.spell_ready =
            runtime::deadline(character.runtime.combat.spell_ready.max(tick), attack_delay)?;
        self.record_item_animation(tick, character, &stack.item, "eat")?;
        Ok(vec![GameEvent::FoodEaten {
            item: stack.item,
            healed: character.hitpoints - before,
        }])
    }

    pub(crate) fn set_setting(
        &self,
        character: &mut CharacterState,
        setting: &CharacterSetting,
    ) -> GameResult<GameEvent> {
        match setting {
            CharacterSetting::Run(value) => {
                let policy = self
                    .content
                    .mechanics
                    .run
                    .as_ref()
                    .ok_or_else(|| unavailable("Run policy is not bound."))?;
                if *value && character.run_energy < policy.activation_minimum {
                    return Err(GameError::new(
                        GameErrorCode::RequirementNotMet,
                        "Insufficient run energy to enable running.",
                    ));
                }
                if *value {
                    self.run_cost(character)?;
                }
                character.runtime.settings.run_enabled = Some(*value);
                if let Activity::Walking { running, .. } = &mut character.activity {
                    *running = *value;
                }
            }
            CharacterSetting::AutoRetaliate(value) => {
                character.runtime.settings.auto_retaliate = Some(*value)
            }
            CharacterSetting::DeathAutoEquip(value) => {
                character.runtime.settings.death_auto_equip = Some(*value)
            }
            CharacterSetting::DeathSupplyPiles(value) => {
                character.runtime.settings.death_supply_piles = Some(*value)
            }
        }
        Ok(GameEvent::SettingChanged {
            setting: setting.clone(),
        })
    }

    pub(crate) fn run_cost(&self, character: &CharacterState) -> GameResult<u16> {
        let policy = self
            .content
            .mechanics
            .run
            .as_ref()
            .ok_or_else(|| unavailable("Run policy is not bound."))?;
        let agility = self.level(character, &policy.agility, policy.levels.basis)?;
        if !(policy.levels.minimum..=policy.levels.maximum).contains(&agility) {
            return Err(unavailable(
                "Agility is outside the run-policy source domain.",
            ));
        }
        let formula = policy.drain.require()?;
        let grams = self.carried_weight(character)?;
        let minimum = i64::from(formula.weight_minimum_grams);
        let maximum = i64::from(formula.weight_maximum_grams);
        if maximum <= minimum || formula.agility_scale == 0 || agility > formula.agility_scale {
            return Err(invalid_content("Invalid run-drain domain."));
        }
        let weight = grams.clamp(minimum, maximum) - minimum;
        let denominator = (maximum - minimum) as u128;
        let numerator = u128::from(formula.base) * denominator
            + u128::from(formula.weight_scale) * weight as u128;
        let (numerator, denominator) = if formula.floor_weight_term_before_agility {
            (numerator / denominator, 1)
        } else {
            (numerator, denominator)
        };
        let cost = source_math::rounded(
            numerator * u128::from(formula.agility_scale - agility),
            denominator * u128::from(formula.agility_scale),
            &formula.rounding,
        )?;
        u16::try_from(cost).map_err(|_| invalid_content("Run cost exceeds the energy range."))
    }

    pub(crate) fn carried_weight(&self, character: &CharacterState) -> GameResult<i64> {
        let mut grams = 0_i64;
        for (stack, equipped) in character
            .inventory
            .slots
            .iter()
            .flatten()
            .map(|stack| (stack, false))
            .chain(character.equipment.values().map(|stack| (stack, true)))
        {
            let weight = self
                .content
                .items
                .get(&stack.item)
                .ok_or_else(|| unknown("Unknown weighted item."))?
                .weight
                .as_ref()
                .ok_or_else(|| unavailable(format!("No weight binding for {}.", stack.item)))?
                .require()?;
            let count = match if equipped {
                &weight.equipment
            } else {
                &weight.inventory
            } {
                WeightContribution::None => 0,
                WeightContribution::OncePerStack => 1,
                WeightContribution::PerUnit => stack.quantity.get(),
            };
            grams = grams
                .checked_add(i64::from(weight.grams) * i64::from(count))
                .ok_or_else(|| invalid_state("Carried weight overflow."))?;
        }
        Ok(grams)
    }

    pub(crate) fn set_prayer(
        &self,
        character: &mut CharacterState,
        prayer: &PrayerId,
        enabled: bool,
    ) -> GameResult<Vec<GameEvent>> {
        let definition = self
            .content
            .mechanics
            .prayers
            .get(prayer)
            .ok_or_else(|| unknown("Unknown prayer."))?;
        self.prayer_preconditions(character, prayer, enabled)?;
        let mut events = vec![];
        if enabled {
            for excluded in &definition.exclusive_with {
                if character.runtime.combat.active_prayers.remove(excluded) {
                    events.push(GameEvent::PrayerChanged {
                        prayer: excluded.clone(),
                        enabled: false,
                    });
                }
            }
            character
                .runtime
                .combat
                .active_prayers
                .insert(prayer.clone());
        } else {
            character.runtime.combat.active_prayers.remove(prayer);
        }
        events.push(GameEvent::PrayerChanged {
            prayer: prayer.clone(),
            enabled,
        });
        Ok(events)
    }

    pub(crate) fn prayer_preconditions(
        &self,
        character: &CharacterState,
        prayer: &PrayerId,
        enabled: bool,
    ) -> GameResult<()> {
        let definition = self
            .content
            .mechanics
            .prayers
            .get(prayer)
            .ok_or_else(|| unknown("Unknown prayer."))?;
        if !character.interfaces.contains(&definition.interface) {
            return Err(GameError::new(
                GameErrorCode::RequirementNotMet,
                "Prayer interface is locked.",
            ));
        }
        if enabled {
            self.requirements(character, &definition.requirements)?;
            definition.drain.require()?;
            if character.prayer_points == 0 {
                return Err(GameError::new(
                    GameErrorCode::RequirementNotMet,
                    "No prayer points.",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn advance_vitals(
        &self,
        tick: u64,
        character: &mut CharacterState,
        context: &TickContext,
        running: bool,
        combat: bool,
    ) -> GameResult<Vec<GameEvent>> {
        let mut events = Vec::new();
        if let Some(policy) = &self.content.mechanics.run {
            let regeneration = policy.regeneration.require()?;
            if !running
                && !context.paused(character, &regeneration.pauses, None, running, combat)?
            {
                let level = self.level(character, &policy.agility, policy.levels.basis)?;
                if !(policy.levels.minimum..=policy.levels.maximum).contains(&level)
                    || regeneration.skill_divisor == 0
                {
                    return Err(unavailable("Unbound run-regeneration level domain."));
                }
                character.run_energy = character
                    .run_energy
                    .saturating_add(level / regeneration.skill_divisor)
                    .saturating_add(regeneration.additive_units)
                    .min(MAX_RUN_ENERGY);
            }
        }
        if let Some(policy) = &self.content.mechanics.vitals {
            for regeneration in &policy.regeneration {
                let interval = *regeneration.interval_ticks.require()?;
                if interval == 0 {
                    return Err(invalid_content("Regeneration interval is zero."));
                }
                let ready = character
                    .runtime
                    .regeneration_deadlines
                    .get(&regeneration.vital)
                    .copied()
                    .unwrap_or(u64::from(interval));
                if context.paused(
                    character,
                    &regeneration.pauses,
                    regeneration.idle_after_milliseconds,
                    running,
                    combat,
                )? {
                    character
                        .runtime
                        .regeneration_deadlines
                        .insert(regeneration.vital, runtime::deadline(ready.max(tick), 1)?);
                } else if tick >= ready {
                    self.restore_vital(
                        character,
                        regeneration.vital,
                        &VitalRestoration::Amount {
                            amount: regeneration.amount,
                        },
                    )?;
                    character.runtime.regeneration_deadlines.insert(
                        regeneration.vital,
                        runtime::deadline(tick, u64::from(interval))?,
                    );
                }
            }
        }
        if !character.runtime.combat.active_prayers.is_empty() {
            let online = context
                .actors
                .get(&character.actor_id)
                .ok_or_else(|| unavailable("Prayer drain needs authority-owned online presence."))?
                .online;
            if online {
                let bonus: i64 = character
                    .equipment
                    .values()
                    .map(|stack| {
                        self.content
                            .items
                            .get(&stack.item)
                            .and_then(|item| item.equipment.as_ref())
                            .map_or(0, |equipment| i64::from(equipment.bonuses.prayer))
                    })
                    .sum();
                let mut remainder = character.runtime.combat.prayer_drain.clone().unwrap_or(
                    FractionalAccumulator {
                        numerator: 0,
                        denominator: 1,
                    },
                );
                let mut points = 0_u64;
                for prayer in &character.runtime.combat.active_prayers {
                    let drain = self
                        .content
                        .mechanics
                        .prayers
                        .get(prayer)
                        .ok_or_else(|| unknown("Unknown active prayer."))?
                        .drain
                        .require()?;
                    let period = i64::from(drain.bonus_offset) + bonus;
                    if period <= 0 {
                        return Err(invalid_content(
                            "Prayer bonus makes the drain denominator nonpositive.",
                        ));
                    }
                    let numerator =
                        u64::from(drain.points_per_tick.numerator) * u64::from(drain.bonus_divisor);
                    let denominator = u64::from(drain.points_per_tick.denominator)
                        .checked_mul(period as u64)
                        .ok_or_else(|| invalid_content("Prayer drain denominator overflow."))?;
                    let (whole, next) =
                        source_math::add_fraction(&remainder, numerator, denominator)?;
                    points = points
                        .checked_add(whole)
                        .ok_or_else(|| invalid_state("Prayer drain overflow."))?;
                    remainder = next;
                }
                character.prayer_points =
                    u64::from(character.prayer_points).saturating_sub(points) as u16;
                character.runtime.combat.prayer_drain = Some(remainder);
                if character.prayer_points == 0 {
                    for prayer in std::mem::take(&mut character.runtime.combat.active_prayers) {
                        events.push(GameEvent::PrayerChanged {
                            prayer,
                            enabled: false,
                        });
                    }
                    character.runtime.combat.prayer_drain = None;
                }
            }
        }
        Ok(events)
    }
}
