use super::{mechanics::binding, *};

impl Validator<'_> {
    pub(super) fn execution_definitions(&self) -> GameResult<()> {
        if let Some(policy) = &self.content.mechanics.player_drop {
            if let Some(id) = binding(&policy.ordinary, "mechanics.player_drop.ordinary")? {
                self.ground_policy(id, "mechanics.player_drop.ordinary")?;
            }
            for (stage, value) in &policy.stages {
                if !self.content.tutorial.contains_key(stage) {
                    return Err(invalid(
                        "mechanics.player_drop.stages",
                        "undefined tutorial stage",
                    ));
                }
                if let Some(id) = binding(value, "mechanics.player_drop.stages")? {
                    self.ground_policy(id, "mechanics.player_drop.stages")?;
                }
            }
        }
        if let Some(policy) = &self.content.mechanics.player_combat {
            if let Some(unarmed) = binding(&policy.unarmed, "mechanics.player_combat.unarmed")? {
                self.execution_weapon(unarmed, "mechanics.player_combat.unarmed")?;
                if unarmed.ammunition.is_some() {
                    return Err(invalid(
                        "mechanics.player_combat.unarmed",
                        "unarmed cannot require equipped ammunition",
                    ));
                }
            }
            binding(&policy.engagement, "mechanics.player_combat.engagement")?;
        }
        for item in self.content.items.values() {
            if let Some(weapon) = item
                .equipment
                .as_ref()
                .and_then(|equipment| equipment.weapon.as_ref())
            {
                self.execution_weapon(weapon, &format!("items.{}.equipment.weapon", item.id))?;
            }
        }
        for npc in self.content.npcs.values() {
            let Some(combat) = &npc.combat else { continue };
            let Some(mechanics) = &combat.mechanics else {
                continue;
            };
            let path = format!("npcs.{}.combat.mechanics", npc.id);
            if let Some(policy) = binding(
                &mechanics.loot_ground_policy,
                &format!("{path}.loot_ground_policy"),
            )? {
                self.ground_policy(policy, &path)?;
            }
            binding(&mechanics.attribution, &format!("{path}.attribution"))?;
            if let Some(eligibility) =
                binding(&mechanics.eligibility, &format!("{path}.eligibility"))?
            {
                nonempty(eligibility.len(), &path)?;
                for rule in eligibility {
                    self.state_guard(&rule.guard, &path)?;
                    if let Some(style) = &rule.style {
                        let definition = self
                            .content
                            .mechanics
                            .combat_styles
                            .get(style)
                            .ok_or_else(|| invalid(&path, "undefined eligibility style"))?;
                        if definition.method != rule.method {
                            return Err(invalid(&path, "eligibility style/method disagree"));
                        }
                    }
                }
            }
            if let Some(engagement) = binding(&mechanics.engagement, &format!("{path}.engagement"))?
            {
                if engagement.inactivity_ticks == 0
                    || engagement.leash_range == 0
                    || engagement.reset_life_on_return && !engagement.return_to_spawn
                    || combat.aggressive != engagement.aggression.is_some()
                {
                    return Err(invalid(
                        &path,
                        "invalid engagement bounds/return policy or aggression mismatch",
                    ));
                }
                if let Some(aggression) = &engagement.aggression {
                    if aggression.acquisition_range == 0
                        || aggression.acquisition_range > engagement.leash_range
                    {
                        return Err(invalid(&path, "aggression range must fit the source leash"));
                    }
                    self.state_guard(&aggression.guard, &path)?;
                }
            }
        }
        for (id, traversal) in &self.content.mechanics.traversal {
            let path = format!("mechanics.traversal.{id}");
            if id != &traversal.id || traversal.scope == CounterScope::Character {
                return Err(invalid(
                    &path,
                    "invalid traversal identity or spatial scope",
                ));
            }
            nonempty(traversal.edges.len(), &path)?;
            self.state_guard(&traversal.guard, &path)?;
            let mut edges = BTreeSet::new();
            for edge in &traversal.edges {
                if edge.from.plane() != edge.to.plane()
                    || u32::from(edge.from.x().abs_diff(edge.to.x()))
                        + u32::from(edge.from.y().abs_diff(edge.to.y()))
                        != 1
                    || !self.collision.contains_key(&edge.from)
                    || !self.collision.contains_key(&edge.to)
                    || !edges.insert((edge.from, edge.to))
                {
                    return Err(invalid(
                        &path,
                        "traversal edges must be unique listed cardinal source edges",
                    ));
                }
                if edge.bidirectional && !edges.insert((edge.to, edge.from)) {
                    return Err(invalid(&path, "duplicate reversed traversal edge"));
                }
            }
        }
        self.collision_groups()?;
        self.morph_collisions()?;
        if let Some(death) = &self.content.mechanics.death {
            binding(&death.timing, "mechanics.death.timing")?;
            if let Some(interfaces) = &death.interfaces {
                if interfaces.grave == interfaces.office {
                    return Err(invalid(
                        "mechanics.death.interfaces",
                        "grave and Office interfaces must be distinct",
                    ));
                }
                for id in [&interfaces.grave, &interfaces.office] {
                    let interface = self.content.interfaces.get(id).ok_or_else(|| {
                        invalid("mechanics.death.interfaces", "unknown recovery interface")
                    })?;
                    if interface.access != InterfaceAccess::Contextual {
                        return Err(invalid(
                            "mechanics.death.interfaces",
                            "recovery interface must be contextual",
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    fn execution_weapon(&self, weapon: &WeaponDefinition, path: &str) -> GameResult<()> {
        nonempty(weapon.styles.len(), path)?;
        unique(weapon.styles.iter(), path)?;
        if !weapon.styles.contains(&weapon.default_style) {
            return Err(invalid(path, "default style is not offered by this weapon"));
        }
        for style in &weapon.styles {
            if !self.content.mechanics.combat_styles.contains_key(style) {
                return Err(invalid(path, "unknown weapon style"));
            }
        }
        Ok(())
    }

    fn collision_groups(&self) -> GameResult<()> {
        let mut assigned = BTreeSet::new();
        let mut membership = BTreeMap::new();
        let mut group_cells = BTreeMap::new();
        let mut placements = BTreeSet::new();
        for transform in self.content.mechanics.object_transforms.values() {
            if !placements.insert((transform.scope as u8, &transform.spawn)) {
                return Err(invalid(
                    "mechanics.object_transforms",
                    "one source placement cannot have multiple transform owners in a scope",
                ));
            }
        }
        for (id, group) in &self.content.mechanics.collision_groups {
            let path = format!("mechanics.collision_groups.{id}");
            if id != &group.id || group.scope == CounterScope::Character {
                return Err(invalid(&path, "invalid collision group identity/scope"));
            }
            nonempty(group.transforms.len(), &path)?;
            let mut combinations = 1_usize;
            let mut coverage = BTreeSet::new();
            let mut initial = BTreeMap::new();
            for id in &group.transforms {
                let transform = self
                    .content
                    .mechanics
                    .object_transforms
                    .get(id)
                    .ok_or_else(|| invalid(&path, "undefined group transform"))?;
                if transform.scope != group.scope || !assigned.insert(id) {
                    return Err(invalid(
                        &path,
                        "transform has a mismatched or multiple group owner",
                    ));
                }
                membership.insert(id, &group.id);
                combinations = combinations
                    .checked_mul(transform.states.len())
                    .filter(|count| *count <= 4096)
                    .ok_or_else(|| invalid(&path, "combined-state product exceeds 4096"))?;
                initial.insert(id.clone(), transform.initial.clone());
                coverage.extend(
                    transform.states[&transform.initial]
                        .collision
                        .iter()
                        .map(|cell| cell.tile),
                );
            }
            for tile in &coverage {
                if group_cells.insert((group.scope as u8, *tile), id).is_some() {
                    return Err(invalid(
                        &path,
                        "distinct combined groups have overlapping collision coverage",
                    ));
                }
            }
            if let Some(states) = binding(&group.states, &path)? {
                if states.len() != combinations {
                    return Err(invalid(
                        &path,
                        "combined collision states must cover the complete member-state product",
                    ));
                }
                let mut seen = BTreeSet::new();
                for state in states {
                    if state.selection.keys().collect::<BTreeSet<_>>()
                        != group.transforms.iter().collect()
                        || !seen.insert(state.selection.clone())
                    {
                        return Err(invalid(
                            &path,
                            "combined selection has missing/duplicate member identities",
                        ));
                    }
                    for (id, selected) in &state.selection {
                        if !self.content.mechanics.object_transforms[id]
                            .states
                            .contains_key(selected)
                        {
                            return Err(invalid(&path, "unknown member state"));
                        }
                    }
                    let cells = state
                        .collision
                        .iter()
                        .map(|cell| cell.tile)
                        .collect::<BTreeSet<_>>();
                    if cells != coverage || cells.len() != state.collision.len() {
                        return Err(invalid(
                            &path,
                            "combined replacement must cover each member cell exactly once",
                        ));
                    }
                    for cell in &state.collision {
                        let index = self.collision.get(&cell.tile).ok_or_else(|| {
                            invalid(&path, "combined state invents a source cell")
                        })?;
                        if state.selection == initial
                            && &self.content.regions[&index.region].cells[index.offset] != cell
                        {
                            return Err(invalid(
                                &path,
                                "initial combined collision differs from native source cells",
                            ));
                        }
                    }
                }
            }
        }
        let mut effective_cells: BTreeMap<(u8, Tile), Vec<&ObjectTransformId>> = BTreeMap::new();
        for (id, transform) in &self.content.mechanics.object_transforms {
            let initial = &transform.states[&transform.initial];
            for cell in &initial.collision {
                if transform.states.values().any(|state| {
                    state
                        .collision
                        .iter()
                        .find(|other| other.tile == cell.tile)
                        .is_some_and(|other| other != cell)
                }) {
                    effective_cells
                        .entry((transform.scope as u8, cell.tile))
                        .or_default()
                        .push(id);
                }
            }
        }
        for owners in effective_cells.values().filter(|owners| owners.len() > 1) {
            let group = membership.get(owners[0]).ok_or_else(|| {
                invalid(
                    "mechanics.collision_groups",
                    "overlapping mutable transforms require a combined group",
                )
            })?;
            if owners
                .iter()
                .any(|owner| membership.get(owner) != Some(group))
            {
                return Err(invalid(
                    "mechanics.collision_groups",
                    "overlapping transforms must share one combined-state owner",
                ));
            }
        }
        Ok(())
    }

    fn morph_collisions(&self) -> GameResult<()> {
        for object in self.content.objects.values() {
            let Some(morph) = &object.morph else { continue };
            let path = format!("objects.{}.morph", object.id);
            let changes = morph
                .variants
                .values()
                .chain(std::iter::once(&morph.fallback))
                .any(|id| {
                    id.as_ref()
                        .and_then(|id| self.content.objects.get(id))
                        .is_none_or(|variant| {
                            variant.size_x != object.size_x
                                || variant.size_y != object.size_y
                                || variant.clip != object.clip
                        })
                });
            if changes && morph.collision.is_none() {
                return Err(invalid(
                    &path,
                    "geometry-changing/absent morph requires a collision transition binding",
                ));
            }
            let Some(binding_value) = &morph.collision else {
                continue;
            };
            let Some(link) = binding(binding_value, &path)? else {
                continue;
            };
            let counter = self
                .content
                .mechanics
                .counters
                .get(&morph.counter)
                .ok_or_else(|| invalid(&path, "unknown morph counter"))?;
            let placed: BTreeSet<_> = self.content.spawns.values().filter(|spawn| matches!(&spawn.kind, SpawnKind::Object { object: id } if id == &object.id))
                .map(|spawn| &spawn.id).collect();
            if link.placements.keys().collect::<BTreeSet<_>>() != placed || placed.is_empty() {
                return Err(invalid(
                    &path,
                    "morph collision must bind every source placement exactly",
                ));
            }
            for (spawn, id) in &link.placements {
                let transform = self
                    .content
                    .mechanics
                    .object_transforms
                    .get(id)
                    .ok_or_else(|| invalid(&path, "unknown morph collision transform"))?;
                if &transform.spawn != spawn
                    || counter.scope != transform.scope
                    || counter.scope == CounterScope::Character
                {
                    return Err(invalid(
                        &path,
                        "collision morph must use the matching placement and spatial counter scope",
                    ));
                }
                let initial_value = match counter.initial {
                    CounterValue::Integer(value) => value,
                    CounterValue::Boolean(value) => i64::from(value),
                };
                let initial_state = link
                    .variants
                    .iter()
                    .find(|case| case.value == initial_value)
                    .map_or(&link.fallback, |case| &case.state);
                if initial_state != &transform.initial {
                    return Err(invalid(
                        &path,
                        "morph initial counter must select the native initial collision state",
                    ));
                }
                let mut seen = BTreeSet::new();
                for case in &link.variants {
                    if !seen.insert(case.value) {
                        return Err(invalid(&path, "duplicate morph collision selector"));
                    }
                    let expected = morph.variants.get(&case.value).ok_or_else(|| {
                        invalid(&path, "collision selector lacks an object morph variant")
                    })?;
                    if transform
                        .states
                        .get(&case.state)
                        .is_none_or(|state| &state.object != expected)
                    {
                        return Err(invalid(
                            &path,
                            "morph identity and collision state disagree",
                        ));
                    }
                }
                if seen != morph.variants.keys().copied().collect()
                    || transform
                        .states
                        .get(&link.fallback)
                        .is_none_or(|state| state.object != morph.fallback)
                {
                    return Err(invalid(
                        &path,
                        "morph collision binding must cover all variants and fallback",
                    ));
                }
            }
        }
        Ok(())
    }
}
