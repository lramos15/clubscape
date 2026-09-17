use std::collections::BTreeMap;

use clubscape_game_types::*;

use crate::{
    ActorEvent, RandomSource, WorldEngine, invalid_content, invalid_state, random, runtime,
    source_math, tag, unavailable, unknown,
};

struct Strike<'a> {
    target: &'a SpawnId,
    life: u64,
    style: &'a CombatStyleDefinition,
    spell: Option<&'a SpellId>,
    outcome: CombatOutcome,
    damage: u16,
    instance: Option<InstanceId>,
    snapshot: Option<&'a ProjectileTargetSnapshot>,
}

impl WorldEngine {
    pub(crate) fn spell_preconditions(
        &self,
        world: &WorldState,
        character: &CharacterState,
        id: &SpellId,
    ) -> GameResult<()> {
        let spell = self
            .content
            .mechanics
            .spells
            .get(id)
            .ok_or_else(|| unknown("Unknown spell."))?;
        if !character.interfaces.contains(&spell.interface) {
            return Err(GameError::new(
                GameErrorCode::RequirementNotMet,
                "Spell interface is locked.",
            ));
        }
        self.requirements(character, &spell.requirements)?;
        self.require_guard(world, character, &spell.guard)?;
        clubscape_simulation::inventory::remove_batch(
            &mut character.inventory.clone(),
            &self.content.items,
            &spell.runes,
        )?;
        match &spell.action {
            SpellAction::Combat { .. } => self.combat_phase_ready(world.tick, character)?,
            SpellAction::Teleport { travel } => self.travel_permission(world, character, travel)?,
        }
        Ok(())
    }

    fn combat_phase_ready(&self, tick: u64, character: &CharacterState) -> GameResult<()> {
        if !matches!(character.runtime.life, LifeState::Alive | LifeState::Legacy)
            || character.hitpoints == 0
        {
            return Err(GameError::new(
                GameErrorCode::RequirementNotMet,
                "This life state cannot attack.",
            ));
        }
        if tick
            < character
                .runtime
                .combat
                .attack_ready
                .max(character.runtime.combat.spell_ready)
        {
            return Err(GameError::new(
                GameErrorCode::Busy,
                "Source attack cooldown is not ready.",
            ));
        }
        Ok(())
    }
}

pub(crate) enum CombatLock {
    State,
    Logout,
    Travel,
}

fn sync_fighting_style(character: &mut CharacterState) {
    if let (Some(selected), Activity::Fighting { style, .. }) =
        (&character.runtime.combat.style, &mut character.activity)
    {
        *style = selected.to_string();
    }
}

impl WorldEngine {
    pub(crate) fn attack_permission(
        &self,
        world: &WorldState,
        character: &CharacterState,
        target: &SpawnId,
    ) -> GameResult<()> {
        let id = character.runtime.combat.style.as_ref().ok_or_else(|| {
            GameError::new(
                GameErrorCode::RequirementNotMet,
                "Select a source combat style.",
            )
        })?;
        let style = self
            .content
            .mechanics
            .combat_styles
            .get(id)
            .ok_or_else(|| unknown("Unknown selected style."))?;
        if style.method == AttackMethod::Magic {
            return Err(GameError::new(
                GameErrorCode::RequirementNotMet,
                "Select a source spell to cast.",
            ));
        }
        let weapon = self.equipped_weapon(character, id)?;
        if !matches!(character.runtime.life, LifeState::Alive | LifeState::Legacy)
            || character.hitpoints == 0
        {
            return Err(GameError::new(
                GameErrorCode::RequirementNotMet,
                "This life state cannot attack.",
            ));
        }
        if world.tick
            < character
                .runtime
                .combat
                .attack_ready
                .max(character.runtime.combat.spell_ready)
        {
            return Err(GameError::new(
                GameErrorCode::Busy,
                "Attack cooldown is not ready.",
            ));
        }
        let interaction = self.attack_interaction(world, character, target, style.reach)?;
        self.require_target(world, character, target, &interaction)?;
        let shape = self.target_shape(
            world,
            character,
            &WorldTarget::Spawn {
                spawn: target.clone(),
            },
        )?;
        if style.method == AttackMethod::Melee
            && style.reach == 1
            && !melee_adjacent(character.tile, shape.tile, shape.width, shape.height)
        {
            return Err(GameError::new(
                GameErrorCode::OutOfReach,
                "Melee requires a cardinal target edge.",
            ));
        }
        let mechanics = shape
            .npc
            .as_ref()
            .and_then(|npc| self.content.npcs.get(npc))
            .and_then(|npc| npc.combat.as_ref())
            .and_then(|combat| combat.mechanics.as_ref())
            .ok_or_else(|| unavailable("Target has no typed combat policy."))?;
        self.attack_eligible(world, character, mechanics, id, style.method)?;
        mechanics.engagement.require()?;
        style.accuracy.require()?;
        style.negative_rolls.require()?;
        style.damage.require()?;
        style.cycle_ticks.require()?;
        self.maximum_player_hit(character, style)?;
        if let Some(projectile) = &style.projectile {
            self.content
                .mechanics
                .projectiles
                .get(projectile)
                .ok_or_else(|| unknown("Unknown projectile."))?
                .timing
                .require()?;
        }
        if let Some(ammo) = &weapon.ammunition {
            let stack = character.equipment.get(&ammo.slot).ok_or_else(|| {
                GameError::new(
                    GameErrorCode::InsufficientItems,
                    "Compatible ammunition must be equipped.",
                )
            })?;
            if !ammo.compatible_items.contains(&stack.item)
                || stack.quantity.get() < ammo.per_attack.get()
            {
                return Err(GameError::new(
                    GameErrorCode::InsufficientItems,
                    "Insufficient compatible ammunition.",
                ));
            }
            ammo.break_chance.require()?;
        }
        Ok(())
    }

    pub(crate) fn select_style(
        &self,
        character: &mut CharacterState,
        id: &CombatStyleId,
    ) -> GameResult<()> {
        self.equipped_weapon(character, id)?;
        if !self.content.mechanics.combat_styles.contains_key(id) {
            return Err(unknown("Unknown combat style."));
        }
        character.runtime.combat.style = Some(id.clone());
        sync_fighting_style(character);
        Ok(())
    }

