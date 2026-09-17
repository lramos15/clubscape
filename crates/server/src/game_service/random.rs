use clubscape_game_types::{GameError, GameErrorCode, GameResult};
use clubscape_world_engine::RandomSource;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use uuid::Uuid;

pub(super) struct TrustedRandom {
    key: [u8; 32],
    domain: Vec<u8>,
    counter: u64,
}

impl TrustedRandom {
    pub fn tick(key: [u8; 32], world: Uuid, tick: u64) -> Self {
        Self::new(key, world, b"tick", &tick.to_be_bytes())
    }

    pub fn command(key: [u8; 32], world: Uuid, operation: Uuid) -> Self {
        Self::new(key, world, b"command", operation.as_bytes())
    }

    fn new(key: [u8; 32], world: Uuid, kind: &[u8], identity: &[u8]) -> Self {
        let mut domain = b"clubscape.world.random.v1\0".to_vec();
        domain.extend_from_slice(world.as_bytes());
        domain.extend_from_slice(kind);
        domain.push(0);
        domain.extend_from_slice(identity);
        Self {
            key,
            domain,
            counter: 0,
        }
    }
}

impl RandomSource for TrustedRandom {
    fn draw_below(&mut self, upper: u32) -> GameResult<u32> {
        if upper == 0 {
            return Err(GameError::new(
                GameErrorCode::InvalidInput,
                "Random domain cannot be empty.",
            ));
        }
        let range = u64::from(u32::MAX) + 1;
        let ceiling = range - range % u64::from(upper);
        for _ in 0..128 {
            let mut mac = Hmac::<Sha256>::new_from_slice(&self.key).map_err(|_| {
                GameError::new(
                    GameErrorCode::Unavailable,
                    "Private randomness is unavailable.",
                )
            })?;
            mac.update(&self.domain);
            mac.update(&self.counter.to_be_bytes());
            self.counter = self.counter.checked_add(1).ok_or_else(|| {
                GameError::new(
                    GameErrorCode::Unavailable,
                    "Private randomness is exhausted.",
                )
            })?;
            let bytes = mac.finalize().into_bytes();
            let value = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            if u64::from(value) < ceiling {
                return Ok(value % upper);
            }
        }
        Err(GameError::new(
            GameErrorCode::Unavailable,
            "Private randomness retry bound exceeded.",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_counter_draws_replay_and_separate_worlds_ticks_and_commands() {
        let world = Uuid::from_u128(1);
        let key = [42; 32];
        let mut first = TrustedRandom::tick(key, world, 1);
        let mut retry = TrustedRandom::tick(key, world, 1);
        let mut next = TrustedRandom::tick(key, world, 2);
        let a: Vec<_> = (0..32)
            .map(|_| first.draw_below(100_000).unwrap())
            .collect();
        let b: Vec<_> = (0..32)
            .map(|_| retry.draw_below(100_000).unwrap())
            .collect();
        let c: Vec<_> = (0..32).map(|_| next.draw_below(100_000).unwrap()).collect();
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.iter().all(|value| *value < 100_000));
        let mut command = TrustedRandom::command(key, world, world);
        assert_ne!(command.draw_below(100_000).unwrap(), a[0]);
        assert!(command.draw_below(0).is_err());
        assert_eq!(command.draw_below(1).unwrap(), 0);
    }
}
