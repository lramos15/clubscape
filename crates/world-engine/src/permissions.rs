use std::collections::BTreeMap;

use clubscape_game_types::*;
use clubscape_simulation::{bank, inventory, navigation::CollisionMap, skills};

use crate::{WorldEngine, invalid_content, invalid_state, unknown};

impl WorldEngine {
    pub(crate) fn authorize_intent(
        &self,
        character: &CharacterState,
        intent: &GameIntent,
    ) -> GameResult<()> {
        let keys = match intent {
            GameIntent::Walk { .. } => vec!["walk".into()],
            GameIntent::Interact { target, action } => vec![
                "interact".into(),
                format!("interact:{target}"),
                format!("interact:{target}:{action}"),
            ],
            GameIntent::InteractWith {
                target: WorldTarget::Spawn { spawn },
                action,
            } => vec![
                "interact_with".into(),
                "interact".into(),
                format!("interact:{spawn}"),
                format!("interact:{spawn}:{action}"),
            ],
            GameIntent::InteractWith { .. } => vec!["interact_with".into(), "interact".into()],
            GameIntent::SelectDialogue { .. } => vec!["dialogue".into(), "select_dialogue".into()],
            GameIntent::OpenInterface { interface } => vec![
                "open_interface".into(),
                format!("open_interface:{interface}"),
            ],
            GameIntent::Equip { .. } => vec!["equip".into()],
            GameIntent::Unequip { .. } => vec!["unequip".into()],
            GameIntent::Drop { .. } => vec!["drop".into()],
            GameIntent::TakeGroundItem { .. } => vec!["take_ground_item".into()],
            GameIntent::UseItem { .. } => vec!["use_item".into()],
            GameIntent::MoveInventory { .. } => vec!["move_inventory".into()],
            GameIntent::Eat { .. } => vec!["eat".into()],
            GameIntent::Produce { recipe, .. }
            | GameIntent::ProduceAt { recipe, .. }
            | GameIntent::ProduceSelected { recipe, .. } => vec![
                "produce".into(),
                "produce_at".into(),
                "produce_selected".into(),
                format!("produce:{recipe}"),
            ],
            GameIntent::BankDeposit { .. } => vec!["bank".into(), "bank_deposit".into()],
            GameIntent::BankWithdraw { .. } => vec!["bank".into(), "bank_withdraw".into()],
            GameIntent::ShopBuy { shop, .. } => {
                vec!["shop".into(), "shop_buy".into(), format!("shop:{shop}")]
            }
            GameIntent::ShopSell { shop, .. } => {
                vec!["shop".into(), "shop_sell".into(), format!("shop:{shop}")]
            }
            GameIntent::SetCombatStyle { .. } => {
                vec!["combat_style".into(), "set_combat_style".into()]
            }
            GameIntent::Cast { spell, .. } => vec!["cast".into(), format!("cast:{spell}")],
            GameIntent::SetPrayer { prayer, .. } => vec![
                "prayer".into(),
                "set_prayer".into(),
                format!("prayer:{prayer}"),
            ],
            GameIntent::SetSetting { .. } => vec!["set_setting".into()],
            GameIntent::ConfirmAppearance { .. } => vec!["confirm_appearance".into()],
            GameIntent::SelectExperience { .. } => vec!["select_experience".into()],
            GameIntent::Reclaim { .. } => vec!["reclaim".into()],
            GameIntent::OpenGrave { .. } => vec!["open_grave".into()],
            GameIntent::OpenDeathOffice => vec!["open_death_office".into()],
            GameIntent::CancelActivity | GameIntent::RequestLogout | GameIntent::CloseInterface => {
                return Ok(());
            }
        };
        self.authorize(character, &keys)
    }

    pub(crate) fn authorize(&self, character: &CharacterState, keys: &[String]) -> GameResult<()> {
        let stage = self
            .content
            .tutorial
            .get(&character.tutorial_stage)
            .ok_or_else(|| unknown("Unknown tutorial stage."))?;
        if stage
            .allowed_actions
            .iter()
            .any(|action| action == "*" || keys.contains(action))
        {
            return Ok(());
        }
        Err(GameError::new(
            GameErrorCode::RequirementNotMet,
            format!(
                "Action {} is locked in stage {}.",
                keys.join(" / "),
                stage.id
            ),
        ))
    }

