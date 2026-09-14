use clubscape_game_types::*;
use clubscape_simulation::{inventory, skills};

use crate::{
    RandomSource, WorldEngine, actions, invalid_content, invalid_state, random, runtime,
    unavailable, unknown,
};

impl WorldEngine {
    pub(crate) fn advance_activity(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        random: &mut impl RandomSource,
    ) -> GameResult<Vec<GameEvent>> {
        if character.hitpoints == 0 {
            return Err(unavailable(
                "A dead actor requires source-defined death/recovery state.",
            ));
        }
        match character.activity.clone() {
            Activity::Idle => Ok(Vec::new()),
            Activity::Walking { mut path, running } => {
                self.authorize(character, &["walk".into()])?;
                if running {
                    return Err(unavailable(
                        "Persisted running needs bound source weights/energy policy.",
                    ));
                }
                let events = self
                    .collision
                    .step_path(&mut character.tile, &mut path, false)?;
                character.region = self
                    .regions_by_tile
                    .get(&character.tile)
                    .ok_or_else(|| unknown("Walked tile has no source region."))?
                    .clone();
                character.activity = if path.is_empty() {
                    Activity::Idle
                } else {
                    Activity::Walking {
                        path,
                        running: false,
                    }
                };
                Ok(events)
            }
            Activity::Gathering { target, next_tick } => {
                if world.tick < next_tick {
                    return Ok(Vec::new());
                }
                self.gather(world, character, &target, random)
            }
            Activity::Producing {
                recipe,
                target,
                remaining,
                next_tick,
            } => {
                if world.tick < next_tick {
                    return Ok(Vec::new());
                }
                self.produce(
                    world,
                    character,
                    &recipe,
                    target.as_ref(),
                    remaining,
                    random,
                )
            }
            Activity::Fighting { .. } => Err(actions::combat_unbound()),
            Activity::Casting { .. } => Err(unavailable(
                "Persisted spell activity lacks compiled spell/timing rules.",
            )),
        }
    }

    pub(crate) fn check_gather(
        &self,
        character: &CharacterState,
        rule: &GatherRule,
    ) -> GameResult<()> {
        skills::check_requirements(
            &character.skills,
            &self.content.skills,
            &[SkillRequirement {
                skill: rule.skill.clone(),
                level: rule.required_level,
            }],
            skills::LevelBasis::Current,
        )?;
        if !rule.tools.is_empty() {
            let mut has_tool = false;
            for tool in &rule.tools {
                has_tool |= self.has_tool(character, tool)?;
            }
            if !has_tool {
                return Err(GameError::new(
                    GameErrorCode::RequirementNotMet,
                    "No usable gathering tool is carried/equipped.",
                ));
            }
        }
        let mut inventory = character.inventory.clone();
        inventory::add(&mut inventory, &self.content.items, &rule.output)?;
        Ok(())
    }

    fn gather(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        target: &SpawnId,
        random: &mut impl RandomSource,
    ) -> GameResult<Vec<GameEvent>> {
        self.authorize(character, &["gather".into(), format!("gather:{target}")])?;
        let index = runtime::read_counter(character, runtime::GATHER_INTERACTION)?
            .checked_sub(1)
            .ok_or_else(|| invalid_state("Gathering state lacks its chosen source interaction."))?;
        let interaction = runtime::interaction(
            &self.content,
            target,
            usize::try_from(index)
                .map_err(|_| invalid_state("Gather interaction index overflow."))?,
        )?;
        let InteractionAction::Gather { rule } = &interaction.action else {
            return Err(invalid_state(
                "Persisted gathering refers to a different action kind.",
            ));
        };
        self.require_target(world, character, target, interaction)?;
        self.check_gather(character, rule)?;
        let level = character
            .skills
            .get(&rule.skill)
            .ok_or_else(|| invalid_state("Gathering skill state is missing."))?
            .current_level;
        let next_tick = runtime::deadline(world.tick, u64::from(rule.attempt_ticks))?;
        character.activity = Activity::Gathering {
            target: target.clone(),
            next_tick,
        };
        if !random::roll(&rule.success, level, random)? {
            return Ok(Vec::new());
        }
        inventory::add(&mut character.inventory, &self.content.items, &rule.output)?;
        let mut events = vec![GameEvent::Gathered {
            target: target.clone(),
            stack: rule.output.clone(),
        }];
        events.extend(skills::award_character_xp(
            character,
            &self.content,
            &[XpReward {
                skill: rule.skill.clone(),
                amount_tenths: rule.xp_tenths,
            }],
            skills::CurrentLevelPolicy::AddBaseLevelGains,
        )?);
        if random::roll(&rule.depletion, level, random)? {
            let entity = world
                .entities
                .get_mut(target)
                .ok_or_else(|| unknown("Gather entity is missing."))?;
            entity.available_at_tick =
                runtime::deadline(world.tick, u64::from(rule.respawn_ticks))?;
            character.activity = Activity::Idle;
            character.flags.remove(runtime::GATHER_INTERACTION);
        }
        Ok(events)
    }

