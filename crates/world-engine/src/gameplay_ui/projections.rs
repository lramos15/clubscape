use super::*;
use crate::{ContextView, Permission};
use clubscape_simulation::{bank, bank_layout};

fn permission(result: GameResult<()>) -> GameResult<UiPermission> {
    let value = Permission::evaluate(result)?;
    Ok(UiPermission {
        allowed: value.allowed,
        code: value.denial.as_ref().map(|error| error.code.clone()),
        reason: value.denial.map(|error| error.message),
    })
}

impl WorldEngine {
    pub fn ui_view(&self, world: &WorldState, actor: &ActorId) -> GameResult<GameplayUiView> {
        self.check_world(world)?;
        let definition = self
            .content
            .ui
            .as_ref()
            .ok_or_else(|| unavailable("game.ui.v1 is not supported by this content."))?;
        let character = world
            .characters
            .get(actor)
            .ok_or_else(|| GameError::new(GameErrorCode::NotOwned, "Unknown UI actor."))?;
        let ui = character
            .runtime
            .ui
            .as_ref()
            .ok_or_else(|| unavailable("UI state must be migrated before projection."))?;
        self.validate_ui_state(character)?;
        let mut active = ui
            .active_interface
            .clone()
            .or_else(|| {
                definition
                    .stage_overlays
                    .get(&character.tutorial_stage)
                    .cloned()
                    .flatten()
            })
            .or_else(|| ui.active_tab.clone());
        let context = self.context_view(world, actor)?;
        let bank_view = match &context {
            ContextView::Bank { view } => {
                active = view.interface.clone();
                Some(self.bank_ui_view(character, view)?)
            }
            _ => None,
        };
        match &context {
            ContextView::Shop { view } => active = view.interface.clone(),
            ContextView::Recovery { views } => {
                active = views.first().and_then(|view| view.interface.clone())
            }
            _ => {}
        }
        let production =
            ui.production
                .as_ref()
                .map(|menu| {
                    let choices =
                        menu.recipes
                            .iter()
                            .map(|id| {
                                let recipe = self.content.recipes.get(id).ok_or_else(|| {
                                    unknown("Production menu recipe disappeared.")
                                })?;
                                Ok(ProductionChoiceUiView {
                                    recipe: id.clone(),
                                    name: recipe.name.clone(),
                                    outputs: recipe
                                        .outputs
                                        .iter()
                                        .map(|stack| self.ui_item(stack))
                                        .collect::<GameResult<_>>()?,
                                    single: permission(self.production_permission(
                                        world,
                                        character,
                                        id,
                                        &menu.target,
                                        1,
                                        ProductionMode::Single,
                                    ))?,
                                    make_x: permission(self.production_permission(
                                        world,
                                        character,
                                        id,
                                        &menu.target,
                                        1,
                                        ProductionMode::MakeX,
                                    ))?,
                                })
                            })
                            .collect::<GameResult<Vec<_>>>()?;
                    Ok(ProductionUiView {
                        id: menu.id.clone(),
                        interface: menu.interface.clone(),
                        target: menu.target.clone(),
                        recipes: choices,
                    })
                })
                .transpose()?;
        if let Some(menu) = &production {
            active = Some(menu.interface.clone());
        }
        if ui.death_preview {
            active = Some(definition.death_preview_interface.clone());
        }
        if let Some(document) = &ui.document {
            active = Some(document.interface.clone());
        }
        if let Some(reward) = ui.rewards.first() {
            active = Some(reward.interface.clone());
        }
        let interfaces = definition
            .stage_interfaces
            .get(&character.tutorial_stage)
            .ok_or_else(|| invalid_content("UI profile lacks the semantic tutorial state."))?
            .iter()
            .map(|rule| {
                let allowed = self.guard(world, character, &rule.guard, None)?;
                let unlocked = character.interfaces.contains(&rule.interface);
                let visibility = if active.as_ref() == Some(&rule.interface) {
                    UiVisibility::Enabled
                } else if rule.unavailable_reason.is_some() {
                    rule.visibility.clone()
                } else if rule.visibility == UiVisibility::Enabled && (!unlocked || !allowed) {
                    UiVisibility::Locked
                } else {
                    rule.visibility.clone()
                };
                let availability = permission(if let Some(reason) = &rule.unavailable_reason {
                    Err(unavailable(reason.clone()))
                } else if rule.interface == definition.death_preview_interface {
                    self.death_preview_permission(world, character)
                } else if self.content.interfaces[&rule.interface].access == InterfaceAccess::Tab
                    || rule.interface == definition.equipment_stats_interface
                {
                    self.open_interface_preconditions(world, character, &rule.interface)
                        .map(|_| ())
                } else if active.as_ref() == Some(&rule.interface) {
                    Ok(())
                } else {
                    Err(GameError::new(
                        GameErrorCode::RequirementNotMet,
                        "Open this source context through its owned interaction.",
                    ))
                })?;
                Ok(InterfaceUiView {
                    interface: rule.interface.clone(),
                    visibility,
                    highlighted: rule.highlighted,
                    permission: availability,
                })
            })
            .collect::<GameResult<_>>()?;
        let mut styles = Vec::new();
        let weapon = character
            .runtime
            .combat
            .style
            .as_ref()
            .map(|style| self.equipped_weapon(character, style))
            .transpose()?;
        for id in weapon.into_iter().flat_map(|weapon| &weapon.styles) {
            let source = self
                .content
                .mechanics
                .combat_styles
                .get(id)
                .ok_or_else(|| unknown("Unknown source combat style."))?;
            let names = character
                .equipment
                .values()
                .find_map(|stack| {
                    definition
                        .weapon_style_names
                        .get(&stack.item)
                        .filter(|names| names.contains_key(id))
                })
                .unwrap_or(&definition.unarmed_style_names);
            let name = names
                .get(id)
                .ok_or_else(|| invalid_content("Source combat-style label is absent."))?;
            let allowed = self
                .input_permission(character)
                .and_then(|()| self.require_ui_free(character))
                .and_then(|()| {
                    self.authorize_intent(
                        character,
                        &GameIntent::SetCombatStyle {
                            style: id.to_string(),
                        },
                    )
                });
            styles.push(AbilityUiView {
                id: source.id.to_string(),
                name: name.clone(),
                selected: character.runtime.combat.style.as_ref() == Some(id),
                visible: true,
                permission: permission(allowed)?,
            });
        }
        let prayers = self
            .content
            .mechanics
            .prayers
            .iter()
            .map(|(id, prayer)| {
                let enabled = character.runtime.combat.active_prayers.contains(id);
                let allowed = self
                    .input_permission(character)
                    .and_then(|()| {
                        self.authorize_intent(
                            character,
                            &GameIntent::SetPrayer {
                                prayer: id.to_string(),
                                enabled: !enabled,
                            },
                        )
                    })
                    .and_then(|()| self.prayer_preconditions(character, id, !enabled));
                Ok(AbilityUiView {
                    id: id.to_string(),
                    name: definition
                        .ability_names
                        .get(id.as_str())
                        .cloned()
                        .ok_or_else(|| invalid_content("Source prayer label is absent."))?,
                    selected: enabled,
                    visible: character.interfaces.contains(&prayer.interface),
                    permission: permission(allowed)?,
                })
            })
            .collect::<GameResult<_>>()?;
        let spells = self.content.mechanics.spells.iter().map(|(id, spell)| {
            let allowed = self.input_permission(character)
                .and_then(|()| self.authorize_intent(character, &GameIntent::Cast { spell: id.to_string(), target: None }))
                .and_then(|()| self.spell_preconditions(world, character, id));
            Ok(AbilityUiView { id: id.to_string(), name: definition.ability_names.get(id.as_str()).cloned()
                .ok_or_else(|| invalid_content("Source spell label is absent."))?,
                selected: matches!(&character.activity, Activity::Casting { spell, .. } if spell == id.as_str()),
                visible: character.interfaces.contains(&spell.interface), permission: permission(allowed)? })
        }).collect::<GameResult<_>>()?;
        let mut choices = BTreeMap::new();
        if let Some(appearance) = &self.content.mechanics.appearance {
            for (key, values) in &appearance.choices {
                choices.insert(
                    key.clone(),
                    values
                        .iter()
                        .map(|value| {
                            Ok(AppearanceChoiceUiView {
                                value: *value,
                                label: None,
                                permission: permission(self.input_permission(character).and_then(
                                    |()| {
                                        self.require_guard(
                                            world,
                                            character,
                                            &appearance.confirmation_guard,
                                        )
                                    },
                                ))?,
                            })
                        })
                        .collect::<GameResult<_>>()?,
                );
            }
        }
        let inventory_actions = self.inventory_ui_actions(world, character)?;
        let recovery = if matches!(context, ContextView::Recovery { .. }) {
            let mut coffer_items = Vec::new();
            for (index, stack) in character
                .inventory
                .slots
                .iter()
                .enumerate()
                .filter_map(|(index, stack)| stack.as_ref().map(|stack| (index, stack)))
            {
                let plan = self.coffer_plan(
                    world,
                    character,
                    index as u8,
                    &stack.item,
                    stack.instance.as_ref().map(|instance| &instance.id),
                    stack.quantity.get(),
                );
                coffer_items.push(InventoryActionsUiView {
                    slot: index as u8,
                    item: stack.item.clone(),
                    instance: stack.instance.as_ref().map(|instance| instance.id.clone()),
                    actions: vec![ItemActionUiView {
                        id: "coffer_offer".into(),
                        label: "Sacrifice".into(),
                        permission: permission(plan.map(|_| ()))?,
                    }],
                });
            }
            let allowed = coffer_items
                .iter()
                .any(|item| item.actions.iter().any(|action| action.permission.allowed));
            Some(RecoveryUiControls {
                coffer_balance: character.runtime.death_coffer.to_string(),
                discard: permission(self.input_permission(character))?,
                coffer_offer: permission(if allowed {
                    Ok(())
                } else {
                    Err(GameError::new(
                        GameErrorCode::RequirementNotMet,
                        "No currently owned item is source-eligible for the coffer.",
                    ))
                })?,
                coffer_items,
            })
        } else {
            None
        };
        Ok(GameplayUiView {
            version: GAMEPLAY_UI_VIEW_VERSION,
            active_tab: ui.active_tab.clone(),
            active_interface: active,
            production,
            reward: ui.rewards.first().cloned(),
            confirmation: ui
                .confirmation
                .as_ref()
                .map(|confirmation| confirmation.view.clone()),
            document: ui.document.clone(),
            interfaces,
            combat_style: character
                .runtime
                .combat
                .style
                .as_ref()
                .map(ToString::to_string),
            combat_styles: styles,
            prayers,
            spells,
            equipment: self.equipment_ui(character)?,
            inventory_actions,
            bank: bank_view,
            kept_on_death: if ui.death_preview {
                Some(self.death_preview(character)?)
            } else {
                None
            },
            recovery,
            appearance: AppearanceUiView {
                choices,
                base: Some(definition.appearance_base.require()?.clone()),
                confirmed: character.runtime.settings.appearance_confirmed,
            },
            public_chat: PublicChatUiView {
                permission: permission(
                    self.input_permission(character).and_then(|()| {
                        self.require_guard(world, character, &definition.chat.guard)
                    }),
                )?,
                maximum_bytes: definition.chat.maximum_bytes,
                channel: "public".into(),
                messages: ui.chat_messages.clone(),
            },
        })
    }

