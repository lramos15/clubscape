mod bank_controls;
mod inventory_actions;
mod production;
mod projections;
mod validation;

use clubscape_game_types::*;
use clubscape_simulation::{bank_layout, skills};
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    ActorEvent, RandomSource, WorldEngine, invalid_content, invalid_state, runtime, tag,
    unavailable, unknown,
};

impl WorldEngine {
    /// Explicit content migration: initializes only missing additive UI metadata.
    /// Historical rewards/chat are not replayed; every owned item/XP/clock remains unchanged.
    pub fn migrate_ui_state(&self, world: &mut WorldState) -> GameResult<()> {
        if self.content.ui.is_none() {
            return Err(unavailable("The target content has no UI profile."));
        }
        let mut draft = world.clone();
        if draft.runtime.ui_version.is_none() {
            if draft
                .characters
                .values()
                .any(|character| character.runtime.ui.is_some())
            {
                return Err(invalid_state(
                    "An unversioned world contains partially initialized UI history.",
                ));
            }
            let definition = &self.content.ui.as_ref().unwrap().bank;
            for character in draft.characters.values_mut() {
                character.runtime.ui =
                    Some(GameplayUiRuntime::from_legacy(&character.bank, definition)?);
            }
            draft.runtime.ui_version = Some(UI_STATE_VERSION);
        }
        for character in draft.characters.values_mut() {
            self.prepare_ui(character)?;
        }
        draft.validate_runtime(&self.content)?;
        *world = draft;
        Ok(())
    }
    pub(crate) fn observe_world_ui(
        &self,
        before: &WorldState,
        after: &mut WorldState,
        events: &[ActorEvent],
    ) -> GameResult<()> {
        for (actor, character) in &mut after.characters {
            let old = before
                .characters
                .get(actor)
                .ok_or_else(|| invalid_state("UI observation lost its actor identity."))?;
            let own: Vec<_> = events
                .iter()
                .filter(|event| &event.actor_id == actor)
                .map(|event| event.event.clone())
                .collect();
            self.observe_ui(old, character, &own)?;
        }
        Ok(())
    }
    pub(crate) fn prepare_ui(&self, character: &mut CharacterState) -> GameResult<()> {
        let Some(_) = &self.content.ui else {
            if character.runtime.ui.is_some() {
                return Err(unavailable(
                    "Persisted UI state requires a UI-enabled content profile.",
                ));
            }
            return Ok(());
        };
        if character.runtime.ui.is_none() {
            return Err(invalid_state(
                "Missing UI history requires checked migration, not a new default.",
            ));
        }
        self.validate_ui_state(character)
    }

    pub(crate) fn observe_ui(
        &self,
        before: &CharacterState,
        after: &mut CharacterState,
        events: &[GameEvent],
    ) -> GameResult<()> {
        let Some(definition) = &self.content.ui else {
            return Ok(());
        };
        if after.runtime.ui.is_none() {
            return Err(invalid_state("UI state was not initialized."));
        }
        let mut added = Vec::new();
        for (quest, reward) in &definition.quest_rewards {
            if before
                .runtime
                .entitlements
                .contains_key(&reward.entitlement)
                || !after.runtime.entitlements.contains_key(&reward.entitlement)
            {
                continue;
            }
            let completed = self
                .content
                .quests
                .get(quest)
                .ok_or_else(|| unknown("Unknown reward quest."))?;
            if after
                .quests
                .get(quest)
                .is_none_or(|state| state.stage != completed.completed_stage)
            {
                return Err(invalid_content(
                    "A presentation entitlement did not complete its source quest.",
                ));
            }
            let id = next_id(after)?;
            added.push(self.quest_presentation(id, quest, reward)?);
        }
        for (skill, state) in &after.skills.clone() {
            let Some(old) = before.skills.get(skill) else {
                return Err(invalid_state("A skill appeared during UI observation."));
            };
            let source = self
                .content
                .skills
                .get(skill)
                .ok_or_else(|| unknown("Unknown rewarded skill."))?;
            let prior = skills::level_for_xp(source, old.xp_tenths)?;
            let current = skills::level_for_xp(source, state.xp_tenths)?;
            if current <= prior {
                continue;
            }
            let id = next_id(after)?;
            added.push(self.level_presentation(id, skill, current)?);
        }
        let bank_changed = before.bank != after.bank
            && before.runtime.ui.as_ref().map(|ui| ui.bank.revision)
                == after.runtime.ui.as_ref().map(|ui| ui.bank.revision);
        let bank = &after.bank;
        let ui = after.runtime.ui.as_mut().unwrap();
        bank_layout::reconcile(bank, &mut ui.bank, bank_changed)?;
        for reward in added {
            let since_quest = ui
                .rewards
                .iter()
                .rposition(|pending| pending.kind == RewardUiKind::Quest)
                .map_or(0, |index| index + 1);
            if reward.kind == RewardUiKind::LevelUp
                && let Some(pending) = ui.rewards[since_quest..].iter_mut().find(|pending| {
                    pending.kind == RewardUiKind::LevelUp && pending.skill == reward.skill
                })
            {
                let id = pending.id.clone();
                *pending = reward;
                pending.id = id.clone();
                pending.continuation = GameplayUiRequest::UiDismiss {
                    presentation_id: id,
                };
            } else {
                if ui.rewards.len() >= 64 {
                    return Err(invalid_state("Source presentation queue is full."));
                }
                ui.rewards.push(reward);
            }
        }
        for event in events {
            if let GameEvent::InterfaceOpened { interface } = event {
                if self
                    .content
                    .interfaces
                    .get(interface)
                    .is_some_and(|definition| definition.access == InterfaceAccess::Tab)
                {
                    ui.active_tab = Some(interface.clone());
                } else if interface == &definition.equipment_stats_interface {
                    ui.active_interface = Some(interface.clone());
                }
            }
        }
        self.validate_ui_state(after)
    }