    pub(crate) fn start_production(
        &self,
        world: &WorldState,
        character: &mut CharacterState,
        recipe: &RecipeId,
        target: Option<&SpawnId>,
        remaining: u32,
    ) -> GameResult<()> {
        self.authorize(character, &["produce".into(), format!("produce:{recipe}")])?;
        let recipe = self
            .content
            .recipes
            .get(recipe)
            .ok_or_else(|| unknown(format!("Unknown recipe {recipe}.")))?;
        if remaining == 0 {
            return Err(invalid_state(
                "A production queue must contain at least one operation.",
            ));
        }
        self.check_recipe_target(world, character, recipe, target)?;
        self.check_recipe(character, recipe)?;
        self.recipe_level(character, recipe)?;
        self.check_outcomes_fit(character, recipe)?;
        runtime::interrupt(character);
        character.activity = Activity::Producing {
            recipe: recipe.id.clone(),
            target: target.cloned(),
            remaining,
            next_tick: runtime::deadline(world.tick, u64::from(recipe.ticks))?,
        };
        Ok(())
    }

    pub(crate) fn check_recipe_target(
        &self,
        world: &WorldState,
        character: &CharacterState,
        recipe: &RecipeDefinition,
        target: Option<&SpawnId>,
    ) -> GameResult<()> {
        let Some(target) = target else {
            if recipe.target_objects.is_empty() {
                return Ok(());
            }
            return Err(GameError::new(
                GameErrorCode::OutOfReach,
                "This recipe requires its source facility.",
            ));
        };
        let spawn = self
            .content
            .spawns
            .get(target)
            .ok_or_else(|| unknown(format!("Unknown production target {target}.")))?;
        if !recipe.target_objects.is_empty()
            && !matches!(&spawn.kind, SpawnKind::Object { object } if recipe.target_objects.contains(object))
        {
            return Err(GameError::new(
                GameErrorCode::RequirementNotMet,
                "Wrong facility for this recipe.",
            ));
        }
        let mut last_error = None;
        for interaction in &spawn.interactions {
            if matches!(&interaction.action, InteractionAction::Production { recipes } if recipes.contains(&recipe.id))
            {
                match self.require_target(world, character, target, interaction) {
                    Ok(()) => return Ok(()),
                    Err(error)
                        if matches!(
                            error.code,
                            GameErrorCode::RequirementNotMet
                                | GameErrorCode::OutOfReach
                                | GameErrorCode::Busy
                        ) =>
                    {
                        last_error = Some(error);
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        Err(last_error.unwrap_or_else(|| {
            GameError::new(
                GameErrorCode::RequirementNotMet,
                "The target does not offer this production recipe.",
            )
        }))
    }

    fn check_recipe(
        &self,
        character: &CharacterState,
        recipe: &RecipeDefinition,
    ) -> GameResult<()> {
        if recipe.ticks == 0 {
            return Err(invalid_content("Zero-tick production is not supported."));
        }
        skills::check_requirements(
            &character.skills,
            &self.content.skills,
            &recipe.requirements,
            skills::LevelBasis::Current,
        )?;
        for tool in &recipe.tools {
            if !self.has_tool(character, tool)? {
                return Err(GameError::new(
                    GameErrorCode::RequirementNotMet,
                    format!("Recipe requires unconsumed tool {tool}."),
                ));
            }
        }
        let mut inventory = character.inventory.clone();
        inventory::remove_batch(&mut inventory, &self.content.items, &recipe.inputs)
    }

    fn has_tool(&self, character: &CharacterState, tool: &ItemId) -> GameResult<bool> {
        Ok(
            inventory::count(&character.inventory, &self.content.items, tool)? > 0
                || character
                    .equipment
                    .values()
                    .any(|stack| &stack.item == tool),
        )
    }

    fn recipe_level(
        &self,
        character: &CharacterState,
        recipe: &RecipeDefinition,
    ) -> GameResult<u16> {
        if recipe.success.numerator_at_level_1 == recipe.success.numerator_at_level_99 {
            return Ok(1);
        }
        let mut skill = None;
        for requirement in &recipe.requirements {
            if skill
                .as_ref()
                .is_some_and(|skill| skill != &requirement.skill)
            {
                return Err(unavailable(
                    "A multi-skill recipe needs an explicit chance-skill binding.",
                ));
            }
            skill = Some(requirement.skill.clone());
        }
        let skill = skill.ok_or_else(|| {
            unavailable("Level-dependent recipe chance needs a chance-skill requirement.")
        })?;
        Ok(character
            .skills
            .get(&skill)
            .ok_or_else(|| invalid_state("Recipe chance skill state is missing."))?
            .current_level)
    }

    fn check_outcomes_fit(
        &self,
        character: &CharacterState,
        recipe: &RecipeDefinition,
    ) -> GameResult<()> {
        let level = self.recipe_level(character, recipe)?;
        let count = random::chance_numerator(&recipe.success, level)?;
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
        inventory: &mut Inventory,
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
        inventory::apply_operations(inventory, &self.content.items, &operations)
    }

    fn produce(
        &self,
        world: &WorldState,
        character: &mut CharacterState,
        recipe: &RecipeId,
        target: Option<&SpawnId>,
        remaining: u32,
        random: &mut impl RandomSource,
    ) -> GameResult<Vec<GameEvent>> {
        if remaining == 0 {
            return Err(invalid_state(
                "Persisted production queue has zero remaining work.",
            ));
        }
        self.authorize(character, &["produce".into(), format!("produce:{recipe}")])?;
        let recipe = self
            .content
            .recipes
            .get(recipe)
            .ok_or_else(|| unknown("Persisted recipe is undefined."))?;
        self.check_recipe_target(world, character, recipe, target)?;
        self.check_recipe(character, recipe)?;
        self.check_outcomes_fit(character, recipe)?;
        let success = random::roll(
            &recipe.success,
            self.recipe_level(character, recipe)?,
            random,
        )?;
        self.recipe_inventory(&mut character.inventory, recipe, success)?;
        let mut events = vec![GameEvent::Produced {
            recipe: recipe.id.clone(),
            outputs: if success {
                recipe.outputs.clone()
            } else {
                recipe.failed_outputs.clone()
            },
        }];
        if success {
            events.extend(skills::award_character_xp(
                character,
                &self.content,
                &recipe.xp,
                skills::CurrentLevelPolicy::AddBaseLevelGains,
            )?);
        }
        character.activity = if remaining == 1 {
            Activity::Idle
        } else {
            Activity::Producing {
                recipe: recipe.id.clone(),
                target: target.cloned(),
                remaining: remaining - 1,
                next_tick: runtime::deadline(world.tick, u64::from(recipe.ticks))?,
            }
        };
        Ok(events)
    }

    pub(crate) fn advance_entities(&self, world: &mut WorldState) -> GameResult<()> {
        for (id, definition) in &self.content.spawns {
            let entity = world
                .entities
                .get_mut(id)
                .ok_or_else(|| unknown(format!("World is missing spawn {id}.")))?;
            if entity.available_at_tick == 0 || entity.available_at_tick > world.tick {
                continue;
            }
            match &definition.kind {
                SpawnKind::Item { stack, .. } => {
                    let prefix = format!("source:{id}:");
                    if world
                        .ground_items
                        .iter()
                        .any(|item| item.id.starts_with(&prefix))
                    {
                        return Err(invalid_state(
                            "A source ground item is still present at its respawn deadline.",
                        ));
                    }
                    world.ground_items.push(self.spawn_ground_item(
                        id,
                        definition.tile,
                        stack,
                        world.tick,
                    ));
                }
                SpawnKind::Npc { npc } => {
                    let npc = self
                        .content
                        .npcs
                        .get(npc)
                        .ok_or_else(|| unknown("Respawning NPC definition is missing."))?;
                    if let Some(combat) = &npc.combat {
                        entity.hitpoints = combat.hitpoints;
                        entity.tile = definition.tile;
                    }
                }
                SpawnKind::Object { .. } => {}
            }
            entity.available_at_tick = 0;
        }
        Ok(())
    }
}
