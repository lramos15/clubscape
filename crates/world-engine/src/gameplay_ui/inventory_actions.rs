use super::*;
use clubscape_simulation::inventory;

impl WorldEngine {
    pub(super) fn selected_item(
        &self,
        character: &CharacterState,
        slot: u8,
        expected: &ItemId,
        instance: Option<&ItemInstanceId>,
    ) -> GameResult<ItemStack> {
        let stack = inventory::stack_at(&character.inventory, usize::from(slot))?;
        if &stack.item != expected || stack.instance.as_ref().map(|value| &value.id) != instance {
            return Err(GameError::new(
                GameErrorCode::StaleCommand,
                "The selected inventory item or instance changed.",
            ));
        }
        Ok(stack.clone())
    }

    pub(super) fn item_action_preconditions(
        &self,
        world: &WorldState,
        character: &CharacterState,
        slot: u8,
        item: &ItemId,
        instance: Option<&ItemInstanceId>,
        action: &str,
    ) -> GameResult<&ItemUiDefinition> {
        self.input_permission(character)?;
        self.require_ui_free(character)?;
        if character.hitpoints == 0
            || matches!(
                character.runtime.life,
                LifeState::Dying { .. } | LifeState::Respawning { .. }
            )
        {
            return Err(GameError::new(
                GameErrorCode::Busy,
                "A source life transition is pending.",
            ));
        }
        let selected = self.selected_item(character, slot, item, instance)?;
        let definition = self
            .content
            .ui
            .as_ref()
            .and_then(|ui| ui.item_actions.get(item))
            .and_then(|actions| actions.iter().find(|entry| entry.id == action))
            .ok_or_else(|| unknown("This item has no bound source action."))?;
        self.require_guard(world, character, &definition.guard)?;
        match &definition.action {
            ItemUiAction::ConsumeRecipe { recipe } => {
                let recipe = self
                    .content
                    .recipes
                    .get(recipe)
                    .ok_or_else(|| unknown("Unknown item-consumption recipe."))?;
                let mechanics = recipe
                    .mechanics
                    .as_ref()
                    .filter(|mechanics| matches!(mechanics.lifecycle, RecipeLifecycle::ConsumeOnly))
                    .ok_or_else(|| {
                        invalid_content(
                            "Selected consumption requires a source ConsumeOnly recipe.",
                        )
                    })?;
                if recipe.inputs.len() != 1
                    || recipe.inputs[0].item != *item
                    || recipe.inputs[0].quantity.get() != 1
                {
                    return Err(invalid_content(
                        "Selected consumption must use exactly its own source item.",
                    ));
                }
                self.authorize(
                    character,
                    &["produce".into(), format!("produce:{}", recipe.id)],
                )?;
                self.check_recipe_target(world, character, recipe, None)?;
                self.check_recipe(world, character, recipe, false)?;
                self.recipe_delay(recipe, true, false)?;
                if character
                    .runtime
                    .action_cooldowns
                    .get(&mechanics.method)
                    .is_some_and(|ready| *ready > world.tick)
                {
                    return Err(GameError::new(
                        GameErrorCode::Busy,
                        "The source item action is not ready.",
                    ));
                }
            }
            ItemUiAction::Drink {
                replacement,
                cooldown,
                delay_ticks,
                restore,
                skills,
            } => {
                if instance.is_some() || selected.quantity.get() != 1 {
                    return Err(unavailable(
                        "The source drink requires one ordinary selected container.",
                    ));
                }
                if !self.content.items.contains_key(replacement) || *delay_ticks.require()? == 0 {
                    return Err(invalid_content(
                        "A source drink requires a replacement and positive independent timer.",
                    ));
                }
                if character
                    .runtime
                    .action_cooldowns
                    .get(cooldown)
                    .is_some_and(|ready| *ready > world.tick)
                {
                    return Err(GameError::new(
                        GameErrorCode::Busy,
                        "The source drink timer is not ready.",
                    ));
                }
                for vital in restore.keys() {
                    self.maximum_vital(character, *vital)?;
                }
                for adjustment in skills {
                    self.level(character, &adjustment.skill, adjustment.basis)?;
                }
            }
            ItemUiAction::Empty { .. } if instance.is_some() || selected.quantity.get() != 1 => {
                return Err(unavailable(
                    "The source empty action requires one ordinary selected container.",
                ));
            }
            ItemUiAction::Unavailable { reason } => return Err(unavailable(reason.clone())),
            _ => {}
        }
        Ok(definition)
    }

