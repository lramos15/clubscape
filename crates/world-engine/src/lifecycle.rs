use std::collections::{BTreeMap, BTreeSet};

use clubscape_game_types::*;
use serde::{Deserialize, Serialize};

use crate::{
    ActorEvent, ActorPresence, TickContext, WorldEngine, combat::CombatLock, invalid_state,
    runtime, tag, unavailable,
};

/// Trusted control-plane transitions after the server validates auth and leases.
/// These are deliberately not player GameIntent state setters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleTransition {
    Join,
    Rejoin,
    Activity,
    RequestedLogout,
    TransportLost,
    AuthenticationRevoked,
    CoordinatorRestart,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresenceView {
    pub state: PresenceState,
    pub connected: bool,
    pub accepts_input: bool,
    pub present_in_world: bool,
}

impl WorldEngine {
    pub(crate) fn input_permission(&self, character: &CharacterState) -> GameResult<()> {
        if matches!(
            character.runtime.presence,
            PresenceState::Disconnecting { .. } | PresenceState::Offline { .. }
        ) {
            return Err(GameError::new(
                GameErrorCode::SessionConflict,
                "Actor must join before new gameplay input.",
            ));
        }
        Ok(())
    }

    pub fn apply_lifecycle(
        &self,
        world: &mut WorldState,
        actor: &ActorId,
        transition: LifecycleTransition,
    ) -> GameResult<Vec<ActorEvent>> {
        self.check_world(world)?;
        let mut draft = world.clone();
        let mut character = draft.characters.remove(actor).ok_or_else(|| {
            GameError::new(
                GameErrorCode::NotOwned,
                "Lifecycle actor is not in this world.",
            )
        })?;
        character.migrate_engine_metadata(&self.content)?;
        let before = character.clone();
        let events = self.lifecycle_draft(&draft, &mut character, transition)?;
        self.check_reward_atomicity(&before, &character)?;
        draft.characters.insert(actor.clone(), character);
        draft.validate_runtime(&self.content)?;
        *world = draft;
        Ok(tag(actor, events))
    }

    /// Reconciles an authority-verified connection set, including after restart.
    /// Missing connections become source-guarded pending disconnects, not erased actors.
    pub fn reconcile_presence(
        &self,
        world: &mut WorldState,
        connected: &BTreeSet<ActorId>,
    ) -> GameResult<Vec<ActorEvent>> {
        self.check_world(world)?;
        if connected
            .iter()
            .any(|actor| !world.characters.contains_key(actor))
        {
            return Err(GameError::new(
                GameErrorCode::NotOwned,
                "Connected set contains an unknown actor.",
            ));
        }
        let mut draft = world.clone();
        let actors: Vec<_> = draft.characters.keys().cloned().collect();
        let mut events = Vec::new();
        for actor in actors {
            let mut character = draft
                .characters
                .remove(&actor)
                .ok_or_else(|| invalid_state("Presence actor disappeared."))?;
            character.migrate_engine_metadata(&self.content)?;
            let transition = if connected.contains(&actor) {
                Some(LifecycleTransition::Rejoin)
            } else if matches!(
                character.runtime.presence,
                PresenceState::Untracked | PresenceState::Connected { .. }
            ) {
                Some(LifecycleTransition::CoordinatorRestart)
            } else {
                None
            };
            if let Some(transition) = transition {
                events.extend(tag(
                    &actor,
                    self.lifecycle_draft(&draft, &mut character, transition)?,
                ));
            }
            draft.characters.insert(actor, character);
        }
        draft.validate_runtime(&self.content)?;
        *world = draft;
        Ok(events)
    }

    pub fn presence_view(&self, world: &WorldState, actor: &ActorId) -> GameResult<PresenceView> {
        self.check_world(world)?;
        let character = world
            .characters
            .get(actor)
            .ok_or_else(|| GameError::new(GameErrorCode::NotOwned, "Unknown presence actor."))?;
        let connected = matches!(character.runtime.presence, PresenceState::Connected { .. });
        Ok(PresenceView {
            state: character.runtime.presence.clone(),
            connected,
            accepts_input: connected,
            present_in_world: connected
                || matches!(
                    character.runtime.presence,
                    PresenceState::Disconnecting { .. }
                ),
        })
    }

    /// Derives mechanical presence and idle time only from acknowledged lifecycle state.
    pub fn tick_context(&self, world: &WorldState) -> GameResult<TickContext> {
        self.check_world(world)?;
        let mut context = TickContext {
            actors: BTreeMap::new(),
        };
        for (actor, character) in &world.characters {
            let presence = self.tracked_presence(world.tick, character)?
                .ok_or_else(|| unavailable("Actor presence is untracked; reconcile verified connections or provide an explicit legacy TickContext."))?;
            context.actors.insert(actor.clone(), presence);
        }
        Ok(context)
    }

    pub(crate) fn effective_context(
        &self,
        world: &WorldState,
        supplied: &TickContext,
    ) -> GameResult<TickContext> {
        let mut context = supplied.clone();
        for (actor, character) in &world.characters {
            if let Some(presence) = self.tracked_presence(world.tick, character)? {
                context.actors.insert(actor.clone(), presence);
            }
        }
        Ok(context)
    }