    pub(crate) fn production_permission(
        &self,
        world: &WorldState,
        character: &CharacterState,
        id: &RecipeId,
        target: &WorldTarget,
        quantity: u32,
        mode: ProductionMode,
    ) -> GameResult<()> {
        self.input_permission(character)?;
        self.authorize(character, &["produce".into(), format!("produce:{id}")])?;
        let recipe = self
            .content
            .recipes
            .get(id)
            .ok_or_else(|| unknown("Unknown source recipe."))?;
        Quantity::new(quantity)?;
        if mode == ProductionMode::Single && quantity != 1 {
            return Err(invalid_state("Invalid production selection mode/quantity."));
        }
        self.check_recipe_target(world, character, recipe, Some(target))?;
        self.check_recipe(world, character, recipe, false)?;
        self.recipe_delay(recipe, mode == ProductionMode::Single, false)?;
        self.check_outcomes_fit(character, recipe)
    }

    pub(crate) fn ui_item(&self, stack: &ItemStack) -> GameResult<UiItem> {
        let item = self
            .content
            .items
            .get(&stack.item)
            .ok_or_else(|| unknown("Unknown projected item."))?;
        Ok(UiItem {
            item: item.id.clone(),
            name: item.name.clone(),
            quantity: stack.quantity.get(),
            source_id: item.source_id,
            asset: item.asset.clone(),
            instance_id: stack.instance.as_ref().map(|instance| instance.id.clone()),
            charges: stack
                .instance
                .as_ref()
                .and_then(|instance| instance.charges.as_ref())
                .map(|charges| charges.remaining),
        })
    }