    pub(crate) fn level(
        &self,
        character: &CharacterState,
        skill: &SkillId,
        basis: SkillLevelBasis,
    ) -> GameResult<u16> {
        let definition = self
            .content
            .skills
            .get(skill)
            .ok_or_else(|| unknown(format!("Unknown skill {skill}.")))?;
        let state = character
            .skills
            .get(skill)
            .ok_or_else(|| invalid_state(format!("Missing skill state {skill}.")))?;
        let base = skills::level_for_xp(definition, state.xp_tenths)?;
        Ok(match basis {
            SkillLevelBasis::Base => base,
            SkillLevelBasis::Current => state.current_level,
        })
    }

    pub(crate) fn requirements(
        &self,
        character: &CharacterState,
        requirements: &[SkillRequirement],
    ) -> GameResult<()> {
        for requirement in requirements {
            skills::check_requirements(
                &character.skills,
                &self.content.skills,
                std::slice::from_ref(requirement),
                match requirement.basis {
                    SkillLevelBasis::Base => skills::LevelBasis::Base,
                    SkillLevelBasis::Current => skills::LevelBasis::Current,
                },
            )?;
        }
        Ok(())
    }

    pub(crate) fn owned_count(
        &self,
        character: &CharacterState,
        item: &ItemId,
        scope: &OwnershipScope,
    ) -> GameResult<u32> {
        let inventory_count = || inventory::count(&character.inventory, &self.content.items, item);
        let equipment_count = || {
            character
                .equipment
                .values()
                .filter(|stack| &stack.item == item)
                .try_fold(0_u32, |sum, stack| {
                    sum.checked_add(stack.quantity.get())
                        .ok_or_else(|| invalid_state("Owned item count overflow."))
                })
        };
        match scope {
            OwnershipScope::Inventory => inventory_count(),
            OwnershipScope::Equipment => equipment_count(),
            OwnershipScope::InventoryAndEquipment => inventory_count()?
                .checked_add(equipment_count()?)
                .ok_or_else(|| invalid_state("Owned item count overflow.")),
            OwnershipScope::Bank => bank::count(&character.bank, &self.content.items, item),
        }
    }

    pub(crate) fn counter_value(
        &self,
        world: &WorldState,
        character: &CharacterState,
        id: &CounterId,
    ) -> GameResult<CounterValue> {
        let definition = self
            .content
            .mechanics
            .counters
            .get(id)
            .ok_or_else(|| unknown("Unknown counter."))?;
        let counters = match definition.scope {
            CounterScope::Character => &character.runtime.counters,
            CounterScope::World => &world.runtime.counters,
            CounterScope::Instance => {
                &world
                    .runtime
                    .instances
                    .get(character.runtime.instance.as_ref().ok_or_else(|| {
                        GameError::new(
                            GameErrorCode::RequirementNotMet,
                            "Counter needs an instance.",
                        )
                    })?)
                    .ok_or_else(|| invalid_state("Unknown instance."))?
                    .counters
            }
        };
        let value = *counters
            .get(id)
            .ok_or_else(|| invalid_state("Persisted counter is missing; migrate explicitly."))?;
        definition.validate_value(value)?;
        Ok(value)
    }

    pub(crate) fn guard(
        &self,
        world: &WorldState,
        character: &CharacterState,
        guard: &Guard,
        event: Option<&GameEvent>,
    ) -> GameResult<bool> {
        Ok(self.guard_value(world, character, guard, event, 0)? == Some(true))
    }

