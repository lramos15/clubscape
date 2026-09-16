use std::collections::btree_map::Entry;

use super::*;

impl Validator<'_> {
    pub(super) fn regions(&mut self) -> GameResult<()> {
        for region in self.content.regions.values() {
            let path = format!("regions.{}", region.id);
            text(&region.name, &path, 256)?;
            if region.min.x() > region.max.x()
                || region.min.y() > region.max.y()
                || region.min.plane() > region.max.plane()
            {
                return Err(invalid(
                    &path,
                    "region bounds must be inclusive and ordered on every axis",
                ));
            }
            nonempty(region.cells.len(), &format!("{path}.cells"))?;
            bounded(region.source_map_squares.len(), &path)?;
            unique(region.source_map_squares.iter(), &path)?;
            for (offset, cell) in region.cells.iter().enumerate() {
                let path = format!("{path}.cells[{offset}]");
                if !contains(region, cell.tile) {
                    return Err(invalid(
                        &path,
                        "collision cell is outside its region bounds",
                    ));
                }
                match self.collision.entry(cell.tile) {
                    Entry::Occupied(previous) => {
                        return Err(invalid(
                            &path,
                            format!(
                                "duplicate collision cell; tile is already owned by {} (even identical duplicates are forbidden)",
                                previous.get().region,
                            ),
                        ));
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(CellIndex {
                            region: region.id.clone(),
                            offset,
                        });
                    }
                }
                if self.collision.len() > crate::decode::MAX_DATA_NODES {
                    return Err(invalid(&path, "total collision-cell budget exceeded"));
                }
            }
        }
        Ok(())
    }

    pub(super) fn location(
        &self,
        region: &RegionId,
        tile: Tile,
        walkable: bool,
        path: &str,
    ) -> GameResult<()> {
        if !self.content.regions.contains_key(region) {
            return Err(invalid(path, format!("undefined region {region}")));
        }
        let index = self
            .collision
            .get(&tile)
            .ok_or_else(|| invalid(path, "tile has no explicit collision definition"))?;
        if &index.region != region {
            return Err(invalid(
                path,
                format!("tile is explicitly owned by {}, not {region}", index.region),
            ));
        }
        if walkable && !self.content.regions[region].cells[index.offset].walkable {
            return Err(invalid(path, "destination/actor tile is not walkable"));
        }
        Ok(())
    }

    pub(super) fn spawns(&self) -> GameResult<BTreeMap<Tile, Vec<SpawnId>>> {
        let mut positions = BTreeSet::new();
        let mut spawns_at = BTreeMap::<Tile, Vec<SpawnId>>::new();
        for spawn in self.content.spawns.values() {
            let path = format!("spawns.{}", spawn.id);
            self.location(&spawn.region, spawn.tile, false, &path)?;
            if spawn.facing > 7 {
                return Err(invalid(
                    &path,
                    "facing must be a canonical 0..=7 direction, not an unmapped source angle",
                ));
            }
            self.source_placement(spawn, &path)?;
            let (kind, definition) = match &spawn.kind {
                SpawnKind::Npc { npc } => {
                    let definition = self
                        .content
                        .npcs
                        .get(npc)
                        .ok_or_else(|| invalid(&path, format!("undefined NPC {npc}")))?;
                    self.npc_placement(spawn, definition, &path)?;
                    ("npc", npc.as_str())
                }
                SpawnKind::Object { object } => {
                    if !self.content.objects.contains_key(object) {
                        return Err(invalid(&path, format!("undefined object {object}")));
                    }
                    ("object", object.as_str())
                }
                SpawnKind::Item {
                    stack,
                    respawn_ticks,
                } => {
                    self.stacks(std::slice::from_ref(stack), &path, false)?;
                    if *respawn_ticks == 0 {
                        return Err(invalid(&path, "item respawn duration must be positive"));
                    }
                    ("item", stack.item.as_str())
                }
            };
            if !positions.insert((spawn.tile, kind, definition)) {
                return Err(invalid(
                    &path,
                    "duplicate placement of the same kind and definition at one tile",
                ));
            }
            bounded(spawn.interactions.len(), &path)?;
            unique(
                spawn
                    .interactions
                    .iter()
                    .map(|interaction| &interaction.name),
                &path,
            )?;
            for (index, interaction) in spawn.interactions.iter().enumerate() {
                let path = format!("{path}.interactions[{index}]");
                text(&interaction.name, &path, 160)?;
                self.state_guard(&interaction.guard, &format!("{path}.guard"))?;
                match &interaction.action {
                    InteractionAction::Effects { effects } => {
                        nonempty(effects.len(), &path)?;
                        self.state_effects(effects, &path)?;
                    }
                    InteractionAction::Dialogue { dialogue } => {
                        if !self.content.dialogues.contains_key(dialogue) {
                            return Err(invalid(&path, format!("undefined dialogue {dialogue}")));
                        }
                    }
                    InteractionAction::Gather { rule } => {
                        if matches!(spawn.kind, SpawnKind::Item { .. }) {
                            return Err(invalid(
                                &path,
                                "gathering requires an NPC or object placement",
                            ));
                        }
                        self.gather(rule, &path)?;
                    }
                    InteractionAction::Production { recipes } => {
                        nonempty(recipes.len(), &path)?;
                        unique(recipes.iter(), &path)?;
                        for recipe in recipes {
                            let recipe = self.content.recipes.get(recipe).ok_or_else(|| {
                                invalid(&path, format!("undefined recipe {recipe}"))
                            })?;
                            if recipe.item_on_target.is_some() {
                                return Err(invalid(
                                    &path,
                                    "item-on-only recipe cannot also be offered by a Production menu",
                                ));
                            }
                            if !recipe.target_objects.is_empty() {
                                let SpawnKind::Object { object } = &spawn.kind else {
                                    return Err(invalid(
                                        &path,
                                        "recipe requires an object target, not this spawn kind",
                                    ));
                                };
                                if !recipe.target_objects.contains(object) {
                                    return Err(invalid(
                                        &path,
                                        format!("{} does not allow object {object}", recipe.id),
                                    ));
                                }
                            }
                        }
                    }
                    InteractionAction::Bank | InteractionAction::Shop { .. } => {
                        if matches!(spawn.kind, SpawnKind::Item { .. }) {
                            return Err(invalid(
                                &path,
                                "bank/shop interaction requires an NPC or object",
                            ));
                        }
                        if let InteractionAction::Shop { shop } = &interaction.action
                            && !self.content.shops.contains_key(shop)
                        {
                            return Err(invalid(&path, format!("undefined shop {shop}")));
                        }
                    }
                    InteractionAction::Attack => {
                        let SpawnKind::Npc { npc } = &spawn.kind else {
                            return Err(invalid(&path, "Attack requires an NPC spawn"));
                        };
                        if self.content.npcs[npc].combat.is_none() {
                            return Err(invalid(&path, "Attack requires an NPC combat definition"));
                        }
                    }
                    InteractionAction::Travel {
                        destination,
                        region,
                    } => {
                        self.location(region, *destination, true, &path)?;
                    }
                    InteractionAction::TravelVia { travel } => self.travel(travel, &path)?,
                    InteractionAction::OpenBank {
                        interface,
                        before_open,
                    }
                    | InteractionAction::OpenShop {
                        interface,
                        before_open,
                        ..
                    } => {
                        if matches!(spawn.kind, SpawnKind::Item { .. }) {
                            return Err(invalid(
                                &path,
                                "contextual bank/shop requires an NPC or object",
                            ));
                        }
                        self.interface(interface, &path)?;
                        if self.content.interfaces[interface].access != InterfaceAccess::Contextual
                        {
                            return Err(invalid(
                                &path,
                                "bank/shop presentation requires a contextual interface",
                            ));
                        }
                        self.state_effects(before_open, &path)?;
                        if let InteractionAction::OpenShop { shop, .. } = &interaction.action
                            && !self.content.shops.contains_key(shop)
                        {
                            return Err(invalid(&path, "undefined contextual shop"));
                        }
                        self.before_presentation(before_open, &path)?;
                    }
                    InteractionAction::Unavailable { reason } => {
                        text(reason, &path, MAX_TEXT_BYTES)?;
                    }
                }
            }
            spawns_at
                .entry(spawn.tile)
                .or_default()
                .push(spawn.id.clone());
        }
        Ok(spawns_at)
    }

    fn gather(&self, rule: &GatherRule, path: &str) -> GameResult<()> {
        self.requirement(
            &SkillRequirement {
                skill: rule.skill.clone(),
                level: rule.required_level,
                basis: rule
                    .mechanics
                    .as_ref()
                    .map_or(SkillLevelBasis::Current, |mechanics| mechanics.levels.basis),
            },
            path,
        )?;
        bounded(rule.tools.len(), path)?;
        unique(rule.tools.iter(), path)?;
        for tool in &rule.tools {
            if self.item(tool, path)?.unnoted_variant.is_some() {
                return Err(invalid(path, "a noted item cannot be a gathering tool"));
            }
        }
        self.stacks(std::slice::from_ref(&rule.output), path, true)?;
        self.xp(
            &[XpReward {
                skill: rule.skill.clone(),
                amount_tenths: rule.xp_tenths,
            }],
            path,
        )?;
        self.gather_mechanics(rule, path)?;
        chance(&rule.success, &format!("{path}.success"), true)?;
        chance(&rule.depletion, &format!("{path}.depletion"), false)?;
        Ok(())
    }
}

fn contains(region: &RegionDefinition, tile: Tile) -> bool {
    (region.min.x()..=region.max.x()).contains(&tile.x())
        && (region.min.y()..=region.max.y()).contains(&tile.y())
        && (region.min.plane()..=region.max.plane()).contains(&tile.plane())
}