    fn inventory_ui_actions(
        &self,
        world: &WorldState,
        character: &CharacterState,
    ) -> GameResult<Vec<InventoryActionsUiView>> {
        let definitions = &self.content.ui.as_ref().unwrap().item_actions;
        character
            .inventory
            .slots
            .iter()
            .enumerate()
            .filter_map(|(slot, stack)| stack.as_ref().map(|stack| (slot, stack)))
            .map(|(slot, stack)| {
                let actions = definitions
                    .get(&stack.item)
                    .into_iter()
                    .flatten()
                    .map(|action| {
                        let result = self
                            .item_action_preconditions(
                                world,
                                character,
                                slot as u8,
                                &stack.item,
                                stack.instance.as_ref().map(|instance| &instance.id),
                                &action.id,
                            )
                            .map(|_| ());
                        Ok(ItemActionUiView {
                            id: action.id.clone(),
                            label: action.label.clone(),
                            permission: permission(result)?,
                        })
                    })
                    .collect::<GameResult<_>>()?;
                Ok(InventoryActionsUiView {
                    slot: slot as u8,
                    item: stack.item.clone(),
                    instance: stack.instance.as_ref().map(|instance| instance.id.clone()),
                    actions,
                })
            })
            .collect()
    }

    fn bank_ui_view(
        &self,
        character: &CharacterState,
        view: &crate::BankView,
    ) -> GameResult<BankUiView> {
        let layout = &character.runtime.ui.as_ref().unwrap().bank;
        bank_layout::validate(&character.bank, layout)?;
        let tabs: BTreeSet<_> = std::iter::once(0)
            .chain(layout.entries.iter().map(|entry| entry.tab))
            .collect();
        let tabs = tabs
            .into_iter()
            .map(|tab| {
                let entries: Vec<_> = layout
                    .entries
                    .iter()
                    .filter(|entry| tab == 0 || entry.tab == tab)
                    .collect();
                BankTabUiView {
                    tab,
                    first_entry: entries.first().map(|entry| entry.id.to_string()),
                    entries: entries.len() as u32,
                }
            })
            .collect();
        let entries = layout
            .entries
            .iter()
            .map(|entry| {
                Ok(BankEntryUiView {
                    id: entry.id.to_string(),
                    slot: entry.slot,
                    tab: entry.tab,
                    item: entry.item.clone(),
                    placeholder: entry.placeholder,
                    value: character
                        .bank
                        .slots
                        .get(usize::from(entry.slot))
                        .and_then(Option::as_ref)
                        .map(|stack| self.ui_item(stack))
                        .transpose()?,
                })
            })
            .collect::<GameResult<_>>()?;
        let available = if view.deposit.allowed {
            bank::plan_equipment_deposit(character, &self.content).map(|_| ())
        } else {
            Err(view
                .deposit
                .denial
                .clone()
                .ok_or_else(|| invalid_state("Missing bank refusal."))?)
        };
        Ok(BankUiView {
            revision: layout.revision.to_string(),
            capacity: character.bank.capacity,
            selected_tab: layout.selected_tab,
            insert_mode: layout.insert,
            placeholders: layout.placeholders,
            amount: layout.amount,
            noted: layout.noted,
            tabs,
            entries,
            deposit_equipment: permission(available)?,
            unavailable_containers: self
                .content
                .ui
                .as_ref()
                .unwrap()
                .bank
                .unavailable_containers
                .iter()
                .map(|control| ItemActionUiView {
                    id: control.id.clone(),
                    label: control.label.clone(),
                    permission: UiPermission {
                        allowed: false,
                        code: Some(GameErrorCode::Unavailable),
                        reason: Some(control.reason.clone()),
                    },
                })
                .collect(),
        })
    }

