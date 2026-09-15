use super::*;

impl WorldEngine {
    pub(super) fn quest_presentation(
        &self,
        id: String,
        quest: &QuestId,
        source: &QuestUiDefinition,
    ) -> GameResult<RewardUiView> {
        Ok(RewardUiView {
            id: id.clone(),
            kind: RewardUiKind::Quest,
            interface: source.interface.clone(),
            title: source.title.clone(),
            lines: source.lines.clone(),
            items: source
                .items
                .iter()
                .map(|item| self.ui_item(item))
                .collect::<GameResult<_>>()?,
            xp: source
                .xp
                .iter()
                .map(|xp| UiXpAward {
                    skill: xp.skill.clone(),
                    amount_tenths: xp.amount_tenths.to_string(),
                })
                .collect(),
            quest_points: source.quest_points,
            quest: Some(quest.clone()),
            skill: None,
            level: None,
            continuation: GameplayUiRequest::UiDismiss {
                presentation_id: id,
            },
        })
    }

    pub(super) fn level_presentation(
        &self,
        id: String,
        skill: &SkillId,
        level: u16,
    ) -> GameResult<RewardUiView> {
        let source = self
            .content
            .skills
            .get(skill)
            .ok_or_else(|| unknown("Unknown presented skill."))?;
        let definition = &self
            .content
            .ui
            .as_ref()
            .ok_or_else(|| unavailable("UI content is absent."))?
            .level_up;
        Ok(RewardUiView {
            id: id.clone(),
            kind: RewardUiKind::LevelUp,
            interface: definition.interface.clone(),
            title: definition
                .title
                .replace("{skill}", &source.name)
                .replace("{level}", &level.to_string()),
            lines: vec![
                definition
                    .line
                    .replace("{skill}", &source.name)
                    .replace("{level}", &level.to_string()),
            ],
            items: Vec::new(),
            xp: Vec::new(),
            quest_points: 0,
            quest: None,
            skill: Some(skill.clone()),
            level: Some(level),
            continuation: GameplayUiRequest::UiDismiss {
                presentation_id: id,
            },
        })
    }

    pub(super) fn validate_ui_state(&self, character: &CharacterState) -> GameResult<()> {
        let definition = self
            .content
            .ui
            .as_ref()
            .ok_or_else(|| unavailable("UI content is absent."))?;
        let ui = character
            .runtime
            .ui
            .as_ref()
            .ok_or_else(|| unavailable("UI state needs explicit initialization."))?;
        ui.validate_shape()?;
        bank_layout::validate(&character.bank, &ui.bank)?;
        if ui.bank.entries.iter().any(|entry| {
            entry.tab > definition.bank.maximum_tabs
                || self
                    .content
                    .items
                    .get(&entry.item)
                    .is_none_or(|item| item.unnoted_variant.is_some())
        }) {
            return Err(invalid_state(
                "Bank metadata references an unsupported tab or noncanonical item.",
            ));
        }
        for reward in &ui.rewards {
            let expected = match reward.kind {
                RewardUiKind::Quest => {
                    let quest = reward.quest.as_ref().ok_or_else(|| {
                        invalid_state("Quest presentation has no source identity.")
                    })?;
                    let source = definition
                        .quest_rewards
                        .get(quest)
                        .ok_or_else(|| invalid_state("Unknown source quest presentation."))?;
                    if !character
                        .runtime
                        .entitlements
                        .contains_key(&source.entitlement)
                        || character.quests.get(quest).is_none_or(|state| {
                            state.stage != self.content.quests[quest].completed_stage
                        })
                    {
                        return Err(invalid_state(
                            "Quest presentation has no committed completion entitlement.",
                        ));
                    }
                    self.quest_presentation(reward.id.clone(), quest, source)?
                }
                RewardUiKind::LevelUp => {
                    let skill = reward
                        .skill
                        .as_ref()
                        .ok_or_else(|| invalid_state("Level presentation has no skill."))?;
                    let level = reward
                        .level
                        .ok_or_else(|| invalid_state("Level presentation has no earned level."))?;
                    if level < 2 || level > self.level(character, skill, SkillLevelBasis::Base)? {
                        return Err(invalid_state(
                            "Level presentation exceeds the actor's actually earned level.",
                        ));
                    }
                    self.level_presentation(reward.id.clone(), skill, level)?
                }
            };
            if &expected != reward {
                return Err(invalid_state(
                    "Persisted presentation disagrees with its committed source payload.",
                ));
            }
        }
        if let Some(menu) = &ui.production
            && menu
                .recipes
                .iter()
                .any(|recipe| definition.production_interfaces.get(recipe) != Some(&menu.interface))
        {
            return Err(invalid_state(
                "Production context disagrees with its source menu binding.",
            ));
        }
        if let Some(selection) = ui
            .production
            .as_ref()
            .and_then(|menu| menu.inventory_selection.as_ref())
        {
            self.production_inventory_recipes(selection, &ui.production.as_ref().unwrap().recipes)?;
        }
        if let Some(selection) = &ui.production_input {
            let recipe = match &character.activity {
                Activity::Producing {
                    recipe,
                    target: None,
                    ..
                }
                | Activity::ProducingAt {
                    recipe,
                    target: None,
                    ..
                }
                | Activity::ProducingSelected {
                    recipe,
                    target: None,
                    ..
                } => recipe,
                _ => {
                    return Err(invalid_state(
                        "Selected source inputs have no matching inventory production activity.",
                    ));
                }
            };
            self.production_inventory_recipes(selection, std::slice::from_ref(recipe))?;
        }
        if let Some(document) = &ui.document
            && !definition.item_actions.values().flatten().any(|action| matches!(&action.action,
                ItemUiAction::Read { interface, title, pages, map_asset, native_map }
                    if interface == &document.interface && title == &document.title && pages == &document.pages
                        && map_asset == &document.map_asset && native_map == &document.native_map))
        {
            return Err(invalid_state("Persisted document is not its bound source text or native map."));
        }
        if let Some(confirmation) = &ui.confirmation {
            let valid = match &confirmation.action {
                UiConfirmationAction::Discard { items, .. } => {
                    confirmation.view.kind == "discard_recovery"
                        && confirmation.view.credit.is_none()
                        && !items.is_empty()
                        && items.len() <= 256
                        && items
                            .iter()
                            .map(|item| &item.id)
                            .collect::<BTreeSet<_>>()
                            .len()
                            == items.len()
                        && items
                            .iter()
                            .all(|item| item.quantity > 0 && item.quantity <= MAX_STACK_QUANTITY)
                }
                UiConfirmationAction::Coffer {
                    item,
                    quantity,
                    credit,
                    ..
                } => {
                    confirmation.view.kind == "coffer_offer"
                        && confirmation
                            .view
                            .credit
                            .as_ref()
                            .is_some_and(|value| value == &credit.to_string())
                        && definition.coffer.require()?.eligible_items.contains(item)
                        && *quantity > 0
                        && *quantity <= MAX_STACK_QUANTITY
                }
            };
            if !valid {
                return Err(invalid_state("Invalid typed source confirmation payload."));
            }
            for item in &confirmation.view.items {
                let source = self
                    .content
                    .items
                    .get(&item.item)
                    .ok_or_else(|| invalid_state("Unknown confirmation item."))?;
                if item.name != source.name
                    || item.source_id != source.source_id
                    || item.asset != source.asset
                    || item.quantity == 0
                    || item.quantity > MAX_STACK_QUANTITY
                {
                    return Err(invalid_state(
                        "Confirmation item does not match source metadata.",
                    ));
                }
            }
        }
        Ok(())
    }
}
