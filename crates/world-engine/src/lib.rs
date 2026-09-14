//! Transactional headless actions over caller-validated shared content/state.
//! This crate does not certify the real M1 journey; see its integration blockers.

mod actions;
mod activities;
mod commerce;
mod permissions;
mod progression;
mod runtime;
mod validation;

pub mod random;
pub mod source_math;

use std::collections::BTreeMap;
use std::sync::Arc;

use clubscape_game_types::*;
use clubscape_simulation::navigation::CollisionMap;
use serde::{Deserialize, Serialize};

pub use random::RandomSource;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorEvent {
    pub actor_id: ActorId,
    pub event: GameEvent,
}

#[derive(Clone, Debug)]
pub struct WorldEngine {
    content: Arc<GameContent>,
    collision: CollisionMap,
    regions_by_tile: BTreeMap<Tile, RegionId>,
}

impl WorldEngine {
    /// Requires compiler-validated content. Local checks are additional, not a compiler.
    pub fn new(content: Arc<GameContent>) -> GameResult<Self> {
        validation::content(&content)?;
        let collision = CollisionMap::from_regions(content.regions.values())?;
        let regions_by_tile = content
            .regions
            .iter()
            .flat_map(|(id, region)| region.cells.iter().map(|cell| (cell.tile, id.clone())))
            .collect();
        let engine = Self {
            content,
            collision,
            regions_by_tile,
        };
        engine.validate_destination(
            &engine.content.initial_state.region,
            engine.content.initial_state.tile,
        )?;
        Ok(engine)
    }

    pub fn content(&self) -> &GameContent {
        &self.content
    }

    pub fn initial_world(&self) -> GameResult<WorldState> {
        let mut world = WorldState {
            schema_version: GAME_SCHEMA_VERSION,
            content_revision: self.content.revision.clone(),
            tick: 0,
            revision: 0,
            characters: BTreeMap::new(),
            entities: BTreeMap::new(),
            shops: BTreeMap::new(),
            ground_items: Vec::new(),
            runtime: WorldRuntime::from_initial(&self.content),
        };
        for (id, spawn) in &self.content.spawns {
            let hitpoints = match &spawn.kind {
                SpawnKind::Npc { npc } => self
                    .content
                    .npcs
                    .get(npc)
                    .ok_or_else(|| unknown(format!("Unknown NPC {npc}.")))?
                    .combat
                    .as_ref()
                    .map_or(0, |combat| combat.hitpoints),
                _ => 0,
            };
            world.entities.insert(
                id.clone(),
                EntityState {
                    tile: spawn.tile,
                    hitpoints,
                    available_at_tick: 0,
                    flags: BTreeMap::new(),
                    runtime: EntityRuntime::default(),
                },
            );
            if let SpawnKind::Item { stack, .. } = &spawn.kind {
                world
                    .ground_items
                    .push(self.spawn_ground_item(id, spawn.tile, stack, 0));
            }
        }
        for (id, shop) in &self.content.shops {
            world.shops.insert(
                id.clone(),
                ShopState {
                    stock: shop
                        .stock
                        .iter()
                        .map(|row| (row.item.clone(), row.base_stock))
                        .collect(),
                    next_restock_tick: self.next_restock(shop, 0)?,
                },
            );
        }
        Ok(world)
    }

    /// Name/appearance authorization belongs to character creation. Appearance never grants progress.
    pub fn character_from_initial(
        &self,
        actor_id: ActorId,
        display_name: impl Into<String>,
        appearance: BTreeMap<String, u32>,
    ) -> GameResult<CharacterState> {
        let initial = &self.content.initial_state;
        let character = CharacterState {
            schema_version: GAME_SCHEMA_VERSION,
            actor_id,
            display_name: display_name.into(),
            appearance,
            region: initial.region.clone(),
            tile: initial.tile,
            inventory: initial.inventory.clone(),
            equipment: initial.equipment.clone(),
            bank: initial.bank.clone(),
            skills: initial.skills.clone(),
            hitpoints: initial.hitpoints,
            prayer_points: initial.prayer_points,
            run_energy: initial.run_energy,
            quest_points: initial.quest_points,
            tutorial_stage: initial.tutorial_stage.clone(),
            quests: initial.quests.clone(),
            flags: initial.flags.clone(),
            interfaces: initial.interfaces.clone(),
            activity: Activity::Idle,
            dialogue: None,
            last_action_tick: 0,
            last_command_sequence: 0,
            runtime: CharacterRuntime::from_initial(&self.content),
        };
        validation::character(&character, &self.content)?;
        Ok(character)
    }