    fn equipment_ui(&self, character: &CharacterState) -> GameResult<EquipmentUiView> {
        let mut bonuses = CombatBonuses::default();
        for kind in [
            AttackType::Stab,
            AttackType::Slash,
            AttackType::Crush,
            AttackType::Ranged,
            AttackType::Magic,
        ] {
            bonuses.attack.insert(
                kind,
                i16::try_from(self.equipment_bonus(character, |bonus| {
                    i32::from(*bonus.attack.get(&kind).unwrap_or(&0))
                })?)
                .map_err(|_| invalid_state("Equipment projection overflow."))?,
            );
            bonuses.defence.insert(
                kind,
                i16::try_from(self.equipment_bonus(character, |bonus| {
                    i32::from(*bonus.defence.get(&kind).unwrap_or(&0))
                })?)
                .map_err(|_| invalid_state("Equipment projection overflow."))?,
            );
        }
        bonuses.melee_strength = i16::try_from(
            self.equipment_bonus(character, |bonus| i32::from(bonus.melee_strength))?,
        )
        .map_err(|_| invalid_state("Equipment projection overflow."))?;
        bonuses.ranged_strength = i16::try_from(
            self.equipment_bonus(character, |bonus| i32::from(bonus.ranged_strength))?,
        )
        .map_err(|_| invalid_state("Equipment projection overflow."))?;
        bonuses.magic_damage_percent = i16::try_from(
            self.equipment_bonus(character, |bonus| i32::from(bonus.magic_damage_percent))?,
        )
        .map_err(|_| invalid_state("Equipment projection overflow."))?;
        bonuses.prayer =
            i16::try_from(self.equipment_bonus(character, |bonus| i32::from(bonus.prayer))?)
                .map_err(|_| invalid_state("Equipment projection overflow."))?;
        Ok(EquipmentUiView {
            bonuses,
            weight_grams: self.carried_weight(character)?.to_string(),
            slots: self.content.equipment_slots.clone(),
        })
    }

