use clubscape_game_types::*;
use clubscape_simulation::{equipment, inventory};

use crate::{
    RandomSource, WorldEngine, invalid_content, invalid_state, progression::EffectFrame, runtime,
    unavailable, unknown,
};

impl WorldEngine {
    pub(crate) fn intent(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        intent: &GameIntent,
        rng: &mut impl RandomSource,
    ) -> GameResult<Vec<GameEvent>> {
        let mut events = Vec::new();
        if character.runtime.pending_travel.is_some()
            && !matches!(
                intent,
                GameIntent::OpenInterface { .. } | GameIntent::CloseInterface
            )
        {
            let cause = match intent {
                GameIntent::Walk { .. } => InterruptionCause::Movement,
                GameIntent::RequestLogout => InterruptionCause::Logout,
                _ => InterruptionCause::AnotherAction,
            };
            events.extend(self.interrupt_travel(character, cause)?);
        }
        match intent {
            GameIntent::Walk {
                destination,
                running,
            } => {
                if *running && character.runtime.settings.run_enabled != Some(true) {
                    let policy = self
                        .content
                        .mechanics
                        .run
                        .as_ref()
                        .ok_or_else(|| unavailable("Run policy is not bound."))?;
                    if character.run_energy < policy.activation_minimum {
                        return Err(GameError::new(
                            GameErrorCode::RequirementNotMet,
                            "Insufficient run energy.",
                        ));
                    }
                    self.run_cost(character)?;
                }
                let path = self.route(world, character, *destination)?;
                runtime::interrupt(character)?;
                if !path.is_empty() {
                    character.activity = Activity::Walking {
                        path,
                        running: *running,
                    };
                }
            }
            GameIntent::Interact { target, action } => events.extend(self.interact(
                world,
                character,
                &WorldTarget::Spawn {
                    spawn: target.clone(),
                },
                action,
                rng,
            )?),
            GameIntent::InteractWith { target, action } => {
                events.extend(self.interact(world, character, target, action, rng)?)
            }
            GameIntent::SelectDialogue { speaker, choice } => {
                events.extend(self.select_dialogue(world, character, speaker, choice, rng)?)
            }
            GameIntent::OpenInterface { interface } => {
                let definition = self
                    .content
                    .interfaces
                    .get(interface)
                    .ok_or_else(|| unknown("Unknown interface."))?;
                if !character.interfaces.contains(interface) {
                    return Err(GameError::new(
                        GameErrorCode::RequirementNotMet,
                        "Interface is locked.",
                    ));
                }
                if definition.access != InterfaceAccess::Tab {
                    return Err(unavailable(
                        "Contextual interface requires a source interaction.",
                    ));
                }
                self.close_interfaces(character, &mut events)?;
                events.push(GameEvent::InterfaceOpened {
                    interface: interface.clone(),
                });
                events.push(GameEvent::InterfacePresented {
                    interface: interface.clone(),
                    context: InterfaceContext::Tab,
                });
            }
            GameIntent::CloseInterface => self.close_interfaces(character, &mut events)?,
            GameIntent::Equip { inventory_slot } => {
                let item = inventory::stack_at(&character.inventory, usize::from(*inventory_slot))?;
                let requirements = self
                    .content
                    .items
                    .get(&item.item)
                    .and_then(|item| item.equipment.as_ref())
                    .ok_or_else(|| invalid_state("Item is not equipment."))?
                    .requirements
                    .clone();
                self.requirements(character, &requirements)?;
                let basis = if requirements
                    .iter()
                    .all(|requirement| requirement.basis == SkillLevelBasis::Base)
                {
                    Some(clubscape_simulation::skills::LevelBasis::Base)
                } else if requirements
                    .iter()
                    .all(|requirement| requirement.basis == SkillLevelBasis::Current)
                {
                    Some(clubscape_simulation::skills::LevelBasis::Current)
                } else {
                    None
                };
                let equipped = if let Some(basis) = basis {
                    equipment::equip_with_level_basis(
                        character,
                        &self.content,
                        usize::from(*inventory_slot),
                        basis,
                    )?
                } else {
                    // The primitive has a whole-operation basis; all mixed source requirements
                    // were checked above. Only its displacement transaction remains to run.
                    let mut content = (*self.content).clone();
                    content
                        .items
                        .get_mut(&item.item)
                        .and_then(|item| item.equipment.as_mut())
                        .ok_or_else(|| invalid_state("Missing equipment definition."))?
                        .requirements
                        .clear();
                    equipment::equip(character, &content, usize::from(*inventory_slot))?
                };
                events.push(equipped);
                self.close_interfaces(character, &mut events)?;
            }
            GameIntent::Unequip { slot } => {
                equipment::unequip(character, &self.content, slot)?;
                self.close_interfaces(character, &mut events)?;
            }
            GameIntent::Drop { .. } => {
                return Err(unavailable(
                    "Ground policies exist, but mechanics has no ordinary/stage-specific player-drop policy selector.",
                ));
            }
            GameIntent::TakeGroundItem { ground_item_id } => {
                let stack = self.take_ground_item(world, character, ground_item_id)?;
                events.push(GameEvent::ItemTransferred {
                    from: ContainerKind::Ground,
                    to: ContainerKind::Inventory,
                    items: vec![stack],
                });
            }
            GameIntent::UseItem {
                inventory_slot,
                target,
            } => self.use_item(world, character, usize::from(*inventory_slot), target)?,
            GameIntent::MoveInventory { from, to } => inventory::swap(
                &mut character.inventory,
                &self.content.items,
                usize::from(*from),
                usize::from(*to),
            )?,
            GameIntent::Eat { inventory_slot } => {
                events.extend(self.eat(world.tick, character, usize::from(*inventory_slot))?);
                self.close_interfaces(character, &mut events)?;
                if !matches!(character.activity, Activity::Fighting { .. }) {
                    runtime::interrupt(character)?;
                }
            }
            GameIntent::Produce {
                recipe,
                target,
                quantity,
            } => self.start_production(
                world,
                character,
                recipe,
                target.clone().map(|spawn| WorldTarget::Spawn { spawn }),
                quantity.get(),
            )?,
            GameIntent::ProduceAt {
                recipe,
                target,
                quantity,
            } => self.start_production(world, character, recipe, target.clone(), quantity.get())?,
            GameIntent::BankDeposit {
                banker,
                inventory_slot,
                quantity,
            } => {
                let before = character.inventory.clone();
                self.deposit(
                    world,
                    character,
                    banker,
                    usize::from(*inventory_slot),
                    *quantity,
                )?;
                events.push(GameEvent::ItemTransferred {
                    from: ContainerKind::Inventory,
                    to: ContainerKind::Bank,
                    items: removed_items(&before, &character.inventory)?,
                });
            }
            GameIntent::BankWithdraw {
                banker,
                bank_slot,
                quantity,
                noted,
            } => {
                let before = character.inventory.clone();
                self.withdraw(
                    world,
                    character,
                    banker,
                    usize::from(*bank_slot),
                    *quantity,
                    *noted,
                )?;
                events.push(GameEvent::ItemTransferred {
                    from: ContainerKind::Bank,
                    to: ContainerKind::Inventory,
                    items: removed_items(&character.inventory, &before)?,
                });
            }
            GameIntent::ShopBuy {
                shop,
                item_index,
                quantity,
            } => self.buy(world, character, shop, usize::from(*item_index), *quantity)?,
            GameIntent::ShopSell {
                shop,
                inventory_slot,
                quantity,
            } => self.sell(
                world,
                character,
                shop,
                usize::from(*inventory_slot),
                *quantity,
            )?,
            GameIntent::SetCombatStyle { style } => {
                self.select_style(character, &CombatStyleId::new(style.clone())?)?
            }
            GameIntent::Cast { spell, target } => events.extend(self.cast(
                world,
                character,
                &SpellId::new(spell.clone())?,
                target.as_ref(),
                rng,
            )?),
            GameIntent::SetPrayer { prayer, enabled } => events.extend(self.set_prayer(
                character,
                &PrayerId::new(prayer.clone())?,
                *enabled,
            )?),
            GameIntent::SetSetting { setting } => {
                events.push(self.set_setting(character, setting)?)
            }
            GameIntent::ConfirmAppearance { appearance } => {
                let definition = self
                    .content
                    .mechanics
                    .appearance
                    .as_ref()
                    .ok_or_else(|| unavailable("Appearance choices are not bound."))?;
                self.require_guard(world, character, &definition.confirmation_guard)?;
                if character.runtime.settings.appearance_confirmed {
                    return Err(GameError::new(
                        GameErrorCode::RequirementNotMet,
                        "Appearance is already confirmed.",
                    ));
                }
                if appearance.len() != definition.choices.len()
                    || appearance.iter().any(|(key, value)| {
                        !definition
                            .choices
                            .get(key)
                            .is_some_and(|choices| choices.contains(value))
                    })
                {
                    return Err(invalid_state("Unsupported source appearance choice."));
                }
                character.appearance = appearance.clone();
                character.runtime.settings.appearance_confirmed = true;
                events.push(GameEvent::AppearanceConfirmed);
            }
            GameIntent::SelectExperience { experience } => {
                let definition = self
                    .content
                    .mechanics
                    .experiences
                    .get(experience)
                    .ok_or_else(|| unknown("Unknown experience option."))?;
                self.require_guard(world, character, &definition.selection_guard)?;
                character.runtime.settings.experience = Some(experience.clone());
                events.push(GameEvent::ExperienceSelected {
                    experience: experience.clone(),
                });
            }
            GameIntent::Reclaim {
                death,
                storage,
                items,
            } => events.extend(self.reclaim(world, character, death, *storage, items)?),
            GameIntent::CancelActivity => runtime::interrupt(character)?,
            GameIntent::RequestLogout => {
                if self.in_combat(world, character)? {
                    return Err(GameError::new(
                        GameErrorCode::Busy,
                        "Cannot log out during source combat.",
                    ));
                }
                runtime::interrupt(character)?;
            }
        }
        Ok(events)
    }