    /// Authentication, dedupe and durable commit are the caller's responsibility.
    /// Call at the authoritative tick boundary, not directly on every network arrival.
    pub fn apply_intent(
        &self,
        world: &mut WorldState,
        actor: &ActorId,
        intent: &GameIntent,
        random: &mut impl RandomSource,
    ) -> GameResult<Vec<ActorEvent>> {
        self.check_world(world)?;
        let mut draft = world.clone();
        let mut character = draft.characters.remove(actor).ok_or_else(|| {
            GameError::new(
                GameErrorCode::NotOwned,
                "Authenticated actor is not in this world.",
            )
        })?;
        validation::character(&character, &self.content)?;
        self.validate_destination(&character.region, character.tile)?;
        if &character.actor_id != actor {
            return Err(invalid_state(
                "Character map key does not match authenticated actor.",
            ));
        }
        if character.flags.contains_key(runtime::COMMAND_SEEN)
            && character.last_action_tick >= draft.tick
        {
            return Err(GameError::new(
                GameErrorCode::Busy,
                "An action was already accepted this tick.",
            ));
        }
        if character.hitpoints == 0 {
            return Err(unavailable(
                "Death/recovery state and source policy are not yet bound.",
            ));
        }
        if matches!(
            character.activity,
            Activity::Fighting { .. } | Activity::Casting { .. }
        ) {
            return Err(unavailable(
                "Persisted combat/spell state needs bound interruption and logout rules.",
            ));
        }
        self.authorize_intent(&character, intent)?;
        let before = character.clone();
        let mut events = self.intent(&mut draft, &mut character, intent, random)?;
        self.progress(&mut character, &before, &mut events)?;
        self.validate_open_dialogue(&draft, &character)?;
        self.check_reward_atomicity(&before, &character)?;
        validation::character(&character, &self.content)?;
        character.last_action_tick = draft.tick;
        runtime::set_counter(&mut character, runtime::COMMAND_SEEN, 1)?;
        draft.characters.insert(actor.clone(), character);
        *world = draft;
        Ok(tag(actor, events))
    }

    /// Standalone scheduler: advance exactly one source tick, then process its due work.
    /// Use `process_advanced_tick` when the storage transaction already advanced the clock.
    /// Neither entry point changes revisions or command sequences.
    /// Expected activity interruption returns a Message, not a forged success event.
    pub fn tick(
        &self,
        world: &mut WorldState,
        random: &mut impl RandomSource,
    ) -> GameResult<Vec<ActorEvent>> {
        self.check_world(world)?;
        let mut draft = world.clone();
        draft.tick = runtime::deadline(draft.tick, 1)?;
        let events = self.process_tick_draft(&mut draft, random)?;
        *world = draft;
        Ok(events)
    }

    /// Process due work at the positive tick already supplied by authoritative storage.
    /// Leaves `WorldState::tick` unchanged, including on error; the caller owns advancement,
    /// exactly-once admission and durable rollback/acknowledgement.
    pub fn process_advanced_tick(
        &self,
        world: &mut WorldState,
        random: &mut impl RandomSource,
    ) -> GameResult<Vec<ActorEvent>> {
        self.check_world(world)?;
        if world.tick == 0 {
            return Err(invalid_state(
                "Tick processing requires an authoritatively advanced positive tick.",
            ));
        }
        let mut draft = world.clone();
        let events = self.process_tick_draft(&mut draft, random)?;
        *world = draft;
        Ok(events)
    }

    fn process_tick_draft(
        &self,
        draft: &mut WorldState,
        random: &mut impl RandomSource,
    ) -> GameResult<Vec<ActorEvent>> {
        self.advance_entities(draft)?;
        self.restock(draft)?;
        draft
            .ground_items
            .retain(|item| item.expires_at_tick > draft.tick);
        let actors: Vec<_> = draft.characters.keys().cloned().collect();
        let mut result = Vec::new();
        for actor in actors {
            let stored = draft
                .characters
                .get(&actor)
                .ok_or_else(|| invalid_state("Actor disappeared during a pure tick."))?;
            validation::character(stored, &self.content)?;
            self.validate_destination(&stored.region, stored.tile)?;
            if stored.actor_id != actor {
                return Err(invalid_state(
                    "Tick character map key does not match its actor.",
                ));
            }
            if stored.hitpoints == 0 {
                return Err(unavailable(
                    "A dead actor requires source-defined death/recovery state.",
                ));
            }
            if matches!(&stored.activity, Activity::Idle)
                || matches!(&stored.activity,
                    Activity::Gathering { next_tick, .. } | Activity::Producing { next_tick, .. }
                    if *next_tick > draft.tick)
            {
                continue;
            }
            // Each actor's effects may touch entities/ground stock as well as the character.
            // A failed actor operation must not leak those changes into an interruption.
            let mut attempt = draft.clone();
            let mut character = attempt
                .characters
                .remove(&actor)
                .ok_or_else(|| invalid_state("Actor disappeared during a pure tick."))?;
            let before = character.clone();
            let operation = self
                .advance_activity(&mut attempt, &mut character, random)
                .and_then(|mut events| {
                    self.progress(&mut character, &before, &mut events)?;
                    self.check_reward_atomicity(&before, &character)?;
                    validation::character(&character, &self.content)?;
                    Ok(events)
                });
            match operation {
                Ok(events) => {
                    attempt.characters.insert(actor.clone(), character);
                    *draft = attempt;
                    result.extend(tag(&actor, events));
                }
                Err(error) if is_interruption(&error.code) => {
                    let character = draft
                        .characters
                        .get_mut(&actor)
                        .ok_or_else(|| invalid_state("Actor disappeared during interruption."))?;
                    runtime::interrupt(character);
                    result.extend(tag(
                        &actor,
                        vec![GameEvent::Message {
                            text: format!("Activity stopped: {}", error.message),
                        }],
                    ));
                }
                Err(error) => return Err(error),
            }
        }
        Ok(result)
    }