    pub(crate) fn equipped_weapon<'a>(
        &'a self,
        character: &CharacterState,
        style: &CombatStyleId,
    ) -> GameResult<&'a WeaponDefinition> {
        let mut selected = None;
        for stack in character.equipment.values() {
            if let Some(weapon) = self
                .content
                .items
                .get(&stack.item)
                .and_then(|item| item.equipment.as_ref())
                .and_then(|equipment| equipment.weapon.as_ref())
                && weapon.styles.contains(style)
            {
                if selected.is_some() {
                    return Err(invalid_content("Multiple equipped weapons own this style."));
                }
                selected = Some(weapon);
            }
        }
        if let Some(weapon) = selected {
            return Ok(weapon);
        }
        let any_weapon = character.equipment.values().any(|stack| {
            self.content
                .items
                .get(&stack.item)
                .and_then(|item| item.equipment.as_ref())
                .is_some_and(|equipment| {
                    equipment.weapon.is_some() || !equipment.attack_styles.is_empty()
                })
        });
        if !any_weapon && let Some(policy) = &self.content.mechanics.player_combat {
            let unarmed = policy.unarmed.require()?;
            if unarmed.styles.contains(style) {
                return Ok(unarmed);
            }
        }
        Err(GameError::new(
            GameErrorCode::RequirementNotMet,
            "Style is not offered by the equipped weapon or source unarmed profile.",
        ))
    }

    pub(crate) fn refresh_combat_style(&self, character: &mut CharacterState) -> GameResult<()> {
        let mut weapon = None;
        let mut legacy_weapon = false;
        for stack in character.equipment.values() {
            let equipment = self
                .content
                .items
                .get(&stack.item)
                .and_then(|item| item.equipment.as_ref())
                .ok_or_else(|| invalid_state("Equipped item lacks equipment data."))?;
            if let Some(current) = &equipment.weapon
                && weapon.replace(current).is_some()
            {
                return Err(invalid_content("Multiple equipped weapon profiles."));
            }
            legacy_weapon |= !equipment.attack_styles.is_empty();
        }
        if weapon.is_none()
            && !legacy_weapon
            && let Some(policy) = &self.content.mechanics.player_combat
        {
            weapon = Some(policy.unarmed.require()?);
        }
        if let Some(weapon) = weapon {
            if !weapon.styles.contains(&weapon.default_style) {
                return Err(invalid_content("Weapon default style is not offered."));
            }
            if character
                .runtime
                .combat
                .style
                .as_ref()
                .is_none_or(|style| !weapon.styles.contains(style))
            {
                character.runtime.combat.style = Some(weapon.default_style.clone());
            }
        } else if !legacy_weapon {
            character.runtime.combat.style = None;
        }
        sync_fighting_style(character);
        Ok(())
    }

    pub(crate) fn cast(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        id: &SpellId,
        target: Option<&SpawnId>,
        rng: &mut impl RandomSource,
    ) -> GameResult<Vec<GameEvent>> {
        let spell = self
            .content
            .mechanics
            .spells
            .get(id)
            .ok_or_else(|| unknown("Unknown spell."))?;
        self.spell_preconditions(world, character, id)?;
        match &spell.action {
            SpellAction::Combat { style, .. } => {
                let target =
                    target.ok_or_else(|| invalid_state("Combat spell requires a target."))?;
                self.player_attack(world, character, target, style, Some(id), rng)
            }
            SpellAction::Teleport { travel } => {
                if target.is_some() {
                    return Err(invalid_state("Travel spell cannot target an NPC."));
                }
                clubscape_simulation::inventory::remove_batch(
                    &mut character.inventory,
                    &self.content.items,
                    &spell.runes,
                )?;
                let mut events = self.start_travel(world, character, travel, rng)?;
                events.extend(self.award_xp(character, &spell.launch_xp)?);
                self.record_actor_action(
                    world.tick,
                    character,
                    ObservedAction {
                        activity: "casting".into(),
                        action_id: None,
                        target: None,
                        recipe_id: None,
                        style_id: None,
                        spell_id: Some(id.clone()),
                        animation: None,
                    },
                    character
                        .runtime
                        .pending_travel
                        .as_ref()
                        .map(|travel| travel.completes_at_tick),
                    crate::observer::ObservationUpdate {
                        new_instance: true,
                        completed: character.runtime.pending_travel.is_none(),
                        executed: true,
                    },
                )?;
                Ok(events)
            }
        }
    }

    pub(crate) fn player_attack(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        target: &SpawnId,
        style_id: &CombatStyleId,
        spell_id: Option<&SpellId>,
        rng: &mut impl RandomSource,
    ) -> GameResult<Vec<GameEvent>> {
        let style = self
            .content
            .mechanics
            .combat_styles
            .get(style_id)
            .ok_or_else(|| unknown("Unknown combat style."))?;
        if spell_id.is_none() {
            self.attack_permission(world, character, target)?;
        }
        self.combat_phase_ready(world.tick, character)?;
        let mut spent = Vec::new();
        let (weapon, projectile_id, launch_xp) = if let Some(spell_id) = spell_id {
            self.authorize(character, &["cast".into(), format!("cast:{spell_id}")])?;
            let spell = self
                .content
                .mechanics
                .spells
                .get(spell_id)
                .ok_or_else(|| unknown("Unknown spell."))?;
            self.require_guard(world, character, &spell.guard)?;
            self.requirements(character, &spell.requirements)?;
            if !character.interfaces.contains(&spell.interface) {
                return Err(GameError::new(
                    GameErrorCode::RequirementNotMet,
                    "Spell interface is locked.",
                ));
            }
            let SpellAction::Combat {
                style: bound_style,
                projectile,
            } = &spell.action
            else {
                return Err(invalid_state("Not a combat spell."));
            };
            if bound_style != style_id {
                return Err(invalid_state("Spell style mismatch."));
            }
            spent = spell.runes.clone();
            (None, Some(projectile), &spell.launch_xp[..])
        } else {
            let spawn = self
                .content
                .spawns
                .get(target)
                .ok_or_else(|| unknown("Unknown attack target."))?;
            let mut keys = vec!["interact".into(), format!("interact:{target}")];
            keys.extend(
                spawn
                    .interactions
                    .iter()
                    .filter(|action| matches!(action.action, InteractionAction::Attack))
                    .map(|action| format!("interact:{target}:{}", action.name)),
            );
            self.authorize(character, &keys)?;
            if style.method == AttackMethod::Magic {
                return Err(GameError::new(
                    GameErrorCode::RequirementNotMet,
                    "Magic attacks require a source spell and rune checks.",
                ));
            }
            (
                Some(self.equipped_weapon(character, style_id)?),
                style.projectile.as_ref(),
                &[][..],
            )
        };
        style.accuracy.require()?;
        style.negative_rolls.require()?;
        let cycle = *style.cycle_ticks.require()?;
        if cycle == 0 {
            return Err(invalid_content("Combat cycle is zero."));
        }
        let timing = projectile_id
            .map(|id| {
                self.content
                    .mechanics
                    .projectiles
                    .get(id)
                    .ok_or_else(|| unknown("Unknown projectile."))
                    .and_then(|definition| definition.timing.require())
            })
            .transpose()?;
        let interaction = self.attack_interaction(world, character, target, style.reach)?;
        self.require_target(world, character, target, &interaction)?;
        let shape = self.target_shape(
            world,
            character,
            &WorldTarget::Spawn {
                spawn: target.clone(),
            },
        )?;
        if style.method == AttackMethod::Melee
            && style.reach == 1
            && !melee_adjacent(character.tile, shape.tile, shape.width, shape.height)
        {
            return Err(GameError::new(
                GameErrorCode::OutOfReach,
                "One-tile melee requires an adjacent cardinal target edge.",
            ));
        }
        let npc_id = shape
            .npc
            .as_ref()
            .ok_or_else(|| invalid_state("Only a combat NPC can be attacked."))?;
        let npc = self
            .content
            .npcs
            .get(npc_id)
            .ok_or_else(|| unknown("Unknown combat NPC."))?;
        let combat = npc.combat.as_ref().ok_or_else(|| {
            GameError::new(GameErrorCode::RequirementNotMet, "NPC is not attackable.")
        })?;
        let mechanics = combat.mechanics.as_ref().ok_or_else(|| {
            unavailable("Legacy NPC combat is not a typed source combat contract.")
        })?;
        self.attack_eligible(world, character, mechanics, style_id, style.method)?;
        mechanics.engagement.require()?;
        let entity = runtime::entity(world, character.runtime.instance.as_ref(), target)?;
        let life = entity.runtime.life;
        let hp = entity.hitpoints;
        let attack =
            self.maximum_player_roll(character, &style.attack, style.attack_type, false)?;
        let defence_stat = mechanics
            .defence_stats
            .get(&style.attack_type)
            .ok_or_else(|| invalid_content("NPC lacks a defence stat selector."))?;
        let effective = (i64::from(npc_stat(combat, *defence_stat))
            + i64::from(*mechanics.effective_level_bonus.require()?))
        .max(0);
        let defence = source_math::maximum_accuracy_roll(
            u32::try_from(effective)
                .map_err(|_| invalid_content("NPC effective level overflow."))?,
            i32::from(*combat.bonuses.defence.get(&style.attack_type).unwrap_or(&0)),
        )?;
        let maximum = self.maximum_player_hit(character, style)?;
        let damage_policy = style.damage.require()?;
        if let Some(weapon) = weapon {
            if let Some(ammo) = &weapon.ammunition {
                let stack = character.equipment.get(&ammo.slot).ok_or_else(|| {
                    GameError::new(
                        GameErrorCode::InsufficientItems,
                        "Compatible ammunition must be equipped.",
                    )
                })?;
                if !ammo.compatible_items.contains(&stack.item)
                    || stack.quantity.get() < ammo.per_attack.get()
                {
                    return Err(GameError::new(
                        GameErrorCode::InsufficientItems,
                        "Insufficient compatible equipped ammunition.",
                    ));
                }
                ammo.break_chance.require()?;
                let policy = self
                    .content
                    .mechanics
                    .ground_policies
                    .get(&ammo.ground_policy)
                    .ok_or_else(|| unknown("Unknown ammo ground policy."))?;
                policy.public_after.require()?;
                policy.expires_after.require()?;
                spent.push(ItemStack {
                    item: stack.item.clone(),
                    quantity: ammo.per_attack,
                    instance: None,
                });
            } else if style.method == AttackMethod::Ranged {
                return Err(invalid_content(
                    "Ranged weapon has no explicit ammunition policy.",
                ));
            }
        }
        if spell_id.is_some() {
            clubscape_simulation::inventory::remove_batch(
                &mut character.inventory,
                &self.content.items,
                &spent,
            )?;
        }
        if let Some(ammo) = weapon.and_then(|weapon| weapon.ammunition.as_ref()) {
            let stack = character
                .equipment
                .get_mut(&ammo.slot)
                .ok_or_else(|| invalid_state("Ammunition disappeared."))?;
            let remaining = stack.quantity.get() - ammo.per_attack.get();
            if remaining == 0 {
                character.equipment.remove(&ammo.slot);
            } else {
                stack.quantity = Quantity::new(remaining)?;
            }
        }
        let hit = source_math::opposed_accuracy(attack, defence, rng)?;
        let rolled = if hit {
            random::draw(rng, u32::from(maximum) + 1)? as u16
        } else {
            0
        };
        let damage = if hit && maximum > 0 {
            rolled
                .max(damage_policy.successful_minimum)
                .min(maximum.max(damage_policy.successful_minimum))
        } else {
            0
        };
        let damage = if damage_policy.cap_to_remaining_hitpoints {
            damage.min(hp)
        } else {
            damage
        };
        let outcome = if hit {
            CombatOutcome::Hit
        } else {
            CombatOutcome::Miss
        };
        let deadline = runtime::deadline(world.tick, u64::from(cycle))?;
        let mut events = self.interrupt_travel_for_combat(character)?;
        runtime::interrupt(character)?;
        character.runtime.combat.attack_ready = deadline;
        if spell_id.is_some() {
            character.runtime.combat.spell_ready = deadline;
        }
        character.runtime.combat.target = Some(target.clone());
        character.runtime.combat.last_combat_tick = Some(world.tick);
        runtime::entity_mut(world, character.runtime.instance.as_ref(), target)?
            .runtime
            .last_combat_tick = Some(world.tick);
        if spell_id.is_none() {
            character.activity = Activity::Fighting {
                target: target.clone(),
                style: style_id.to_string(),
                next_tick: deadline,
            };
        }
        if mechanics.retaliation {
            let now = world.tick;
            let entity = runtime::entity_mut(world, character.runtime.instance.as_ref(), target)?;
            let newly_engaged = entity.runtime.retaliation_target.is_none();
            entity.runtime.retaliation_target = Some(character.actor_id.clone());
            if newly_engaged && entity.runtime.attack_ready <= now {
                entity.runtime.attack_ready =
                    runtime::deadline(now, u64::from(combat.attack_speed_ticks))?;
            }
        }
        if let Some(ammo) = weapon.and_then(|weapon| weapon.ammunition.as_ref()) {
            let chance = ammo.break_chance.require()?;
            if !ratio_roll(chance, rng)? {
                let stack = spent
                    .last()
                    .ok_or_else(|| invalid_state("No spent ammunition."))?
                    .clone();
                self.put_ground(
                    world,
                    stack,
                    &character.actor_id,
                    &RuntimeLocation {
                        tile: shape.tile,
                        region: self
                            .regions_by_tile
                            .get(&shape.tile)
                            .ok_or_else(|| unknown("Target tile has no region."))?
                            .clone(),
                        instance: character.runtime.instance.clone(),
                    },
                    &ammo.ground_policy,
                )?;
            }
        }
        let strike = Strike {
            target,
            life,
            style,
            spell: spell_id,
            outcome,
            damage,
            instance: character.runtime.instance.clone(),
            snapshot: None,
        };
        if let (Some(id), Some(timing)) = (projectile_id, timing) {
            let launch = runtime::deadline(world.tick, u64::from(timing.launch_delay_ticks))?;
            let distance = shape
                .distance_from(character.tile)
                .ok_or_else(|| invalid_state("Projectile crosses planes."))?;
            let flight = u64::from(timing.base_flight_ticks)
                + source_math::rounded(
                    u128::from(distance) * u128::from(timing.ticks_per_tile.numerator),
                    u128::from(timing.ticks_per_tile.denominator),
                    &timing.rounding,
                )?;
            let impact = runtime::deadline(launch, flight)?;
            if launch == world.tick {
                events.extend(self.award_xp(character, launch_xp)?);
            }
            if launch == world.tick && (timing.damage_on_launch || impact == world.tick) {
                events.extend(self.apply_strike(world, character, &strike, rng)?);
            }
            if impact > world.tick || launch > world.tick {
                let id_number = world
                    .runtime
                    .projectiles
                    .iter()
                    .map(|projectile| projectile.id)
                    .max()
                    .unwrap_or(0)
                    .checked_add(1)
                    .ok_or_else(|| invalid_state("Projectile ID overflow."))?;
                world.runtime.projectiles.push(PendingProjectile {
                    id: id_number,
                    definition: id.clone(),
                    source: Combatant::Player {
                        actor: character.actor_id.clone(),
                    },
                    target: Combatant::Npc {
                        spawn: target.clone(),
                        life,
                        instance: character.runtime.instance.clone(),
                    },
                    style: style_id.clone(),
                    spell: spell_id.cloned(),
                    launched_at_tick: launch,
                    impacts_at_tick: impact,
                    outcome,
                    damage,
                    resources_spent: spent,
                    target_snapshot: Some(ProjectileTargetSnapshot {
                        npc: npc_id.clone(),
                        location: RuntimeLocation {
                            region: self
                                .regions_by_tile
                                .get(&shape.tile)
                                .ok_or_else(|| unknown("Target has no source region."))?
                                .clone(),
                            tile: shape.tile,
                            instance: character.runtime.instance.clone(),
                        },
                    }),
                });
            }
        } else {
            events.extend(self.award_xp(character, launch_xp)?);
            events.extend(self.apply_strike(world, character, &strike, rng)?);
        }
        {
            let completed =
                spell_id.is_some() || !matches!(character.activity, Activity::Fighting { .. });
            self.record_actor_action(
                world.tick,
                character,
                ObservedAction {
                    activity: if spell_id.is_some() {
                        "casting"
                    } else {
                        "fighting"
                    }
                    .into(),
                    action_id: None,
                    target: Some(WorldTarget::Spawn {
                        spawn: target.clone(),
                    }),
                    recipe_id: None,
                    style_id: Some(style_id.clone()),
                    spell_id: spell_id.cloned(),
                    animation: None,
                },
                Some(deadline),
                crate::observer::ObservationUpdate {
                    new_instance: spell_id.is_some(),
                    completed,
                    executed: true,
                },
            )?;
        }
        Ok(events)
    }

    fn attack_interaction(
        &self,
        world: &WorldState,
        character: &CharacterState,
        target: &SpawnId,
        reach: u16,
    ) -> GameResult<InteractionDefinition> {
        let definition = self
            .content
            .spawns
            .get(target)
            .ok_or_else(|| unknown("Unknown attack target."))?;
        for interaction in &definition.interactions {
            if matches!(interaction.action, InteractionAction::Attack)
                && self.guard(world, character, &interaction.guard, None)?
            {
                return Ok(InteractionDefinition {
                    reach,
                    ..interaction.clone()
                });
            }
        }
        Err(GameError::new(
            GameErrorCode::RequirementNotMet,
            "No guarded source Attack interaction permits this target.",
        ))
    }

    fn effective(
        &self,
        character: &CharacterState,
        formula: &EffectiveLevelFormula,
    ) -> GameResult<u32> {
        let mut level = u64::from(self.level(character, &formula.skill, formula.basis)?);
        if !formula.prayer_before_style {
            level = level
                .checked_add_signed(i64::from(formula.style_bonus))
                .ok_or_else(|| invalid_content("Negative styled effective level."))?;
        }
        for id in &character.runtime.combat.active_prayers {
            let prayer = self
                .content
                .mechanics
                .prayers
                .get(id)
                .ok_or_else(|| unknown("Unknown active prayer."))?;
            for modifier in &prayer.modifiers {
                if modifier.skill == formula.skill {
                    level = source_math::rounded(
                        u128::from(level) * u128::from(modifier.multiplier.numerator),
                        u128::from(modifier.multiplier.denominator),
                        &modifier.rounding,
                    )?;
                }
            }
        }
        if formula.prayer_before_style {
            level = level
                .checked_add_signed(i64::from(formula.style_bonus))
                .ok_or_else(|| invalid_content("Negative effective level."))?;
        }
        level = level
            .checked_add_signed(i64::from(formula.constant_bonus))
            .ok_or_else(|| invalid_content("Effective level overflow."))?;
        u32::try_from(level).map_err(|_| invalid_content("Effective level exceeds roll bounds."))
    }

    pub(crate) fn maximum_player_roll(
        &self,
        character: &CharacterState,
        formula: &EffectiveLevelFormula,
        attack_type: AttackType,
        defence: bool,
    ) -> GameResult<u32> {
        let bonus = self.equipment_bonus(character, |bonuses| {
            i32::from(
                *(if defence {
                    &bonuses.defence
                } else {
                    &bonuses.attack
                })
                .get(&attack_type)
                .unwrap_or(&0),
            )
        })?;
        source_math::maximum_accuracy_roll(self.effective(character, formula)?, bonus)
    }

    pub(crate) fn equipment_bonus(
        &self,
        character: &CharacterState,
        project: impl Fn(&CombatBonuses) -> i32,
    ) -> GameResult<i32> {
        let mut result = 0_i32;
        for stack in character.equipment.values() {
            let definition = self
                .content
                .items
                .get(&stack.item)
                .and_then(|item| item.equipment.as_ref())
                .ok_or_else(|| invalid_state("Equipped item lacks equipment data."))?;
            result = result
                .checked_add(project(&definition.bonuses))
                .ok_or_else(|| invalid_content("Equipment bonus overflow."))?;
        }
        Ok(result)
    }

    fn maximum_player_hit(
        &self,
        character: &CharacterState,
        style: &CombatStyleDefinition,
    ) -> GameResult<u16> {
        let maximum = match style.maximum_hit.require()? {
            MaximumHitFormula::Fixed { hit } => u64::from(*hit),
            MaximumHitFormula::LevelTable { skill, basis, hits } => {
                let level = self.level(character, skill, *basis)?;
                u64::from(
                    *hits
                        .range(..=level)
                        .next_back()
                        .ok_or_else(|| unavailable("No source maximum-hit band for this level."))?
                        .1,
                )
            }
            MaximumHitFormula::Strength {
                level,
                equipment_offset,
                additive,
                divisor,
            } => {
                let effective = self.effective(character, level)?;
                let bonus = self.equipment_bonus(character, |bonuses| {
                    i32::from(if style.method == AttackMethod::Ranged {
                        bonuses.ranged_strength
                    } else {
                        bonuses.melee_strength
                    })
                })?;
                let strength = (i64::from(bonus) + i64::from(*equipment_offset)).max(0);
                source_math::rounded(
                    u128::from(effective) * strength as u128 + u128::from(*additive),
                    u128::from(*divisor),
                    &IntegerRounding::Floor,
                )?
            }
        };
        let maximum = if style.method == AttackMethod::Magic {
            let percent =
                self.equipment_bonus(character, |bonuses| i32::from(bonuses.magic_damage_percent))?;
            source_math::rounded(
                u128::from(maximum) * (100 + i64::from(percent)).max(0) as u128,
                100,
                &IntegerRounding::Floor,
            )?
        } else {
            maximum
        };
        u16::try_from(maximum).map_err(|_| invalid_content("Maximum hit exceeds shared HP range."))
    }

    fn apply_strike(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        strike: &Strike<'_>,
        rng: &mut impl RandomSource,
    ) -> GameResult<Vec<GameEvent>> {
        let entity = match runtime::entity(world, strike.instance.as_ref(), strike.target) {
            Ok(entity) => Some(entity),
            Err(error)
                if strike.snapshot.is_some()
                    && matches!(
                        error.code,
                        GameErrorCode::UnknownContent | GameErrorCode::InvalidInput
                    ) =>
            {
                None
            }
            Err(error) => return Err(error),
        };
        let viable = entity.is_some_and(|entity| {
            entity.runtime.life == strike.life
                && entity.hitpoints > 0
                && entity.available_at_tick <= world.tick
        });
        let outcome = if viable {
            strike.outcome
        } else {
            CombatOutcome::Invalidated
        };
        let damage = if viable && outcome == CombatOutcome::Hit {
            strike.damage.min(
                entity
                    .ok_or_else(|| invalid_state("Viable target disappeared."))?
                    .hitpoints,
            )
        } else {
            0
        };
        let tile = entity
            .map(|entity| entity.tile)
            .or_else(|| strike.snapshot.map(|snapshot| snapshot.location.tile))
            .ok_or_else(|| {
                invalid_state("Invalidated legacy projectile has no target location snapshot.")
            })?;
        let mut events = vec![GameEvent::CombatResolved {
            target: strike.target.clone(),
            life: strike.life,
            style: strike.style.id.clone(),
            method: strike.style.method,
            outcome,
            damage,
        }];
        if let Some(spell) = strike.spell {
            events.push(GameEvent::SpellResolved {
                spell: spell.clone(),
                target: strike.target.clone(),
                damage,
                tile,
                outcome: match outcome {
                    CombatOutcome::Hit => SpellOutcome::Hit,
                    CombatOutcome::Miss => SpellOutcome::Splash,
                    CombatOutcome::Invalidated => SpellOutcome::Invalidated,
                },
            });
        }
        if !viable {
            return Ok(events);
        }
        let now = world.tick;
        if damage > 0 {
            let entity = runtime::entity_mut(world, strike.instance.as_ref(), strike.target)?;
            entity.hitpoints -= damage;
            let order = entity
                .runtime
                .contributions
                .values()
                .filter(|entry| entry.last_hit_tick == now)
                .map(|entry| entry.last_hit_order)
                .max()
                .map_or(Ok(0), |order| {
                    order
                        .checked_add(1)
                        .ok_or_else(|| invalid_state("Combat ordering overflow."))
                })?;
            let entry = entity
                .runtime
                .contributions
                .entry(character.actor_id.clone())
                .or_insert(DamageContribution {
                    damage: 0,
                    first_hit_tick: now,
                    first_hit_order: order,
                    last_hit_tick: now,
                    last_hit_order: order,
                    methods: BTreeMap::new(),
                    methods_complete: true,
                });
            entry.damage = entry
                .damage
                .checked_add(u64::from(damage))
                .ok_or_else(|| invalid_state("Damage credit overflow."))?;
            entry.last_hit_tick = now;
            entry.last_hit_order = order;
            let method = entry
                .methods
                .entry(strike.style.method)
                .or_insert(MethodContribution {
                    damage: 0,
                    first_hit_tick: now,
                    first_hit_order: order,
                    last_hit_tick: now,
                    last_hit_order: order,
                });
            method.damage = method
                .damage
                .checked_add(u64::from(damage))
                .ok_or_else(|| invalid_state("Method damage overflow."))?;
            method.last_hit_tick = now;
            method.last_hit_order = order;
            let rewards = strike
                .style
                .damage_xp
                .iter()
                .map(|reward| {
                    Ok(XpReward {
                        skill: reward.skill.clone(),
                        amount_tenths: source_math::rounded(
                            u128::from(damage) * u128::from(reward.tenths_per_damage.numerator),
                            u128::from(reward.tenths_per_damage.denominator),
                            &reward.rounding,
                        )?,
                    })
                })
                .collect::<GameResult<Vec<_>>>()?;
            events.extend(self.award_xp(character, &rewards)?);
        }
        if runtime::entity(world, strike.instance.as_ref(), strike.target)?.hitpoints == 0 {
            let spawn = self
                .content
                .spawns
                .get(strike.target)
                .ok_or_else(|| unknown("Unknown defeated spawn."))?;
            let SpawnKind::Npc { npc } = &spawn.kind else {
                return Err(invalid_state("Defeated target is not an NPC."));
            };
            let definition = self
                .content
                .npcs
                .get(npc)
                .and_then(|npc| npc.combat.as_ref())
                .and_then(|combat| combat.mechanics.as_ref())
                .ok_or_else(|| unknown("Missing defeat policy."))?;
            let credited = self
                .kill_winner(
                    runtime::entity(world, strike.instance.as_ref(), strike.target)?,
                    definition.credit.require()?,
                )?
                .ok_or_else(|| invalid_state("Defeat has no legitimate damage contributor."))?;
            let owner = if credited == character.actor_id {
                character.clone()
            } else {
                world
                    .characters
                    .get(&credited)
                    .ok_or_else(|| {
                        unavailable("Credited actor state must be loaded to resolve loot guards.")
                    })?
                    .clone()
            };
            let method = attributed_method(
                runtime::entity(world, strike.instance.as_ref(), strike.target)?,
                &credited,
                strike.style.method,
                definition.attribution.require()?,
            )?;
            let loot = self.loot(world, &owner, &definition.loot, rng)?;
            if !loot.is_empty() {
                if world
                    .ground_items
                    .len()
                    .checked_add(loot.len())
                    .is_none_or(|total| total > 32_768)
                {
                    return Err(GameError::new(
                        GameErrorCode::InventoryFull,
                        "NPC loot exceeds available ground capacity.",
                    ));
                }
                let policy = definition.loot_ground_policy.require()?;
                let location = RuntimeLocation {
                    region: self
                        .regions_by_tile
                        .get(&tile)
                        .ok_or_else(|| unknown("Loot tile has no region."))?
                        .clone(),
                    tile,
                    instance: strike.instance.clone(),
                };
                for (ordinal, stack) in loot.into_iter().enumerate() {
                    self.put_ground_from(
                        world,
                        stack,
                        &credited,
                        &location,
                        policy,
                        GroundProducer::NpcLoot {
                            spawn: strike.target.clone(),
                            life: strike.life,
                            instance: strike.instance.clone(),
                            ordinal: u32::try_from(ordinal)
                                .map_err(|_| invalid_state("Loot ordinal overflow."))?,
                        },
                    )?;
                }
            }
            let respawn = runtime::duration(definition.respawn.require()?, rng)?;
            let ready = runtime::deadline(world.tick, respawn)?;
            let entity = runtime::entity_mut(world, strike.instance.as_ref(), strike.target)?;
            if entity.runtime.loot_resolved {
                return Err(invalid_state("NPC life was already resolved."));
            }
            entity.runtime.loot_resolved = true;
            entity.available_at_tick = ready;
            entity.runtime.retaliation_target = None;
            entity.runtime.kill = Some(KillResolution {
                life: strike.life,
                at_tick: now,
                credited: credited.clone(),
                method,
                npc: npc.clone(),
            });
            events.push(GameEvent::NpcKilled {
                target: strike.target.clone(),
                npc: npc.clone(),
                life: strike.life,
                method: if credited == character.actor_id {
                    method
                } else {
                    strike.style.method
                },
                credited: credited == character.actor_id,
                tile,
            });
            if character.runtime.combat.target.as_ref() == Some(strike.target) {
                runtime::interrupt(character)?;
            }
        }
        Ok(events)
    }

    pub(crate) fn loot(
        &self,
        world: &WorldState,
        owner: &CharacterState,
        pools: &[LootPool],
        rng: &mut impl RandomSource,
    ) -> GameResult<Vec<ItemStack>> {
        let mut totals = BTreeMap::<ItemId, u32>::new();
        self.loot_into(world, owner, pools, rng, &mut totals, 0)?;
        totals
            .into_iter()
            .map(|(item, amount)| {
                Ok(ItemStack {
                    item,
                    quantity: Quantity::new(amount)?,
                    instance: None,
                })
            })
            .collect()
    }

    fn loot_into(
        &self,
        world: &WorldState,
        owner: &CharacterState,
        pools: &[LootPool],
        rng: &mut impl RandomSource,
        totals: &mut BTreeMap<ItemId, u32>,
        depth: usize,
    ) -> GameResult<()> {
        if depth > 64 {
            return Err(invalid_content("Loot nesting is too deep."));
        }
        for pool in pools {
            let items = match pool {
                LootPool::Guaranteed { items } => Some(items),
                LootPool::Independent { chance, items } => {
                    if ratio_roll(chance, rng)? {
                        Some(items)
                    } else {
                        None
                    }
                }
                LootPool::Exclusive {
                    total_weight,
                    entries,
                } => {
                    let mut roll = random::draw(rng, *total_weight)?;
                    let mut selected = None;
                    for entry in entries {
                        if roll < entry.weight {
                            selected = Some(&entry.items);
                            break;
                        }
                        roll -= entry.weight;
                    }
                    Some(selected.ok_or_else(|| {
                        invalid_content("Exclusive loot weights do not cover the roll.")
                    })?)
                }
                LootPool::Conditional { guard, pools } => {
                    if self.guard(world, owner, guard, None)? {
                        self.loot_into(world, owner, pools, rng, totals, depth + 1)?;
                    }
                    None
                }
                LootPool::Unresolved { reason, .. } => return Err(unavailable(reason.clone())),
            };
            if let Some(items) = items {
                for entry in items {
                    let min = entry.minimum.get();
                    let max = entry.maximum.get();
                    let count = if min == max {
                        min
                    } else {
                        min.checked_add(random::draw(
                            rng,
                            max.checked_sub(min)
                                .and_then(|n| n.checked_add(1))
                                .ok_or_else(|| invalid_content("Invalid loot quantity range."))?,
                        )?)
                        .ok_or_else(|| invalid_content("Loot quantity overflow."))?
                    };
                    let total = totals.entry(entry.item.clone()).or_default();
                    *total = total
                        .checked_add(count)
                        .filter(|total| *total <= MAX_STACK_QUANTITY)
                        .ok_or_else(|| invalid_content("Combined loot overflows an item stack."))?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn kill_winner(
        &self,
        entity: &EntityState,
        policy: &KillCreditPolicy,
    ) -> GameResult<Option<ActorId>> {
        Ok(entity
            .runtime
            .contributions
            .iter()
            .min_by(|(actor_a, a), (actor_b, b)| {
                b.damage
                    .cmp(&a.damage)
                    .then_with(|| match policy {
                        KillCreditPolicy::MostDamageThenFirstContributor => {
                            (a.first_hit_tick, a.first_hit_order)
                                .cmp(&(b.first_hit_tick, b.first_hit_order))
                        }
                        KillCreditPolicy::MostDamageThenLastContributor => {
                            (b.last_hit_tick, b.last_hit_order)
                                .cmp(&(a.last_hit_tick, a.last_hit_order))
                        }
                    })
                    .then_with(|| actor_a.cmp(actor_b))
            })
            .map(|(actor, _)| actor.clone()))
    }

    pub(crate) fn advance_projectiles(
        &self,
        world: &mut WorldState,
        rng: &mut impl RandomSource,
    ) -> GameResult<Vec<ActorEvent>> {
        let pending = world.runtime.projectiles.clone();
        let mut events = Vec::new();
        for projectile in pending {
            if projectile.launched_at_tick > world.tick {
                continue;
            }
            let definition = self
                .content
                .mechanics
                .projectiles
                .get(&projectile.definition)
                .ok_or_else(|| unknown("Unknown pending projectile."))?;
            let timing = definition.timing.require()?;
            let due = if timing.damage_on_launch {
                projectile.launched_at_tick == world.tick
            } else {
                projectile.impacts_at_tick <= world.tick
            };
            let launch = projectile.launched_at_tick == world.tick;
            if !due && !launch {
                continue;
            }
            let Combatant::Player { actor } = &projectile.source else {
                return Err(unavailable(
                    "NPC projectile source needs a bound outgoing style.",
                ));
            };
            let Combatant::Npc {
                spawn,
                life,
                instance,
            } = &projectile.target
            else {
                return Err(invalid_content("M1 projectile must target a source NPC."));
            };
            let mut character = world.characters.remove(actor).ok_or_else(|| {
                unavailable("Pending projectile owner must remain loaded until resolution.")
            })?;
            let before = character.clone();
            let style = self
                .content
                .mechanics
                .combat_styles
                .get(&projectile.style)
                .ok_or_else(|| unknown("Unknown projectile style."))?;
            let mut own = Vec::new();
            if launch && let Some(spell) = &projectile.spell {
                let definition = self
                    .content
                    .mechanics
                    .spells
                    .get(spell)
                    .ok_or_else(|| unknown("Unknown pending spell."))?;
                own.extend(self.award_xp(&mut character, &definition.launch_xp)?);
            }
            if due {
                let mut outcome = projectile.outcome;
                if timing.recheck_target_on_impact {
                    let mut perspective = character.clone();
                    perspective.runtime.instance = instance.clone();
                    match self.resolve_shape(
                        world,
                        &perspective,
                        &WorldTarget::Spawn {
                            spawn: spawn.clone(),
                        },
                        true,
                    ) {
                        Ok(shape)
                            if projectile.target_snapshot.as_ref().is_none_or(|snapshot| {
                                shape.npc.as_ref() == Some(&snapshot.npc)
                            }) => {}
                        Ok(_) => outcome = CombatOutcome::Invalidated,
                        Err(error)
                            if matches!(
                                error.code,
                                GameErrorCode::Busy
                                    | GameErrorCode::NotOwned
                                    | GameErrorCode::OutOfReach
                                    | GameErrorCode::UnknownContent
                                    | GameErrorCode::InvalidInput
                            ) =>
                        {
                            outcome = CombatOutcome::Invalidated
                        }
                        Err(error) => return Err(error),
                    }
                }
                own.extend(self.apply_strike(
                    world,
                    &mut character,
                    &Strike {
                        target: spawn,
                        life: *life,
                        style,
                        spell: projectile.spell.as_ref(),
                        outcome,
                        damage: projectile.damage,
                        instance: instance.clone(),
                        snapshot: projectile.target_snapshot.as_ref(),
                    },
                    rng,
                )?);
            }
            events.extend(self.dispatch_kill_credit(world, instance.as_ref(), &own, rng)?);
            self.progress(world, &mut character, &before, &mut own, rng)?;
            self.check_reward_atomicity(&before, &character)?;
            world.characters.insert(actor.clone(), character);
            events.extend(tag(actor, own));
        }
        world
            .runtime
            .projectiles
            .retain(|projectile| projectile.impacts_at_tick > world.tick);
        Ok(events)
    }

    pub(crate) fn in_combat(
        &self,
        world: &WorldState,
        character: &CharacterState,
    ) -> GameResult<bool> {
        self.combat_locked(world, character, CombatLock::State)
    }

    pub(crate) fn combat_locked(
        &self,
        world: &WorldState,
        character: &CharacterState,
        lock: CombatLock,
    ) -> GameResult<bool> {
        let entities = match &character.runtime.instance {
            Some(id) => {
                &world
                    .runtime
                    .instances
                    .get(id)
                    .ok_or_else(|| invalid_state("Unknown combat instance."))?
                    .entities
            }
            None => &world.entities,
        };
        let active = entities.values().any(|entity| {
            entity.hitpoints > 0
                && entity.runtime.retaliation_target.as_ref() == Some(&character.actor_id)
        }) || matches!(
            character.activity,
            Activity::Fighting { .. } | Activity::Casting { .. }
        );
        if active {
            return Ok(true);
        }
        if let Some(policy) = &self.content.mechanics.player_combat {
            if let Some(last) = character.runtime.combat.last_combat_tick {
                let policy = policy.engagement.require()?;
                let delay = match lock {
                    CombatLock::State => policy.combat_state_ticks,
                    CombatLock::Logout => policy.logout_lock_ticks,
                    CombatLock::Travel => policy.travel_lock_ticks,
                };
                return Ok(world.tick < runtime::deadline(last, u64::from(delay))?);
            }
            return Ok(false);
        }
        Ok(character.runtime.combat.target.is_some())
    }

    fn attack_eligible(
        &self,
        world: &WorldState,
        character: &CharacterState,
        npc: &NpcCombatMechanics,
        style: &CombatStyleId,
        method: AttackMethod,
    ) -> GameResult<()> {
        for rule in npc.eligibility.require()? {
            if rule.method == method
                && rule.style.as_ref().is_none_or(|id| id == style)
                && self.guard(world, character, &rule.guard, None)?
            {
                return Ok(());
            }
        }
        Err(GameError::new(
            GameErrorCode::RequirementNotMet,
            "Source target rules do not permit this requested attack method/style.",
        ))
    }

    fn interrupt_travel_for_combat(
        &self,
        character: &mut CharacterState,
    ) -> GameResult<Vec<GameEvent>> {
        if character.runtime.pending_travel.is_some() {
            self.interrupt_travel(character, InterruptionCause::Combat)
        } else {
            Ok(vec![])
        }
    }
}

pub(crate) fn npc_stat(combat: &NpcCombatDefinition, stat: NpcCombatStat) -> u16 {
    match stat {
        NpcCombatStat::Attack => combat.attack,
        NpcCombatStat::Strength => combat.strength,
        NpcCombatStat::Defence => combat.defence,
        NpcCombatStat::Ranged => combat.ranged,
        NpcCombatStat::Magic => combat.magic,
    }
}

pub(crate) fn melee_adjacent(actor: Tile, target: Tile, width: u8, height: u8) -> bool {
    if actor.plane() != target.plane() {
        return false;
    }
    let (x, y) = (u32::from(actor.x()), u32::from(actor.y()));
    let (left, bottom) = (u32::from(target.x()), u32::from(target.y()));
    let (right, top) = (left + u32::from(width), bottom + u32::from(height));
    (y >= bottom && y < top && (x + 1 == left || x == right))
        || (x >= left && x < right && (y + 1 == bottom || y == top))
}

fn attributed_method(
    entity: &EntityState,
    actor: &ActorId,
    finishing: AttackMethod,
    policy: &KillMethodPolicy,
) -> GameResult<AttackMethod> {
    if *policy == KillMethodPolicy::FinishingAttack {
        return Ok(finishing);
    }
    let contribution = entity
        .runtime
        .contributions
        .get(actor)
        .ok_or_else(|| invalid_state("Credited actor has no contribution."))?;
    if !contribution.methods_complete {
        return Err(unavailable(
            "Legacy contribution method history needs explicit migration for this attribution policy.",
        ));
    }
    contribution
        .methods
        .iter()
        .min_by(|(method_a, a), (method_b, b)| {
            let damage = match policy {
                KillMethodPolicy::MostDamageThenFirstMethod
                | KillMethodPolicy::MostDamageThenLastMethod => b.damage.cmp(&a.damage),
                _ => std::cmp::Ordering::Equal,
            };
            damage
                .then_with(|| match policy {
                    KillMethodPolicy::LastContributingMethod
                    | KillMethodPolicy::MostDamageThenLastMethod => {
                        (b.last_hit_tick, b.last_hit_order)
                            .cmp(&(a.last_hit_tick, a.last_hit_order))
                    }
                    _ => (a.first_hit_tick, a.first_hit_order)
                        .cmp(&(b.first_hit_tick, b.first_hit_order)),
                })
                .then_with(|| method_a.cmp(method_b))
        })
        .map(|(method, _)| *method)
        .ok_or_else(|| invalid_state("Complete contribution has no method history."))
}

fn ratio_roll(ratio: &Ratio, rng: &mut impl RandomSource) -> GameResult<bool> {
    if ratio.denominator == 0 || ratio.numerator > ratio.denominator {
        return Err(invalid_content("Invalid probability ratio."));
    }
    if ratio.numerator == 0 || ratio.numerator == ratio.denominator {
        return Ok(ratio.numerator != 0);
    }
    Ok(random::draw(rng, ratio.denominator)? < ratio.numerator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{self as s, v2 as v};

    #[test]
    fn exclusive_loot_uses_one_weighted_result_with_guaranteed_and_independent_pools() {
        let (engine, world) = s::setup(v::content());
        let entry = |name| LootEntry {
            item: s::item(name),
            minimum: s::quantity(1),
            maximum: s::quantity(1),
        };
        let pools = vec![
            LootPool::Guaranteed {
                items: vec![entry("egg")],
            },
            LootPool::Exclusive {
                total_weight: 4,
                entries: vec![
                    WeightedLoot {
                        weight: 1,
                        items: vec![],
                    },
                    WeightedLoot {
                        weight: 2,
                        items: vec![entry("coins")],
                    },
                    WeightedLoot {
                        weight: 1,
                        items: vec![entry("hammer")],
                    },
                ],
            },
            LootPool::Independent {
                chance: Ratio {
                    numerator: 1,
                    denominator: 5,
                },
                items: vec![entry("arrow")],
            },
        ];
        for (roll, primary) in [
            (0, None),
            (1, Some("coins")),
            (2, Some("coins")),
            (3, Some("hammer")),
        ] {
            let loot = engine
                .loot(
                    &world,
                    s::state(&world),
                    &pools,
                    &mut v::Rolls::new(&[roll, 0]),
                )
                .unwrap();
            assert!(loot.contains(&s::stack("egg", 1)));
            assert!(loot.contains(&s::stack("arrow", 1)));
            assert_eq!(loot.len(), if primary.is_some() { 3 } else { 2 });
            if let Some(name) = primary {
                assert!(loot.contains(&s::stack(name, 1)));
            }
        }
    }

    #[test]
    fn conditional_loot_reads_authoritative_world_membership_and_unresolved_is_not_zero() {
        let (engine, mut world) = s::setup(v::content());
        let pools = vec![LootPool::Conditional {
            guard: Guard::MembersWorld,
            pools: vec![LootPool::Unresolved {
                reason: "unobserved member supplement".into(),
                source: s::source(),
            }],
        }];
        assert!(
            engine
                .loot(&world, s::state(&world), &pools, &mut s::NeverDraw)
                .unwrap()
                .is_empty()
        );
        world.runtime.members = Some(true);
        assert_eq!(
            engine
                .loot(&world, s::state(&world), &pools, &mut s::NeverDraw)
                .unwrap_err()
                .code,
            GameErrorCode::Unavailable
        );
    }

    #[test]
    fn aggregate_loot_overflow_fails_instead_of_truncating_guaranteed_items() {
        let (engine, world) = s::setup(v::content());
        let pools = vec![LootPool::Guaranteed {
            items: vec![
                LootEntry {
                    item: s::item("coins"),
                    minimum: s::quantity(MAX_STACK_QUANTITY),
                    maximum: s::quantity(MAX_STACK_QUANTITY),
                },
                LootEntry {
                    item: s::item("coins"),
                    minimum: s::quantity(1),
                    maximum: s::quantity(1),
                },
            ],
        }];
        assert_eq!(
            engine
                .loot(&world, s::state(&world), &pools, &mut s::NeverDraw)
                .unwrap_err()
                .code,
            GameErrorCode::InvalidContent
        );
    }
}
