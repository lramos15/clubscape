use clubscape_game_types::*;
use clubscape_simulation::inventory;

use crate::{
    RandomSource, WorldEngine, invalid_content, invalid_state, progression::EffectFrame, random,
    runtime, unavailable, unknown,
};

impl WorldEngine {
    pub(crate) fn advance_activity(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        rng: &mut impl RandomSource,
    ) -> GameResult<Vec<GameEvent>> {
        if character.runtime.pending_fire.is_some() {
            return self.advance_fire(world, character, rng);
        }
        match character.activity.clone() {
            Activity::Idle => Ok(vec![]),
            Activity::Walking { mut path, running } => {
                self.authorize(character, &["walk".into()])?;
                let requested = running || character.runtime.settings.run_enabled == Some(true);
                let running = requested && character.run_energy > 0 && path.len() > 1;
                let cost = if running {
                    self.run_cost(character)?
                } else {
                    0
                };
                let map = self.collision_for(world, character.runtime.instance.as_ref())?;
                let events = map.step_path(&mut character.tile, &mut path, running)?;
                character.region = self
                    .regions_by_tile
                    .get(&character.tile)
                    .ok_or_else(|| unknown("Moved tile has no region."))?
                    .clone();
                if events.len() == 2 {
                    character.run_energy = character.run_energy.saturating_sub(cost);
                    if character.run_energy == 0
                        && self
                            .content
                            .mechanics
                            .run
                            .as_ref()
                            .is_some_and(|policy| policy.disable_on_exhaustion)
                    {
                        character.runtime.settings.run_enabled = Some(false);
                    }
                }
                character.activity = if path.is_empty() {
                    Activity::Idle
                } else {
                    Activity::Walking {
                        path,
                        running: requested && character.run_energy > 0,
                    }
                };
                Ok(events)
            }
            Activity::Gathering { target, next_tick } => {
                if world.tick < next_tick {
                    Ok(vec![])
                } else {
                    self.gather(world, character, &target, rng)
                }
            }
            Activity::Producing {
                recipe,
                target,
                remaining,
                next_tick,
            } => {
                if world.tick < next_tick {
                    Ok(vec![])
                } else {
                    self.produce(
                        world,
                        character,
                        &recipe,
                        target.map(|spawn| WorldTarget::Spawn { spawn }),
                        remaining,
                        rng,
                    )
                }
            }
            Activity::ProducingAt {
                recipe,
                target,
                remaining,
                next_tick,
            } => {
                if world.tick < next_tick {
                    Ok(vec![])
                } else {
                    self.produce(world, character, &recipe, target, remaining, rng)
                }
            }
            Activity::Fighting {
                target,
                style,
                next_tick,
            } => {
                if world.tick < next_tick.max(character.runtime.combat.attack_ready) {
                    return Ok(vec![]);
                }
                self.player_attack(
                    world,
                    character,
                    &target,
                    &CombatStyleId::new(style)?,
                    None,
                    rng,
                )
            }
            Activity::Casting {
                spell,
                target,
                completes_at,
            } => {
                if world.tick < completes_at {
                    return Ok(vec![]);
                }
                let spell = SpellId::new(spell)?;
                let definition = self
                    .content
                    .mechanics
                    .spells
                    .get(&spell)
                    .ok_or_else(|| unknown("Unknown pending spell."))?;
                let SpellAction::Combat { style, .. } = &definition.action else {
                    return Err(invalid_state("Teleport was stored as a combat cast."));
                };
                let target =
                    target.ok_or_else(|| invalid_state("Pending combat spell has no target."))?;
                character.activity = Activity::Idle;
                self.player_attack(world, character, &target, style, Some(&spell), rng)
            }
        }
    }

