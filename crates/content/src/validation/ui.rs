use super::*;

impl Validator<'_> {
    pub(super) fn gameplay_ui(&self) -> GameResult<()> {
        let Some(ui) = &self.content.ui else {
            return Ok(());
        };
        if let Some(audio) = &ui.audio_authority {
            self.audio_authority(audio)?;
        }
        if ui.version != GAMEPLAY_UI_VIEW_VERSION
            || ui.bank.maximum_tabs == 0
            || ui.bank.maximum_tabs > 9
            || ui.chat.maximum_bytes == 0
            || ui.chat.maximum_bytes > 160
            || ui.chat.radius == 0
            || ui.chat.radius > 32
            || ui.chat.messages_per_window == 0
            || ui.chat.messages_per_window > 128
            || ui.chat.window_ticks == 0
            || !ui.stage_interfaces.keys().eq(self.content.tutorial.keys())
            || !ui.stage_overlays.keys().eq(self.content.tutorial.keys())
        {
            return Err(invalid(
                "ui",
                "invalid version, source bounds or incomplete semantic tutorial-state coverage",
            ));
        }
        self.interface(&ui.level_up.interface, "ui.level_up.interface")?;
        self.interface(
            &ui.equipment_stats_interface,
            "ui.equipment_stats_interface",
        )?;
        if self.content.interfaces[&ui.equipment_stats_interface].access
            != InterfaceAccess::Contextual
        {
            return Err(invalid(
                "ui.equipment_stats_interface",
                "equipment statistics require the declared self-owned contextual interface",
            ));
        }
        self.interface(&ui.death_preview_interface, "ui.death_preview_interface")?;
        text(&ui.level_up.title, "ui.level_up.title", 512)?;
        text(&ui.level_up.line, "ui.level_up.line", 2048)?;
        if ui.bank.unavailable_containers.len() > 32
            || ui
                .bank
                .unavailable_containers
                .iter()
                .map(|control| &control.id)
                .collect::<BTreeSet<_>>()
                .len()
                != ui.bank.unavailable_containers.len()
        {
            return Err(invalid(
                "ui.bank.unavailable_containers",
                "invalid declared container-control bound/identity",
            ));
        }
        for control in &ui.bank.unavailable_containers {
            token(&control.id, "ui.bank.container.id")?;
            text(&control.label, "ui.bank.container.label", 128)?;
            text(&control.reason, "ui.bank.container.reason", 2048)?;
        }
        self.guard(&ui.chat.guard, "ui.chat.guard")?;
        if let Some(recovery) = &ui.recovery {
            for rule in [&recovery.grave_bank, &recovery.office_bank] {
                match rule {
                    RecoveryBankRule::Allowed { guard, .. } => {
                        self.guard(guard, "ui.recovery.bank.guard")?
                    }
                    RecoveryBankRule::Unavailable { reason, .. } => {
                        text(reason, "ui.recovery.bank.reason", 2048)?
                    }
                }
            }
        }
        for (stage, rules) in &ui.stage_interfaces {
            let ids: BTreeSet<_> = rules.iter().map(|rule| &rule.interface).collect();
            if ids.len() != rules.len()
                || ids.len() != self.content.interfaces.len()
                || !ids.into_iter().eq(self.content.interfaces.keys())
            {
                return Err(invalid(
                    "ui.stage_interfaces",
                    format!("{stage} does not classify every source interface exactly once"),
                ));
            }
            for rule in rules {
                self.guard(&rule.guard, "ui.interface.guard")?;
                if let Some(reason) = &rule.unavailable_reason {
                    text(reason, "ui.interface.unavailable_reason", 2048)?;
                }
            }
            if let Some(id) = &ui.stage_overlays[stage] {
                self.interface(id, "ui.stage_overlay")?;
            }
        }
        for (recipe, interface) in &ui.production_interfaces {
            if !self.content.recipes.contains_key(recipe) || ui.direct_production.contains(recipe) {
                return Err(invalid(
                    "ui.production",
                    "unknown recipe or conflicting direct/menu source behavior",
                ));
            }
            self.interface(interface, "ui.production.interface")?;
            if self.content.interfaces[interface].access != InterfaceAccess::Contextual {
                return Err(invalid(
                    "ui.production",
                    "production requires a contextual source interface",
                ));
            }
        }
        if ui
            .direct_production
            .iter()
            .any(|recipe| !self.content.recipes.contains_key(recipe))
        {
            return Err(invalid(
                "ui.direct_production",
                "unknown one-click source recipe",
            ));
        }
        let abilities: BTreeSet<_> = self
            .content
            .mechanics
            .prayers
            .keys()
            .map(ToString::to_string)
            .chain(
                self.content
                    .mechanics
                    .spells
                    .keys()
                    .map(ToString::to_string),
            )
            .collect();
        if !abilities.iter().eq(ui.ability_names.keys()) {
            return Err(invalid(
                "ui.ability_names",
                "every declared ability needs exactly its own source label",
            ));
        }
        for name in ui.ability_names.values() {
            text(name, "ui.ability_name", 128)?;
        }
        let weapons: BTreeMap<_, _> = self
            .content
            .items
            .iter()
            .filter_map(|(id, item)| {
                item.equipment
                    .as_ref()
                    .and_then(|equipment| equipment.weapon.as_ref())
                    .map(|weapon| (id, weapon))
            })
            .collect();
        if !weapons.keys().copied().eq(ui.weapon_style_names.keys()) {
            return Err(invalid(
                "ui.weapon_style_names",
                "every declared weapon needs its native source style labels",
            ));
        }
        for (item, weapon) in weapons {
            let styles: BTreeSet<_> = weapon.styles.iter().collect();
            if !styles.into_iter().eq(ui.weapon_style_names[item].keys()) {
                return Err(invalid(
                    "ui.weapon_style_names",
                    "source style labels do not match the weapon's actual choices",
                ));
            }
            for name in ui.weapon_style_names[item].values() {
                text(name, "ui.weapon_style_name", 128)?;
            }
        }
        if let Some(policy) = &self.content.mechanics.player_combat
            && let Some(unarmed) = mechanics::binding(&policy.unarmed, "ui.unarmed_styles")?
        {
            let styles: BTreeSet<_> = unarmed.styles.iter().collect();
            if !styles.into_iter().eq(ui.unarmed_style_names.keys()) {
                return Err(invalid(
                    "ui.unarmed_style_names",
                    "unarmed source labels do not match the actual choices",
                ));
            }
        }
        for name in ui.unarmed_style_names.values() {
            text(name, "ui.unarmed_style_name", 128)?;
        }
        for interaction in self
            .content
            .spawns
            .values()
            .flat_map(|spawn| &spawn.interactions)
            .chain(
                self.content
                    .mechanics
                    .temporary_objects
                    .values()
                    .flat_map(|object| &object.interactions),
            )
        {
            if let InteractionAction::Production { recipes } = &interaction.action {
                if recipes.len() == 1 && ui.direct_production.contains(&recipes[0]) {
                    continue;
                }
                let first = recipes
                    .first()
                    .and_then(|recipe| ui.production_interfaces.get(recipe))
                    .ok_or_else(|| {
                        invalid(
                            "ui.production",
                            "source production interaction lacks a menu binding",
                        )
                    })?;
                if recipes
                    .iter()
                    .any(|recipe| ui.production_interfaces.get(recipe) != Some(first))
                {
                    return Err(invalid("ui.production", "one menu mixes source interfaces"));
                }
            }
        }
        let mut once = BTreeMap::new();
        let mut effects: Vec<_> = self
            .content
            .dialogues
            .values()
            .flat_map(|dialogue| &dialogue.nodes)
            .flat_map(|node| &node.choices)
            .flat_map(|choice| &choice.effects)
            .chain(
                self.content
                    .tutorial
                    .values()
                    .flat_map(|stage| &stage.transitions)
                    .flat_map(|edge| &edge.effects),
            )
            .chain(
                self.content
                    .quests
                    .values()
                    .flat_map(|quest| &quest.transitions)
                    .flat_map(|edge| &edge.effects),
            )
            .collect();
        while let Some(effect) = effects.pop() {
            match effect {
                Effect::Once {
                    entitlement,
                    effects: nested,
                } => {
                    if let Some(previous) = once.insert(entitlement, nested)
                        && previous != nested
                        && ui
                            .quest_rewards
                            .values()
                            .any(|reward| &reward.entitlement == entitlement)
                    {
                        return Err(invalid(
                            "ui.quest_reward",
                            "one presentation entitlement has divergent source effect batches",
                        ));
                    }
                    effects.extend(nested);
                }
                Effect::Conditional {
                    effects: nested, ..
                } => effects.extend(nested),
                _ => {}
            }
        }
        let mut entitlements = BTreeSet::new();
        for (quest, presentation) in &ui.quest_rewards {
            if !entitlements.insert(&presentation.entitlement) {
                return Err(invalid(
                    "ui.quest_reward",
                    "one source reward entitlement cannot present multiple quests",
                ));
            }
            let source = self
                .content
                .quests
                .get(quest)
                .ok_or_else(|| invalid("ui.quest_rewards", "unknown quest"))?;
            self.interface(&presentation.interface, "ui.quest_reward.interface")?;
            text(&presentation.title, "ui.quest_reward.title", 512)?;
            if presentation.lines.is_empty() || presentation.lines.len() > 64 {
                return Err(invalid(
                    "ui.quest_reward.lines",
                    "missing or oversized source reward text",
                ));
            }
            for line in &presentation.lines {
                text(line, "ui.quest_reward.line", 2048)?;
            }
            let batch = once.get(&presentation.entitlement).ok_or_else(|| {
                invalid("ui.quest_reward", "no matching atomic source entitlement")
            })?;
            let mut xp = BTreeMap::new();
            let mut items = BTreeMap::new();
            let mut points = 0_u32;
            let mut completes = false;
            for effect in *batch {
                match effect {
                    Effect::GiveItems { items: values } => {
                        for item in values {
                            *items.entry(item.item.clone()).or_insert(0_u64) +=
                                u64::from(item.quantity.get());
                        }
                    }
                    Effect::AwardXp { rewards } => {
                        for award in rewards {
                            let total = xp.entry(award.skill.clone()).or_insert(0_u64);
                            *total = total.checked_add(award.amount_tenths).ok_or_else(|| {
                                invalid("ui.quest_reward", "source XP payload exceeds its bound")
                            })?;
                        }
                    }
                    Effect::AddQuestPoints { amount } => points += u32::from(*amount),
                    Effect::SetQuestStage { quest: id, stage }
                        if id == quest && stage == &source.completed_stage =>
                    {
                        completes = true
                    }
                    Effect::Conditional { .. } | Effect::Once { .. } | Effect::Grant { .. } => {
                        return Err(invalid(
                            "ui.quest_reward",
                            "reward payload cannot guess a conditional/nested grant branch",
                        ));
                    }
                    _ => {}
                }
            }
            let expected_xp: BTreeMap<_, _> = presentation
                .xp
                .iter()
                .map(|reward| (reward.skill.clone(), reward.amount_tenths))
                .collect();
            let expected_items: BTreeMap<_, _> = presentation
                .items
                .iter()
                .map(|item| (item.item.clone(), u64::from(item.quantity.get())))
                .collect();
            if !completes
                || xp != expected_xp
                || items != expected_items
                || points != presentation.quest_points
                || expected_xp.len() != presentation.xp.len()
                || expected_items.len() != presentation.items.len()
            {
                return Err(invalid(
                    "ui.quest_reward",
                    "presentation does not equal its actual source entitlement effects",
                ));
            }
        }
        for (item, actions) in &ui.item_actions {
            let item = self.item(item, "ui.item_actions.item")?;
            let mut ids = BTreeSet::new();
            for action in actions {
                if !ids.insert(&action.id) {
                    return Err(invalid("ui.item_actions", "duplicate source operation"));
                }
                token(&action.id, "ui.item_action.id")?;
                text(&action.label, "ui.item_action.label", 128)?;
                self.guard(&action.guard, "ui.item_action.guard")?;
                if item.unnoted_variant.is_some()
                    && !matches!(action.action, ItemUiAction::Unavailable { .. })
                {
                    return Err(invalid(
                        "ui.item_action",
                        "a note cannot expose its base item's consumable/read operation",
                    ));
                }
                match &action.action {
                    ItemUiAction::ConsumeRecipe { recipe } => {
                        let recipe =
                            self.content.recipes.get(recipe).ok_or_else(|| {
                                invalid("ui.item_action", "unknown consume recipe")
                            })?;
                        if recipe.inputs.len() != 1
                            || recipe.inputs[0].item != item.id
                            || recipe.inputs[0].quantity.get() != 1
                            || !recipe.outputs.is_empty()
                            || !recipe.failed_outputs.is_empty()
                            || !recipe.target_objects.is_empty()
                            || !matches!(recipe.success.domain, ChanceDomain::Constant)
                            || recipe.success.numerator_at_level_1 != recipe.success.denominator
                            || recipe.mechanics.as_ref().is_none_or(|mechanics| {
                                !matches!(mechanics.lifecycle, RecipeLifecycle::ConsumeOnly)
                            })
                        {
                            return Err(invalid(
                                "ui.item_action",
                                "selected consumption requires its actual one-item ConsumeOnly source recipe",
                            ));
                        }
                    }
                    ItemUiAction::Drink {
                        replacement,
                        delay_ticks,
                        restore,
                        skills,
                        ..
                    } => {
                        self.item(replacement, "ui.drink.replacement")?;
                        if let Some(delay) =
                            mechanics::binding(delay_ticks, "ui.drink.delay_ticks")?
                            && (*delay == 0 || *delay > 1000)
                        {
                            return Err(invalid("ui.drink", "invalid independent drink timer"));
                        }
                        for adjustment in skills {
                            self.skill(&adjustment.skill, "ui.drink.skill")?;
                            if adjustment.percent.denominator == 0 {
                                return Err(invalid(
                                    "ui.drink",
                                    "invalid skill adjustment divisor",
                                ));
                            }
                        }
                        if restore.is_empty() && skills.is_empty() {
                            return Err(invalid("ui.drink", "source drink has no bound effect"));
                        }
                    }
                    ItemUiAction::Empty { replacement } => {
                        self.item(replacement, "ui.empty.replacement")?;
                    }
                    ItemUiAction::Read {
                        interface,
                        title,
                        pages,
                        native_map,
                        ..
                    } => {
                        self.interface(interface, "ui.read.interface")?;
                        text(title, "ui.read.title", 512)?;
                        if (!native_map && pages.is_empty())
                            || pages.len() > 128
                            || pages.iter().map(String::len).sum::<usize>() > 32768
                        {
                            return Err(invalid(
                                "ui.read.pages",
                                "missing or oversized source pages",
                            ));
                        }
                        for page in pages {
                            text(page, "ui.read.page", 16384)?;
                        }
                    }
                    ItemUiAction::Unavailable { reason } => {
                        text(reason, "ui.item_action.reason", 2048)?
                    }
                }
            }
        }
        if let Some(coffer) = mechanics::binding(&ui.coffer, "ui.coffer")? {
            if coffer.credit.denominator == 0
                || coffer.credit.numerator == 0
                || coffer.maximum_balance == 0
                || coffer.maximum_balance > i64::MAX as u64
                || coffer.minimum_value == 0
            {
                return Err(invalid("ui.coffer", "invalid source valuation bounds"));
            }
            for item in &coffer.eligible_items {
                if !self.item(item, "ui.coffer.item")?.tradable
                    || !coffer.exchange_values.contains_key(item)
                {
                    return Err(invalid(
                        "ui.coffer",
                        "eligible item lacks tradability or separate exchange valuation",
                    ));
                }
                for item in coffer.exchange_values.keys() {
                    self.item(item, "ui.coffer.exchange_value")?;
                }
            }
        }
        if let Some(base) = mechanics::binding(&ui.appearance_base, "ui.appearance_base")? {
            token(base.asset.as_str(), "ui.appearance_base.asset")?;
            text(&base.adaptation, "ui.appearance_base.adaptation", 256)?;
        }
        Ok(())
    }
}