    fn guard_value(
        &self,
        world: &WorldState,
        character: &CharacterState,
        guard: &Guard,
        event: Option<&GameEvent>,
        depth: usize,
    ) -> GameResult<Option<bool>> {
        if depth > 64 {
            return Err(invalid_content(
                "Guard nesting exceeds the supported bound.",
            ));
        }
        let value = match guard {
            Guard::Always => Some(true),
            Guard::All { guards } | Guard::Any { guards } => {
                let all = matches!(guard, Guard::All { .. });
                let mut missing = false;
                for guard in guards {
                    match self.guard_value(world, character, guard, event, depth + 1)? {
                        Some(value) if value != all => return Ok(Some(!all)),
                        None => missing = true,
                        _ => {}
                    }
                }
                if missing { None } else { Some(all) }
            }
            Guard::Not { guard } => self
                .guard_value(world, character, guard, event, depth + 1)?
                .map(|value| !value),
            Guard::Flag { name, equals } => character.flags.get(name).map(|value| value == equals),
            Guard::TutorialStage { stage } => Some(&character.tutorial_stage == stage),
            Guard::QuestStage { quest, stage } => character
                .quests
                .get(quest)
                .map(|state| &state.stage == stage),
            Guard::HasItems { items } | Guard::OwnsItems { items, .. } => {
                let scope = match guard {
                    Guard::OwnsItems { scope, .. } => scope,
                    _ => &OwnershipScope::Inventory,
                };
                let mut totals = BTreeMap::<&ItemId, u32>::new();
                for stack in items {
                    let count = totals.entry(&stack.item).or_default();
                    *count = count
                        .checked_add(stack.quantity.get())
                        .ok_or_else(|| invalid_content("Guard item total overflow."))?;
                }
                let mut owns = true;
                for (item, count) in totals {
                    owns &= self.owned_count(character, item, scope)? >= count;
                }
                Some(owns)
            }
            Guard::Equipped { item } => Some(
                character
                    .equipment
                    .values()
                    .any(|stack| &stack.item == item),
            ),
            Guard::SkillAtLeast { requirement } => Some(
                self.level(character, &requirement.skill, requirement.basis)? >= requirement.level,
            ),
            Guard::InterfaceUnlocked { interface } => {
                Some(character.interfaces.contains(interface))
            }
            Guard::Within { tile, distance } => {
                let tile =
                    self.map_instance_tile(world, character.runtime.instance.as_ref(), *tile)?;
                Some(
                    character
                        .tile
                        .distance(tile)
                        .is_some_and(|actual| actual <= *distance),
                )
            }
            Guard::FreeCapacity { container, slots } => {
                let free = match container {
                    ContainerKind::Inventory => character
                        .inventory
                        .slots
                        .iter()
                        .filter(|slot| slot.is_none())
                        .count(),
                    ContainerKind::Bank => {
                        usize::from(character.bank.capacity)
                            - character.bank.slots.iter().flatten().count()
                    }
                    ContainerKind::Equipment => self
                        .content
                        .equipment_slots
                        .iter()
                        .filter(|slot| {
                            clubscape_simulation::equipment::occupant(
                                &character.equipment,
                                &self.content,
                                slot,
                            )
                            .is_ok_and(|item| item.is_none())
                        })
                        .count(),
                    ContainerKind::Grave => self
                        .content
                        .mechanics
                        .death
                        .as_ref()
                        .map(|policy| usize::from(policy.grave_capacity))
                        .ok_or_else(|| unknown("Missing grave policy."))?
                        .saturating_sub(
                            character
                                .runtime
                                .active_death
                                .as_ref()
                                .and_then(|id| world.runtime.deaths.get(id))
                                .and_then(|record| record.grave.as_ref())
                                .map_or(0, |grave| grave.items.len()),
                        ),
                    ContainerKind::DeathOffice => self
                        .content
                        .mechanics
                        .death
                        .as_ref()
                        .map(|policy| usize::from(policy.office_capacity))
                        .ok_or_else(|| unknown("Missing Office policy."))?
                        .saturating_sub(
                            world
                                .runtime
                                .deaths
                                .values()
                                .filter(|record| record.owner == character.actor_id)
                                .flat_map(|record| &record.office)
                                .map(|item| recovery_slot_key(&item.stack))
                                .collect::<std::collections::BTreeSet<_>>()
                                .len(),
                        ),
                    ContainerKind::Ground => {
                        return Err(invalid_content("Ground has no finite slot-capacity guard."));
                    }
                };
                Some(free >= usize::from(*slots))
            }
            Guard::Counter { counter, predicate } => {
                let value = self.counter_value(world, character, counter)?;
                Some(match predicate {
                    CounterPredicate::Equals { value: expected } => value == *expected,
                    CounterPredicate::IntegerRange { minimum, maximum } => {
                        matches!(value, CounterValue::Integer(value) if (*minimum..=*maximum).contains(&value))
                    }
                })
            }
            Guard::EntitlementClaimed { entitlement } => Some(matches!(
                character.runtime.entitlements.get(entitlement),
                Some(
                    EntitlementState::Claimed { .. }
                        | EntitlementState::Grant { complete: true, .. }
                )
            )),
            Guard::Event { condition } => Some(condition.matches(event.ok_or_else(|| {
                invalid_content("Event predicates cannot run as pre-action guards.")
            })?)),
            Guard::Experience { experience } => character
                .runtime
                .settings
                .experience
                .as_ref()
                .map(|value| value == experience),
            Guard::Setting { setting } => match setting {
                CharacterSetting::Run(value) => character
                    .runtime
                    .settings
                    .run_enabled
                    .map(|actual| actual == *value),
                CharacterSetting::AutoRetaliate(value) => character
                    .runtime
                    .settings
                    .auto_retaliate
                    .map(|actual| actual == *value),
                CharacterSetting::DeathAutoEquip(value) => character
                    .runtime
                    .settings
                    .death_auto_equip
                    .map(|actual| actual == *value),
                CharacterSetting::DeathSupplyPiles(value) => character
                    .runtime
                    .settings
                    .death_supply_piles
                    .map(|actual| actual == *value),
            },
            Guard::Life { phase } => Some(matches!(
                (phase, &character.runtime.life),
                (LifePhase::Alive, LifeState::Alive)
                    | (LifePhase::Dying, LifeState::Dying { .. })
                    | (
                        LifePhase::FirstDeathOffice,
                        LifeState::FirstDeathOffice { .. }
                    )
                    | (LifePhase::Respawning, LifeState::Respawning { .. })
            )),
            Guard::DeathTopics { topics } => {
                Some(topics.is_subset(&character.runtime.death_topics))
            }
            Guard::Charges {
                item,
                charge_kind,
                minimum,
            } => Some(character.inventory.slots.iter().flatten().any(|stack| {
                &stack.item == item
                    && stack
                        .instance
                        .as_ref()
                        .and_then(|instance| instance.charges.as_ref())
                        .is_some_and(|charges| {
                            &charges.kind == charge_kind && charges.remaining >= *minimum
                        })
            })),
            Guard::MembersWorld => world.runtime.members,
        };
        Ok(value)
    }