    pub(crate) fn item_ui_action(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        slot: u8,
        item: &ItemId,
        instance: Option<&ItemInstanceId>,
        action: &str,
    ) -> GameResult<Vec<GameEvent>> {
        let definition =
            self.item_action_preconditions(world, character, slot, item, instance, action)?;
        match &definition.action {
            ItemUiAction::ConsumeRecipe { recipe } => {
                let recipe = &self.content.recipes[recipe];
                let mechanics = recipe.mechanics.as_ref().unwrap();
                let ready = runtime::deadline(world.tick, self.recipe_delay(recipe, true, false)?)?;
                runtime::interrupt(character)?;
                character.activity = Activity::InventoryAction {
                    slot,
                    item: item.clone(),
                    instance: instance.cloned(),
                    action: action.into(),
                    completes_at: ready,
                };
                character
                    .runtime
                    .action_cooldowns
                    .insert(mechanics.method.clone(), ready);
                Ok(Vec::new())
            }
            ItemUiAction::Drink {
                replacement,
                cooldown,
                delay_ticks,
                restore,
                skills,
            } => {
                let delay = *delay_ticks.require()?;
                self.replace_selected(character, slot, item, replacement)?;
                for (vital, restoration) in restore {
                    self.restore_vital(character, *vital, restoration)?;
                }
                for adjustment in skills {
                    let base = self.level(character, &adjustment.skill, SkillLevelBasis::Base)?;
                    let basis = self.level(character, &adjustment.skill, adjustment.basis)?;
                    if adjustment.percent.denominator == 0 {
                        return Err(invalid_content(
                            "A source skill adjustment has zero denominator.",
                        ));
                    }
                    let amount = u64::from(basis) * u64::from(adjustment.percent.numerator)
                        / u64::from(adjustment.percent.denominator)
                        + u64::from(adjustment.additive);
                    let state = character
                        .skills
                        .get_mut(&adjustment.skill)
                        .ok_or_else(|| unknown("Unknown adjusted skill."))?;
                    state.current_level = if adjustment.drain {
                        state.current_level.saturating_sub(
                            u16::try_from(amount)
                                .map_err(|_| invalid_content("Skill adjustment overflow."))?,
                        )
                    } else {
                        state.current_level.max(
                            u16::try_from(u64::from(base) + amount)
                                .map_err(|_| invalid_content("Skill adjustment overflow."))?,
                        )
                    };
                }
                character.runtime.action_cooldowns.insert(
                    cooldown.clone(),
                    runtime::deadline(world.tick, u64::from(delay))?,
                );
                // Drinks preserve combat/movement and never touch food/attack/spell cooldowns.
                Ok(vec![GameEvent::Message {
                    text: format!("You drink the {}.", self.content.items[item].name),
                }])
            }
            ItemUiAction::Empty { replacement } => {
                self.replace_selected(character, slot, item, replacement)?;
                Ok(Vec::new())
            }
            ItemUiAction::Read {
                interface,
                title,
                pages,
                map_asset,
                native_map,
            } => {
                let id = next_id(character)?;
                let mut events = Vec::new();
                self.close_interfaces(character, &mut events)?;
                ui_mut(character)?.document = Some(DocumentUiView {
                    id,
                    interface: interface.clone(),
                    title: title.clone(),
                    pages: pages.clone(),
                    page: 0,
                    map_asset: map_asset.clone(),
                    native_map: *native_map,
                });
                events.push(GameEvent::InterfaceOpened {
                    interface: interface.clone(),
                });
                Ok(events)
            }
            ItemUiAction::Unavailable { reason } => Err(unavailable(reason.clone())),
        }
    }

    fn replace_selected(
        &self,
        character: &mut CharacterState,
        slot: u8,
        old: &ItemId,
        new: &ItemId,
    ) -> GameResult<()> {
        let selected = inventory::stack_at(&character.inventory, usize::from(slot))?;
        if &selected.item != old || selected.quantity.get() != 1 || selected.instance.is_some() {
            return Err(GameError::new(
                GameErrorCode::StaleCommand,
                "Selected container is no longer one ordinary item.",
            ));
        }
        if !self.content.items.contains_key(new) {
            return Err(unknown("Unknown source replacement item."));
        }
        character.inventory.slots[usize::from(slot)] = Some(ItemStack {
            item: new.clone(),
            quantity: Quantity::new(1)?,
            instance: None,
        });
        inventory::validate(&character.inventory, &self.content.items)
    }