    fn interact(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        target: &WorldTarget,
        action: &str,
        rng: &mut impl RandomSource,
    ) -> GameResult<Vec<GameEvent>> {
        let (index, interaction) = self
            .target_interactions(world, target)?
            .iter()
            .enumerate()
            .find(|(_, interaction)| interaction.name == action)
            .ok_or_else(|| unknown("Target has no such source interaction."))?;
        if matches!(interaction.action, InteractionAction::Attack) {
            let WorldTarget::Spawn { spawn } = target else {
                return Err(invalid_content("Temporary scenery is not a combat NPC."));
            };
            self.require_guard(world, character, &interaction.guard)?;
            let style = character.runtime.combat.style.clone().ok_or_else(|| {
                GameError::new(
                    GameErrorCode::RequirementNotMet,
                    "Select a permitted combat style first.",
                )
            })?;
            let mut events = self.player_attack(world, character, spawn, &style, None, rng)?;
            events.push(GameEvent::Interacted {
                target: spawn.clone(),
                action: action.into(),
            });
            return Ok(events);
        }
        self.require_world_target(world, character, target, interaction)?;
        runtime::interrupt(character)?;
        let mut frame = EffectFrame::default();
        match &interaction.action {
            InteractionAction::Effects { effects } => {
                self.effects(world, character, effects, rng, &mut frame)?
            }
            InteractionAction::Dialogue { dialogue } => {
                let WorldTarget::Spawn { spawn } = target else {
                    return Err(unavailable(
                        "Dialogue speaker state cannot bind a temporary object.",
                    ));
                };
                self.open_dialogue(world, character, spawn, dialogue, index)?;
            }
            InteractionAction::Gather { rule } => {
                let WorldTarget::Spawn { spawn } = target else {
                    return Err(unavailable(
                        "Gathering schedule currently binds static source spawns only.",
                    ));
                };
                self.authorize(character, &["gather".into(), format!("gather:{spawn}")])?;
                self.check_gather(character, rule)?;
                // Capacity is checked before any first attempt; alternate catches recheck their own output.
                inventory::add(
                    &mut character.inventory.clone(),
                    &self.content.items,
                    &rule.output,
                )?;
                let mut next_tick =
                    runtime::deadline(world.tick, self.gather_delay(character, rule, false)?)?;
                if let Some(mechanics) = &rule.mechanics {
                    next_tick = next_tick.max(
                        character
                            .runtime
                            .action_cooldowns
                            .get(&mechanics.method)
                            .copied()
                            .unwrap_or(0),
                    );
                    character
                        .runtime
                        .action_cooldowns
                        .insert(mechanics.method.clone(), next_tick);
                }
                character.activity = Activity::Gathering {
                    target: spawn.clone(),
                    next_tick,
                };
                runtime::schedule_mut(character)?.gather_interaction = Some(
                    u32::try_from(index + 1)
                        .map_err(|_| invalid_state("Gather index overflow."))?,
                );
            }
            InteractionAction::Production { recipes } => {
                if recipes.is_empty() {
                    return Err(unavailable("No compiled production choices."));
                }
                frame.events.push(GameEvent::Message {
                    text: "Select a permitted source recipe.".into(),
                });
            }
            InteractionAction::Bank | InteractionAction::OpenBank { .. } => {
                self.authorize(character, &["bank".into()])?;
                let WorldTarget::Spawn { spawn } = target else {
                    return Err(unavailable("Bank context needs a source spawn."));
                };
                if let InteractionAction::OpenBank {
                    interface,
                    before_open,
                } = &interaction.action
                {
                    self.require_context_interface(character, interface)?;
                    self.effects(world, character, before_open, rng, &mut frame)?;
                    frame.events.push(GameEvent::InterfacePresented {
                        interface: interface.clone(),
                        context: InterfaceContext::Bank {
                            banker: spawn.clone(),
                        },
                    });
                }
                runtime::open_access(character, true, spawn, index)?;
            }
            InteractionAction::Shop { shop } | InteractionAction::OpenShop { shop, .. } => {
                self.authorize(character, &["shop".into(), format!("shop:{shop}")])?;
                let WorldTarget::Spawn { spawn } = target else {
                    return Err(unavailable("Shop context needs a source spawn."));
                };
                if let InteractionAction::OpenShop {
                    interface,
                    before_open,
                    ..
                } = &interaction.action
                {
                    self.require_context_interface(character, interface)?;
                    self.effects(world, character, before_open, rng, &mut frame)?;
                    frame.events.push(GameEvent::InterfacePresented {
                        interface: interface.clone(),
                        context: InterfaceContext::Shop {
                            spawn: spawn.clone(),
                            shop: shop.clone(),
                        },
                    });
                }
                runtime::open_access(character, false, spawn, index)?;
            }
            InteractionAction::Travel {
                destination,
                region,
            } => {
                self.effects(
                    world,
                    character,
                    &[Effect::Travel {
                        region: region.clone(),
                        tile: *destination,
                    }],
                    rng,
                    &mut frame,
                )?;
            }
            InteractionAction::TravelVia { travel } => frame
                .events
                .extend(self.start_travel(world, character, travel, rng)?),
            InteractionAction::Unavailable { reason } => return Err(unavailable(reason.clone())),
            InteractionAction::Attack => unreachable!(),
        }
        if let WorldTarget::Spawn { spawn } = target {
            frame.events.push(GameEvent::Interacted {
                target: spawn.clone(),
                action: action.into(),
            });
        }
        Ok(frame.events)
    }