    pub(super) fn death_preview_permission(
        &self,
        world: &WorldState,
        character: &CharacterState,
    ) -> GameResult<()> {
        self.input_permission(character)?;
        self.require_ui_free(character)?;
        let definition = self
            .content
            .ui
            .as_ref()
            .ok_or_else(|| unavailable("UI source is absent."))?;
        let interface = &definition.death_preview_interface;
        if !character.interfaces.contains(interface)
            || self.content.tutorial[&character.tutorial_stage].nonfatal_combat
        {
            return Err(GameError::new(
                GameErrorCode::RequirementNotMet,
                "Normal unsafe death preview is locked in this source state.",
            ));
        }
        let rule = definition.stage_interfaces[&character.tutorial_stage]
            .iter()
            .find(|rule| &rule.interface == interface)
            .ok_or_else(|| invalid_content("Death preview lacks its source interface rule."))?;
        if let Some(reason) = &rule.unavailable_reason {
            return Err(unavailable(reason.clone()));
        }
        self.require_guard(world, character, &rule.guard)?;
        self.death_preview(character).map(|_| ())
    }

    pub(super) fn death_preview(
        &self,
        character: &CharacterState,
    ) -> GameResult<DeathPreviewUiView> {
        if !matches!(character.runtime.life, LifeState::Alive)
            || character.runtime.instance.is_some()
        {
            return Err(unavailable(
                "The death preview covers normal unsafe, non-PvP world deaths.",
            ));
        }
        let policy = self
            .content
            .mechanics
            .death
            .as_ref()
            .ok_or_else(|| unavailable("Death policy is absent."))?;
        let provider = &self.content.mechanics.value_providers[&policy.value_provider];
        let mut kept = Vec::new();
        let mut lost = Vec::new();
        let mut grave_fee = 0_u64;
        let mut office_fee = 0_u64;
        for part in self.retention_plan(character)? {
            if part.kept > 0 {
                let mut shown = self.ui_item(&part.stack)?;
                shown.quantity = part.kept;
                kept.push(shown);
            }
            let quantity = part.stack.quantity.get() - part.kept;
            if quantity > 0 {
                let mut shown = self.ui_item(&part.stack)?;
                shown.quantity = quantity;
                lost.push(shown);
                grave_fee = grave_fee
                    .checked_add(crate::death::recovery_fee_value(
                        policy.grave_fee.require()?,
                        part.value,
                        0,
                        quantity,
                    )?)
                    .ok_or_else(|| invalid_state("Preview fee overflow."))?;
                office_fee = office_fee
                    .checked_add(crate::death::recovery_fee_value(
                        policy.office_fee.require()?,
                        part.value,
                        0,
                        quantity,
                    )?)
                    .ok_or_else(|| invalid_state("Preview fee overflow."))?;
            }
        }
        if let RecoveryFee::Bands { maximum_total, .. } = policy.grave_fee.require()? {
            grave_fee = grave_fee.min(*maximum_total);
        }
        if let RecoveryFee::Bands { maximum_total, .. } = policy.office_fee.require()? {
            office_fee = office_fee.min(*maximum_total);
        }
        Ok(DeathPreviewUiView {
            scope: "normal_unsafe_non_pvp".into(),
            kept,
            lost,
            full_grave_fee: grave_fee.to_string(),
            full_office_fee: office_fee.to_string(),
            value_revision: provider.revision.clone(),
        })
    }

