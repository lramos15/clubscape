use serde::Serialize;

use super::*;

pub(super) fn binding<'a, T>(value: &'a SourceBinding<T>, path: &str) -> GameResult<Option<&'a T>> {
    match value {
        SourceBinding::Bound { value, .. } => Ok(Some(value)),
        SourceBinding::Unresolved { reason, .. } => {
            text(reason, path, MAX_TEXT_BYTES)?;
            Ok(None)
        }
    }
}

fn ratio(value: Ratio, probability: bool, path: &str) -> GameResult<()> {
    if value.denominator == 0 || (probability && value.numerator > value.denominator) {
        return Err(invalid(path, "invalid rational denominator or probability"));
    }
    Ok(())
}

fn duration(value: &TickDuration, path: &str) -> GameResult<()> {
    let valid = match value {
        TickDuration::Fixed { ticks } => *ticks > 0,
        TickDuration::UniformInclusive { minimum, maximum } => *minimum > 0 && minimum <= maximum,
    };
    if !valid {
        return Err(invalid(
            path,
            "duration must be a positive, ordered bounded tick range",
        ));
    }
    Ok(())
}

fn cadence(value: &ActionCadence, path: &str) -> GameResult<()> {
    for (name, value) in [
        ("single", &value.single),
        ("first", &value.first),
        ("repeat", &value.repeat),
    ] {
        if binding(value, &format!("{path}.{name}"))? == Some(&0) {
            return Err(invalid(
                path,
                "action cadence must be positive, not an invented zero",
            ));
        }
    }
    binding(&value.menu_delay, path)?;
    Ok(())
}

fn placement(value: &SourceObjectPlacement, path: &str) -> GameResult<()> {
    let expected = match value.shape {
        0..=3 => ObjectLayer::Wall,
        4..=8 => ObjectLayer::WallDecoration,
        9..=21 => ObjectLayer::GameObject,
        22 => ObjectLayer::FloorDecoration,
        _ => return Err(invalid(path, "unknown source object shape")),
    };
    if value.quarter_turns > 3 || value.layer != expected {
        return Err(invalid(path, "object shape/layer/orientation mismatch"));
    }

    Ok(())
}

fn morph_value(counter: &CounterDefinition, value: i64, path: &str) -> GameResult<()> {
    let value = match counter.value_type {
        CounterType::Boolean if value == 0 || value == 1 => CounterValue::Boolean(value == 1),
        CounterType::Boolean => {
            return Err(invalid(path, "boolean morph selectors must be zero or one"));
        }
        CounterType::Integer { .. } => CounterValue::Integer(value),
    };
    counter
        .validate_value(value)
        .map_err(|error| invalid(path, error))
}

