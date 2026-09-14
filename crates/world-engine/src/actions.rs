use clubscape_game_types::*;
use clubscape_simulation::{equipment, inventory, skills};

use crate::{
    RandomSource, WorldEngine, invalid_content, invalid_state, runtime, unavailable, unknown,
};

impl WorldEngine {
    pub(crate) fn intent(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        intent: &GameIntent,
        _random: &mut impl RandomSource,
    ) -> GameResult<Vec<GameEvent>> {
        let mut events = Vec::new();
        match intent {
            GameIntent::Walk {
                destination,
                running,
            } => {
                if *running {
                    return Err(unavailable(
                        "Running requires source item weights and a bound Agility/run policy; no free-running fallback.",
                    ));
                }
                let path = self
                    .collision
                    .find_path(character.tile, *destination, 16_384)?;
                runtime::interrupt(character);
                if !path.is_empty() {
                    character.activity = Activity::Walking {
                        path,
                        running: false,
                    };
                }
            }
            GameIntent::Interact { target, action } => {
                let spawn = self
                    .content
                    .spawns
                    .get(target)
                    .ok_or_else(|| unknown(format!("Unknown target {target}.")))?;
                let (index, interaction) = spawn
                    .interactions
                    .iter()
                    .enumerate()
                    .find(|(_, interaction)| &interaction.name == action)
                    .ok_or_else(|| {
                        unknown(format!("Target {target} has no interaction {action}."))
                    })?;
                self.require_target(world, character, target, interaction)?;
                runtime::interrupt(character);
                match &interaction.action {
                    InteractionAction::Effects { effects } => {
                        self.effects(character, effects, &mut events, 0)?
                    }
                    InteractionAction::Dialogue { dialogue } => {
                        self.open_dialogue(character, target, dialogue, index)?;
                    }
                    InteractionAction::Gather { rule } => {
                        self.authorize(character, &["gather".into(), format!("gather:{target}")])?;
                        self.check_gather(character, rule)?;
                        character.activity = Activity::Gathering {
                            target: target.clone(),
                            next_tick: runtime::deadline(
                                world.tick,
                                u64::from(rule.attempt_ticks),
                            )?,
                        };
                        runtime::set_counter(
                            character,
                            runtime::GATHER_INTERACTION,
                            index as u64 + 1,
                        )?;
                    }
                    InteractionAction::Production { recipes } => {
                        if recipes.is_empty() {
                            return Err(unavailable(
                                "This production interaction has no compiled recipes.",
                            ));
                        }
                        events.push(GameEvent::Message {
                            text: "Select a permitted recipe at this facility.".into(),
                        });
                    }
                    InteractionAction::Bank => {
                        self.authorize(character, &["bank".into()])?;
                        runtime::open_access(character, "bank", target, index)?;
                    }
                    InteractionAction::Shop { shop } => {
                        self.authorize(character, &["shop".into(), format!("shop:{shop}")])?;
                        runtime::open_access(character, "shop", target, index)?;
                    }
                    InteractionAction::Attack => return Err(combat_unbound()),
                    InteractionAction::Travel {
                        destination,
                        region,
                    } => {
                        self.effects(
                            character,
                            &[Effect::Travel {
                                region: region.clone(),
                                tile: *destination,
                            }],
                            &mut events,
                            0,
                        )?;
                    }
                    InteractionAction::Unavailable { reason } => {
                        return Err(unavailable(reason.clone()));
                    }
                }
                events.push(GameEvent::Interacted {
                    target: target.clone(),
                    action: action.clone(),
                });
            }
            GameIntent::SelectDialogue { speaker, choice } => {
                self.select_dialogue(world, character, speaker, choice, &mut events)?;
            }
            GameIntent::OpenInterface { interface } => {
                if !self.content.interfaces.contains_key(interface) {
                    return Err(unknown(format!("Unknown interface {interface}.")));
                }
                if !character.interfaces.contains(interface) {
                    return Err(GameError::new(
                        GameErrorCode::RequirementNotMet,
                        "Interface is locked.",
                    ));
                }
                character.dialogue = None;
                character.flags.remove(runtime::DIALOGUE_INTERACTION);
                runtime::close_access(character);
                events.push(GameEvent::InterfaceOpened {
                    interface: interface.clone(),
                });
            }
            GameIntent::CloseInterface => {
                character.dialogue = None;
                character.flags.remove(runtime::DIALOGUE_INTERACTION);
                runtime::close_access(character);
            }
            GameIntent::Equip { inventory_slot } => {
                runtime::interrupt(character);
                events.push(equipment::equip(
                    character,
                    &self.content,
                    usize::from(*inventory_slot),
                )?);
            }
            GameIntent::Unequip { slot } => {
                runtime::interrupt(character);
                equipment::unequip(character, &self.content, slot)?;
            }
            GameIntent::Drop { .. } => {
                return Err(unavailable(
                    "Ordinary ground-item visibility/expiry and tutorial-only drop policy are not bound.",
                ));
            }
            GameIntent::TakeGroundItem { ground_item_id } => {
                self.take_ground_item(world, character, ground_item_id)?;
            }
            GameIntent::UseItem {
                inventory_slot,
                target,
            } => {
                self.use_item(world, character, usize::from(*inventory_slot), target)?;
            }
            GameIntent::MoveInventory { from, to } => {
                inventory::swap(
                    &mut character.inventory,
                    &self.content.items,
                    usize::from(*from),
                    usize::from(*to),
                )?;
            }
            GameIntent::Eat { inventory_slot } => {
                self.eat(world.tick, character, usize::from(*inventory_slot))?;
            }
            GameIntent::Produce {
                recipe,
                target,
                quantity,
            } => {
                self.start_production(world, character, recipe, target.as_ref(), quantity.get())?;
            }
            GameIntent::BankDeposit {
                banker,
                inventory_slot,
                quantity,
            } => {
                self.deposit(
                    world,
                    character,
                    banker,
                    usize::from(*inventory_slot),
                    *quantity,
                )?;
            }
            GameIntent::BankWithdraw {
                banker,
                bank_slot,
                quantity,
                noted,
            } => {
                self.withdraw(
                    world,
                    character,
                    banker,
                    usize::from(*bank_slot),
                    *quantity,
                    *noted,
                )?;
            }
            GameIntent::ShopBuy {
                shop,
                item_index,
                quantity,
            } => {
                self.buy(world, character, shop, usize::from(*item_index), *quantity)?;
            }
            GameIntent::ShopSell {
                shop,
                inventory_slot,
                quantity,
            } => {
                self.sell(
                    world,
                    character,
                    shop,
                    usize::from(*inventory_slot),
                    *quantity,
                )?;
            }
            GameIntent::SetCombatStyle { .. } => return Err(combat_unbound()),
            GameIntent::Cast { .. } => {
                return Err(unavailable(
                    "Spell definitions (costs, XP, range, timing, guards and projectile/teleport state) are not bound.",
                ));
            }
            GameIntent::SetPrayer { .. } => {
                return Err(unavailable(
                    "Prayer definitions, modifiers and persisted fractional drain state are not bound.",
                ));
            }
            GameIntent::CancelActivity | GameIntent::RequestLogout => runtime::interrupt(character),
        }
        Ok(events)
    }