    pub(super) fn send_public_chat(
        &self,
        world: &mut WorldState,
        character: &mut CharacterState,
        channel: &str,
        input: &str,
    ) -> GameResult<Vec<ActorEvent>> {
        let policy = &self.content.ui.as_ref().unwrap().chat;
        if channel != "public" {
            return Err(unavailable("Only source public chat is supported."));
        }
        self.require_guard(world, character, &policy.guard)?;
        let (text, colour, effect) = chat_text(input, usize::from(policy.maximum_bytes))?;
        let ui = ui_mut(character)?;
        ui.chat_ticks
            .retain(|tick| world.tick.saturating_sub(*tick) < u64::from(policy.window_ticks));
        if ui.chat_ticks.len() >= usize::from(policy.messages_per_window) {
            return Err(GameError::new(
                GameErrorCode::Busy,
                "Public chat admission capacity exceeded.",
            ));
        }
        ui.chat_ticks.push(world.tick);
        let next = next_id(character)?;
        let id = format!("chat.{}.{}", character.actor_id, next);
        let line = PublicChatLine {
            id,
            actor: character.actor_id.clone(),
            sender: character.display_name.clone(),
            channel: "public".into(),
            text,
            colour,
            effect,
        };
        let mut events = vec![ActorEvent {
            actor_id: character.actor_id.clone(),
            event: GameEvent::PublicChat { line: line.clone() },
        }];
        push_chat(character, line.clone())?;
        for other in world.characters.values_mut() {
            if !matches!(other.runtime.presence, PresenceState::Connected { .. })
                || other.runtime.instance != character.runtime.instance
                || other
                    .tile
                    .distance(character.tile)
                    .is_none_or(|distance| distance > policy.radius)
            {
                continue;
            }
            self.prepare_ui(other)?;
            push_chat(other, line.clone())?;
            events.push(ActorEvent {
                actor_id: other.actor_id.clone(),
                event: GameEvent::PublicChat { line: line.clone() },
            });
        }
        Ok(events)
    }
}

fn push_chat(character: &mut CharacterState, line: PublicChatLine) -> GameResult<()> {
    let ui = ui_mut(character)?;
    ui.chat_messages.push(line);
    if ui.chat_messages.len() > 100 {
        ui.chat_messages.remove(0);
    }
    Ok(())
}

fn chat_text(input: &str, maximum: usize) -> GameResult<(String, u8, u8)> {
    if input.trim().is_empty()
        || input.starts_with('/')
        || input.chars().count() > maximum
        || input
            .chars()
            .any(|ch| ch.is_control() || matches!(ch, '<' | '>') || !cp1252(ch))
    {
        return Err(invalid_state(
            "Public chat requires bounded plain CP1252 text without control tags or private-channel prefixes.",
        ));
    }
    let mut text = input;
    let mut colour = 0;
    let mut effect = 0;
    for (index, prefix) in [
        "yellow:", "red:", "green:", "cyan:", "purple:", "white:", "flash1:", "flash2:", "flash3:",
        "glow1:", "glow2:", "glow3:", "rainbow:",
    ]
    .iter()
    .enumerate()
    {
        if text
            .get(..prefix.len())
            .is_some_and(|part| part.eq_ignore_ascii_case(prefix))
        {
            colour = index as u8;
            text = &text[prefix.len()..];
            break;
        }
    }
    for (index, prefix) in ["wave:", "wave2:", "shake:", "scroll:", "slide:"]
        .iter()
        .enumerate()
    {
        if text
            .get(..prefix.len())
            .is_some_and(|part| part.eq_ignore_ascii_case(prefix))
        {
            effect = index as u8 + 1;
            text = &text[prefix.len()..];
            break;
        }
    }
    if text.trim().is_empty() {
        return Err(invalid_state("A public chat line needs visible text."));
    }
    Ok((text.to_owned(), colour, effect))
}

fn cp1252(ch: char) -> bool {
    (' '..='~').contains(&ch)
        || ('\u{a0}'..='\u{ff}').contains(&ch)
        || "€‚ƒ„…†‡ˆ‰Š‹ŒŽ‘’“”•–—˜™š›œžŸ".contains(ch)
}