    fn require_context_interface(
        &self,
        character: &CharacterState,
        id: &InterfaceId,
    ) -> GameResult<()> {
        let interface = self
            .content
            .interfaces
            .get(id)
            .ok_or_else(|| unknown("Unknown contextual interface."))?;
        if interface.access != InterfaceAccess::Contextual {
            return Err(invalid_content(
                "A container cannot present a tab as context.",
            ));
        }
        if !character.interfaces.contains(id) {
            return Err(GameError::new(
                GameErrorCode::RequirementNotMet,
                "Contextual interface is locked.",
            ));
        }
        Ok(())
    }

    fn close_interfaces(
        &self,
        character: &mut CharacterState,
        events: &mut Vec<GameEvent>,
    ) -> GameResult<()> {
        if let Some(access) = &runtime::schedule(character)?.access {
            let session = match access {
                ContainerSession::Bank { session } | ContainerSession::Shop { session } => session,
            };
            let interaction = runtime::interaction(
                &self.content,
                &session.spawn,
                session.interaction as usize - 1,
            )?;
            if let InteractionAction::OpenBank { interface, .. }
            | InteractionAction::OpenShop { interface, .. } = &interaction.action
            {
                events.push(GameEvent::InterfaceClosed {
                    interface: interface.clone(),
                });
            }
        }
        character.dialogue = None;
        runtime::schedule_mut(character)?.dialogue_interaction = None;
        runtime::close_access(character)
    }