    fn tracked_presence(
        &self,
        tick: u64,
        character: &CharacterState,
    ) -> GameResult<Option<ActorPresence>> {
        let online = match character.runtime.presence {
            PresenceState::Untracked => return Ok(None),
            PresenceState::Connected { .. } | PresenceState::Disconnecting { .. } => true,
            PresenceState::Offline { .. } => false,
        };
        let idle_milliseconds = if let Some(last) = character.runtime.last_active_tick {
            tick.checked_sub(last)
                .and_then(|elapsed| elapsed.checked_mul(TICK_MILLISECONDS))
                .ok_or_else(|| {
                    invalid_state("Presence input clock is future-dated or overflows.")
                })?
        } else if online {
            return Err(unavailable(
                "Mechanically present legacy actor needs its authoritative input clock migrated.",
            ));
        } else {
            0
        };
        Ok(Some(ActorPresence {
            online,
            idle_milliseconds,
            grave_interface: None,
        }))
    }

    fn lifecycle_draft(
        &self,
        world: &WorldState,
        character: &mut CharacterState,
        transition: LifecycleTransition,
    ) -> GameResult<Vec<GameEvent>> {
        match transition {
            LifecycleTransition::Join | LifecycleTransition::Rejoin => {
                if !matches!(character.runtime.presence, PresenceState::Connected { .. }) {
                    character.runtime.presence = PresenceState::Connected {
                        joined_at_tick: world.tick,
                    };
                    character.runtime.last_active_tick = Some(world.tick);
                }
                Ok(vec![])
            }
            LifecycleTransition::Activity => {
                if !matches!(character.runtime.presence, PresenceState::Connected { .. }) {
                    return Err(GameError::new(
                        GameErrorCode::SessionConflict,
                        "Only a joined connection can report user activity.",
                    ));
                }
                character.runtime.last_active_tick = Some(world.tick);
                Ok(vec![])
            }
            LifecycleTransition::RequestedLogout => self.source_logout(world, character),
            loss => {
                if matches!(character.runtime.presence, PresenceState::Offline { .. }) {
                    return Ok(vec![]);
                }
                let reason = match loss {
                    LifecycleTransition::TransportLost => ConnectionLoss::TransportLost,
                    LifecycleTransition::AuthenticationRevoked => {
                        ConnectionLoss::AuthenticationRevoked
                    }
                    LifecycleTransition::CoordinatorRestart => ConnectionLoss::CoordinatorRestart,
                    _ => unreachable!(),
                };
                let since_tick = match character.runtime.presence {
                    PresenceState::Disconnecting { since_tick, .. } => since_tick,
                    _ => world.tick,
                };
                character.runtime.presence = PresenceState::Disconnecting { reason, since_tick };
                let mut events = Vec::new();
                self.close_interfaces(character, &mut events)?;
                if !matches!(
                    character.activity,
                    Activity::Fighting { .. } | Activity::Casting { .. }
                ) && character.runtime.pending_travel.is_none()
                {
                    runtime::interrupt(character)?;
                }
                match self.source_logout(world, character) {
                    Ok(logout) => events.extend(logout),
                    Err(error) if error.code == GameErrorCode::Busy => {}
                    Err(error) => return Err(error),
                }
                Ok(events)
            }
        }
    }

    pub(crate) fn source_logout(
        &self,
        world: &WorldState,
        character: &mut CharacterState,
    ) -> GameResult<Vec<GameEvent>> {
        if matches!(character.runtime.life, LifeState::Dying { .. } | LifeState::Respawning { .. })
            || world.runtime.projectiles.iter().any(|projectile| matches!(&projectile.source, Combatant::Player { actor } if actor == &character.actor_id))
            || self.combat_locked(world, character, CombatLock::Logout)? {
            return Err(GameError::new(GameErrorCode::Busy, "Source combat/projectile/life transition prevents logout."));
        }
        let mut events = Vec::new();
        if character.runtime.pending_travel.is_some() {
            events.extend(self.interrupt_travel(character, InterruptionCause::Logout)?);
        }
        self.close_interfaces(character, &mut events)?;
        runtime::interrupt(character)?;
        character.runtime.presence = PresenceState::Offline {
            since_tick: world.tick,
        };
        Ok(events)
    }

    pub(crate) fn advance_presence(&self, world: &mut WorldState) -> GameResult<Vec<ActorEvent>> {
        let actors: Vec<_> = world
            .characters
            .iter()
            .filter(|(_, character)| {
                matches!(
                    character.runtime.presence,
                    PresenceState::Disconnecting { .. }
                )
            })
            .map(|(actor, _)| actor.clone())
            .collect();
        let mut events = Vec::new();
        for actor in actors {
            let mut character = world
                .characters
                .remove(&actor)
                .ok_or_else(|| invalid_state("Disconnecting actor disappeared."))?;
            match self.source_logout(world, &mut character) {
                Ok(own) => events.extend(tag(&actor, own)),
                Err(error) if error.code == GameErrorCode::Busy => {}
                Err(error) => return Err(error),
            }
            world.characters.insert(actor, character);
        }
        Ok(events)
    }
}