    pub(crate) fn complete_inventory_action(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        rng: &mut impl RandomSource,
    ) -> GameResult<Vec<GameEvent>> {
        let Activity::InventoryAction {
            slot,
            item,
            instance,
            action,
            ..
        } = character.activity.clone()
        else {
            return Err(invalid_state("No selected inventory action is pending."));
        };
        self.selected_item(character, slot, &item, instance.as_ref())?;
        let definition = self
            .content
            .ui
            .as_ref()
            .and_then(|ui| ui.item_actions.get(&item))
            .and_then(|actions| actions.iter().find(|entry| entry.id == action))
            .ok_or_else(|| unknown("Pending item action is no longer defined."))?;
        self.require_guard(world, character, &definition.guard)?;
        let ItemUiAction::ConsumeRecipe { recipe } = &definition.action else {
            return Err(invalid_state("Unexpected delayed item action."));
        };
        let recipe = &self.content.recipes[recipe];
        self.authorize(
            character,
            &["produce".into(), format!("produce:{}", recipe.id)],
        )?;
        self.check_recipe(world, character, recipe, false)?;
        let mechanics = recipe.mechanics.as_ref().unwrap();
        if recipe.success.numerator(1)? != recipe.success.denominator {
            return Err(invalid_content(
                "A selected ConsumeOnly item action cannot invent a random consumption outcome.",
            ));
        }
        inventory::remove_from_slot(
            &mut character.inventory,
            &self.content.items,
            usize::from(slot),
            Quantity::new(1)?,
        )?;
        character.activity = Activity::Idle;
        let mut frame = crate::progression::EffectFrame::default();
        self.effects(
            world,
            character,
            &mechanics.success_effects,
            rng,
            &mut frame,
        )?;
        let mut events = self.award_xp(character, &recipe.xp)?;
        events.extend(frame.events);
        events.push(GameEvent::ProductionResolved {
            recipe: recipe.id.clone(),
            method: mechanics.method.clone(),
            facility: None,
            outcome: ProductionOutcome::Success,
            outputs: Vec::new(),
        });
        Ok(events)
    }

    pub(super) fn require_recovery_context(
        &self,
        character: &CharacterState,
        death: &DeathId,
        storage: RecoveryStorage,
    ) -> GameResult<()> {
        let allowed = match (&runtime::schedule(character)?.access, storage) {
            (Some(ContainerSession::Grave { death: current, .. }), RecoveryStorage::Grave) => {
                current == death
            }
            (Some(ContainerSession::DeathOffice { .. }), RecoveryStorage::DeathOffice) => true,
            _ => false,
        };
        if !allowed {
            return Err(GameError::new(
                GameErrorCode::NotOwned,
                "Recovery requires its actual owned current context.",
            ));
        }
        Ok(())
    }

    pub(super) fn open_discard_confirmation(
        &self,
        world: &WorldState,
        character: &mut CharacterState,
        death: &DeathId,
        storage: RecoveryStorage,
        selected: &[RecoveryItemId],
    ) -> GameResult<()> {
        self.require_recovery_context(character, death, storage)?;
        let view = self.recovery_view_for(world, character, death, storage)?;
        if selected.is_empty()
            || selected.len() > 256
            || selected.iter().collect::<BTreeSet<_>>().len() != selected.len()
        {
            return Err(invalid_state("Select distinct current recovery entries."));
        }
        let mut items = Vec::new();
        let mut expected = Vec::new();
        for id in selected {
            let entry = view
                .entries
                .iter()
                .find(|entry| &entry.id == id)
                .ok_or_else(|| {
                    GameError::new(
                        GameErrorCode::NotOwned,
                        "Recovery entry is not in this context.",
                    )
                })?;
            items.push(self.ui_item(&entry.stack)?);
            expected.push(UiRecoverySelection {
                id: id.clone(),
                quantity: entry.stack.quantity.get(),
                storage: entry.current_storage,
            });
        }
        let id = next_id(character)?;
        ui_mut(character)?.confirmation = Some(UiConfirmation {
            view: ConfirmationUiView {
                id,
                kind: "discard_recovery".into(),
                title: "Discard items".into(),
                lines: vec!["These items will be permanently discarded.".into()],
                items,
                credit: None,
            },
            action: UiConfirmationAction::Discard {
                death: death.clone(),
                storage,
                items: expected,
            },
        });
        Ok(())
    }

