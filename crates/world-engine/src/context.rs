use std::collections::{BTreeMap, BTreeSet};

use clubscape_game_types::*;

use crate::{invalid_state, unavailable};

/// Trusted server/session facts for this tick, never accepted from a game intent.
#[derive(Clone, Debug, Default)]
pub struct TickContext {
    pub actors: BTreeMap<ActorId, ActorPresence>,
}

#[derive(Clone, Debug)]
pub struct ActorPresence {
    pub online: bool,
    pub idle_milliseconds: u64,
    pub grave_interface: Option<DeathId>,
}

impl TickContext {
    /// Explicit opt-in for a standalone simulation in which every loaded actor is active.
    pub fn all_active(world: &WorldState) -> Self {
        Self {
            actors: world
                .characters
                .keys()
                .map(|actor| {
                    (
                        actor.clone(),
                        ActorPresence {
                            online: true,
                            idle_milliseconds: 0,
                            grave_interface: None,
                        },
                    )
                })
                .collect(),
        }
    }

    pub(crate) fn paused(
        &self,
        character: &CharacterState,
        pauses: &BTreeSet<ClockPause>,
        idle_after: Option<u32>,
        running: bool,
        combat: bool,
    ) -> GameResult<bool> {
        let mut paused = false;
        for pause in pauses {
            paused |= match pause {
                ClockPause::FirstDeathOffice => {
                    matches!(character.runtime.life, LifeState::FirstDeathOffice { .. })
                }
                ClockPause::Running => running,
                ClockPause::Combat => combat,
                ClockPause::Offline | ClockPause::Idle | ClockPause::GraveInterface => {
                    let facts = self.actors.get(&character.actor_id).ok_or_else(|| unavailable(
                        "Clock requires authority-owned online/idle/interface facts; use the tick API with TickContext."))?;
                    match pause {
                        ClockPause::Offline => !facts.online,
                        ClockPause::Idle => {
                            facts.idle_milliseconds
                                > u64::from(idle_after.ok_or_else(|| {
                                    invalid_state("An idle-paused clock lacks an idle threshold.")
                                })?)
                        }
                        ClockPause::GraveInterface => {
                            facts.grave_interface.is_some()
                                && facts.grave_interface == character.runtime.active_death
                        }
                        _ => unreachable!(),
                    }
                }
            };
        }
        Ok(paused)
    }
}
