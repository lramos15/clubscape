use crate::{ActorEvent, WorldEngine, invalid_state, runtime, tag, unavailable, unknown};
use clubscape_game_types::*;

impl WorldEngine {
    pub(crate) fn grave_access(
        &self,
        world: &WorldState,
        character: &CharacterState,
        id: &DeathId,
    ) -> GameResult<()> {
        let policy = self
            .content
            .mechanics
            .death
            .as_ref()
            .ok_or_else(|| unavailable("Death policy is not configured."))?;
        let record = world
            .runtime
            .deaths
            .get(id)
            .ok_or_else(|| GameError::new(GameErrorCode::NotOwned, "Unknown death record."))?;
        if record.owner != character.actor_id {
            return Err(GameError::new(
                GameErrorCode::NotOwned,
                "Grave belongs to another actor.",
            ));
        }
        let grave = record
            .grave
            .as_ref()
            .ok_or_else(|| GameError::new(GameErrorCode::NotOwned, "Grave is no longer active."))?;
        if grave.location.instance != character.runtime.instance
            || character
                .tile
                .distance(grave.location.tile)
                .is_none_or(|distance| distance > policy.reclaim_range)
            || policy.require_line_of_sight
                && !self
                    .collision_for(world, character.runtime.instance.as_ref())?
                    .line_of_sight(character.tile, grave.location.tile)
        {
            return Err(GameError::new(
                GameErrorCode::OutOfReach,
                "Grave is outside source range, sight or instance.",
            ));
        }
        Ok(())
    }

    pub(crate) fn office_access(
        &self,
        world: &WorldState,
        character: &CharacterState,
    ) -> GameResult<()> {
        let policy = self
            .content
            .mechanics
            .death
            .as_ref()
            .ok_or_else(|| unavailable("Death policy is not configured."))?;
        let office = policy.first_office.require()?;
        let allowed = match (&office.instance, &character.runtime.instance) {
            (Some(template), Some(id)) => world.runtime.instances.get(id).is_some_and(|state| {
                &state.template == template && state.owner.as_ref() == Some(&character.actor_id)
            }),
            (None, None) => character.region == office.region,
            _ => false,
        };
        if !allowed {
            return Err(GameError::new(
                GameErrorCode::OutOfReach,
                "Source Office presence is required.",
            ));
        }
        Ok(())
    }

    pub(crate) fn open_grave(
        &self,
        world: &WorldState,
        character: &mut CharacterState,
        id: &DeathId,
    ) -> GameResult<Vec<GameEvent>> {
        self.grave_access(world, character, id)?;
        let interface = self
            .content
            .mechanics
            .death
            .as_ref()
            .and_then(|policy| policy.interfaces.as_ref())
            .ok_or_else(|| unavailable("Recovery interfaces are not configured."))?
            .grave
            .clone();
        self.recovery_interface_unlocked(character, &interface)?;
        let mut events = Vec::new();
        self.close_interfaces(character, &mut events)?;
        runtime::interrupt(character)?;
        runtime::schedule_mut(character)?.access = Some(ContainerSession::Grave {
            death: id.clone(),
            interface: interface.clone(),
        });
        events.push(GameEvent::InterfacePresented {
            interface,
            context: InterfaceContext::Grave { death: id.clone() },
        });
        Ok(events)
    }

    pub(crate) fn open_death_office(
        &self,
        world: &WorldState,
        character: &mut CharacterState,
    ) -> GameResult<Vec<GameEvent>> {
        self.office_access(world, character)?;
        let interface = self
            .content
            .mechanics
            .death
            .as_ref()
            .and_then(|policy| policy.interfaces.as_ref())
            .ok_or_else(|| unavailable("Recovery interfaces are not configured."))?
            .office
            .clone();
        self.recovery_interface_unlocked(character, &interface)?;
        let mut events = Vec::new();
        self.close_interfaces(character, &mut events)?;
        runtime::interrupt(character)?;
        runtime::schedule_mut(character)?.access = Some(ContainerSession::DeathOffice {
            interface: interface.clone(),
        });
        events.push(GameEvent::InterfacePresented {
            interface,
            context: InterfaceContext::DeathOffice,
        });
        Ok(events)
    }

    fn recovery_interface_unlocked(
        &self,
        character: &CharacterState,
        id: &InterfaceId,
    ) -> GameResult<()> {
        let interface = self
            .content
            .interfaces
            .get(id)
            .ok_or_else(|| unknown("Unknown recovery interface."))?;
        if interface.access != InterfaceAccess::Contextual {
            return Err(invalid_state("Recovery interface is not contextual."));
        }
        if !character.interfaces.contains(id) {
            return Err(GameError::new(
                GameErrorCode::RequirementNotMet,
                "Recovery interface is locked.",
            ));
        }
        Ok(())
    }

    pub(crate) fn refresh_recovery_sessions(
        &self,
        world: &mut WorldState,
    ) -> GameResult<Vec<ActorEvent>> {
        let mut close = Vec::new();
        for (actor, character) in &world.characters {
            let access = match &runtime::schedule(character)?.access {
                Some(ContainerSession::Grave { death, .. }) => {
                    self.grave_access(world, character, death)
                }
                Some(ContainerSession::DeathOffice { .. }) => self.office_access(world, character),
                _ => continue,
            };
            match access {
                Ok(()) => {}
                Err(error)
                    if matches!(
                        error.code,
                        GameErrorCode::NotOwned
                            | GameErrorCode::OutOfReach
                            | GameErrorCode::RequirementNotMet
                    ) =>
                {
                    close.push(actor.clone())
                }
                Err(error) => return Err(error),
            }
        }
        let mut events = Vec::new();
        for actor in close {
            let character = world
                .characters
                .get_mut(&actor)
                .ok_or_else(|| invalid_state("Recovery actor disappeared."))?;
            let mut own = Vec::new();
            self.close_interfaces(character, &mut own)?;
            events.extend(tag(&actor, own));
        }
        Ok(events)
    }
}