    pub(crate) fn require_ui_modal_clear(
        &self,
        character: &CharacterState,
        intent: &GameIntent,
    ) -> GameResult<()> {
        if !matches!(intent, GameIntent::RequestLogout | GameIntent::Ui { .. })
            && character
                .runtime
                .ui
                .as_ref()
                .is_some_and(|ui| !ui.rewards.is_empty() || ui.confirmation.is_some())
        {
            return Err(GameError::new(
                GameErrorCode::Busy,
                "Dismiss or answer the current source presentation first.",
            ));
        }
        Ok(())
    }

    pub fn apply_ui_request(
        &self,
        world: &mut WorldState,
        actor: &ActorId,
        request: &GameplayUiRequest,
        rng: &mut impl RandomSource,
    ) -> GameResult<Vec<ActorEvent>> {
        self.check_world(world)?;
        if self.content.ui.is_none() {
            return Err(unavailable("game.ui.v1 is not configured."));
        }
        let mut draft = world.clone();
        let mut character = draft
            .characters
            .remove(actor)
            .ok_or_else(|| GameError::new(GameErrorCode::NotOwned, "Unknown UI actor."))?;
        character.migrate_engine_metadata(&self.content)?;
        self.prepare_ui(&mut character)?;
        self.input_permission(&character)?;
        let timed = self.ui_request_requires_tick(request)?;
        if timed
            && runtime::schedule(&character)?.command_seen
            && character.last_action_tick >= draft.tick
        {
            return Err(GameError::new(
                GameErrorCode::Busy,
                "An action was already accepted this tick.",
            ));
        }
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
        let before = character.clone();
        let mut events = Vec::new();
        let mut recipients = Vec::new();
        match request {
            GameplayUiRequest::UiDismiss { presentation_id } => {
                let ui = ui_mut(&mut character)?;
                if ui
                    .rewards
                    .first()
                    .is_some_and(|reward| &reward.id == presentation_id)
                {
                    ui.rewards.remove(0);
                } else if ui
                    .document
                    .as_ref()
                    .is_some_and(|document| &document.id == presentation_id)
                {
                    ui.document = None;
                } else {
                    return Err(GameError::new(
                        GameErrorCode::StaleCommand,
                        "The presentation no longer matches this dismissal.",
                    ));
                }
            }
            GameplayUiRequest::UiDocumentPage { document_id, page } => {
                let document = ui_mut(&mut character)?
                    .document
                    .as_mut()
                    .filter(|document| &document.id == document_id)
                    .ok_or_else(|| {
                        GameError::new(
                            GameErrorCode::StaleCommand,
                            "The document is no longer open.",
                        )
                    })?;
                if usize::from(*page) >= document.pages.len() {
                    return Err(invalid_state("Document page is out of range."));
                }
                document.page = *page;
            }
            GameplayUiRequest::ProductionSelect {
                menu_id,
                recipe,
                quantity,
                mode,
            } => {
                self.require_ui_free(&character)?;
                let menu = ui_mut(&mut character)?
                    .production
                    .clone()
                    .filter(|menu| &menu.id == menu_id)
                    .ok_or_else(|| {
                        GameError::new(
                            GameErrorCode::StaleCommand,
                            "Production menu is no longer open.",
                        )
                    })?;
                if !menu.recipes.contains(recipe) || menu.instance != character.runtime.instance {
                    return Err(GameError::new(
                        GameErrorCode::NotOwned,
                        "Recipe does not belong to the open source facility.",
                    ));
                }
                if let Some(selection) = &menu.inventory_selection {
                    self.production_inventory_permission(&character, selection, &menu.recipes)?;
                }
                self.production_permission(
                    &draft,
                    &character,
                    recipe,
                    menu.target.as_ref(),
                    *quantity,
                    *mode,
                )?;
                self.start_selected_production(
                    &mut draft,
                    &mut character,
                    recipe,
                    menu.target,
                    *quantity,
                    *mode,
                )?;
            }
            GameplayUiRequest::ItemAction {
                inventory_slot,
                expected_item,
                expected_instance,
                action,
            } => {
                self.require_ui_free(&character)?;
                events.extend(self.item_ui_action(
                    &mut draft,
                    &mut character,
                    *inventory_slot,
                    expected_item,
                    expected_instance.as_ref(),
                    action,
                )?);
            }
            GameplayUiRequest::OpenDeathPreview => {
                self.death_preview_permission(&draft, &character)?;
                ui_mut(&mut character)?.death_preview = true;
            }
            GameplayUiRequest::RequestRecoveryDiscard {
                death,
                storage,
                items,
            } => {
                self.require_ui_free(&character)?;
                self.open_discard_confirmation(&draft, &mut character, death, *storage, items)?;
            }
            GameplayUiRequest::CofferOffer {
                inventory_slot,
                expected_item,
                expected_instance,
                quantity,
            } => {
                self.require_ui_free(&character)?;
                self.open_coffer_confirmation(
                    &draft,
                    &mut character,
                    *inventory_slot,
                    expected_item,
                    expected_instance.as_ref(),
                    *quantity,
                )?;
            }
            GameplayUiRequest::UiConfirm {
                confirmation_id,
                accept,
            } => {
                events.extend(self.confirm_ui(
                    &mut draft,
                    &mut character,
                    confirmation_id,
                    *accept,
                )?);
            }
            GameplayUiRequest::PublicChat { channel, text } => {
                recipients = self.send_public_chat(&mut draft, &mut character, channel, text)?;
            }
            bank => {
                self.require_ui_free(&character)?;
                events.extend(self.bank_ui_action(&draft, &mut character, bank)?);
            }
        }
        self.refresh_combat_style(&mut character)?;
        self.progress(&mut draft, &mut character, &before, &mut events, rng)?;
        self.check_reward_atomicity(&before, &character)?;
        self.session_close_event(&before, &character, &mut events)?;
        self.observe_ui(&before, &mut character, &events)?;
        if timed {
            character.last_action_tick = draft.tick;
            runtime::schedule_mut(&mut character)?.command_seen = true;
        }
        character.runtime.last_active_tick = Some(draft.tick);
        crate::validation::character(&character, &self.content)?;
        draft.characters.insert(actor.clone(), character);
        draft.validate_runtime(&self.content)?;
        *world = draft;
        recipients.extend(tag(actor, events));
        Ok(recipients)
    }

