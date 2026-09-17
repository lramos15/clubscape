use super::*;

impl Validator<'_> {
    pub(super) fn skills(&self) -> GameResult<()> {
        let mut ids = BTreeMap::new();
        for skill in self.content.skills.values() {
            let path = format!("skills.{}", skill.id);
            text(&skill.name, &format!("{path}.name"), 256)?;
            source_id(&mut ids, skill.source_id, skill.id.as_str(), &path)?;
            let thresholds = &skill.xp_thresholds_tenths;
            if thresholds.is_empty()
                || thresholds.len() > usize::from(u16::MAX)
                || thresholds[0] != 0
                || thresholds.windows(2).any(|pair| pair[0] >= pair[1])
                || skill.maximum_xp_tenths == 0
                || thresholds
                    .last()
                    .is_some_and(|last| *last > skill.maximum_xp_tenths)
            {
                return Err(invalid(
                    &path,
                    "XP thresholds must start at 0, strictly increase, fit u16 levels, and not exceed positive maximum XP",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn items(&self) -> GameResult<()> {
        let mut ids = BTreeMap::new();
        for item in self.content.items.values() {
            let path = format!("items.{}", item.id);
            text(&item.name, &format!("{path}.name"), 256)?;
            if let Some(number) = item.source_id {
                source_id(&mut ids, number, item.id.as_str(), &path)?;
            }
            if item.base_value > MAX_STACK_QUANTITY || item.healing == Some(0) {
                return Err(invalid(
                    &path,
                    "item value exceeds the quantity limit or healing is zero",
                ));
            }
            if item.noted_variant.is_some() && item.unnoted_variant.is_some() {
                return Err(invalid(
                    &path,
                    "an item cannot be both a note and a base item",
                ));
            }
            if let Some(note) = &item.noted_variant {
                let note = self.item(note, &path)?;
                if !item.stackable.is_never()
                    || note.id == item.id
                    || note.unnoted_variant.as_ref() != Some(&item.id)
                {
                    return Err(invalid(
                        &path,
                        "noted_variant requires a nonstackable base and a reciprocal, distinct note",
                    ));
                }
            }
            if let Some(base) = &item.unnoted_variant {
                let base = self.item(base, &path)?;
                if !item.stackable.is_always()
                    || item.equipment.is_some()
                    || item.healing.is_some()
                    || base.id == item.id
                    || !base.stackable.is_never()
                    || base.noted_variant.as_ref() != Some(&item.id)
                {
                    return Err(invalid(
                        &path,
                        "a note must stack, be unusable as equipment/food, and reciprocally reference a distinct nonstackable base",
                    ));
                }
            }
            if let Some(equipment) = &item.equipment {
                let path = format!("{path}.equipment");
                nonempty(equipment.occupied_slots.len(), &path)?;
                unique(equipment.occupied_slots.iter(), &path)?;
                if !equipment.occupied_slots.contains(&equipment.slot) {
                    return Err(invalid(
                        &path,
                        "occupied_slots must include the primary slot",
                    ));
                }
                for slot in &equipment.occupied_slots {
                    if !self.slots.contains(slot) {
                        return Err(invalid(&path, format!("undefined equipment slot {slot}")));
                    }
                }
                self.item_mechanics(item, &path)?;
                self.requirements(&equipment.requirements, &path)?;
                bonuses(&equipment.bonuses, &path)?;
                bounded(equipment.attack_styles.len(), &path)?;
                unique(equipment.attack_styles.iter(), &path)?;
                for style in &equipment.attack_styles {
                    token(style, &path)?;
                }
                if equipment.attack_speed_ticks == Some(0)
                    || equipment.attack_speed_ticks.is_some() == equipment.attack_styles.is_empty()
                {
                    return Err(invalid(
                        &path,
                        "attack styles and a positive attack speed must be supplied together",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn objects_and_npcs(&self) -> GameResult<()> {
        let mut object_ids = BTreeMap::new();
        for object in self.content.objects.values() {
            let path = format!("objects.{}", object.id);
            text(&object.name, &path, 256)?;
            source_id(&mut object_ids, object.source_id, object.id.as_str(), &path)?;
            if object.size_x == 0 || object.size_y == 0 {
                return Err(invalid(&path, "object dimensions must be positive"));
            }
            self.object_mechanics(object, &path)?;
        }
        let mut npc_ids = BTreeMap::new();
        for npc in self.content.npcs.values() {
            let path = format!("npcs.{}", npc.id);
            text(&npc.name, &path, 256)?;
            source_id(&mut npc_ids, npc.source_id, npc.id.as_str(), &path)?;
            if npc.size == 0 {
                return Err(invalid(&path, "NPC size must be positive"));
            }
            self.npc_navigation(npc, &path)?;
            if let Some(combat) = &npc.combat {
                if combat.hitpoints == 0
                    || combat.attack_speed_ticks == 0
                    || (combat.mechanics.is_none()
                        && combat.respawn_ticks.is_none_or(|ticks| ticks == 0))
                {
                    return Err(invalid(
                        &path,
                        "combat hitpoints, attack duration, and respawn duration must be positive",
                    ));
                }
                bonuses(&combat.bonuses, &path)?;
                self.npc_combat_mechanics(combat, &path)?;
                bounded(combat.drops.len(), &path)?;
                let mut totals = BTreeMap::<&ItemId, u64>::new();
                for (index, drop) in combat.drops.iter().enumerate() {
                    let path = format!("{path}.combat.drops[{index}]");
                    self.item(&drop.item, &path)?;
                    if drop.minimum_quantity == 0
                        || drop.minimum_quantity > drop.maximum_quantity
                        || drop.maximum_quantity > MAX_STACK_QUANTITY
                        || drop.denominator == 0
                        || drop.numerator > drop.denominator
                    {
                        return Err(invalid(&path, "invalid drop quantity range or probability"));
                    }
                    if drop.numerator != 0 {
                        let total = totals.entry(&drop.item).or_default();
                        *total += u64::from(drop.maximum_quantity);
                        if *total > u64::from(MAX_STACK_QUANTITY) {
                            return Err(invalid(
                                &path,
                                "simultaneous independent drops could overflow an item stack",
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn interface_definitions(&self) -> GameResult<()> {
        let mut source_ids = BTreeMap::new();
        for interface in self.content.interfaces.values() {
            let path = format!("interfaces.{}", interface.id);
            text(&interface.name, &format!("{path}.name"), 256)?;
            bounded(interface.source_ids.len(), &path)?;
            unique(interface.source_ids.iter(), &path)?;
            for number in &interface.source_ids {
                source_id(&mut source_ids, *number, interface.id.as_str(), &path)?;
            }
        }
        Ok(())
    }

    pub(super) fn recipes(&self) -> GameResult<()> {
        for recipe in self.content.recipes.values() {
            let path = format!("recipes.{}", recipe.id);
            text(&recipe.name, &path, 256)?;
            nonempty(recipe.inputs.len(), &format!("{path}.inputs"))?;
            if !recipe.mechanics.as_ref().is_some_and(|mechanics| {
                matches!(
                    mechanics.lifecycle,
                    RecipeLifecycle::Firemaking { .. } | RecipeLifecycle::ConsumeOnly
                )
            }) {
                nonempty(recipe.outputs.len(), &format!("{path}.outputs"))?;
            }
            if recipe
                .mechanics
                .as_ref()
                .is_some_and(|m| matches!(m.lifecycle, RecipeLifecycle::ConsumeOnly))
                && !recipe.outputs.is_empty()
            {
                return Err(invalid(
                    &path,
                    "consume-only recipes cannot claim direct output items",
                ));
            }
            self.stacks(&recipe.inputs, &format!("{path}.inputs"), true)?;
            self.stacks(&recipe.outputs, &format!("{path}.outputs"), true)?;
            self.stacks(
                &recipe.failed_outputs,
                &format!("{path}.failed_outputs"),
                true,
            )?;
            let tools_path = format!("{path}.tools");
            bounded(recipe.tools.len(), &tools_path)?;
            unique(recipe.tools.iter(), &tools_path)?;
            for tool_id in &recipe.tools {
                let tool = self.item(tool_id, &tools_path)?;
                if tool.unnoted_variant.is_some() {
                    return Err(invalid(
                        &tools_path,
                        "a noted item cannot be a reusable recipe tool",
                    ));
                }
                if recipe.inputs.iter().any(|input| &input.item == tool_id) {
                    return Err(invalid(
                        &tools_path,
                        format!("required tool {tool_id} must not be consumed as a recipe input"),
                    ));
                }
            }
            self.recipe_tool_capacity(recipe, &tools_path)?;
            self.requirements(&recipe.requirements, &path)?;
            self.xp(&recipe.xp, &path)?;
            self.recipe_mechanics(recipe, &path)?;
            chance(&recipe.success, &format!("{path}.success"), true)?;
            bounded(recipe.target_objects.len(), &path)?;
            unique(recipe.target_objects.iter(), &path)?;
            for object in &recipe.target_objects {
                if !self.content.objects.contains_key(object) {
                    return Err(invalid(&path, format!("undefined target object {object}")));
                }
            }
            if let Some(rule) = recipe
                .item_on_target_rule()
                .map_err(|error| invalid(&format!("{path}.item_on_target"), error.message))?
            {
                self.state_guard(&rule.guard, &format!("{path}.item_on_target.guard"))?;
            }
        }
        Ok(())
    }

    fn recipe_tool_capacity(&self, recipe: &RecipeDefinition, path: &str) -> GameResult<()> {
        let input_slots: usize = recipe
            .inputs
            .iter()
            .map(|input| {
                if self.content.items[&input.item].stackable.is_always() {
                    1
                } else {
                    input.quantity.get() as usize
                }
            })
            .sum();
        let equipped_needed = recipe
            .tools
            .len()
            .saturating_sub(INVENTORY_SLOTS - input_slots);
        if equipped_needed == 0 {
            return Ok(());
        }
        let candidates: Vec<_> = recipe
            .tools
            .iter()
            .filter_map(|id| self.content.items[id].equipment.as_ref())
            .collect();
        let impossible = || {
            invalid(
                path,
                "required tools and inputs cannot be held in 28 inventory slots and non-overlapping equipment",
            )
        };
        if equipped_needed > candidates.len() || equipped_needed > self.slots.len() {
            return Err(impossible());
        }
        let mut occupied = BTreeSet::new();
        let mut chosen = Vec::<usize>::new();
        let mut next = 0;
        let mut steps = MAX_TOOL_PLACEMENT_STEPS;
        let mut step = || {
            steps = steps
                .checked_sub(1)
                .ok_or_else(|| invalid(path, "recipe tool-placement work budget exceeded"))?;
            Ok::<_, GameError>(())
        };
        loop {
            step()?;
            if chosen.len() == equipped_needed {
                return Ok(());
            }
            if candidates.len() - next < equipped_needed - chosen.len() {
                let Some(last) = chosen.pop() else {
                    return Err(impossible());
                };
                for slot in &candidates[last].occupied_slots {
                    step()?;
                    occupied.remove(slot);
                }
                next = last + 1;
                continue;
            }
            let candidate = next;
            next += 1;
            let mut available = true;
            for slot in &candidates[candidate].occupied_slots {
                step()?;
                if occupied.contains(slot) {
                    available = false;
                    break;
                }
            }
            if available {
                for slot in &candidates[candidate].occupied_slots {
                    step()?;
                    occupied.insert(slot);
                }
                chosen.push(candidate);
            }
        }
    }

    pub(super) fn shops(&self) -> GameResult<()> {
        for shop in self.content.shops.values() {
            let path = format!("shops.{}", shop.id);
            text(&shop.name, &path, 256)?;
            let currency = self.item(&shop.currency, &path)?;
            if !currency.stackable.is_always() || currency.unnoted_variant.is_some() {
                return Err(invalid(
                    &path,
                    "shop currency must be an unnoted stackable item",
                ));
            }
            if shop.stock.is_empty() && !shop.accepts_general_items {
                return Err(invalid(&path, "a non-general shop must define stock"));
            }
            if shop.stock.len() > usize::from(u16::MAX) + 1 {
                return Err(invalid(&path, "stock exceeds the shop item-index range"));
            }
            unique(shop.stock.iter().map(|stock| &stock.item), &path)?;
            for (index, stock) in shop.stock.iter().enumerate() {
                let path = format!("{path}.stock[{index}]");
                self.item(&stock.item, &path)?;
                if stock.item == shop.currency
                    || stock.base_stock > MAX_STACK_QUANTITY
                    || stock.restock_ticks == 0
                    || stock.buy_price == 0
                    || stock.buy_price > MAX_STACK_QUANTITY
                    || stock.sell_price > stock.buy_price
                {
                    return Err(invalid(
                        &path,
                        "invalid currency/stock, restock duration, or buy/sell price contract",
                    ));
                }
                if let Some(mechanics) = &stock.mechanics {
                    self.shop_line(mechanics, &path)?;
                    self.stock_price_bounds(&stock.item, mechanics, &path)?;
                    self.stock_projection(stock, &path)?;
                }
            }
            if let Some(policy) = &shop.unstocked {
                match policy {
                    UnstockedShopPolicy::Reject if shop.accepts_general_items => {
                        return Err(invalid(
                            &path,
                            "general-shop flag conflicts with rejected unstocked items",
                        ));
                    }
                    UnstockedShopPolicy::Accept {
                        maximum_lines,
                        rule,
                        base_stock,
                    } => {
                        if matches!(rule.pricing, ShopPricing::Fixed) {
                            return Err(invalid(
                                &path,
                                "unstocked items need a price formula, not missing fixed-price rows",
                            ));
                        }
                        if !shop.accepts_general_items
                            || *maximum_lines == 0
                            || *base_stock > MAX_STACK_QUANTITY
                        {
                            return Err(invalid(
                                &path,
                                "invalid general-shop stock-line capacity or base stock",
                            ));
                        }
                        self.shop_line(rule, &path)?;
                        for item in self.content.items.keys() {
                            self.stock_price_bounds(item, rule, &path)?;
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    pub(super) fn initial_state(&self) -> GameResult<()> {
        let initial = &self.content.initial_state;
        self.location(&initial.region, initial.tile, true, "initial_state.tile")?;
        self.tutorial_stage(&initial.tutorial_stage, "initial_state.tutorial_stage")?;
        if initial.hitpoints == 0 {
            return Err(invalid(
                "initial_state.hitpoints",
                "a new character must be alive",
            ));
        }
        if initial.run_energy > MAX_RUN_ENERGY {
            return Err(invalid(
                "initial_state.run_energy",
                format!("run energy must be 0..={MAX_RUN_ENERGY} hundredths of one percent"),
            ));
        }
        let mut stackable = BTreeSet::new();
        for (index, stack) in initial.inventory.slots.iter().enumerate() {
            if let Some(stack) = stack {
                let path = format!("initial_state.inventory.slots[{index}]");
                let item = self.item(&stack.item, &path)?;
                if !item.stackable.is_always() && stack.quantity.get() != 1 {
                    return Err(invalid(
                        &path,
                        "nonstackable inventory items occupy separate slots",
                    ));
                }
                if item.stackable.is_always() && !stackable.insert(&item.id) {
                    return Err(invalid(
                        &path,
                        "a stackable item cannot occupy multiple inventory slots",
                    ));
                }
            }
        }
        if initial.bank.capacity == 0
            || initial.bank.slots.len() > usize::from(initial.bank.capacity)
        {
            return Err(invalid(
                "initial_state.bank",
                "bank capacity must be positive and contain all represented slots",
            ));
        }
        let mut bank_items = BTreeSet::new();
        for (index, stack) in initial.bank.slots.iter().enumerate() {
            if let Some(stack) = stack {
                let path = format!("initial_state.bank.slots[{index}]");
                let item = self.item(&stack.item, &path)?;
                if item.unnoted_variant.is_some() || !bank_items.insert(&item.id) {
                    return Err(invalid(
                        &path,
                        "bank entries must be unique unnoted item stacks",
                    ));
                }
            }
        }
        if initial.skills.len() != self.content.skills.len() {
            return Err(invalid(
                "initial_state.skills",
                "every defined skill needs exactly one explicit initial state",
            ));
        }
        for (id, state) in &initial.skills {
            let path = format!("initial_state.skills.{id}");
            let skill = self.skill(id, &path)?;
            let level = skill
                .xp_thresholds_tenths
                .partition_point(|xp| *xp <= state.xp_tenths);
            if state.xp_tenths > skill.maximum_xp_tenths
                || usize::from(state.current_level) != level
            {
                return Err(invalid(
                    &path,
                    "initial XP must be within maximum XP and current level must match the unboosted XP level",
                ));
            }
        }
        let mut occupied = BTreeMap::new();
        for (slot, stack) in &initial.equipment {
            let path = format!("initial_state.equipment.{slot}");
            let item = self.item(&stack.item, &path)?;
            let equipment = item
                .equipment
                .as_ref()
                .ok_or_else(|| invalid(&path, "item has no equipment definition"))?;
            if slot != &equipment.slot || !self.slots.contains(slot) {
                return Err(invalid(
                    &path,
                    "equipment entries must use the item's defined primary slot",
                ));
            }
            if !item.stackable.is_always() && stack.quantity.get() != 1 {
                return Err(invalid(
                    &path,
                    "nonstackable equipment quantity must be one",
                ));
            }
            for used in &equipment.occupied_slots {
                if let Some(previous) = occupied.insert(used, &item.id) {
                    return Err(invalid(
                        &path,
                        format!(
                            "occupied-slot conflict at {used} between {previous} and {}",
                            item.id
                        ),
                    ));
                }
            }
            for requirement in &equipment.requirements {
                if initial
                    .skills
                    .get(&requirement.skill)
                    .is_none_or(|state| state.current_level < requirement.level)
                {
                    return Err(invalid(
                        &path,
                        "initial equipment skill requirement is not met",
                    ));
                }
            }
        }
        if initial.quests.len() != self.content.quests.len() {
            return Err(invalid(
                "initial_state.quests",
                "every defined quest needs exactly one explicit initial state",
            ));
        }
        for (id, state) in &initial.quests {
            let path = format!("initial_state.quests.{id}");
            self.quest_stage(id, &state.stage, &path)?;
            let quest = &self.content.quests[id];
            if state.stage != quest.initial_stage {
                return Err(invalid(
                    &path,
                    "normal-account creation cannot start a quest advanced or completed",
                ));
            }
            bounded(state.flags.len(), &path)?;
            for flag in state.flags.keys() {
                token(flag, &path)?;
            }
        }
        if initial.quest_points != 0 {
            return Err(invalid(
                "initial_state.quest_points",
                "normal-account creation cannot pre-award quest points",
            ));
        }
        bounded(initial.flags.len(), "initial_state.flags")?;
        for flag in initial.flags.keys() {
            token(flag, "initial_state.flags")?;
            if flag.starts_with("__world_engine.") {
                return Err(invalid(
                    "initial_state.flags",
                    "source flags cannot initialize reserved engine metadata",
                ));
            }
        }
        unique(initial.interfaces.iter(), "initial_state.interfaces")?;
        for interface in &initial.interfaces {
            self.interface(interface, "initial_state.interfaces")?;
        }
        self.inventory_layout(&initial.inventory, "initial_state.inventory")?;
        self.equipment_layout(&initial.equipment, "initial_state.equipment")?;
        self.bank_layout(&initial.bank.slots, "initial_state.bank")?;
        let mut instances = BTreeSet::new();
        for stack in initial
            .inventory
            .slots
            .iter()
            .flatten()
            .chain(initial.equipment.values())
            .chain(initial.bank.slots.iter().flatten())
        {
            if let Some(instance) = &stack.instance
                && !instances.insert(&instance.id)
            {
                return Err(invalid(
                    "initial_state",
                    "item instance appears in multiple containers",
                ));
            }
        }
        Ok(())
    }
}