impl Validator<'_> {
    pub(super) fn mechanic_sources(
        &self,
        mode: ValidationMode,
        counts: &mut EvidenceCounts,
        unresolved: &mut Vec<String>,
    ) -> GameResult<()> {
        source_tree(
            &self.content.mechanics,
            "mechanics",
            mode,
            counts,
            unresolved,
        )?;
        source_tree(&self.content.ui, "ui", mode, counts, unresolved)?;
        for item in self.content.items.values() {
            let path = format!("items.{}", item.id);
            source_tree(
                &item.stackable,
                &format!("{path}.stackable"),
                mode,
                counts,
                unresolved,
            )?;
            source_tree(
                &item.weight,
                &format!("{path}.weight"),
                mode,
                counts,
                unresolved,
            )?;
            source_tree(
                &item.charges,
                &format!("{path}.charges"),
                mode,
                counts,
                unresolved,
            )?;
            if let Some(equipment) = &item.equipment {
                source_tree(
                    &equipment.weapon,
                    &format!("{path}.equipment.weapon"),
                    mode,
                    counts,
                    unresolved,
                )?;
            }
        }
        for npc in self.content.npcs.values() {
            let path = format!("npcs.{}", npc.id);
            source_tree(
                &npc.navigation,
                &format!("{path}.navigation"),
                mode,
                counts,
                unresolved,
            )?;
            if let Some(combat) = &npc.combat {
                source_tree(
                    &combat.mechanics,
                    &format!("{path}.combat.mechanics"),
                    mode,
                    counts,
                    unresolved,
                )?;
            }
            for object in self.content.objects.values() {
                source_tree(
                    &object.morph,
                    &format!("objects.{}.morph", object.id),
                    mode,
                    counts,
                    unresolved,
                )?;
            }
        }
        for recipe in self.content.recipes.values() {
            source_tree(
                &recipe.mechanics,
                &format!("recipes.{}.mechanics", recipe.id),
                mode,
                counts,
                unresolved,
            )?;
        }
        for spawn in self.content.spawns.values() {
            for (index, interaction) in spawn.interactions.iter().enumerate() {
                if let InteractionAction::Gather { rule } = &interaction.action {
                    source_tree(
                        &rule.mechanics,
                        &format!(
                            "spawns.{}.interactions[{index}].action.rule.mechanics",
                            spawn.id
                        ),
                        mode,
                        counts,
                        unresolved,
                    )?;
                }
            }
        }
        for shop in self.content.shops.values() {
            source_tree(
                &shop.unstocked,
                &format!("shops.{}.unstocked", shop.id),
                mode,
                counts,
                unresolved,
            )?;
            for (index, row) in shop.stock.iter().enumerate() {
                source_tree(
                    &row.mechanics,
                    &format!("shops.{}.stock[{index}].mechanics", shop.id),
                    mode,
                    counts,
                    unresolved,
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn mechanics(&self) -> GameResult<()> {
        let definitions = &self.content.mechanics;
        macro_rules! keys {
            ($($map:ident),+ $(,)?) => {$(
                bounded(definitions.$map.len(), concat!("mechanics.", stringify!($map)))?;
                for (id, value) in &definitions.$map {
                    if id != &value.id {
                        return Err(invalid(concat!("mechanics.", stringify!($map)), "map key differs from contained ID"));
                    }
                }
            )+};
        }
        keys!(
            counters,
            grants,
            entitlements,
            reconciliations,
            object_transforms,
            temporary_objects,
            ground_policies,
            instances,
            travels,
            experiences,
            combat_styles,
            spells,
            projectiles,
            prayers,
            value_providers
        );
        let mut variables = BTreeSet::new();
        for counter in definitions.counters.values() {
            let path = format!("mechanics.counters.{}", counter.id);
            counter
                .validate_value(counter.initial)
                .map_err(|error| invalid(&path, error))?;
            if let Some(variable) = &counter.source_variable {
                let key = match variable {
                    SourceVariable::Varp { id } => (0, *id),
                    SourceVariable::Varbit { id } => (1, *id),
                };
                if !variables.insert((counter.scope as u8, key)) {
                    return Err(invalid(
                        &path,
                        "duplicate source variable mapping in one scope",
                    ));
                }
            }
            let initial = &self.content.initial_state.runtime.counters;
            if initial.len()
                != definitions
                    .counters
                    .values()
                    .filter(|counter| counter.scope == CounterScope::Character)
                    .count()
            {
                return Err(invalid(
                    "initial_state.runtime.counters",
                    "every character counter needs exactly one explicit initial value",
                ));
            }
            for (id, value) in initial {
                let counter = self.counter(id, "initial_state.runtime.counters")?;
                if counter.scope != CounterScope::Character || value != &counter.initial {
                    return Err(invalid(
                        "initial_state.runtime.counters",
                        "initial counter scope/value does not match its declaration",
                    ));
                }
            }
        }
        self.grants_and_reconciliations()?;
        self.dynamic_world()?;
        self.travel_definitions()?;
        self.combat_definitions()?;
        self.vital_definitions()?;
        self.death_definitions()?;
        self.execution_definitions()?;
        Ok(())
    }

    fn level_domain(&self, skill: &SkillId, levels: LevelDomain, path: &str) -> GameResult<()> {
        if levels.minimum == 0 || levels.minimum > levels.maximum {
            return Err(invalid(path, "invalid skill/method level domain"));
        }
        self.requirement(
            &SkillRequirement {
                skill: skill.clone(),
                level: levels.maximum,
                basis: levels.basis,
            },
            path,
        )
    }

    pub(super) fn item_mechanics(&self, item: &ItemDefinition, path: &str) -> GameResult<()> {
        if let Stackability::Conditional { source_mode, rule } = &item.stackable {
            if *source_mode != 2 {
                return Err(invalid(
                    path,
                    "conditional source stackability requires mode 2",
                ));
            }
            if let Some(rule) = binding(rule, path)? {
                if rule.stack_in.is_empty() {
                    return Err(invalid(
                        path,
                        "a conditional rule must name its stackable container contexts",
                    ));
                }
                if rule.stack_in.len() == 6 && !rule.require_same_origin {
                    return Err(invalid(
                        path,
                        "an unconditional rule must not be disguised as source mode 2",
                    ));
                }
            }
        }
        if let Some(weight) = &item.weight {
            if let Some(weight) = binding(weight, path)?
                && weight.grams == i32::MIN
            {
                return Err(invalid(
                    path,
                    "item weight must support checked signed negation",
                ));
            }
        } else if self.content.mechanics.run.is_some() {
            return Err(invalid(
                path,
                "run policy requires an explicit weight binding for every item",
            ));
        }
        if let Some(charges) = &item.charges {
            if charges.maximum == 0
                || charges.maximum > MAX_STACK_QUANTITY
                || !item.stackable.is_never()
                || item.unnoted_variant.is_some()
                || (item.id != charges.empty_variant && item.id != charges.charged_variant)
            {
                return Err(invalid(
                    path,
                    "charged containers require bounded per-instance, unnoted nonstackable variants",
                ));
            }
            for variant in [&charges.empty_variant, &charges.charged_variant] {
                let variant = self.item(variant, path)?;
                if variant.charges.as_ref() != Some(charges) {
                    return Err(invalid(
                        path,
                        "charge variants must share one reciprocal charge definition",
                    ));
                }
            }
        }
        if let Some(weapon) = item
            .equipment
            .as_ref()
            .and_then(|equipment| equipment.weapon.as_ref())
        {
            if item.equipment.as_ref().is_some_and(|equipment| {
                equipment.attack_speed_ticks.is_some() || !equipment.attack_styles.is_empty()
            }) {
                return Err(invalid(
                    path,
                    "typed weapon styles cannot also select legacy style/cadence fields",
                ));
            }
            nonempty(weapon.styles.len(), path)?;
            unique(weapon.styles.iter(), path)?;
            for id in &weapon.styles {
                let style = self
                    .content
                    .mechanics
                    .combat_styles
                    .get(id)
                    .ok_or_else(|| invalid(path, format!("undefined combat style {id}")))?;
                if style.method == AttackMethod::Ranged && weapon.ammunition.is_none() {
                    return Err(invalid(
                        path,
                        "ranged weapon needs explicit compatible ammunition",
                    ));
                }
            }
            if let Some(ammo) = &weapon.ammunition {
                if !self.slots.contains(&ammo.slot) {
                    return Err(invalid(path, "undefined ammunition slot"));
                }
                nonempty(ammo.compatible_items.len(), path)?;
                unique(ammo.compatible_items.iter(), path)?;
                for id in &ammo.compatible_items {
                    if self
                        .item(id, path)?
                        .equipment
                        .as_ref()
                        .is_none_or(|equipment| equipment.slot != ammo.slot)
                    {
                        return Err(invalid(
                            path,
                            "compatible ammunition must occupy the declared slot",
                        ));
                    }
                }
                if let Some(chance) = binding(&ammo.break_chance, path)? {
                    ratio(*chance, true, path)?;
                }
                self.ground_policy(&ammo.ground_policy, path)?;
            }
        }
        Ok(())
    }

    pub(super) fn item_instance(&self, stack: &ItemStack, path: &str) -> GameResult<()> {
        let item = self.item(&stack.item, path)?;
        if let Some(instance) = &stack.instance {
            if stack.quantity.get() != 1 {
                return Err(invalid(
                    path,
                    "an item instance represents exactly one owned item",
                ));
            }
            if let Some(charges) = &instance.charges {
                let definition = item
                    .charges
                    .as_ref()
                    .ok_or_else(|| invalid(path, "charges on an uncharged item"))?;
                if charges.kind != definition.kind
                    || charges.remaining > definition.maximum
                    || (charges.remaining == 0 && item.id != definition.empty_variant)
                    || (charges.remaining > 0 && item.id != definition.charged_variant)
                {
                    return Err(invalid(
                        path,
                        "instance charges do not match their source definition/variant",
                    ));
                }
            } else if item.charges.is_some() {
                return Err(invalid(path, "charged item instance has no charge state"));
            }
            if let Some(origin) = &instance.origin {
                if origin.acquired_at_tick > i64::MAX as u64
                    || origin.npc.is_some() != origin.life.is_some()
                {
                    return Err(invalid(path, "invalid item origin lifetime/tick"));
                }
                if let Some(spawn) = &origin.npc {
                    self.spawn_definition(spawn, path)?;
                }
            }
        } else if item.charges.is_some() {
            return Err(invalid(
                path,
                "charged items require an explicit per-item instance",
            ));
        }
        Ok(())
    }

    pub(super) fn gather_mechanics(&self, rule: &GatherRule, path: &str) -> GameResult<()> {
        let Some(mechanics) = &rule.mechanics else {
            if rule.attempt_ticks.is_none_or(|ticks| ticks == 0)
                || rule.respawn_ticks.is_none_or(|ticks| ticks == 0)
            {
                return Err(invalid(
                    path,
                    "legacy gather requires explicit positive attempt/respawn durations",
                ));
            }
            return Ok(());
        };
        if rule.attempt_ticks.is_some() || rule.respawn_ticks.is_some() {
            return Err(invalid(
                path,
                "source mechanics cannot also select legacy fixed gather timing",
            ));
        }
        self.level_domain(&rule.skill, mechanics.levels, path)?;
        if rule.required_level != mechanics.levels.minimum {
            return Err(invalid(
                path,
                "gather requirement and method minimum disagree",
            ));
        }
        self.chance_skill_domain(&rule.success, &rule.skill, path)?;
        cadence(&mechanics.cadence, path)?;
        unique(mechanics.tool_cadences.iter().map(|tool| &tool.tool), path)?;
        for tool in &mechanics.tool_cadences {
            if !rule.tools.contains(&tool.tool) || tool.location == OwnershipScope::Bank {
                return Err(invalid(
                    path,
                    "tool cadence must name an available gathering tool/location",
                ));
            }
            cadence(&tool.cadence, path)?;
        }
        if let Some(respawn) = binding(&mechanics.respawn, path)? {
            duration(respawn, path)?;
        }
        for alternative in &mechanics.alternatives {
            self.requirement(&alternative.requirement, path)?;
            if alternative.requirement.skill != rule.skill {
                return Err(invalid(path, "alternative catch must use the method skill"));
            }
            chance(&alternative.chance, path, true)?;
            self.chance_skill_domain(&alternative.chance, &rule.skill, path)?;
            self.stacks(std::slice::from_ref(&alternative.output), path, true)?;
            self.xp(
                &[XpReward {
                    skill: rule.skill.clone(),
                    amount_tenths: alternative.xp_tenths,
                }],
                path,
            )?;
        }
        if let Some(relocation) = &mechanics.relocation
            && let Some(relocation) = binding(relocation, path)?
        {
            nonempty(relocation.locations.len(), path)?;
            unique(relocation.locations.iter(), path)?;
            if relocation.exclude_current && relocation.locations.len() < 2 {
                return Err(invalid(
                    path,
                    "relocation excluding current needs at least two locations",
                ));
            }
            duration(&relocation.interval, path)?;
            for spawn in &relocation.locations {
                let spawn = self.spawn_definition(spawn, path)?;
                if !matches!(&spawn.kind, SpawnKind::Npc { npc } if matches!(&self.content.npcs[npc].navigation, NpcNavigation::Stationary { .. }))
                {
                    return Err(invalid(
                        path,
                        "resource relocation targets must be stationary NPC placements",
                    ));
                }
            }
        }
        Ok(())
    }

    fn chance_skill_domain(
        &self,
        chance: &ChanceRule,
        skill: &SkillId,
        path: &str,
    ) -> GameResult<()> {
        if let ChanceDomain::Skill { levels } = chance.domain {
            self.level_domain(skill, levels, path)?;
        }
        Ok(())
    }

    pub(super) fn recipe_mechanics(&self, recipe: &RecipeDefinition, path: &str) -> GameResult<()> {
        let Some(mechanics) = &recipe.mechanics else {
            if recipe.ticks.is_none_or(|ticks| ticks == 0) {
                return Err(invalid(
                    path,
                    "legacy recipe needs an explicit positive duration",
                ));
            }
            return Ok(());
        };
        if recipe.ticks.is_some() {
            return Err(invalid(
                path,
                "source recipe cannot also select legacy uniform timing",
            ));
        }
        cadence(&mechanics.cadence, path)?;
        self.state_guard(&mechanics.guard, path)?;
        match (&recipe.success.domain, &mechanics.chance_skill) {
            (ChanceDomain::Skill { .. }, Some(skill)) => {
                self.chance_skill_domain(&recipe.success, skill, path)?
            }
            (ChanceDomain::Skill { .. }, None) => {
                return Err(invalid(
                    path,
                    "level-dependent recipe requires an explicit chance skill",
                ));
            }
            (ChanceDomain::Constant, Some(skill)) => {
                self.skill(skill, path)?;
            }
            (ChanceDomain::Constant, None) => {}
        }
        if mechanics.tool_ownership == OwnershipScope::Bank {
            return Err(invalid(path, "a banked tool is not a production tool"));
        }
        if mechanics.tool_ownership == OwnershipScope::Equipment {
            for tool in &recipe.tools {
                if self.item(tool, path)?.equipment.is_none() {
                    return Err(invalid(
                        path,
                        "equipment-only recipe tool cannot be equipped",
                    ));
                }
            }
        }
        self.xp(&mechanics.failed_xp, path)?;
        self.state_effects(&mechanics.success_effects, path)?;
        self.state_effects(&mechanics.failure_effects, path)?;
        if let RecipeLifecycle::Firemaking {
            ground_input,
            fire,
            step_priority,
            retain_ground_input_on_failure,
        } = &mechanics.lifecycle
        {
            if !self.content.mechanics.temporary_objects.contains_key(fire) {
                return Err(invalid(path, "undefined temporary fire definition"));
            }
            if recipe.inputs.len() != 1
                || &recipe.inputs[0].item != ground_input
                || recipe.inputs[0].quantity.get() != 1
                || !*retain_ground_input_on_failure
                || !recipe.failed_outputs.is_empty()
            {
                return Err(invalid(
                    path,
                    "firemaking must retain its single placed owned input on failure",
                ));
            }
            if step_priority.len() != 4 {
                return Err(invalid(
                    path,
                    "firemaking must explicitly order all four cardinal step attempts",
                ));
            }
            let mut seen = BTreeSet::new();
            for direction in step_priority {
                let (x, y) = direction.offset();
                if x.abs() + y.abs() != 1 || !seen.insert(direction.mask()) {
                    return Err(invalid(
                        path,
                        "invalid or duplicate firemaking step direction",
                    ));
                }
            }
        }
        Ok(())
    }

    fn grants_and_reconciliations(&self) -> GameResult<()> {
        let mechanics = &self.content.mechanics;
        for grant in mechanics.grants.values() {
            let path = format!("mechanics.grants.{}", grant.id);
            if !matches!(grant.target, ContainerKind::Inventory | ContainerKind::Bank) {
                return Err(invalid(
                    &path,
                    "grants target inventory or bank, not recovery/equipment ownership",
                ));
            }
            nonempty(grant.lines.len(), &path)?;
            unique(grant.lines.iter().map(|line| &line.item), &path)?;
            let mut slots = 0_u64;
            for line in &grant.lines {
                let item = self.item(&line.item, &path)?;
                if item.charges.is_some() {
                    return Err(invalid(
                        &path,
                        "quantity grants cannot create charged item instances",
                    ));
                }
                if grant.target == ContainerKind::Bank && item.unnoted_variant.is_some() {
                    return Err(invalid(&path, "bank grants must target unnoted items"));
                }
                if line.mode == GrantMode::MissingOnly && line.quantity.get() != 1 {
                    return Err(invalid(
                        &path,
                        "missing-only grant supplies one item; use top_up for bounded totals",
                    ));
                }
                slots += if item.stackable.is_always() || grant.target == ContainerKind::Bank {
                    1
                } else {
                    u64::from(line.quantity.get())
                };
            }
            if grant.target == ContainerKind::Inventory
                && grant.capacity == CapacityPolicy::Atomic
                && slots > INVENTORY_SLOTS as u64
            {
                return Err(invalid(&path, "atomic grant cannot fit an empty inventory"));
            }
            if let Some(entitlement) = &grant.entitlement
                && !matches!(&self.entitlement(entitlement, &path)?.purpose, EntitlementPurpose::Grant { grant: id } if id == &grant.id)
            {
                return Err(invalid(
                    &path,
                    "grant entitlement has a different purpose/owner",
                ));
            }
        }
        for entitlement in mechanics.entitlements.values() {
            let path = format!("mechanics.entitlements.{}", entitlement.id);
            match &entitlement.purpose {
                EntitlementPurpose::AtomicReward => {}
                EntitlementPurpose::Grant { grant } => {
                    if mechanics
                        .grants
                        .get(grant)
                        .and_then(|grant| grant.entitlement.as_ref())
                        != Some(&entitlement.id)
                    {
                        return Err(invalid(&path, "grant entitlement must be reciprocal"));
                    }
                }
                EntitlementPurpose::Reconciliation { reconciliation } => {
                    if mechanics
                        .reconciliations
                        .get(reconciliation)
                        .map(|value| &value.entitlement)
                        != Some(&entitlement.id)
                    {
                        return Err(invalid(
                            &path,
                            "reconciliation entitlement must be reciprocal",
                        ));
                    }
                }
            }
        }
        for reconciliation in mechanics.reconciliations.values() {
            let path = format!("mechanics.reconciliations.{}", reconciliation.id);
            if !matches!(&self.entitlement(&reconciliation.entitlement, &path)?.purpose, EntitlementPurpose::Reconciliation { reconciliation: id } if id == &reconciliation.id)
            {
                return Err(invalid(
                    &path,
                    "reconciliation entitlement purpose mismatch",
                ));
            }
            if let Some(policies) = binding(&reconciliation.policies, &path)? {
                nonempty(policies.len(), &path)?;
                let mut replaced = BTreeSet::new();
                for policy in policies {
                    let container = match policy {
                        ContainerReconciliation::Preserve => None,
                        ContainerReconciliation::ReplaceInventory { inventory } => {
                            self.inventory_layout(inventory, &path)?;
                            Some(ContainerKind::Inventory)
                        }
                        ContainerReconciliation::ReplaceEquipment { equipment } => {
                            self.equipment_layout(equipment, &path)?;
                            Some(ContainerKind::Equipment)
                        }
                        ContainerReconciliation::ReplaceBank { slots } => {
                            if slots.len() > usize::from(self.content.initial_state.bank.capacity) {
                                return Err(invalid(
                                    &path,
                                    "bank reconciliation exceeds declared capacity",
                                ));
                            }
                            self.bank_layout(slots, &path)?;
                            Some(ContainerKind::Bank)
                        }
                        ContainerReconciliation::RemoveItems { container, items } => {
                            if !matches!(
                                container,
                                ContainerKind::Inventory
                                    | ContainerKind::Equipment
                                    | ContainerKind::Bank
                            ) {
                                return Err(invalid(
                                    &path,
                                    "reconciliation cannot erase ground/grave/Office ownership",
                                ));
                            }
                            unique(items.iter(), &path)?;
                            for item in items {
                                self.item(item, &path)?;
                            }
                            Some(*container)
                        }
                    };
                    if let Some(container) = container
                        && !replaced.insert(container)
                    {
                        return Err(invalid(
                            &path,
                            "multiple reconciliation policies write one container",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn inventory_layout(&self, inventory: &Inventory, path: &str) -> GameResult<()> {
        let mut stacks = BTreeSet::new();
        let mut instances = BTreeSet::new();
        for stack in inventory.slots.iter().flatten() {
            self.item_instance(stack, path)?;
            let item = self.item(&stack.item, path)?;
            if !item.stackable.is_always() && stack.quantity.get() != 1 {
                return Err(invalid(
                    path,
                    "nonstackable/conditional inventory entries require per-slot items",
                ));
            }
            if item.stackable.is_always() && !stacks.insert(&stack.item) {
                return Err(invalid(path, "duplicate inventory stack"));
            }
            if let Some(instance) = &stack.instance
                && !instances.insert(&instance.id)
            {
                return Err(invalid(path, "duplicate item instance"));
            }
        }
        Ok(())
    }

    pub(super) fn equipment_layout(
        &self,
        equipment: &BTreeMap<SlotId, ItemStack>,
        path: &str,
    ) -> GameResult<()> {
        let mut occupied = BTreeSet::new();
        for (slot, stack) in equipment {
            self.item_instance(stack, path)?;
            let item = self.item(&stack.item, path)?;
            let definition = item
                .equipment
                .as_ref()
                .ok_or_else(|| invalid(path, "unequippable layout item"))?;
            if slot != &definition.slot
                || (!item.stackable.is_always() && stack.quantity.get() != 1)
            {
                return Err(invalid(path, "invalid equipment layout slot/quantity"));
            }
            for used in &definition.occupied_slots {
                if !self.slots.contains(used) || !occupied.insert(used) {
                    return Err(invalid(
                        path,
                        "equipment layout overlaps or uses an undefined slot",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn bank_layout(&self, slots: &[Option<ItemStack>], path: &str) -> GameResult<()> {
        let mut items = BTreeSet::new();
        let mut instances = BTreeSet::new();
        for stack in slots.iter().flatten() {
            self.item_instance(stack, path)?;
            if self.item(&stack.item, path)?.unnoted_variant.is_some() {
                return Err(invalid(path, "bank layout contains a note"));
            }
            if let Some(instance) = &stack.instance {
                if !instances.insert(&instance.id) {
                    return Err(invalid(path, "duplicate bank item instance"));
                }
            } else if !items.insert(&stack.item) {
                return Err(invalid(path, "duplicate bank item stack"));
            }
        }
        Ok(())
    }

    pub(super) fn entitlement(
        &self,
        id: &EntitlementId,
        path: &str,
    ) -> GameResult<&EntitlementDefinition> {
        self.content
            .mechanics
            .entitlements
            .get(id)
            .ok_or_else(|| invalid(path, format!("undefined entitlement {id}")))
    }

    pub(super) fn counter(&self, id: &CounterId, path: &str) -> GameResult<&CounterDefinition> {
        self.content
            .mechanics
            .counters
            .get(id)
            .ok_or_else(|| invalid(path, format!("undefined counter {id}")))
    }

    pub(super) fn spawn_definition(
        &self,
        id: &SpawnId,
        path: &str,
    ) -> GameResult<&SpawnDefinition> {
        self.content
            .spawns
            .get(id)
            .ok_or_else(|| invalid(path, format!("undefined spawn {id}")))
    }

    pub(super) fn ground_policy(&self, id: &GroundPolicyId, path: &str) -> GameResult<()> {
        if !self.content.mechanics.ground_policies.contains_key(id) {
            return Err(invalid(path, format!("undefined ground policy {id}")));
        }
        Ok(())
    }

    fn dynamic_world(&self) -> GameResult<()> {
        for definition in self.content.mechanics.object_transforms.values() {
            let path = format!("mechanics.object_transforms.{}", definition.id);
            let spawn = self.spawn_definition(&definition.spawn, &path)?;
            if definition.scope == CounterScope::Character {
                return Err(invalid(
                    &path,
                    "physical object/collision states belong to a world or live instance",
                ));
            }
            if !matches!(spawn.kind, SpawnKind::Object { .. })
                || !definition.states.contains_key(&definition.initial)
            {
                return Err(invalid(
                    &path,
                    "object transform needs an object spawn and declared initial state",
                ));
            }
            let initial = &definition.states[&definition.initial];
            let source_object = match &spawn.kind {
                SpawnKind::Object { object } => object,
                _ => unreachable!(),
            };
            let mut initial_object = Some(source_object);
            if let Some(morph) = self
                .content
                .objects
                .get(source_object)
                .and_then(|object| object.morph.as_ref())
                && let Some(SourceBinding::Bound { value: link, .. }) = &morph.collision
                && link.placements.get(&spawn.id) == Some(&definition.id)
            {
                let counter = self.counter(&morph.counter, &path)?;
                let value = match counter.initial {
                    CounterValue::Integer(value) => value,
                    CounterValue::Boolean(value) => i64::from(value),
                };
                initial_object = morph
                    .variants
                    .get(&value)
                    .unwrap_or(&morph.fallback)
                    .as_ref();
            }
            if initial.tile != spawn.tile || initial.object.as_ref() != initial_object {
                return Err(invalid(
                    &path,
                    "initial transform must preserve its declared source spawn placement",
                ));
            }
            let coverage: BTreeSet<_> = initial.collision.iter().map(|cell| cell.tile).collect();
            for cell in &initial.collision {
                let index = self
                    .collision
                    .get(&cell.tile)
                    .ok_or_else(|| invalid(&path, "initial transform cell is unmapped"))?;
                if &self.content.regions[&index.region].cells[index.offset] != cell {
                    return Err(invalid(
                        &path,
                        "initial transform collision differs from the source scene",
                    ));
                }
            }
            for state in definition.states.values() {
                placement(&state.placement, &path)?;
                if let Some(object) = &state.object
                    && !self.content.objects.contains_key(object)
                {
                    return Err(invalid(&path, "undefined transform object"));
                }
                if state.tile.plane() != spawn.tile.plane()
                    || !self.collision.contains_key(&state.tile)
                {
                    return Err(invalid(
                        &path,
                        "object-state placement is unmapped or changes plane",
                    ));
                }
                if state
                    .collision
                    .iter()
                    .map(|cell| cell.tile)
                    .collect::<BTreeSet<_>>()
                    != coverage
                {
                    return Err(invalid(
                        &path,
                        "transform states must replace the same explicit collision coverage",
                    ));
                }
                let mut seen = BTreeSet::new();
                for cell in &state.collision {
                    if !self.collision.contains_key(&cell.tile) || !seen.insert(cell.tile) {
                        return Err(invalid(
                            &path,
                            "transform collision must replace unique explicitly mapped cells",
                        ));
                    }
                }
            }
        }
        for definition in self.content.mechanics.temporary_objects.values() {
            let path = format!("mechanics.temporary_objects.{}", definition.id);
            if !self.content.objects.contains_key(&definition.object) {
                return Err(invalid(&path, "undefined temporary object"));
            }
            if let Some(lifetime) = binding(&definition.lifetime, &path)? {
                duration(lifetime, &path)?;
            }
            self.state_guard(&definition.placement_guard, &path)?;
            unique(
                definition
                    .interactions
                    .iter()
                    .map(|interaction| &interaction.name),
                &path,
            )?;
            for interaction in &definition.interactions {
                text(&interaction.name, &path, 160)?;
                self.state_guard(&interaction.guard, &path)?;
                match &interaction.action {
                    InteractionAction::Production { recipes } => {
                        nonempty(recipes.len(), &path)?;
                        unique(recipes.iter(), &path)?;
                        for recipe in recipes {
                            if self.content.recipes.get(recipe).is_none_or(|recipe| {
                                !recipe.target_objects.contains(&definition.object)
                            }) {
                                return Err(invalid(
                                    &path,
                                    "temporary production facility must offer recipes bound to its object definition",
                                ));
                            }
                        }
                    }
                    InteractionAction::Effects { effects } => self.state_effects(effects, &path)?,
                    InteractionAction::Unavailable { reason } => {
                        text(reason, &path, MAX_TEXT_BYTES)?
                    }
                    _ => {
                        return Err(invalid(
                            &path,
                            "unsupported temporary-object interaction; use production, effects or explicit unavailability",
                        ));
                    }
                }
            }
            self.stacks(&definition.expired_items, &path, false)?;
            self.ground_policy(&definition.ground_policy, &path)?;
        }
        for definition in self.content.mechanics.ground_policies.values() {
            let path = format!("mechanics.ground_policies.{}", definition.id);
            let public = binding(&definition.public_after, &path)?;
            let expiry = binding(&definition.expires_after, &path)?;
            if let Some(clock) = &definition.clock {
                binding(clock, &format!("{path}.clock"))?;
            }
            if expiry == Some(&Some(0)) {
                return Err(invalid(
                    &path,
                    "ground expiry must be positive or explicitly absent",
                ));
            }
            if let (Some(Some(public)), Some(Some(expiry))) = (public, expiry)
                && public > expiry
            {
                return Err(invalid(&path, "ground visibility begins after expiry"));
            }
        }
        Ok(())
    }

    pub(super) fn npc_navigation(&self, npc: &NpcDefinition, path: &str) -> GameResult<()> {
        if let Some(morph) = &npc.morph {
            let counter = self.counter(&morph.counter, path)?;
            if counter.source_variable.is_none() {
                return Err(invalid(
                    path,
                    "source NPC morph requires a source-variable counter",
                ));
            }
            for (value, variant) in &morph.variants {
                morph_value(counter, *value, path)?;
                if variant
                    .as_ref()
                    .is_some_and(|id| !self.content.npcs.contains_key(id))
                {
                    return Err(invalid(path, "undefined NPC morph variant"));
                }
            }
            if morph
                .fallback
                .as_ref()
                .is_some_and(|id| !self.content.npcs.contains_key(id))
            {
                return Err(invalid(path, "undefined NPC morph fallback"));
            }
        }
        match &npc.navigation {
            NpcNavigation::Mobile { step_ticks, .. } => {
                if binding(step_ticks, path)? == Some(&0) {
                    return Err(invalid(path, "mobile NPC step cadence must be positive"));
                }
            }
            NpcNavigation::Stationary { anchor } => match anchor {
                StationaryAnchor::Walkable => {}
                StationaryAnchor::NonWalkingResource { access_tiles }
                | StationaryAnchor::SceneryBound { access_tiles, .. }
                | StationaryAnchor::ScriptedActor { access_tiles, .. } => {
                    nonempty(access_tiles.len(), path)?;
                    if access_tiles.iter().collect::<BTreeSet<_>>().len() != access_tiles.len() {
                        return Err(invalid(path, "duplicate stationary access tile"));
                    }
                    for tile in access_tiles {
                        let cell = self
                            .collision
                            .get(tile)
                            .ok_or_else(|| invalid(path, "stationary access tile is unmapped"))?;
                        self.location(&cell.region, *tile, true, path)?;
                    }
                    if let StationaryAnchor::SceneryBound { object, .. } = anchor
                        && !self.content.objects.contains_key(object)
                    {
                        return Err(invalid(path, "undefined scenery anchor object"));
                    }
                    if matches!(anchor, StationaryAnchor::ScriptedActor { .. })
                        && npc.combat.is_some()
                    {
                        return Err(invalid(
                            path,
                            "scripted nonwalking anchors cannot exempt mobile combat actors",
                        ));
                    }
                }
            },
        }
        Ok(())
    }

    pub(super) fn npc_placement(
        &self,
        spawn: &SpawnDefinition,
        npc: &NpcDefinition,
        path: &str,
    ) -> GameResult<()> {
        let walkable = !matches!(
            &npc.navigation,
            NpcNavigation::Stationary {
                anchor: StationaryAnchor::NonWalkingResource { .. }
                    | StationaryAnchor::SceneryBound { .. }
                    | StationaryAnchor::ScriptedActor { .. }
            }
        );
        for x in 0..npc.size {
            for y in 0..npc.size {
                let tile = spawn
                    .tile
                    .offset(i16::from(x), i16::from(y))
                    .ok_or_else(|| invalid(path, "NPC footprint coordinate overflow"))?;
                let cell = self
                    .collision
                    .get(&tile)
                    .ok_or_else(|| invalid(path, "NPC footprint has an unmapped cell"))?;
                self.location(&cell.region, tile, walkable, path)?;
            }
        }
        if let NpcNavigation::Stationary { anchor } = &npc.navigation {
            let access = match anchor {
                StationaryAnchor::Walkable => return Ok(()),
                StationaryAnchor::NonWalkingResource { access_tiles } => {
                    if !spawn
                        .interactions
                        .iter()
                        .any(|action| matches!(action.action, InteractionAction::Gather { .. }))
                    {
                        return Err(invalid(
                            path,
                            "nonwalking resource anchor must offer a real gathering method",
                        ));
                    }
                    access_tiles
                }
                StationaryAnchor::SceneryBound {
                    object,
                    access_tiles,
                } => {
                    let anchored = self.content.spawns.values().any(|candidate| {
                        matches!(&candidate.kind, SpawnKind::Object { object: id } if id == object)
                            && candidate.tile.plane() == spawn.tile.plane()
                            && self.content.objects.get(object).is_some_and(|definition| {
                                let rotated = candidate
                                    .placement
                                    .as_ref()
                                    .is_some_and(|placement| placement.quarter_turns % 2 == 1);
                                let (width, height) = if rotated {
                                    (definition.size_y, definition.size_x)
                                } else {
                                    (definition.size_x, definition.size_y)
                                };
                                spawn.tile.x() >= candidate.tile.x()
                                    && spawn.tile.y() >= candidate.tile.y()
                                    && spawn.tile.x() - candidate.tile.x() < u16::from(width)
                                    && spawn.tile.y() - candidate.tile.y() < u16::from(height)
                            })
                    });
                    if !anchored {
                        return Err(invalid(
                            path,
                            "scenery-bound NPC has no matching occupied scenery placement",
                        ));
                    }
                    access_tiles
                }
                StationaryAnchor::ScriptedActor { access_tiles, .. } => access_tiles,
            };
            let maximum_x = spawn.tile.x() + u16::from(npc.size - 1);
            let maximum_y = spawn.tile.y() + u16::from(npc.size - 1);
            if !access.iter().any(|tile| {
                let dx = tile.x().abs_diff(tile.x().clamp(spawn.tile.x(), maximum_x));
                let dy = tile.y().abs_diff(tile.y().clamp(spawn.tile.y(), maximum_y));
                tile.plane() == spawn.tile.plane()
                    && spawn
                        .interactions
                        .iter()
                        .any(|interaction| dx.max(dy) <= interaction.reach)
            }) {
                return Err(invalid(
                    path,
                    "stationary anchor has no declared access tile within interaction reach",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn object_mechanics(&self, object: &ObjectDefinition, path: &str) -> GameResult<()> {
        if object
            .clip
            .as_ref()
            .is_some_and(|clip| clip.access_blocked_sides & !15 != 0)
        {
            return Err(invalid(path, "object access sides use only cardinal bits"));
        }
        if let Some(morph) = &object.morph {
            let counter = self.counter(&morph.counter, path)?;
            if counter.source_variable.is_none() {
                return Err(invalid(
                    path,
                    "source object morph requires a declared source-variable counter",
                ));
            }
            for (value, variant) in &morph.variants {
                morph_value(counter, *value, path)?;
                if let Some(variant) = variant
                    && !self.content.objects.contains_key(variant)
                {
                    return Err(invalid(path, "undefined source morph variant"));
                }
            }
            if morph
                .fallback
                .as_ref()
                .is_some_and(|id| !self.content.objects.contains_key(id))
            {
                return Err(invalid(path, "undefined source morph fallback"));
            }
        }
        Ok(())
    }

    pub(super) fn source_placement(&self, spawn: &SpawnDefinition, path: &str) -> GameResult<()> {
        if let Some(value) = &spawn.placement {
            if !matches!(spawn.kind, SpawnKind::Object { .. }) {
                return Err(invalid(path, "object placement fields on a non-object"));
            }
            placement(value, path)?;
        }
        Ok(())
    }

    pub(super) fn world_location(&self, location: &WorldLocation, path: &str) -> GameResult<()> {
        self.location(&location.region, location.tile, true, path)?;
        if let Some(instance) = &location.instance {
            let definition = self
                .content
                .mechanics
                .instances
                .get(instance)
                .ok_or_else(|| invalid(path, "undefined destination instance template"))?;
            if !definition.chunks.iter().any(|chunk| {
                chunk.destination_region == location.region
                    && chunk.destination_origin.plane() == location.tile.plane()
                    && location.tile.x() >= chunk.destination_origin.x()
                    && location.tile.y() >= chunk.destination_origin.y()
                    && location.tile.x() - chunk.destination_origin.x()
                        < u16::from(definition.chunk_size)
                    && location.tile.y() - chunk.destination_origin.y()
                        < u16::from(definition.chunk_size)
            }) {
                return Err(invalid(
                    path,
                    "instance destination is not covered by a declared live chunk mapping",
                ));
            }
        }
        Ok(())
    }

    fn travel_definitions(&self) -> GameResult<()> {
        for definition in self.content.mechanics.instances.values() {
            let path = format!("mechanics.instances.{}", definition.id);
            if definition.chunk_size == 0 || definition.chunk_size > 64 {
                return Err(invalid(&path, "instance chunk size must be 1..64"));
            }
            nonempty(definition.chunks.len(), &path)?;
            let mut destinations = BTreeSet::new();
            for chunk in &definition.chunks {
                if chunk.quarter_turns > 3 {
                    return Err(invalid(&path, "invalid instance rotation"));
                }
                for x in 0..definition.chunk_size {
                    for y in 0..definition.chunk_size {
                        let source = chunk
                            .source_origin
                            .offset(i16::from(x), i16::from(y))
                            .ok_or_else(|| invalid(&path, "source chunk overflows coordinates"))?;
                        let destination = chunk
                            .destination_origin
                            .offset(i16::from(x), i16::from(y))
                            .ok_or_else(|| {
                                invalid(&path, "destination chunk overflows coordinates")
                            })?;
                        self.location(&chunk.source_region, source, false, &path)?;
                        self.location(&chunk.destination_region, destination, false, &path)?;
                        if !destinations.insert(destination) {
                            return Err(invalid(&path, "overlapping live instance chunks"));
                        }
                    }
                }
            }
        }
        for experience in self.content.mechanics.experiences.values() {
            let path = format!("mechanics.experiences.{}", experience.id);
            text(&experience.name, &path, 256)?;
            self.state_guard(&experience.selection_guard, &path)?;
        }
        if let Some(appearance) = &self.content.mechanics.appearance {
            nonempty(appearance.choices.len(), "mechanics.appearance")?;
            self.state_guard(&appearance.confirmation_guard, "mechanics.appearance")?;
            for (name, options) in &appearance.choices {
                token(name, "mechanics.appearance")?;
                nonempty(options.len(), "mechanics.appearance")?;
            }
        }
        for travel in self.content.mechanics.travels.values() {
            let path = format!("mechanics.travels.{}", travel.id);
            self.state_guard(&travel.guard, &path)?;
            binding(&travel.channel_ticks, &path)?;
            binding(&travel.cooldown_ticks, &path)?;
            binding(&travel.cooldown_start, &path)?;
            if let Some(destination) = binding(&travel.destination, &path)? {
                match destination {
                    TravelDestination::Fixed { location } => {
                        self.world_location(location, &path)?
                    }
                    TravelDestination::Experience { branches } => {
                        nonempty(branches.len(), &path)?;
                        if branches.len() != self.content.mechanics.experiences.len() {
                            return Err(invalid(
                                &path,
                                "arrival branches must cover every declared experience choice",
                            ));
                        }
                        for (experience, location) in branches {
                            if !self.content.mechanics.experiences.contains_key(experience) {
                                return Err(invalid(&path, "undefined experience arrival branch"));
                            }
                            self.world_location(location, &path)?;
                        }
                    }
                    TravelDestination::PreviousRespawn => {
                        if self.content.mechanics.death.is_none() {
                            return Err(invalid(&path, "previous respawn requires a death policy"));
                        }
                    }
                }
            }
            self.state_effects(&travel.completion_effects, &path)?;
        }
        Ok(())
    }

    fn effective_level(&self, formula: &EffectiveLevelFormula, path: &str) -> GameResult<()> {
        self.skill(&formula.skill, path)?;
        Ok(())
    }

    fn combat_definitions(&self) -> GameResult<()> {
        let definitions = &self.content.mechanics;
        for style in definitions.combat_styles.values() {
            let path = format!("mechanics.combat_styles.{}", style.id);
            self.effective_level(&style.attack, &path)?;
            self.effective_level(&style.defence, &path)?;
            binding(&style.accuracy, &path)?;
            binding(&style.negative_rolls, &path)?;
            binding(&style.damage, &path)?;
            if binding(&style.cycle_ticks, &path)? == Some(&0) || style.reach == 0 {
                return Err(invalid(&path, "combat cycle/reach must be positive"));
            }
            if !matches!(
                (style.method, style.attack_type),
                (
                    AttackMethod::Melee,
                    AttackType::Stab | AttackType::Slash | AttackType::Crush
                ) | (AttackMethod::Ranged, AttackType::Ranged)
                    | (AttackMethod::Magic, AttackType::Magic)
            ) {
                return Err(invalid(&path, "combat method/attack type mismatch"));
            }
            if let Some(formula) = binding(&style.maximum_hit, &path)? {
                match formula {
                    MaximumHitFormula::Strength { level, divisor, .. } => {
                        self.effective_level(level, &path)?;
                        if *divisor == 0 {
                            return Err(invalid(&path, "maximum-hit divisor is zero"));
                        }
                    }
                    MaximumHitFormula::LevelTable { skill, basis, hits } => {
                        nonempty(hits.len(), &path)?;
                        for level in hits.keys() {
                            self.requirement(
                                &SkillRequirement {
                                    skill: skill.clone(),
                                    level: *level,
                                    basis: *basis,
                                },
                                &path,
                            )?;
                        }
                    }
                    MaximumHitFormula::Fixed { .. } => {}
                }
            }
            unique(style.damage_xp.iter().map(|xp| &xp.skill), &path)?;
            for xp in &style.damage_xp {
                self.skill(&xp.skill, &path)?;
                ratio(xp.tenths_per_damage, false, &path)?;
            }
            if style
                .projectile
                .as_ref()
                .is_some_and(|id| !definitions.projectiles.contains_key(id))
            {
                return Err(invalid(&path, "undefined style projectile"));
            }
        }
        for projectile in definitions.projectiles.values() {
            let path = format!("mechanics.projectiles.{}", projectile.id);
            if let Some(timing) = binding(&projectile.timing, &path)? {
                ratio(timing.ticks_per_tile, false, &path)?;
                let flight = u64::from(timing.ticks_per_tile.numerator) * 16_383
                    / u64::from(timing.ticks_per_tile.denominator);
                if flight
                    + u64::from(timing.base_flight_ticks)
                    + u64::from(timing.launch_delay_ticks)
                    > u64::from(u32::MAX)
                {
                    return Err(invalid(
                        &path,
                        "projectile timing exceeds bounded tick duration",
                    ));
                }
            }
        }
        for spell in definitions.spells.values() {
            let path = format!("mechanics.spells.{}", spell.id);
            self.interface(&spell.interface, &path)?;
            self.requirements(&spell.requirements, &path)?;
            self.state_guard(&spell.guard, &path)?;
            self.stacks(&spell.runes, &path, true)?;
            for rune in &spell.runes {
                let item = self.item(&rune.item, &path)?;
                if !item.stackable.is_always() || item.unnoted_variant.is_some() {
                    return Err(invalid(&path, "rune costs must be unnoted stackable items"));
                }
            }
            self.xp(&spell.launch_xp, &path)?;
            match &spell.action {
                SpellAction::Combat { style, projectile } => {
                    if definitions
                        .combat_styles
                        .get(style)
                        .is_none_or(|style| style.method != AttackMethod::Magic)
                        || !definitions.projectiles.contains_key(projectile)
                    {
                        return Err(invalid(
                            &path,
                            "combat spell requires a magic style and defined projectile",
                        ));
                    }
                }
                SpellAction::Teleport { travel } => self.travel(travel, &path)?,
            }
        }
        for prayer in definitions.prayers.values() {
            let path = format!("mechanics.prayers.{}", prayer.id);
            self.interface(&prayer.interface, &path)?;
            self.requirements(&prayer.requirements, &path)?;
            unique(
                prayer.modifiers.iter().map(|modifier| &modifier.skill),
                &path,
            )?;
            for modifier in &prayer.modifiers {
                self.skill(&modifier.skill, &path)?;
                ratio(modifier.multiplier, false, &path)?;
                if modifier.multiplier.numerator == 0 {
                    return Err(invalid(&path, "prayer multiplier must be positive"));
                }
            }
            if let Some(drain) = binding(&prayer.drain, &path)? {
                ratio(drain.points_per_tick, false, &path)?;
                if drain.points_per_tick.numerator == 0
                    || drain.bonus_divisor == 0
                    || drain.bonus_offset == 0
                {
                    return Err(invalid(
                        &path,
                        "prayer drain/bonus parameters must be positive",
                    ));
                }
            }
            unique(prayer.exclusive_with.iter(), &path)?;
            for id in &prayer.exclusive_with {
                let other = definitions
                    .prayers
                    .get(id)
                    .ok_or_else(|| invalid(&path, "undefined exclusive prayer"))?;
                if id == &prayer.id || !other.exclusive_with.contains(&prayer.id) {
                    return Err(invalid(
                        &path,
                        "prayer exclusions must be distinct and reciprocal",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn npc_combat_mechanics(
        &self,
        combat: &NpcCombatDefinition,
        path: &str,
    ) -> GameResult<()> {
        if let Some(mechanics) = &combat.mechanics {
            if !combat.drops.is_empty() || combat.respawn_ticks.is_some() {
                return Err(invalid(
                    path,
                    "typed NPC mechanics cannot also select legacy loot/respawn fields",
                ));
            }
            if mechanics.reach == 0 {
                return Err(invalid(path, "NPC combat reach must be positive"));
            }
            if mechanics.defence_stats.len() != 5 {
                return Err(invalid(
                    path,
                    "NPC defence must explicitly bind all five attack types",
                ));
            }
            binding(&mechanics.effective_level_bonus, path)?;
            binding(&mechanics.accuracy, path)?;
            binding(&mechanics.negative_rolls, path)?;
            binding(&mechanics.damage, path)?;
            binding(&mechanics.credit, path)?;
            if let Some(respawn) = binding(&mechanics.respawn, path)? {
                duration(respawn, path)?;
            }
            self.loot(&mechanics.loot, path)?;
        }
        Ok(())
    }

    fn loot(&self, pools: &[LootPool], path: &str) -> GameResult<BTreeMap<ItemId, u64>> {
        let mut totals = BTreeMap::<ItemId, u64>::new();
        for pool in pools {
            let quantities = match pool {
                LootPool::Guaranteed { items } => self.loot_entries(items, path)?,
                LootPool::Independent { chance, items } => {
                    ratio(*chance, true, path)?;
                    let quantities = self.loot_entries(items, path)?;
                    if chance.numerator == 0 {
                        BTreeMap::new()
                    } else {
                        quantities
                    }
                }
                LootPool::Exclusive {
                    total_weight,
                    entries,
                } => {
                    nonempty(entries.len(), path)?;
                    let mut sum = 0_u64;
                    let mut maxima = BTreeMap::new();
                    for entry in entries {
                        if entry.weight == 0 {
                            return Err(invalid(path, "exclusive loot weight must be positive"));
                        }
                        sum = sum
                            .checked_add(u64::from(entry.weight))
                            .ok_or_else(|| invalid(path, "loot weight overflow"))?;
                        let quantities = if entry.items.is_empty() {
                            BTreeMap::new()
                        } else {
                            self.loot_entries(&entry.items, path)?
                        };
                        for (item, quantity) in quantities {
                            let max = maxima.entry(item).or_insert(0);
                            *max = (*max).max(quantity);
                        }
                    }
                    if sum != u64::from(*total_weight) || *total_weight == 0 {
                        return Err(invalid(
                            path,
                            "exclusive weights must sum to the declared denominator; include explicit no-drop outcomes",
                        ));
                    }
                    maxima
                }
                LootPool::Conditional { guard, pools } => {
                    self.state_guard(guard, path)?;
                    self.loot(pools, path)?
                }
                LootPool::Unresolved { reason, .. } => {
                    text(reason, path, MAX_TEXT_BYTES)?;
                    BTreeMap::new()
                }
            };
            for (item, quantity) in quantities {
                let total = totals.entry(item).or_default();
                *total = total
                    .checked_add(quantity)
                    .ok_or_else(|| invalid(path, "loot quantity overflow"))?;
                if *total > u64::from(MAX_STACK_QUANTITY) {
                    return Err(invalid(
                        path,
                        "simultaneous loot pools exceed the item stack bound",
                    ));
                }
            }
        }
        Ok(totals)
    }

    fn loot_entries(&self, entries: &[LootEntry], path: &str) -> GameResult<BTreeMap<ItemId, u64>> {
        nonempty(entries.len(), path)?;
        let mut quantities = BTreeMap::<ItemId, u64>::new();
        for entry in entries {
            if self.item(&entry.item, path)?.charges.is_some() || entry.minimum > entry.maximum {
                return Err(invalid(
                    path,
                    "loot entries require ordered quantity bounds and cannot invent charged instances",
                ));
            }
            let quantity = quantities.entry(entry.item.clone()).or_default();
            *quantity += u64::from(entry.maximum.get());
            if *quantity > u64::from(MAX_STACK_QUANTITY) {
                return Err(invalid(path, "loot entry aggregate exceeds stack bounds"));
            }
        }
        Ok(quantities)
    }

    pub(super) fn shop_line(&self, value: &ShopLineMechanics, path: &str) -> GameResult<()> {
        if value.restock.interval_ticks == 0 {
            return Err(invalid(path, "restock interval must be positive"));
        }
        if let Some(RestockPhase::Explicit { first_tick }) = binding(&value.restock.phase, path)?
            && *first_tick > i64::MAX as u64
        {
            return Err(invalid(path, "restock phase deadline overflow"));
        }
        if let ShopPricing::StockSensitive {
            buy,
            sell,
            overstock,
        } = &value.pricing
        {
            binding(overstock, path)?;
            for formula in [buy, sell] {
                if formula.minimum_per_mille > formula.base_per_mille
                    || formula.base_per_mille > formula.maximum_per_mille
                    || formula.minimum_price > MAX_STACK_QUANTITY
                {
                    return Err(invalid(path, "invalid stock-price base/clamp/minimum"));
                }
            }
            if buy.minimum_price == 0 {
                return Err(invalid(path, "buy prices require a positive floor"));
            }
        }
        Ok(())
    }

    pub(super) fn stock_price_bounds(
        &self,
        item: &ItemId,
        rule: &ShopLineMechanics,
        path: &str,
    ) -> GameResult<()> {
        if let ShopPricing::StockSensitive { buy, sell, .. } = &rule.pricing {
            let value = self.item(item, path)?.base_value;
            for price in [buy, sell] {
                let product = u64::from(value) * u64::from(price.maximum_per_mille);
                let rounded = match price.rounding {
                    IntegerRounding::Floor => product / 1000,
                    IntegerRounding::NearestTiesUp => (product + 500) / 1000,
                };
                if rounded.max(u64::from(price.minimum_price)) > u64::from(MAX_STACK_QUANTITY) {
                    return Err(invalid(
                        path,
                        "maximum source stock-sensitive unit price exceeds currency stack bounds",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn stock_projection(&self, row: &ShopItem, path: &str) -> GameResult<()> {
        let Some(rule) = &row.mechanics else {
            return Ok(());
        };
        if row.restock_ticks != rule.restock.interval_ticks {
            return Err(invalid(
                path,
                "stock row and source restock interval disagree",
            ));
        }
        if let ShopPricing::StockSensitive { buy, sell, .. } = &rule.pricing {
            let value = u64::from(self.item(&row.item, path)?.base_value);
            let price = |formula: &StockPriceFormula| {
                let product = value * u64::from(formula.base_per_mille);
                let rounded = match formula.rounding {
                    IntegerRounding::Floor => product / 1000,
                    IntegerRounding::NearestTiesUp => (product + 500) / 1000,
                };
                rounded.max(u64::from(formula.minimum_price))
            };
            if price(buy) != u64::from(row.buy_price) || price(sell) != u64::from(row.sell_price) {
                return Err(invalid(
                    path,
                    "stock row base-stock prices disagree with the source formula",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn before_presentation(&self, effects: &[Effect], path: &str) -> GameResult<()> {
        let mut pending = vec![(effects, false)];
        while let Some((effects, once)) = pending.pop() {
            for effect in effects {
                match effect {
                    Effect::Once { effects, .. } => pending.push((effects, true)),
                    Effect::Conditional { effects, .. } => pending.push((effects, once)),
                    Effect::Grant { grant } => {
                        let definition = &self.content.mechanics.grants[grant];
                        if definition.target != ContainerKind::Bank
                            || (definition.entitlement.is_none() && !once)
                            || definition.capacity != CapacityPolicy::Atomic
                        {
                            return Err(invalid(
                                path,
                                "pre-presentation grants must be atomic, bank-targeted and once-only",
                            ));
                        }
                    }
                    Effect::Message { .. } | Effect::UnlockInterface { .. } => {}
                    _ => {
                        return Err(invalid(
                            path,
                            "pre-presentation effects only authorize contextual UI/unlocks and once-only bank grants",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn vital_definitions(&self) -> GameResult<()> {
        if let Some(run) = &self.content.mechanics.run {
            let path = "mechanics.run";
            self.level_domain(&run.agility, run.levels, path)?;
            if run.activation_minimum == 0 || run.activation_minimum > MAX_RUN_ENERGY {
                return Err(invalid(
                    path,
                    "run activation must use 1..10000 canonical energy units",
                ));
            }
            if self
                .content
                .initial_state
                .runtime
                .settings
                .run_enabled
                .is_none()
            {
                return Err(invalid(path, "initial source run setting is missing"));
            }
            if let Some(drain) = binding(&run.drain, path)? {
                if drain.weight_minimum_grams < 0
                    || drain.weight_minimum_grams >= drain.weight_maximum_grams
                    || drain.agility_scale <= run.levels.maximum
                {
                    return Err(invalid(
                        path,
                        "invalid run weight clamp or Agility scale/domain",
                    ));
                }
                if u64::from(drain.base) + u64::from(drain.weight_scale) > u64::from(u16::MAX) {
                    return Err(invalid(path, "run drain exceeds bounded energy arithmetic"));
                }
            }
            if let Some(regeneration) = binding(&run.regeneration, path)?
                && (regeneration.skill_divisor == 0
                    || u32::from(regeneration.additive_units)
                        + u32::from(run.levels.maximum) / u32::from(regeneration.skill_divisor)
                        > u32::from(MAX_RUN_ENERGY)
                    || !regeneration.pauses.contains(&ClockPause::Offline))
            {
                return Err(invalid(
                    path,
                    "run regeneration has invalid units/divisor or implicit offline recovery",
                ));
            }
        }
        if let Some(vitals) = &self.content.mechanics.vitals {
            let path = "mechanics.vitals";
            self.skill(&vitals.hitpoints_skill, path)?;
            self.skill(&vitals.prayer_skill, path)?;
            binding(&vitals.level_up, path)?;
            if binding(&vitals.food_delay_ticks, path)? == Some(&0)
                || binding(&vitals.food_attack_delay_ticks, path)? == Some(&0)
            {
                return Err(invalid(
                    path,
                    "ordinary food delays must be positive or explicitly unresolved",
                ));
            }
            let mut seen = BTreeSet::new();
            for regeneration in &vitals.regeneration {
                if regeneration.amount == 0
                    || !seen.insert(regeneration.vital)
                    || binding(&regeneration.interval_ticks, path)? == Some(&0)
                    || (regeneration.pauses.contains(&ClockPause::Idle)
                        != regeneration.idle_after_milliseconds.is_some())
                    || regeneration.idle_after_milliseconds == Some(0)
                {
                    return Err(invalid(
                        path,
                        "invalid vital regeneration period/amount/pause policy",
                    ));
                }
            }
            let initial = &self.content.initial_state;
            let hp = initial
                .skills
                .get(&vitals.hitpoints_skill)
                .ok_or_else(|| invalid(path, "initial hitpoints skill missing"))?;
            let prayer = initial
                .skills
                .get(&vitals.prayer_skill)
                .ok_or_else(|| invalid(path, "initial prayer skill missing"))?;
            if initial.hitpoints > hp.current_level || initial.prayer_points > prayer.current_level
            {
                return Err(invalid(
                    path,
                    "initial vitals exceed their bound source skill maxima",
                ));
            }
        }
        let initial = &self.content.initial_state.runtime.settings;
        if initial
            .experience
            .as_ref()
            .is_some_and(|id| !self.content.mechanics.experiences.contains_key(id))
        {
            return Err(invalid(
                "initial_state.runtime.experience",
                "undefined experience choice",
            ));
        }
        Ok(())
    }

    fn death_definitions(&self) -> GameResult<()> {
        for provider in self.content.mechanics.value_providers.values() {
            let path = format!("mechanics.value_providers.{}", provider.id);
            identity(&provider.revision, &path)?;
            if let Some(values) = binding(&provider.values, &path)? {
                for (item, value) in values {
                    self.item(item, &path)?;
                    if *value > i64::MAX as u64 {
                        return Err(invalid(
                            &path,
                            "death valuation exceeds currency arithmetic bound",
                        ));
                    }
                }
            }
        }
        if let Some(death) = &self.content.mechanics.death {
            let path = "mechanics.death";
            let provider = self
                .content
                .mechanics
                .value_providers
                .get(&death.value_provider)
                .ok_or_else(|| {
                    invalid(
                        path,
                        "undefined death valuation provider; base_value is not death value",
                    )
                })?;
            if let Some(values) = binding(&provider.values, path)?
                && self.content.items.keys().any(|id| !values.contains_key(id))
            {
                return Err(invalid(
                    path,
                    "death value snapshot must cover every represented item",
                ));
            }
            if death.retained_unskulled > 64
                || death.protect_item_extra > 64
                || death.grave_active_ticks == 0
                || death.grave_capacity == 0
                || death.office_capacity == 0
                || death.office_capacity > 4096
                || death.grave_capacity > 4096
                || death.idle_after_milliseconds == 0
                || death.reclaim_range == 0
                || self
                    .content
                    .initial_state
                    .runtime
                    .settings
                    .death_auto_equip
                    .is_none()
                || self
                    .content
                    .initial_state
                    .runtime
                    .settings
                    .death_supply_piles
                    .is_none()
            {
                return Err(invalid(
                    path,
                    "death policy has missing settings or invalid capacity/active-time bounds",
                ));
            }
            if death.required_topics
                != BTreeSet::from([DeathTopic::Fees, DeathTopic::Timer, DeathTopic::KeptItems])
                || !death.grave_pauses.contains(&ClockPause::FirstDeathOffice)
                || !death.grave_pauses.contains(&ClockPause::Offline)
            {
                return Err(invalid(
                    path,
                    "first item-losing death requires all topics and explicit Office/offline grave pauses",
                ));
            }
            binding(&death.ties, path)?;
            binding(&death.office_overflow, path)?;
            if let Some(restoration) = binding(&death.restoration, path)? {
                for (vital, effect) in restoration
                    .on_arrival
                    .iter()
                    .chain(&restoration.on_first_office_exit)
                {
                    self.restore_vital(*vital, effect, path)?;
                }
            }
            for destination in [&death.respawn, &death.first_office] {
                if let Some(location) = binding(destination, path)? {
                    self.world_location(location, path)?;
                }
            }
            if let Some(location) = binding(&death.first_office, path)?
                && location.instance.is_none()
            {
                return Err(invalid(
                    path,
                    "first Death Office needs an explicit private instance mapping",
                ));
            }
            for fee in [&death.grave_fee, &death.office_fee] {
                if let Some(fee) = binding(fee, path)? {
                    match fee {
                        RecoveryFee::Bands {
                            bands,
                            maximum_total,
                        } => {
                            nonempty(bands.len(), path)?;
                            if bands[0].minimum_value != 0
                                || bands
                                    .windows(2)
                                    .any(|pair| pair[0].minimum_value >= pair[1].minimum_value)
                                || *maximum_total > i64::MAX as u64
                            {
                                return Err(invalid(
                                    path,
                                    "death fee bands must be ordered, start at zero and have a bounded cap",
                                ));
                            }
                        }
                        RecoveryFee::Percentage { rate, .. } => ratio(*rate, true, path)?,
                    }
                }
            }
            nonempty(death.payment_order.len(), path)?;
            if death.payment_order.iter().collect::<BTreeSet<_>>().len()
                != death.payment_order.len()
            {
                return Err(invalid(path, "duplicate death fee payment source"));
            }
            let currency = self.item(&death.currency, path)?;
            if !currency.stackable.is_always() || currency.unnoted_variant.is_some() {
                return Err(invalid(
                    path,
                    "death fees require unnoted stackable currency",
                ));
            }
            if let Some(repeat) = binding(&death.repeat, path)? {
                if repeat.old_unstackable_per_item_limit == 0 {
                    return Err(invalid(
                        path,
                        "repeat-death per-item limit must be positive",
                    ));
                }
                unique(repeat.old_items_to_office.iter(), path)?;
                unique(repeat.supply_items.iter(), path)?;
                for item in repeat
                    .old_items_to_office
                    .iter()
                    .chain(&repeat.supply_items)
                {
                    self.item(item, path)?;
                }
                self.ground_policy(&repeat.supply_ground_policy, path)?;
            }
        }
        Ok(())
    }

    pub(super) fn travel(&self, id: &TravelId, path: &str) -> GameResult<()> {
        if !self.content.mechanics.travels.contains_key(id) {
            return Err(invalid(path, format!("undefined travel {id}")));
        }
        Ok(())
    }

    pub(super) fn setting(&self, setting: &CharacterSetting, path: &str) -> GameResult<()> {
        let initial = &self.content.initial_state.runtime.settings;
        let declared = match setting {
            CharacterSetting::Run(_) => {
                initial.run_enabled.is_some() && self.content.mechanics.run.is_some()
            }
            CharacterSetting::AutoRetaliate(_) => initial.auto_retaliate.is_some(),
            CharacterSetting::DeathAutoEquip(_) => {
                initial.death_auto_equip.is_some() && self.content.mechanics.death.is_some()
            }
            CharacterSetting::DeathSupplyPiles(_) => {
                initial.death_supply_piles.is_some() && self.content.mechanics.death.is_some()
            }
        };
        if !declared {
            return Err(invalid(
                path,
                "setting has no explicit initial/source policy",
            ));
        }
        Ok(())
    }

    pub(super) fn charge_requirement(
        &self,
        item: &ItemId,
        kind: &ChargeKindId,
        amount: u32,
        path: &str,
    ) -> GameResult<()> {
        let definition = self
            .item(item, path)?
            .charges
            .as_ref()
            .ok_or_else(|| invalid(path, "charge operation references an uncharged item"))?;
        if &definition.kind != kind || amount == 0 || amount > definition.maximum {
            return Err(invalid(
                path,
                "charge kind/amount violates the item definition",
            ));
        }
        Ok(())
    }

    pub(super) fn restore_vital(
        &self,
        vital: Vital,
        restoration: &VitalRestoration,
        path: &str,
    ) -> GameResult<()> {
        let maximum = if vital == Vital::RunEnergy {
            MAX_RUN_ENERGY
        } else {
            let policy = self.content.mechanics.vitals.as_ref().ok_or_else(|| {
                invalid(
                    path,
                    "vital restoration requires explicit HP/prayer skill bindings",
                )
            })?;
            let skill = if vital == Vital::Hitpoints {
                &policy.hitpoints_skill
            } else {
                &policy.prayer_skill
            };
            u16::try_from(self.skill(skill, path)?.xp_thresholds_tenths.len())
                .map_err(|_| invalid(path, "vital level maximum overflow"))?
        };
        match restoration {
            VitalRestoration::Amount { amount } if *amount == 0 => {
                return Err(invalid(path, "restoration amount must be positive"));
            }
            VitalRestoration::Set { amount }
                if *amount > maximum || (vital == Vital::Hitpoints && *amount == 0) =>
            {
                return Err(invalid(
                    path,
                    "restoration set value exceeds source maximum or would fake a death",
                ));
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn event_guard_context(
        &self,
        guard: &Guard,
        event: &str,
        target: Option<&str>,
        path: &str,
    ) -> GameResult<()> {
        let mut pending = vec![guard];
        while let Some(guard) = pending.pop() {
            match guard {
                Guard::Event { condition } => {
                    if condition.event_kind() != event {
                        return Err(invalid(
                            path,
                            "event guard reads facts from a different authoritative event kind",
                        ));
                    }
                    if target
                        .zip(condition.primary_target())
                        .is_some_and(|(target, actual)| target != actual)
                    {
                        return Err(invalid(path, "event guard and transition target disagree"));
                    }
                    if let (Some(target), EventCondition::Kill { npc, .. }) = (target, condition) {
                        let spawn = self.spawn_definition(
                            &SpawnId::new(target).map_err(|error| invalid(path, error))?,
                            path,
                        )?;
                        if !matches!(&spawn.kind, SpawnKind::Npc { npc: actual } if actual == npc) {
                            return Err(invalid(path, "kill-event NPC and target spawn disagree"));
                        }
                    }
                }
                Guard::All { guards } | Guard::Any { guards } => pending.extend(guards),
                Guard::Not { guard } => pending.push(guard),
                _ => {}
            }
        }
        Ok(())
    }

    pub(super) fn mechanic_event_target(
        &self,
        event: &str,
        target: Option<&str>,
        path: &str,
    ) -> GameResult<()> {
        let Some(target) = target else {
            return Ok(());
        };
        macro_rules! reference {
            ($id:ident, $map:ident) => {{
                let id = $id::new(target).map_err(|error| invalid(path, error))?;
                if !self.content.mechanics.$map.contains_key(&id) {
                    return Err(invalid(path, format!("undefined event target {id}")));
                }
            }};
        }
        match event {
            "experience_selected" => reference!(ExperienceId, experiences),
            "spell_resolved" => reference!(SpellId, spells),
            "teleport" => reference!(TravelId, travels),
            "prayer_changed" => reference!(PrayerId, prayers),
            "temporary_object_created" => reference!(TemporaryObjectId, temporary_objects),
            "object_transformed" => reference!(ObjectTransformId, object_transforms),
            "counter_changed" => reference!(CounterId, counters),
            _ => return Err(invalid(path, "unknown mechanic event")),
        }
        Ok(())
    }

    pub(super) fn event_condition(&self, condition: &EventCondition, path: &str) -> GameResult<()> {
        match condition {
            EventCondition::Interaction { target, action } => {
                let spawn = self.spawn_definition(target, path)?;
                if !spawn
                    .interactions
                    .iter()
                    .any(|interaction| &interaction.name == action)
                {
                    return Err(invalid(
                        path,
                        "event references an undefined source interaction",
                    ));
                }
            }
            EventCondition::DialogueChoice { speaker, choice } => {
                let spawn = self.spawn_definition(speaker, path)?;
                let found = spawn.interactions.iter().any(|interaction| {
                    if let InteractionAction::Dialogue { dialogue } = &interaction.action {
                        self.content
                            .dialogues
                            .get(dialogue)
                            .is_some_and(|dialogue| {
                                dialogue.nodes.iter().any(|node| {
                                    node.choices.iter().any(|candidate| &candidate.id == choice)
                                })
                            })
                    } else {
                        false
                    }
                });
                if !found {
                    return Err(invalid(
                        path,
                        "event references an undefined speaker/dialogue choice",
                    ));
                }
            }
            EventCondition::Production {
                recipe,
                method,
                facility,
                outcome,
                output,
            } => {
                let recipe = self
                    .content
                    .recipes
                    .get(recipe)
                    .ok_or_else(|| invalid(path, "undefined event recipe"))?;
                if recipe
                    .mechanics
                    .as_ref()
                    .is_none_or(|mechanics| &mechanics.method != method)
                {
                    return Err(invalid(
                        path,
                        "production event requires the recipe's declared source method",
                    ));
                }
                if let Some(facility) = facility {
                    let spawn = self.spawn_definition(facility, path)?;
                    if !spawn.interactions.iter().any(|interaction| matches!(&interaction.action, InteractionAction::Production { recipes } if recipes.contains(&recipe.id))) {
                        return Err(invalid(path, "production event facility does not offer the recipe"));
                    }
                }
                if let Some(item) = output {
                    let outputs = if *outcome == ProductionOutcome::Success {
                        &recipe.outputs
                    } else {
                        &recipe.failed_outputs
                    };
                    if !outputs.iter().any(|stack| &stack.item == item) {
                        return Err(invalid(
                            path,
                            "production event output does not match the selected success/failure branch",
                        ));
                    }
                }
            }
            EventCondition::Combat { style, .. } => {
                if !self.content.mechanics.combat_styles.contains_key(style) {
                    return Err(invalid(path, "undefined event combat style"));
                }
            }
            EventCondition::Kill { npc, .. } => {
                if self
                    .content
                    .npcs
                    .get(npc)
                    .is_none_or(|npc| npc.combat.is_none())
                {
                    return Err(invalid(path, "kill event requires a combat NPC"));
                }
            }
            EventCondition::Spell {
                spell,
                target,
                outcomes,
            } => {
                let spell = self
                    .content
                    .mechanics
                    .spells
                    .get(spell)
                    .ok_or_else(|| invalid(path, "undefined event spell"))?;
                if !matches!(spell.action, SpellAction::Combat { .. }) || outcomes.is_empty() {
                    return Err(invalid(
                        path,
                        "spell-resolved guard requires explicit combat spell outcomes",
                    ));
                }
                if let Some(target) = target {
                    let spawn = self.spawn_definition(target, path)?;
                    if !matches!(&spawn.kind, SpawnKind::Npc { npc } if self.content.npcs.get(npc).is_some_and(|npc| npc.combat.is_some()))
                    {
                        return Err(invalid(path, "spell event target is not a combat NPC"));
                    }
                }
            }
            EventCondition::Interface { interface, context } => {
                self.interface(interface, path)?;
                match context {
                    InterfaceContext::Tab => {
                        if self.content.interfaces[interface].access != InterfaceAccess::Tab {
                            return Err(invalid(path, "a generic tab event cannot present contextual storage"));
                        }
                    }
                    InterfaceContext::Bank { banker } => {
                        if !self.spawn_definition(banker, path)?.interactions.iter().any(|interaction| matches!(&interaction.action, InteractionAction::OpenBank { interface: id, .. } if id == interface)) {
                            return Err(invalid(path, "bank presentation requires a matching contextual open interaction"));
                        }
                    }
                    InterfaceContext::Shop { spawn, shop } => {
                        if !self.spawn_definition(spawn, path)?.interactions.iter().any(|interaction| matches!(&interaction.action, InteractionAction::OpenShop { interface: id, shop: actual, .. } if id == interface && actual == shop)) {
                            return Err(invalid(path, "shop presentation requires a matching contextual open interaction"));
                        }
                    }
                    InterfaceContext::Grave { .. } => return Err(invalid(path, "immutable content must not hardcode a dynamic death identity")),
                    InterfaceContext::DeathOffice => {
                        if self.content.mechanics.death.is_none() { return Err(invalid(path, "Death Office interface requires a death policy")); }
                    }
                }
            }
            EventCondition::Teleport { travel, .. } => self.travel(travel, path)?,
            EventCondition::Setting { setting } => self.setting(setting, path)?,
            EventCondition::Inspection {
                target,
                explanation,
            } => {
                self.spawn_definition(target, path)?;
                token(explanation, path)?;
            }
            EventCondition::Death { .. } | EventCondition::Recovery { .. } => {
                if self.content.mechanics.death.is_none() {
                    return Err(invalid(
                        path,
                        "death/recovery event requires a source death policy",
                    ));
                }
            }
            EventCondition::DeathTopic { topic } => {
                if self
                    .content
                    .mechanics
                    .death
                    .as_ref()
                    .is_none_or(|death| !death.required_topics.contains(topic))
                {
                    return Err(invalid(path, "unknown death topic event"));
                }
            }
        }
        Ok(())
    }
}

// Traverse only already typed extension records, after bounded rule preflight.
// This keeps provenance coverage in step with nested source bindings, not gameplay JSON interpretation.
fn source_tree(
    value: &impl Serialize,
    path: &str,
    mode: ValidationMode,
    counts: &mut EvidenceCounts,
    unresolved: &mut Vec<String>,
) -> GameResult<()> {
    let value = serde_json::to_value(value).map_err(|error| invalid(path, error))?;
    let mut pending = vec![(path.to_owned(), &value)];
    while let Some((path, value)) = pending.pop() {
        match value {
            serde_json::Value::Object(fields) => {
                if fields.get("status").and_then(serde_json::Value::as_str) == Some("unresolved")
                    || fields.get("kind").and_then(serde_json::Value::as_str) == Some("unresolved")
                {
                    unresolved.push(path.clone());
                }
                for (key, value) in fields {
                    let path = format!("{path}.{key}");
                    if key == "source" {
                        let records: Vec<SourceRecord> = serde_json::from_value(value.clone())
                            .map_err(|error| invalid(&path, error))?;
                        source_records(&records, &path, mode, counts)?;
                    } else {
                        pending.push((path, value));
                    }
                }
            }
            serde_json::Value::Array(values) => {
                pending.extend(
                    values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| (format!("{path}[{index}]"), value)),
                );
            }
            _ => {}
        }
    }
    Ok(())
}