    fn open_dialogue(
        &self,
        character: &mut CharacterState,
        speaker: &SpawnId,
        id: &DialogueId,
        interaction: usize,
    ) -> GameResult<()> {
        let definition = self
            .content
            .dialogues
            .get(id)
            .ok_or_else(|| unknown(format!("Unknown dialogue {id}.")))?;
        let mut entry = None;
        for candidate in &definition.entry_nodes {
            let node = self.dialogue_node(definition, candidate)?;
            if self.guard(character, &node.guard)? {
                if entry.is_some() {
                    return Err(invalid_content("More than one dialogue entry is eligible."));
                }
                entry = Some(node.id.clone());
            }
        }
        let node = entry.ok_or_else(|| {
            GameError::new(
                GameErrorCode::RequirementNotMet,
                "No dialogue entry is unlocked.",
            )
        })?;
        character.dialogue = Some(OpenDialogue {
            id: id.clone(),
            speaker: speaker.clone(),
            node,
        });
        runtime::set_counter(
            character,
            runtime::DIALOGUE_INTERACTION,
            interaction as u64 + 1,
        )
    }

    fn select_dialogue(
        &self,
        world: &WorldState,
        character: &mut CharacterState,
        speaker: &SpawnId,
        choice: &str,
        events: &mut Vec<GameEvent>,
    ) -> GameResult<()> {
        let open = character.dialogue.clone().ok_or_else(|| {
            GameError::new(GameErrorCode::RequirementNotMet, "No dialogue is open.")
        })?;
        if &open.speaker != speaker {
            return Err(GameError::new(
                GameErrorCode::NotOwned,
                "Choice does not belong to the open speaker.",
            ));
        }
        self.validate_open_dialogue(world, character)?;
        let definition = self
            .content
            .dialogues
            .get(&open.id)
            .ok_or_else(|| unknown("Open dialogue is undefined."))?;
        let node = self.dialogue_node(definition, &open.node)?;
        let selection = node
            .choices
            .iter()
            .find(|candidate| candidate.id == choice)
            .ok_or_else(|| {
                GameError::new(
                    GameErrorCode::InvalidInput,
                    "Choice does not belong to the open node.",
                )
            })?;
        self.require_guard(character, &selection.guard)?;
        self.effects(character, &selection.effects, events, 0)?;
        if let Some(next) = &selection.next_node {
            self.dialogue_node(definition, next)?;
            character.dialogue = Some(OpenDialogue {
                node: next.clone(),
                ..open
            });
        } else {
            character.dialogue = None;
            character.flags.remove(runtime::DIALOGUE_INTERACTION);
        }
        events.push(GameEvent::DialogueSelected {
            speaker: speaker.clone(),
            choice: choice.into(),
        });
        Ok(())
    }