    pub(crate) fn require_guard(
        &self,
        world: &WorldState,
        character: &CharacterState,
        guard: &Guard,
    ) -> GameResult<()> {
        if !self.guard(world, character, guard, None)? {
            return Err(GameError::new(
                GameErrorCode::RequirementNotMet,
                "The source interaction guard is not satisfied.",
            ));
        }
        Ok(())
    }

    pub(crate) fn require_target(
        &self,
        world: &WorldState,
        character: &CharacterState,
        target: &SpawnId,
        interaction: &InteractionDefinition,
    ) -> GameResult<()> {
        self.require_world_target(
            world,
            character,
            &WorldTarget::Spawn {
                spawn: target.clone(),
            },
            interaction,
        )
    }

    pub(crate) fn require_world_target(
        &self,
        world: &WorldState,
        character: &CharacterState,
        target: &WorldTarget,
        interaction: &InteractionDefinition,
    ) -> GameResult<()> {
        self.require_guard(world, character, &interaction.guard)?;
        let shape = self.target_shape(world, character, target)?;
        let map = self.collision_for(world, character.runtime.instance.as_ref())?;
        let mut side = 0;
        if character.tile.x() < shape.tile.x() {
            side |= Direction::West.mask();
        }
        if u32::from(character.tile.x()) >= u32::from(shape.tile.x()) + u32::from(shape.width) {
            side |= Direction::East.mask();
        }
        if character.tile.y() < shape.tile.y() {
            side |= Direction::South.mask();
        }
        if u32::from(character.tile.y()) >= u32::from(shape.tile.y()) + u32::from(shape.height) {
            side |= Direction::North.mask();
        }
        if side & shape.blocked_access != 0 {
            return Err(GameError::new(
                GameErrorCode::OutOfReach,
                "This source object side does not permit access.",
            ));
        }
        if let Some(access) = &shape.access
            && !access.contains(&character.tile)
        {
            return Err(GameError::new(
                GameErrorCode::OutOfReach,
                "Use a source-declared stationary-actor access tile.",
            ));
        }
        for dx in 0..shape.width {
            for dy in 0..shape.height {
                let tile = shape
                    .tile
                    .offset(i16::from(dx), i16::from(dy))
                    .ok_or_else(|| invalid_content("Target footprint overflow."))?;
                if character
                    .tile
                    .distance(tile)
                    .is_some_and(|distance| distance <= interaction.reach)
                    && ((map.line_of_sight(character.tile, tile)
                        && (interaction.reach > 1 || touch_edge(&map, character.tile, tile)))
                        || (interaction.reach == 1
                            && self.door_face_reachable(world, character, target, tile)?))
                {
                    return Ok(());
                }
            }
        }
        Err(GameError::new(
            GameErrorCode::OutOfReach,
            "Target is on another plane, out of range, or blocked.",
        ))
    }