    /// Presentation/preferences/chat do not consume a source action phase.
    /// Item/container mutations and production share ordinary tick admission.
    pub fn ui_request_requires_tick(&self, request: &GameplayUiRequest) -> GameResult<bool> {
        Ok(match request {
            GameplayUiRequest::ProductionSelect { .. }
            | GameplayUiRequest::BankDepositEquipment
            | GameplayUiRequest::BankPlaceholder { .. }
            | GameplayUiRequest::BankWithdrawEntry { .. }
            | GameplayUiRequest::UiConfirm { accept: true, .. } => true,
            GameplayUiRequest::ItemAction {
                expected_item,
                action,
                ..
            } => {
                let definition = self
                    .content
                    .ui
                    .as_ref()
                    .and_then(|ui| ui.item_actions.get(expected_item))
                    .and_then(|actions| actions.iter().find(|definition| &definition.id == action))
                    .ok_or_else(|| unknown("This item has no bound source action."))?;
                !matches!(
                    definition.action,
                    ItemUiAction::Read { .. } | ItemUiAction::Unavailable { .. }
                )
            }
            _ => false,
        })
    }

    fn require_ui_free(&self, character: &CharacterState) -> GameResult<()> {
        let ui = character
            .runtime
            .ui
            .as_ref()
            .ok_or_else(|| unavailable("UI state is not initialized."))?;
        if !ui.rewards.is_empty() || ui.confirmation.is_some() {
            return Err(GameError::new(
                GameErrorCode::Busy,
                "The current source presentation must be answered first.",
            ));
        }
        Ok(())
    }