    pub(crate) fn validate_open_dialogue(
        &self,
        world: &WorldState,
        character: &CharacterState,
    ) -> GameResult<()> {
        let Some(open) = &character.dialogue else {
            return Ok(());
        };
        let index = runtime::read_counter(character, runtime::DIALOGUE_INTERACTION)?
            .checked_sub(1)
            .ok_or_else(|| invalid_state("Open dialogue lacks its authenticated interaction."))?;
        let index = usize::try_from(index)
            .map_err(|_| invalid_state("Dialogue interaction index overflow."))?;
        let interaction = runtime::interaction(&self.content, &open.speaker, index)?;
        if !matches!(&interaction.action, InteractionAction::Dialogue { dialogue } if dialogue == &open.id)
        {
            return Err(invalid_state(
                "Open dialogue does not belong to its speaker interaction.",
            ));
        }
        self.require_target(world, character, &open.speaker, interaction)?;
        let definition = self
            .content
            .dialogues
            .get(&open.id)
            .ok_or_else(|| unknown("Undefined open dialogue."))?;
        self.require_guard(
            character,
            &self.dialogue_node(definition, &open.node)?.guard,
        )
    }

    fn dialogue_node<'a>(
        &self,
        definition: &'a DialogueDefinition,
        node: &str,
    ) -> GameResult<&'a DialogueNode> {
        definition
            .nodes
            .iter()
            .find(|candidate| candidate.id == node)
            .ok_or_else(|| {
                unknown(format!(
                    "Undefined node {node} in dialogue {}.",
                    definition.id
                ))
            })
    }

    fn take_ground_item(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        id: &str,
    ) -> GameResult<()> {
        let mut matches = world
            .ground_items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.id == id);
        let (index, item) = matches.next().ok_or_else(|| {
            GameError::new(GameErrorCode::NotOwned, "Ground item no longer exists.")
        })?;
        if matches.next().is_some() {
            return Err(invalid_state("Ground item identity is not unique."));
        }
        if item.expires_at_tick <= world.tick
            || (item
                .owner
                .as_ref()
                .is_some_and(|owner| owner != &character.actor_id)
                && world.tick < item.public_at_tick)
        {
            return Err(GameError::new(
                GameErrorCode::NotOwned,
                "Ground item is expired or private.",
            ));
        }
        // Pickup is a contact action. The caller must walk to the source tile first.
        if character.tile != item.tile {
            return Err(GameError::new(
                GameErrorCode::OutOfReach,
                "Walk onto the ground item's tile before taking it.",
            ));
        }
        inventory::add(&mut character.inventory, &self.content.items, &item.stack)?;
        if let Some(source) = id.strip_prefix("source:") {
            let (spawn, _) = source
                .rsplit_once(':')
                .ok_or_else(|| invalid_state("Malformed source ground identity."))?;
            let spawn = SpawnId::new(spawn)?;
            let definition = self
                .content
                .spawns
                .get(&spawn)
                .ok_or_else(|| unknown("Source item spawn is undefined."))?;
            let SpawnKind::Item {
                respawn_ticks,
                stack,
            } = &definition.kind
            else {
                return Err(invalid_state("Ground item source is not an item spawn."));
            };
            if stack != &item.stack || definition.tile != item.tile {
                return Err(invalid_state(
                    "Ground item does not match its source spawn.",
                ));
            }
            let entity = world
                .entities
                .get_mut(&spawn)
                .ok_or_else(|| unknown("Source item entity is missing."))?;
            entity.available_at_tick = runtime::deadline(world.tick, u64::from(*respawn_ticks))?;
        }
        world.ground_items.remove(index);
        runtime::interrupt(character);
        Ok(())
    }

    fn eat(&self, tick: u64, character: &mut CharacterState, slot: usize) -> GameResult<()> {
        if tick < runtime::read_counter(character, runtime::FOOD_READY)? {
            return Err(GameError::new(
                GameErrorCode::Busy,
                "Ordinary food has a three-tick eating delay.",
            ));
        }
        let stack = inventory::stack_at(&character.inventory, slot)?;
        let definition = self
            .content
            .items
            .get(&stack.item)
            .ok_or_else(|| unknown("Unknown food item."))?;
        let healing = definition
            .healing
            .filter(|healing| *healing > 0)
            .ok_or_else(|| {
                GameError::new(
                    GameErrorCode::InvalidInput,
                    "This item is not ordinary edible food.",
                )
            })?;
        let hitpoints = SkillId::new("skill.hitpoints")?;
        let definition = self.content.skills.get(&hitpoints).ok_or_else(|| {
            unknown("Ordinary food requires the source semantic skill.hitpoints binding.")
        })?;
        let state = character
            .skills
            .get(&hitpoints)
            .ok_or_else(|| invalid_state("Character lacks Hitpoints skill state."))?;
        let maximum = skills::level_for_xp(definition, state.xp_tenths)?;
        inventory::remove_from_slot(
            &mut character.inventory,
            &self.content.items,
            slot,
            Quantity::new(1)?,
        )?;
        character.hitpoints = character
            .hitpoints
            .max(character.hitpoints.saturating_add(healing).min(maximum));
        runtime::set_counter(character, runtime::FOOD_READY, runtime::deadline(tick, 3)?)?;
        let attack_ready = runtime::read_counter(character, runtime::ATTACK_READY)?.max(tick);
        runtime::set_counter(
            character,
            runtime::ATTACK_READY,
            runtime::deadline(attack_ready, 3)?,
        )?;
        runtime::interrupt(character);
        Ok(())
    }

    fn use_item(
        &self,
        world: &WorldState,
        character: &mut CharacterState,
        slot: usize,
        target: &ItemTarget,
    ) -> GameResult<()> {
        let used = inventory::stack_at(&character.inventory, slot)?
            .item
            .clone();
        let (other, spawn) = match target {
            ItemTarget::Inventory { slot: target_slot } => {
                if usize::from(*target_slot) == slot {
                    return Err(invalid_state(
                        "An item cannot be used on the same inventory slot.",
                    ));
                }
                (
                    Some(
                        inventory::stack_at(&character.inventory, usize::from(*target_slot))?
                            .item
                            .clone(),
                    ),
                    None,
                )
            }
            ItemTarget::World { spawn } => (None, Some(spawn)),
        };
        let mut selected = None;
        let mut refusal = None;
        for recipe in self.content.recipes.values() {
            let uses = |item: &ItemId| {
                recipe.inputs.iter().any(|stack| &stack.item == item) || recipe.tools.contains(item)
            };
            if !uses(&used) || other.as_ref().is_some_and(|item| !uses(item)) {
                continue;
            }
            match self.check_recipe_target(world, character, recipe, spawn) {
                Ok(()) => {}
                Err(error)
                    if matches!(
                        error.code,
                        GameErrorCode::RequirementNotMet
                            | GameErrorCode::OutOfReach
                            | GameErrorCode::Busy
                    ) =>
                {
                    refusal = Some(error);
                    continue;
                }
                Err(error) => return Err(error),
            }
            if selected.replace(recipe.id.clone()).is_some() {
                return Err(GameError::new(
                    GameErrorCode::InvalidInput,
                    "Multiple recipes match; select an explicit recipe.",
                ));
            }
        }
        let recipe = selected.ok_or_else(|| {
            refusal.unwrap_or_else(|| {
                unavailable("No compiled production rule binds this item-use combination.")
            })
        })?;
        self.start_production(world, character, &recipe, spawn, 1)
    }
}

pub(crate) fn combat_unbound() -> GameError {
    unavailable(
        "Combat needs bound style attack/defence types, skill/XP rules, ammo/range, NPC retaliation/credit and source death policy.",
    )
}