    fn open_dialogue(
        &self,
        world: &WorldState,
        character: &mut CharacterState,
        speaker: &SpawnId,
        id: &DialogueId,
        index: usize,
    ) -> GameResult<()> {
        let definition = self
            .content
            .dialogues
            .get(id)
            .ok_or_else(|| unknown("Unknown dialogue."))?;
        let mut entry = None;
        for candidate in &definition.entry_nodes {
            let node = self.dialogue_node(definition, candidate)?;
            if self.guard(world, character, &node.guard, None)? {
                if entry.is_some() {
                    return Err(invalid_content("Multiple dialogue entries match."));
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
        runtime::schedule_mut(character)?.dialogue_interaction =
            Some(u32::try_from(index + 1).map_err(|_| invalid_state("Dialogue index overflow."))?);
        Ok(())
    }

    fn select_dialogue(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        speaker: &SpawnId,
        choice: &str,
        rng: &mut impl RandomSource,
    ) -> GameResult<Vec<GameEvent>> {
        let open = character.dialogue.clone().ok_or_else(|| {
            GameError::new(GameErrorCode::RequirementNotMet, "No dialogue is open.")
        })?;
        if &open.speaker != speaker {
            return Err(GameError::new(
                GameErrorCode::NotOwned,
                "Choice does not belong to the opened speaker.",
            ));
        }
        self.validate_open_dialogue(world, character)?;
        let definition = self
            .content
            .dialogues
            .get(&open.id)
            .ok_or_else(|| unknown("Unknown open dialogue."))?;
        let node = self.dialogue_node(definition, &open.node)?;
        let selection = node
            .choices
            .iter()
            .find(|selection| selection.id == choice)
            .ok_or_else(|| invalid_state("Choice does not belong to the open node."))?;
        self.require_guard(world, character, &selection.guard)?;
        let event = GameEvent::DialogueSelected {
            speaker: speaker.clone(),
            choice: choice.into(),
        };
        let mut frame = EffectFrame {
            trigger: Some(event.clone()),
            ..EffectFrame::default()
        };
        self.effects(world, character, &selection.effects, rng, &mut frame)?;
        if let Some(next) = &selection.next_node {
            self.dialogue_node(definition, next)?;
            character.dialogue = Some(OpenDialogue {
                node: next.clone(),
                ..open
            });
        } else {
            character.dialogue = None;
            runtime::schedule_mut(character)?.dialogue_interaction = None;
        }
        frame.events.push(event);
        Ok(frame.events)
    }

    pub(crate) fn validate_open_dialogue(
        &self,
        world: &WorldState,
        character: &CharacterState,
    ) -> GameResult<()> {
        let Some(open) = &character.dialogue else {
            return Ok(());
        };
        let index = runtime::schedule(character)?
            .dialogue_interaction
            .ok_or_else(|| invalid_state("Missing opened dialogue interaction."))?;
        let interaction = runtime::interaction(&self.content, &open.speaker, index as usize - 1)?;
        if !matches!(&interaction.action, InteractionAction::Dialogue { dialogue } if dialogue == &open.id)
        {
            return Err(invalid_state(
                "Dialogue does not belong to that interaction.",
            ));
        }
        self.require_target(world, character, &open.speaker, interaction)?;
        let definition = self
            .content
            .dialogues
            .get(&open.id)
            .ok_or_else(|| unknown("Unknown dialogue."))?;
        self.require_guard(
            world,
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
            .ok_or_else(|| unknown("Undefined dialogue node."))
    }

    fn take_ground_item(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        id: &str,
    ) -> GameResult<ItemStack> {
        let mut matches = world
            .ground_items
            .iter()
            .enumerate()
            .filter(|(_, ground)| ground.id == id);
        let (index, item) = matches.next().ok_or_else(|| {
            GameError::new(GameErrorCode::NotOwned, "Ground item no longer exists.")
        })?;
        if matches.next().is_some() {
            return Err(invalid_state("Duplicate ground-item identity."));
        }
        if item.expires_at_tick <= world.tick
            || (item
                .owner
                .as_ref()
                .is_some_and(|owner| owner != &character.actor_id)
                && item.public_at_tick > world.tick)
        {
            return Err(GameError::new(
                GameErrorCode::NotOwned,
                "Ground item is private or expired.",
            ));
        }
        if let Some(rest) = id.strip_prefix("policy:") {
            let policy = rest
                .split(':')
                .next()
                .ok_or_else(|| invalid_state("Malformed ground policy identity."))?;
            let policy = self
                .content
                .mechanics
                .ground_policies
                .get(&GroundPolicyId::new(policy)?)
                .ok_or_else(|| unknown("Unknown ground policy."))?;
            if item.owner.as_ref() == Some(&character.actor_id) && !policy.owner_can_take {
                return Err(GameError::new(
                    GameErrorCode::NotOwned,
                    "Owner pickup is not allowed by source policy.",
                ));
            }
        }
        if character.tile != item.tile || character.runtime.instance != item.instance {
            return Err(GameError::new(
                GameErrorCode::OutOfReach,
                "Walk onto this ground item's tile in its instance.",
            ));
        }
        let stack = item.stack.clone();
        inventory::add(&mut character.inventory, &self.content.items, &stack)?;
        if let Some(source) = id.strip_prefix("source:") {
            let spawn = SpawnId::new(
                source
                    .split(':')
                    .next()
                    .ok_or_else(|| invalid_state("Malformed spawn ground identity."))?,
            )?;
            let definition = self
                .content
                .spawns
                .get(&spawn)
                .ok_or_else(|| unknown("Unknown source ground spawn."))?;
            let SpawnKind::Item {
                respawn_ticks,
                stack: source_stack,
            } = &definition.kind
            else {
                return Err(invalid_state("Source ground item has another kind."));
            };
            if source_stack != &stack {
                return Err(invalid_state("Source ground stack changed."));
            }
            runtime::entity_mut(world, character.runtime.instance.as_ref(), &spawn)?
                .available_at_tick = runtime::deadline(world.tick, u64::from(*respawn_ticks))?;
        }
        world.ground_items.remove(index);
        runtime::interrupt(character)?;
        Ok(stack)
    }

    fn use_item(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        slot: usize,
        target: &ItemTarget,
    ) -> GameResult<()> {
        let used = inventory::stack_at(&character.inventory, slot)?
            .item
            .clone();
        let (other, facility, ground) = match target {
            ItemTarget::Inventory { slot: other } => {
                if usize::from(*other) == slot {
                    return Err(invalid_state("Cannot use an item on its own slot."));
                }
                (
                    Some(
                        inventory::stack_at(&character.inventory, usize::from(*other))?
                            .item
                            .clone(),
                    ),
                    None,
                    None,
                )
            }
            ItemTarget::World { spawn } => (
                None,
                Some(WorldTarget::Spawn {
                    spawn: spawn.clone(),
                }),
                None,
            ),
            ItemTarget::TemporaryObject { object } => (
                None,
                Some(WorldTarget::TemporaryObject {
                    object: object.clone(),
                }),
                None,
            ),
            ItemTarget::Ground { ground_item_id } => {
                let ground = world
                    .ground_items
                    .iter()
                    .find(|ground| &ground.id == ground_item_id)
                    .ok_or_else(|| {
                        GameError::new(GameErrorCode::NotOwned, "Ground input is gone.")
                    })?;
                if ground.owner.as_ref() != Some(&character.actor_id)
                    || ground.instance != character.runtime.instance
                    || ground.tile != character.tile
                    || ground.expires_at_tick <= world.tick
                {
                    return Err(GameError::new(
                        GameErrorCode::NotOwned,
                        "Ground input is not owned here.",
                    ));
                }
                (
                    Some(ground.stack.item.clone()),
                    None,
                    Some(ground_item_id.clone()),
                )
            }
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
            match self.check_recipe_target(world, character, recipe, facility.as_ref()) {
                Ok(()) => {}
                Err(error) if crate::is_interruption(&error.code) => {
                    refusal = Some(error);
                    continue;
                }
                Err(error) => return Err(error),
            }
            if selected.replace(recipe.id.clone()).is_some() {
                return Err(invalid_state(
                    "Multiple recipes match; select the explicit recipe.",
                ));
            }
        }
        let recipe = selected.ok_or_else(|| {
            refusal.unwrap_or_else(|| unavailable("No source item-use rule matches."))
        })?;
        if let Some(ground_item) = ground {
            let definition = self
                .content
                .recipes
                .get(&recipe)
                .ok_or_else(|| unknown("Unknown recipe."))?;
            let mechanics = definition
                .mechanics
                .as_ref()
                .ok_or_else(|| invalid_content("Ground use needs a typed fire lifecycle."))?;
            if !matches!(mechanics.lifecycle, RecipeLifecycle::Firemaking { .. }) {
                return Err(invalid_content(
                    "Ground input is only bound for firemaking.",
                ));
            }
            self.authorize(character, &["produce".into(), format!("produce:{recipe}")])?;
            runtime::interrupt(character)?;
            let next_attempt_tick = runtime::deadline(
                world.tick,
                runtime::cadence(&mechanics.cadence, true, false)?,
            )?;
            character.runtime.pending_fire = Some(PendingFire {
                recipe,
                ground_item,
                tile: character.tile,
                next_attempt_tick,
            });
            Ok(())
        } else {
            self.start_production(world, character, &recipe, facility, 1)
        }
    }
}

fn removed_items(before: &Inventory, after: &Inventory) -> GameResult<Vec<ItemStack>> {
    let mut counts = std::collections::BTreeMap::<ItemId, i64>::new();
    for stack in before.slots.iter().flatten() {
        *counts.entry(stack.item.clone()).or_default() += i64::from(stack.quantity.get());
    }
    for stack in after.slots.iter().flatten() {
        *counts.entry(stack.item.clone()).or_default() -= i64::from(stack.quantity.get());
    }
    counts
        .into_iter()
        .filter(|(_, amount)| *amount > 0)
        .map(|(item, amount)| {
            Ok(ItemStack {
                item,
                quantity: Quantity::new(
                    u32::try_from(amount)
                        .map_err(|_| invalid_state("Transfer quantity overflow."))?,
                )?,
                instance: None,
            })
        })
        .collect()
}