    pub(crate) fn open_production_menu(
        &self,
        character: &mut CharacterState,
        target: Option<&WorldTarget>,
        inventory_selection: Option<ProductionInventorySelection>,
        action: &str,
        recipes: &[RecipeId],
    ) -> GameResult<Vec<GameEvent>> {
        let definition = self
            .content
            .ui
            .as_ref()
            .ok_or_else(|| unavailable("Source production UI is not configured."))?;
        let interface = recipes
            .first()
            .and_then(|recipe| definition.production_interfaces.get(recipe))
            .ok_or_else(|| {
                invalid_content("Production menu lacks its source interface binding.")
            })?;
        if recipes
            .iter()
            .any(|recipe| definition.production_interfaces.get(recipe) != Some(interface))
        {
            return Err(invalid_content(
                "Production menu spans incompatible source interfaces.",
            ));
        }
        self.require_context_interface(character, interface)?;
        let id = next_id(character)?;
        ui_mut(character)?.production = Some(ProductionUiSession {
            id,
            interface: interface.clone(),
            target: target.cloned(),
            inventory_selection,
            instance: character.runtime.instance.clone(),
            recipes: recipes.to_vec(),
            action: action.into(),
        });
        Ok(vec![GameEvent::InterfaceOpened {
            interface: interface.clone(),
        }])
    }

    pub(crate) fn refresh_production_ui(&self, world: &mut WorldState) -> GameResult<()> {
        let mut close = Vec::new();
        for (actor, character) in &world.characters {
            let Some(menu) = character
                .runtime
                .ui
                .as_ref()
                .and_then(|ui| ui.production.as_ref())
            else {
                continue;
            };
            let gone = matches!(&menu.target, Some(WorldTarget::TemporaryObject { object }) if !world.runtime.temporary_objects.contains_key(object));
            let valid = if gone || menu.instance != character.runtime.instance {
                false
            } else if let Some(target) = &menu.target {
                self.interaction_options(world, actor, target)?
                    .iter()
                    .any(|option| option.name == menu.action && option.permission.allowed)
            } else if let Some(selection) = &menu.inventory_selection {
                match self.production_inventory_permission(character, selection, &menu.recipes) {
                    Ok(()) => true,
                    Err(error) if crate::is_interruption(&error.code) => false,
                    Err(error) => return Err(error),
                }
            } else {
                return Err(invalid_state(
                    "Targetless production menu lost its source inventory selection.",
                ));
            };
            if !valid {
                close.push(actor.clone());
            }
        }
        for actor in close {
            ui_mut(world.characters.get_mut(&actor).unwrap())?.production = None;
        }
        Ok(())
    }
}

pub(super) fn ui_mut(character: &mut CharacterState) -> GameResult<&mut GameplayUiRuntime> {
    character
        .runtime
        .ui
        .as_mut()
        .ok_or_else(|| unavailable("Versioned UI state is not initialized."))
}

pub(super) fn next_id(character: &mut CharacterState) -> GameResult<String> {
    let ui = ui_mut(character)?;
    let id = ui.next_id;
    ui.next_id = id
        .checked_add(1)
        .filter(|value| *value <= i64::MAX as u64)
        .ok_or_else(|| invalid_state("UI identity exhausted."))?;
    Ok(format!("ui.{id}"))
}