    pub(crate) fn check_gather(
        &self,
        character: &CharacterState,
        rule: &GatherRule,
    ) -> GameResult<()> {
        let basis = rule
            .mechanics
            .as_ref()
            .map_or(SkillLevelBasis::Current, |m| m.levels.basis);
        self.requirements(
            character,
            &[SkillRequirement {
                skill: rule.skill.clone(),
                level: rule.required_level,
                basis,
            }],
        )?;
        if let Some(mechanics) = &rule.mechanics {
            let level = self.level(character, &rule.skill, mechanics.levels.basis)?;
            if !(mechanics.levels.minimum..=mechanics.levels.maximum).contains(&level) {
                return Err(unavailable(
                    "Gathering level is outside the source method domain.",
                ));
            }
        }
        if !rule.tools.is_empty()
            && !rule
                .tools
                .iter()
                .map(|item| {
                    self.owned_count(character, item, &OwnershipScope::InventoryAndEquipment)
                })
                .collect::<GameResult<Vec<_>>>()?
                .iter()
                .any(|count| *count > 0)
        {
            return Err(GameError::new(
                GameErrorCode::RequirementNotMet,
                "No usable source gathering tool.",
            ));
        }
        self.gather_delay(character, rule, false)?;
        let mut inventory = character.inventory.clone();
        inventory::add(&mut inventory, &self.content.items, &rule.output)?;
        if let Some(mechanics) = &rule.mechanics {
            for alternative in &mechanics.alternatives {
                if self.level(
                    character,
                    &alternative.requirement.skill,
                    alternative.requirement.basis,
                )? >= alternative.requirement.level
                {
                    inventory::add(
                        &mut character.inventory.clone(),
                        &self.content.items,
                        &alternative.output,
                    )?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn gather_delay(
        &self,
        character: &CharacterState,
        rule: &GatherRule,
        repeat: bool,
    ) -> GameResult<u64> {
        let Some(mechanics) = &rule.mechanics else {
            return runtime::legacy_ticks(rule.attempt_ticks);
        };
        if mechanics.tool_cadences.is_empty() {
            return runtime::cadence(&mechanics.cadence, false, repeat);
        }
        let mut best = None;
        for tool in &mechanics.tool_cadences {
            if self.owned_count(character, &tool.tool, &tool.location)? > 0 {
                let delay = runtime::cadence(&tool.cadence, false, repeat)?;
                best = Some(best.map_or(delay, |prior: u64| prior.min(delay)));
            }
        }
        best.ok_or_else(|| {
            GameError::new(
                GameErrorCode::RequirementNotMet,
                "No tool in the source-required location.",
            )
        })
    }

    fn gather(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        target: &SpawnId,
        rng: &mut impl RandomSource,
    ) -> GameResult<Vec<GameEvent>> {
        self.authorize(character, &["gather".into(), format!("gather:{target}")])?;
        let index = runtime::schedule(character)?
            .gather_interaction
            .ok_or_else(|| invalid_state("Gathering interaction is missing."))?;
        let interaction = runtime::interaction(&self.content, target, index as usize - 1)?;
        let InteractionAction::Gather { rule } = &interaction.action else {
            return Err(invalid_state("Persisted gathering changed kind."));
        };
        self.require_target(world, character, target, interaction)?;
        self.check_gather(character, rule)?;
        let mut output = None;
        if let Some(mechanics) = &rule.mechanics {
            for alternative in &mechanics.alternatives {
                if self.level(
                    character,
                    &alternative.requirement.skill,
                    alternative.requirement.basis,
                )? >= alternative.requirement.level
                {
                    let level = self.chance_level(
                        character,
                        &alternative.requirement.skill,
                        &alternative.chance,
                    )?;
                    if random::roll(&alternative.chance, level, rng)? {
                        output = Some((alternative.output.clone(), alternative.xp_tenths));
                        break;
                    }
                }
            }
        }
        if output.is_none() {
            let level = self.chance_level(character, &rule.skill, &rule.success)?;
            if random::roll(&rule.success, level, rng)? {
                output = Some((rule.output.clone(), rule.xp_tenths));
            }
        }
        let next_tick = runtime::deadline(world.tick, self.gather_delay(character, rule, true)?)?;
        character.activity = Activity::Gathering {
            target: target.clone(),
            next_tick,
        };
        if let Some(mechanics) = &rule.mechanics {
            character
                .runtime
                .action_cooldowns
                .insert(mechanics.method.clone(), next_tick);
        }
        let Some((stack, xp)) = output else {
            return Ok(vec![]);
        };
        inventory::add(&mut character.inventory, &self.content.items, &stack)?;
        let mut events = vec![GameEvent::Gathered {
            target: target.clone(),
            stack,
        }];
        events.extend(self.award_xp(
            character,
            &[XpReward {
                skill: rule.skill.clone(),
                amount_tenths: xp,
            }],
        )?);
        if random::roll(
            &rule.depletion,
            self.chance_level(character, &rule.skill, &rule.depletion)?,
            rng,
        )? {
            let delay = match &rule.mechanics {
                Some(mechanics) => runtime::duration(mechanics.respawn.require()?, rng)?,
                None => runtime::legacy_respawn(rule.respawn_ticks)?,
            };
            runtime::entity_mut(world, character.runtime.instance.as_ref(), target)?
                .available_at_tick = runtime::deadline(world.tick, delay)?;
            character.activity = Activity::Idle;
            runtime::schedule_mut(character)?.gather_interaction = None;
        }
        Ok(events)
    }

    pub(crate) fn chance_level(
        &self,
        character: &CharacterState,
        skill: &SkillId,
        chance: &ChanceRule,
    ) -> GameResult<u16> {
        match chance.domain {
            ChanceDomain::Constant => Ok(1),
            ChanceDomain::Skill { levels } => self.level(character, skill, levels.basis),
        }
    }

    pub(crate) fn start_production(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        recipe_id: &RecipeId,
        target: Option<WorldTarget>,
        remaining: u32,
    ) -> GameResult<()> {
        self.authorize(
            character,
            &["produce".into(), format!("produce:{recipe_id}")],
        )?;
        let recipe = self
            .content
            .recipes
            .get(recipe_id)
            .ok_or_else(|| unknown("Unknown recipe."))?;
        if remaining == 0 {
            return Err(invalid_state("Production quantity is zero."));
        }
        self.check_recipe_target(world, character, recipe, target.as_ref())?;
        self.check_recipe(world, character, recipe, false)?;
        let delay = self.recipe_delay(recipe, remaining == 1, false)?;
        if remaining > 1 {
            self.recipe_delay(recipe, false, true)?;
        }
        let mut next_tick = runtime::deadline(world.tick, delay)?;
        if let Some(mechanics) = &recipe.mechanics {
            next_tick = next_tick.max(
                character
                    .runtime
                    .action_cooldowns
                    .get(&mechanics.method)
                    .copied()
                    .unwrap_or(0),
            );
        }
        runtime::interrupt(character)?;
        if let Some(mechanics) = &recipe.mechanics
            && let RecipeLifecycle::Firemaking {
                ground_input, fire, ..
            } = &mechanics.lifecycle
        {
            if remaining != 1 || target.is_some() {
                return Err(invalid_state(
                    "Firemaking starts one owned ground input at the actor tile.",
                ));
            }
            self.validate_temporary_placement(world, character, fire)?;
            let definition = self
                .content
                .mechanics
                .temporary_objects
                .get(fire)
                .ok_or_else(|| unknown("Unknown fire."))?;
            let input = recipe
                .inputs
                .iter()
                .find(|input| &input.item == ground_input)
                .ok_or_else(|| invalid_content("Missing fire ground input."))?;
            inventory::remove(&mut character.inventory, &self.content.items, input)?;
            let ground_item = self.put_ground(
                world,
                input.clone(),
                &character.actor_id,
                &runtime::location(character),
                &definition.ground_policy,
            )?;
            character.runtime.pending_fire = Some(PendingFire {
                recipe: recipe.id.clone(),
                ground_item,
                tile: character.tile,
                next_attempt_tick: next_tick,
            });
        } else {
            self.check_outcomes_fit(character, recipe)?;
            character.activity =
                production_activity(recipe.id.clone(), target, remaining, next_tick);
        }
        if let Some(mechanics) = &recipe.mechanics {
            character
                .runtime
                .action_cooldowns
                .insert(mechanics.method.clone(), next_tick);
        }
        Ok(())
    }

    pub(crate) fn check_recipe_target(
        &self,
        world: &WorldState,
        character: &CharacterState,
        recipe: &RecipeDefinition,
        target: Option<&WorldTarget>,
    ) -> GameResult<()> {
        let Some(target) = target else {
            return if recipe.target_objects.is_empty() {
                Ok(())
            } else {
                Err(GameError::new(
                    GameErrorCode::OutOfReach,
                    "Recipe needs a source facility.",
                ))
            };
        };
        let shape = self.target_shape(world, character, target)?;
        if !recipe.target_objects.is_empty()
            && !shape
                .object
                .as_ref()
                .is_some_and(|id| recipe.target_objects.contains(id))
        {
            return Err(GameError::new(
                GameErrorCode::RequirementNotMet,
                "Wrong source facility.",
            ));
        }
        let mut refusal = None;
        for interaction in self.target_interactions(world, target)? {
            if matches!(&interaction.action, InteractionAction::Production { recipes } if recipes.contains(&recipe.id))
            {
                match self.require_world_target(world, character, target, interaction) {
                    Ok(()) => return Ok(()),
                    Err(error) if crate::is_interruption(&error.code) => refusal = Some(error),
                    Err(error) => return Err(error),
                }
            }
        }
        Err(refusal.unwrap_or_else(|| {
            GameError::new(
                GameErrorCode::RequirementNotMet,
                "Facility does not offer the recipe.",
            )
        }))
    }

    fn recipe_delay(
        &self,
        recipe: &RecipeDefinition,
        single: bool,
        repeat: bool,
    ) -> GameResult<u64> {
        match &recipe.mechanics {
            Some(mechanics) => runtime::cadence(&mechanics.cadence, single, repeat),
            None => runtime::legacy_ticks(recipe.ticks),
        }
    }

    fn check_recipe(
        &self,
        world: &WorldState,
        character: &CharacterState,
        recipe: &RecipeDefinition,
        ground: bool,
    ) -> GameResult<()> {
        self.requirements(character, &recipe.requirements)?;
        let ownership = recipe
            .mechanics
            .as_ref()
            .map_or(&OwnershipScope::InventoryAndEquipment, |m| {
                &m.tool_ownership
            });
        if let Some(mechanics) = &recipe.mechanics {
            self.require_guard(world, character, &mechanics.guard)?;
        }
        for tool in &recipe.tools {
            if self.owned_count(character, tool, ownership)? == 0 {
                return Err(GameError::new(
                    GameErrorCode::RequirementNotMet,
                    format!("Missing required unconsumed tool {tool}."),
                ));
            }
        }
        let mut draft = character.inventory.clone();
        let inputs: Vec<_> = recipe.inputs.iter().filter(|input| !(ground && recipe.mechanics.as_ref().is_some_and(|mechanics|
            matches!(&mechanics.lifecycle, RecipeLifecycle::Firemaking { ground_input, .. } if ground_input == &input.item)))).cloned().collect();
        inventory::remove_batch(&mut draft, &self.content.items, &inputs)
    }

    fn recipe_level(
        &self,
        character: &CharacterState,
        recipe: &RecipeDefinition,
    ) -> GameResult<u16> {
        if matches!(recipe.success.domain, ChanceDomain::Constant) {
            return Ok(1);
        }
        if let Some(mechanics) = &recipe.mechanics {
            return self.chance_level(
                character,
                mechanics
                    .chance_skill
                    .as_ref()
                    .ok_or_else(|| invalid_content("Recipe chance skill is missing."))?,
                &recipe.success,
            );
        }
        let mut skill = None;
        for requirement in &recipe.requirements {
            if skill.is_some_and(|prior| prior != &requirement.skill) {
                return Err(unavailable(
                    "Legacy recipe chance has multiple skill candidates.",
                ));
            }
            skill = Some(&requirement.skill);
        }
        self.chance_level(
            character,
            skill.ok_or_else(|| unavailable("Legacy recipe chance skill is absent."))?,
            &recipe.success,
        )
    }

    fn check_outcomes_fit(
        &self,
        character: &CharacterState,
        recipe: &RecipeDefinition,
    ) -> GameResult<()> {
        let count = recipe
            .success
            .numerator(self.recipe_level(character, recipe)?)?;
        if count > 0 {
            self.recipe_inventory(&mut character.inventory.clone(), recipe, true)?;
        }
        if count < recipe.success.denominator {
            self.recipe_inventory(&mut character.inventory.clone(), recipe, false)?;
        }
        Ok(())
    }

    fn recipe_inventory(
        &self,
        container: &mut Inventory,
        recipe: &RecipeDefinition,
        success: bool,
    ) -> GameResult<()> {
        let outputs = if success {
            &recipe.outputs
        } else {
            &recipe.failed_outputs
        };
        let operations: Vec<_> = recipe
            .inputs
            .iter()
            .cloned()
            .map(inventory::InventoryOperation::Remove)
            .chain(
                outputs
                    .iter()
                    .cloned()
                    .map(inventory::InventoryOperation::Add),
            )
            .collect();
        inventory::apply_operations(container, &self.content.items, &operations)
    }

    fn produce(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        id: &RecipeId,
        target: Option<WorldTarget>,
        remaining: u32,
        rng: &mut impl RandomSource,
    ) -> GameResult<Vec<GameEvent>> {
        self.authorize(character, &["produce".into(), format!("produce:{id}")])?;
        let recipe = self
            .content
            .recipes
            .get(id)
            .ok_or_else(|| unknown("Unknown pending recipe."))?;
        if remaining == 0 {
            return Err(invalid_state("Pending production has zero work."));
        }
        self.check_recipe_target(world, character, recipe, target.as_ref())?;
        self.check_recipe(world, character, recipe, false)?;
        self.check_outcomes_fit(character, recipe)?;
        let success = random::roll(&recipe.success, self.recipe_level(character, recipe)?, rng)?;
        self.recipe_inventory(&mut character.inventory, recipe, success)?;
        let outputs = if success {
            recipe.outputs.clone()
        } else {
            recipe.failed_outputs.clone()
        };
        let event = match &recipe.mechanics {
            Some(mechanics) => GameEvent::ProductionResolved {
                recipe: id.clone(),
                method: mechanics.method.clone(),
                facility: target.clone(),
                outcome: if success {
                    ProductionOutcome::Success
                } else {
                    ProductionOutcome::Failure
                },
                outputs,
            },
            None => GameEvent::Produced {
                recipe: id.clone(),
                outputs,
            },
        };
        let mut frame = EffectFrame {
            trigger: Some(event.clone()),
            ..EffectFrame::default()
        };
        frame.events.push(event);
        let original_location = runtime::location(character);
        if success {
            frame.events.extend(self.award_xp(character, &recipe.xp)?);
        } else if let Some(mechanics) = &recipe.mechanics {
            frame
                .events
                .extend(self.award_xp(character, &mechanics.failed_xp)?);
        }
        if let Some(mechanics) = &recipe.mechanics {
            self.effects(
                world,
                character,
                if success {
                    &mechanics.success_effects
                } else {
                    &mechanics.failure_effects
                },
                rng,
                &mut frame,
            )?;
        }
        let finished = remaining == 1
            || character.runtime.pending_travel.is_some()
            || runtime::location(character) != original_location;
        let next_tick = if finished {
            world.tick
        } else {
            runtime::deadline(world.tick, self.recipe_delay(recipe, false, true)?)?
        };
        if let Some(mechanics) = &recipe.mechanics {
            character
                .runtime
                .action_cooldowns
                .insert(mechanics.method.clone(), next_tick);
        }
        character.activity = if finished {
            Activity::Idle
        } else {
            production_activity(id.clone(), target, remaining - 1, next_tick)
        };
        Ok(frame.events)
    }

    fn advance_fire(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        rng: &mut impl RandomSource,
    ) -> GameResult<Vec<GameEvent>> {
        let pending = character
            .runtime
            .pending_fire
            .clone()
            .ok_or_else(|| invalid_state("Missing pending fire."))?;
        if world.tick < pending.next_attempt_tick {
            return Ok(vec![]);
        }
        self.authorize(
            character,
            &["produce".into(), format!("produce:{}", pending.recipe)],
        )?;
        let recipe = self
            .content
            .recipes
            .get(&pending.recipe)
            .ok_or_else(|| unknown("Unknown fire recipe."))?;
        let mechanics = recipe
            .mechanics
            .as_ref()
            .ok_or_else(|| invalid_state("Fire lifecycle vanished."))?;
        let RecipeLifecycle::Firemaking {
            ground_input,
            fire,
            step_priority,
            retain_ground_input_on_failure,
        } = &mechanics.lifecycle
        else {
            return Err(invalid_state("Fire lifecycle changed."));
        };
        if character.tile != pending.tile {
            return Err(GameError::new(
                GameErrorCode::OutOfReach,
                "Moved away from the placed log.",
            ));
        }
        let index = world
            .ground_items
            .iter()
            .position(|ground| {
                ground.id == pending.ground_item
                    && ground.tile == pending.tile
                    && ground.instance == character.runtime.instance
                    && ground.owner.as_ref() == Some(&character.actor_id)
                    && &ground.stack.item == ground_input
            })
            .ok_or_else(|| {
                GameError::new(
                    GameErrorCode::NotOwned,
                    "Placed firemaking input is no longer owned.",
                )
            })?;
        self.check_recipe(world, character, recipe, true)?;
        self.validate_temporary_placement(world, character, fire)?;
        let success = random::roll(&recipe.success, self.recipe_level(character, recipe)?, rng)?;
        let event = GameEvent::ProductionResolved {
            recipe: recipe.id.clone(),
            method: mechanics.method.clone(),
            facility: None,
            outcome: if success {
                ProductionOutcome::Success
            } else {
                ProductionOutcome::Failure
            },
            outputs: if success {
                recipe.outputs.clone()
            } else {
                recipe.failed_outputs.clone()
            },
        };
        let mut frame = EffectFrame {
            trigger: Some(event.clone()),
            ..EffectFrame::default()
        };
        frame.events.push(event);
        if success {
            world.ground_items.remove(index);
            let inputs: Vec<_> = recipe
                .inputs
                .iter()
                .filter(|input| &input.item != ground_input)
                .cloned()
                .collect();
            inventory::remove_batch(&mut character.inventory, &self.content.items, &inputs)?;
            inventory::add_batch(
                &mut character.inventory,
                &self.content.items,
                &recipe.outputs,
            )?;
            frame
                .events
                .push(self.create_temporary(world, character, fire, rng)?);
            frame.events.extend(self.award_xp(character, &recipe.xp)?);
            self.effects(
                world,
                character,
                &mechanics.success_effects,
                rng,
                &mut frame,
            )?;
            character.runtime.pending_fire = None;
            let map = self.collision_for(world, character.runtime.instance.as_ref())?;
            for direction in step_priority {
                let (dx, dy) = direction.offset();
                if let Some(tile) = character.tile.offset(dx, dy)
                    && map.can_step(character.tile, tile)
                {
                    character.tile = tile;
                    character.region = self
                        .regions_by_tile
                        .get(&tile)
                        .ok_or_else(|| unknown("Fire step has no region."))?
                        .clone();
                    frame.events.push(GameEvent::Moved { tile });
                    break;
                }
            }
        } else {
            inventory::add_batch(
                &mut character.inventory,
                &self.content.items,
                &recipe.failed_outputs,
            )?;
            frame
                .events
                .extend(self.award_xp(character, &mechanics.failed_xp)?);
            self.effects(
                world,
                character,
                &mechanics.failure_effects,
                rng,
                &mut frame,
            )?;
            if *retain_ground_input_on_failure {
                let next_attempt_tick = runtime::deadline(
                    world.tick,
                    runtime::cadence(&mechanics.cadence, false, true)?,
                )?;
                character.runtime.pending_fire = Some(PendingFire {
                    next_attempt_tick,
                    ..pending
                });
            } else {
                world.ground_items.remove(index);
                character.runtime.pending_fire = None;
            }
        }
        Ok(frame.events)
    }
}

fn production_activity(
    recipe: RecipeId,
    target: Option<WorldTarget>,
    remaining: u32,
    next_tick: u64,
) -> Activity {
    match target {
        Some(WorldTarget::TemporaryObject { .. }) => Activity::ProducingAt {
            recipe,
            target,
            remaining,
            next_tick,
        },
        target => Activity::Producing {
            recipe,
            target: target.map(|target| match target {
                WorldTarget::Spawn { spawn } => spawn,
                _ => unreachable!(),
            }),
            remaining,
            next_tick,
        },
    }
}