    pub(super) fn coffer_plan(
        &self,
        world: &WorldState,
        character: &CharacterState,
        slot: u8,
        expected: &ItemId,
        instance: Option<&ItemInstanceId>,
        quantity: u32,
    ) -> GameResult<u64> {
        if !matches!(
            runtime::schedule(character)?.access,
            Some(ContainerSession::DeathOffice { .. })
        ) {
            return Err(GameError::new(
                GameErrorCode::NotOwned,
                "Open the source Office context before a coffer offer.",
            ));
        }
        self.office_access(world, character)?;
        let selected = self.selected_item(character, slot, expected, instance)?;
        let rules = self
            .content
            .ui
            .as_ref()
            .ok_or_else(|| unavailable("UI source is absent."))?
            .coffer
            .require()?;
        if instance.is_some()
            || !rules.eligible_items.contains(expected)
            || !self.content.items[expected].tradable
        {
            return Err(GameError::new(
                GameErrorCode::RequirementNotMet,
                "This source item is not eligible for the coffer.",
            ));
        }
        if quantity == 0 || quantity > selected.quantity.get() {
            return Err(GameError::new(
                GameErrorCode::InsufficientItems,
                "Coffer offer exceeds the selected stack.",
            ));
        }
        let value = *rules
            .exchange_values
            .get(expected)
            .ok_or_else(|| unavailable("Source exchange valuation is unavailable."))?;
        if value < rules.minimum_value || rules.credit.denominator == 0 {
            return Err(GameError::new(
                GameErrorCode::RequirementNotMet,
                "The individual source value is below coffer eligibility.",
            ));
        }
        let credit = crate::source_math::rounded(
            u128::from(value) * u128::from(quantity) * u128::from(rules.credit.numerator),
            u128::from(rules.credit.denominator),
            &IntegerRounding::Floor,
        )?;
        if character
            .runtime
            .death_coffer
            .checked_add(credit)
            .is_none_or(|total| total > rules.maximum_balance)
        {
            return Err(GameError::new(
                GameErrorCode::StackOverflow,
                "The source coffer capacity would be exceeded.",
            ));
        }
        Ok(credit)
    }

    pub(super) fn open_coffer_confirmation(
        &self,
        world: &WorldState,
        character: &mut CharacterState,
        slot: u8,
        item: &ItemId,
        instance: Option<&ItemInstanceId>,
        quantity: u32,
    ) -> GameResult<()> {
        let credit = self.coffer_plan(world, character, slot, item, instance, quantity)?;
        let mut shown = self.ui_item(&self.selected_item(character, slot, item, instance)?)?;
        shown.quantity = quantity;
        let id = next_id(character)?;
        ui_mut(character)?.confirmation = Some(UiConfirmation {
            view: ConfirmationUiView {
                id,
                kind: "coffer_offer".into(),
                title: "Death's Coffer".into(),
                lines: vec!["The offered items cannot be returned.".into()],
                items: vec![shown],
                credit: Some(credit.to_string()),
            },
            action: UiConfirmationAction::Coffer {
                slot,
                item: item.clone(),
                instance: instance.cloned(),
                quantity,
                credit,
            },
        });
        Ok(())
    }

    pub(super) fn confirm_ui(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        id: &str,
        accept: bool,
    ) -> GameResult<Vec<GameEvent>> {
        let pending = ui_mut(character)?
            .confirmation
            .clone()
            .filter(|pending| pending.view.id == id)
            .ok_or_else(|| {
                GameError::new(
                    GameErrorCode::StaleCommand,
                    "The confirmation has expired or was already answered.",
                )
            })?;
        if accept {
            match pending.action {
                UiConfirmationAction::Coffer {
                    slot,
                    item,
                    instance,
                    quantity,
                    credit,
                } => {
                    if self.coffer_plan(
                        world,
                        character,
                        slot,
                        &item,
                        instance.as_ref(),
                        quantity,
                    )? != credit
                    {
                        return Err(GameError::new(
                            GameErrorCode::StaleCommand,
                            "Coffer valuation changed.",
                        ));
                    }
                    inventory::remove_from_slot(
                        &mut character.inventory,
                        &self.content.items,
                        usize::from(slot),
                        Quantity::new(quantity)?,
                    )?;
                    character.runtime.death_coffer += credit;
                }
                UiConfirmationAction::Discard {
                    death,
                    storage,
                    items,
                } => {
                    self.require_recovery_context(character, &death, storage)?;
                    let current = self.recovery_view_for(world, character, &death, storage)?;
                    for expected in &items {
                        if !current.entries.iter().any(|entry| {
                            entry.id == expected.id
                                && entry.stack.quantity.get() == expected.quantity
                                && entry.current_storage == expected.storage
                        }) {
                            return Err(GameError::new(
                                GameErrorCode::StaleCommand,
                                "A recovery entry changed after confirmation.",
                            ));
                        }
                    }
                    let record = world
                        .runtime
                        .deaths
                        .get_mut(&death)
                        .ok_or_else(|| invalid_state("Recovery record disappeared."))?;
                    let selected: BTreeSet<_> = items.into_iter().map(|entry| entry.id).collect();
                    record.office.retain(|entry| !selected.contains(&entry.id));
                    if let Some(grave) = &mut record.grave {
                        grave.items.retain(|entry| !selected.contains(&entry.id));
                    }
                    record.discarded.extend(selected);
                }
            }
        }
        ui_mut(character)?.confirmation = None;
        Ok(Vec::new())
    }
}