    fn door_face_reachable(
        &self,
        world: &WorldState,
        character: &CharacterState,
        target: &WorldTarget,
        tile: Tile,
    ) -> GameResult<bool> {
        let WorldTarget::Spawn { spawn } = target else {
            return Ok(false);
        };
        if character.tile.distance(tile) != Some(1)
            || character.tile.x() != tile.x() && character.tile.y() != tile.y()
        {
            return Ok(false);
        }
        let states = match &character.runtime.instance {
            Some(id) => {
                &world
                    .runtime
                    .instances
                    .get(id)
                    .ok_or_else(|| invalid_state("Unknown door instance."))?
                    .object_states
            }
            None => &world.runtime.object_states,
        };
        for (id, selected) in states {
            let definition = self
                .content
                .mechanics
                .object_transforms
                .get(id)
                .ok_or_else(|| unknown("Unknown door transform."))?;
            if &definition.spawn != spawn
                || !definition
                    .states
                    .get(selected)
                    .is_some_and(|state| state.door == Some(DoorPosition::Closed))
            {
                continue;
            }
            // A closed door's near face is reachable without moving through it. Only its
            // explicit open collision replacement is removed from this contact query.
            for (open_id, _) in definition
                .states
                .iter()
                .filter(|(_, state)| state.door == Some(DoorPosition::Open))
            {
                let mut query_world = world.clone();
                let states = match &character.runtime.instance {
                    Some(id) => {
                        &mut query_world
                            .runtime
                            .instances
                            .get_mut(id)
                            .ok_or_else(|| invalid_state("Unknown door instance."))?
                            .object_states
                    }
                    None => &mut query_world.runtime.object_states,
                };
                states.insert(id.clone(), open_id.clone());
                let map = self.collision_for(&query_world, character.runtime.instance.as_ref())?;
                if map.line_of_sight(character.tile, tile) && touch_edge(&map, character.tile, tile)
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

pub(crate) fn touch_edge(map: &CollisionMap, from: Tile, to: Tile) -> bool {
    if from == to {
        return true;
    }
    let Some(direction) = Direction::ALL.iter().find(|direction| {
        let (dx, dy) = direction.offset();
        from.offset(dx, dy) == Some(to)
    }) else {
        return false;
    };
    let edge = |a, b, d: Direction| {
        let (Some(a), Some(b)) = (map.cell(a), map.cell(b)) else {
            return false;
        };
        a.blocked_movement & d.mask() == 0 && b.blocked_movement & d.opposite().mask() == 0
    };
    if !edge(from, to, *direction) {
        return false;
    }
    let (dx, dy) = direction.offset();
    if dx == 0 || dy == 0 {
        return true;
    }
    let (Some(x), Some(y)) = (from.offset(dx, 0), from.offset(0, dy)) else {
        return false;
    };
    let h = if dx > 0 {
        Direction::East
    } else {
        Direction::West
    };
    let v = if dy > 0 {
        Direction::North
    } else {
        Direction::South
    };
    map.cell(x).is_some_and(|cell| cell.walkable)
        && map.cell(y).is_some_and(|cell| cell.walkable)
        && edge(from, x, h)
        && edge(x, to, v)
        && edge(from, y, v)
        && edge(y, to, h)
}