    fn check_world(&self, world: &WorldState) -> GameResult<()> {
        world.runtime.validate_shape()?;
        for entity in world.entities.values() {
            entity.runtime.validate_shape()?;
            if entity.runtime.attack_ready != 0
                || entity.runtime.retaliation_target.is_some()
                || !entity.runtime.contributions.is_empty()
                || entity.runtime.loot_resolved
                || entity.runtime.next_movement_tick.is_some()
            {
                return Err(unavailable(
                    "Persisted NPC scheduling requires mechanics-v2 execution; pending state was preserved.",
                ));
            }
        }
        if !world.runtime.temporary_objects.is_empty()
            || !world.runtime.projectiles.is_empty()
            || !world.runtime.instances.is_empty()
            || !world.runtime.deaths.is_empty()
            || !world.runtime.stock_deadlines.is_empty()
            || world.runtime.object_states.iter().any(|(id, state)| {
                self.content
                    .mechanics
                    .object_transforms
                    .get(id)
                    .is_none_or(|definition| &definition.initial != state)
            })
        {
            return Err(unavailable(
                "Persisted dynamic mechanics require mechanics-v2 execution; pending state was preserved.",
            ));
        }
        if world.schema_version != GAME_SCHEMA_VERSION
            || world.content_revision != self.content.revision
        {
            return Err(invalid_state(
                "World schema/content revision does not match this engine.",
            ));
        }
        if world.tick > i64::MAX as u64 {
            return Err(invalid_state(
                "World tick exceeds persisted deadline range.",
            ));
        }
        Ok(())
    }

    fn validate_destination(&self, region: &RegionId, tile: Tile) -> GameResult<()> {
        if self.regions_by_tile.get(&tile) != Some(region) {
            return Err(GameError::new(
                GameErrorCode::OutOfReach,
                "Destination has no matching region cell.",
            ));
        }
        if !self.collision.cell(tile).is_some_and(|cell| cell.walkable) {
            return Err(GameError::new(
                GameErrorCode::Blocked,
                "Destination is not walkable.",
            ));
        }
        Ok(())
    }

    fn spawn_ground_item(
        &self,
        spawn: &SpawnId,
        tile: Tile,
        stack: &ItemStack,
        tick: u64,
    ) -> GroundItem {
        GroundItem {
            id: format!("source:{spawn}:{tick}"),
            tile,
            stack: stack.clone(),
            owner: None,
            public_at_tick: tick,
            expires_at_tick: u64::MAX,
            instance: None,
        }
    }
}

fn tag(actor: &ActorId, events: Vec<GameEvent>) -> Vec<ActorEvent> {
    events
        .into_iter()
        .map(|event| ActorEvent {
            actor_id: actor.clone(),
            event,
        })
        .collect()
}

fn is_interruption(code: &GameErrorCode) -> bool {
    matches!(
        code,
        GameErrorCode::NotOwned
            | GameErrorCode::InsufficientItems
            | GameErrorCode::InventoryFull
            | GameErrorCode::StackOverflow
            | GameErrorCode::RequirementNotMet
            | GameErrorCode::OutOfReach
            | GameErrorCode::Blocked
            | GameErrorCode::Busy
    )
}

pub(crate) fn unknown(message: impl Into<String>) -> GameError {
    GameError::new(GameErrorCode::UnknownContent, message)
}
pub(crate) fn invalid_content(message: impl Into<String>) -> GameError {
    GameError::new(GameErrorCode::InvalidContent, message)
}
pub(crate) fn invalid_state(message: impl Into<String>) -> GameError {
    GameError::new(GameErrorCode::InvalidInput, message)
}
pub(crate) fn unavailable(message: impl Into<String>) -> GameError {
    GameError::new(GameErrorCode::Unavailable, message)
}
