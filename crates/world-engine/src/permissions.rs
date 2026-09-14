use clubscape_game_types::*;
use clubscape_simulation::{inventory, skills};

use crate::{WorldEngine, invalid_content, unknown};

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
            GameIntent::SelectDialogue { .. } => vec!["dialogue".into()],
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
            GameIntent::Produce { recipe, .. } => {
                vec!["produce".into(), format!("produce:{recipe}")]
            }
            GameIntent::BankDeposit { .. } | GameIntent::BankWithdraw { .. } => vec!["bank".into()],
            GameIntent::ShopBuy { shop, .. } | GameIntent::ShopSell { shop, .. } => {
                vec!["shop".into(), format!("shop:{shop}")]
            }
            GameIntent::SetCombatStyle { .. } => vec!["combat_style".into()],
            GameIntent::Cast { spell, .. } => vec!["cast".into(), format!("cast:{spell}")],
            GameIntent::SetPrayer { prayer, .. } => {
                vec!["prayer".into(), format!("prayer:{prayer}")]
            }
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
            .ok_or_else(|| unknown(format!("Unknown stage {}.", character.tutorial_stage)))?;
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

    pub(crate) fn guard(&self, character: &CharacterState, guard: &Guard) -> GameResult<bool> {
        Ok(self.guard_value(character, guard, 0)? == Some(true))
    }

    fn guard_value(
        &self,
        character: &CharacterState,
        guard: &Guard,
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
                let values = guards
                    .iter()
                    .map(|guard| self.guard_value(character, guard, depth + 1))
                    .collect::<GameResult<Vec<_>>>()?;
                if values.contains(&Some(!all)) {
                    Some(!all)
                } else if values.contains(&None) {
                    None
                } else {
                    Some(all)
                }
            }
            Guard::Not { guard } => self
                .guard_value(character, guard, depth + 1)?
                .map(|value| !value),
            Guard::Flag { name, equals } => character.flags.get(name).map(|value| value == equals),
            Guard::TutorialStage { stage } => Some(&character.tutorial_stage == stage),
            Guard::QuestStage { quest, stage } => character
                .quests
                .get(quest)
                .map(|state| &state.stage == stage),
            Guard::HasItems { items } => {
                let mut inventory = character.inventory.clone();
                match inventory::remove_batch(&mut inventory, &self.content.items, items) {
                    Ok(()) => Some(true),
                    Err(error) if error.code == GameErrorCode::InsufficientItems => Some(false),
                    Err(error) => return Err(error),
                }
            }
            Guard::Equipped { item } => Some(
                character
                    .equipment
                    .values()
                    .any(|stack| &stack.item == item),
            ),
            Guard::SkillAtLeast { requirement } => {
                match skills::check_requirements(
                    &character.skills,
                    &self.content.skills,
                    std::slice::from_ref(requirement),
                    skills::LevelBasis::Current,
                ) {
                    Ok(()) => Some(true),
                    Err(error) if error.code == GameErrorCode::RequirementNotMet => Some(false),
                    Err(error) => return Err(error),
                }
            }
            Guard::InterfaceUnlocked { interface } => {
                Some(character.interfaces.contains(interface))
            }
            Guard::Within { tile, distance } => Some(
                character
                    .tile
                    .distance(*tile)
                    .is_some_and(|actual| actual <= *distance),
            ),
        };
        Ok(value)
    }

    pub(crate) fn require_guard(
        &self,
        character: &CharacterState,
        guard: &Guard,
    ) -> GameResult<()> {
        if !self.guard(character, guard)? {
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
        self.require_guard(character, &interaction.guard)?;
        let spawn = self
            .content
            .spawns
            .get(target)
            .ok_or_else(|| unknown(format!("Unknown interaction target {target}.")))?;
        let entity = world
            .entities
            .get(target)
            .ok_or_else(|| unknown(format!("World has no state for spawn {target}.")))?;
        if entity.available_at_tick > world.tick {
            return Err(GameError::new(
                GameErrorCode::Busy,
                "Target is depleted or awaiting respawn.",
            ));
        }
        let (width, height) = match &spawn.kind {
            SpawnKind::Object { object } => {
                let object = self
                    .content
                    .objects
                    .get(object)
                    .ok_or_else(|| unknown(format!("Unknown object {object}.")))?;
                if spawn.facing > 3 {
                    return Err(invalid_content(
                        "Object facing must be bound as quarter turns 0 through 3.",
                    ));
                }
                if spawn.facing % 2 == 1 {
                    (object.size_y, object.size_x)
                } else {
                    (object.size_x, object.size_y)
                }
            }
            SpawnKind::Npc { npc } => {
                let npc = self
                    .content
                    .npcs
                    .get(npc)
                    .ok_or_else(|| unknown(format!("Unknown NPC {npc}.")))?;
                if npc.combat.is_some() && entity.hitpoints == 0 {
                    return Err(GameError::new(GameErrorCode::Busy, "NPC is defeated."));
                }
                (npc.size, npc.size)
            }
            SpawnKind::Item { .. } => (1, 1),
        };
        if width == 0 || height == 0 {
            return Err(invalid_content(
                "Interaction target has an empty footprint.",
            ));
        }
        for dx in 0..width {
            for dy in 0..height {
                let Some(tile) = entity.tile.offset(i16::from(dx), i16::from(dy)) else {
                    continue;
                };
                if character
                    .tile
                    .distance(tile)
                    .is_some_and(|distance| distance <= interaction.reach)
                    && self.collision.line_of_sight(character.tile, tile)
                    && (interaction.reach > 1 || self.touch_edge(character.tile, tile))
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

    fn touch_edge(&self, from: Tile, to: Tile) -> bool {
        if from == to {
            return true;
        }
        let Some(direction) = Direction::ALL.iter().find(|direction| {
            let (dx, dy) = direction.offset();
            from.offset(dx, dy) == Some(to)
        }) else {
            return false;
        };
        let edge = |from, to, direction: Direction| {
            let (Some(a), Some(b)) = (self.collision.cell(from), self.collision.cell(to)) else {
                return false;
            };
            a.blocked_movement & direction.mask() == 0
                && b.blocked_movement & direction.opposite().mask() == 0
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
        let horizontal = if dx > 0 {
            Direction::East
        } else {
            Direction::West
        };
        let vertical = if dy > 0 {
            Direction::North
        } else {
            Direction::South
        };
        self.collision.cell(x).is_some_and(|cell| cell.walkable)
            && self.collision.cell(y).is_some_and(|cell| cell.walkable)
            && edge(from, x, horizontal)
            && edge(x, to, vertical)
            && edge(from, y, vertical)
            && edge(y, to, horizontal)
    }
}
