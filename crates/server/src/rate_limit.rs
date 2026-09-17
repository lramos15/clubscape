use std::{
    collections::{HashMap, VecDeque},
    net::IpAddr,
    sync::Mutex,
    time::{Duration, Instant},
};

pub(crate) const ATTEMPTS_PER_MINUTE: usize = 20;
pub(crate) const MAX_PEERS: usize = 1_024;
const WINDOW: Duration = Duration::from_secs(60);

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LimitError {
    Exhausted { retry_after_seconds: u32 },
    Poisoned,
}

#[derive(Default)]
pub(crate) struct RateLimiter {
    peers: Mutex<HashMap<IpAddr, VecDeque<Instant>>>,
}

impl RateLimiter {
    pub(crate) fn admit(&self, peer: IpAddr) -> Result<(), LimitError> {
        self.admit_at(peer, Instant::now())
    }

    fn admit_at(&self, peer: IpAddr, now: Instant) -> Result<(), LimitError> {
        let mut peers = self.peers.lock().map_err(|_| LimitError::Poisoned)?;
        peers.retain(|_, attempts| {
            while attempts
                .front()
                .is_some_and(|time| now.saturating_duration_since(*time) >= WINDOW)
            {
                attempts.pop_front();
            }
            !attempts.is_empty()
        });
        if !peers.contains_key(&peer) && peers.len() >= MAX_PEERS {
            let remaining = peers
                .values()
                .filter_map(|attempts| attempts.back())
                .map(|last| WINDOW.saturating_sub(now.saturating_duration_since(*last)))
                .min()
                .unwrap_or(WINDOW);
            return Err(LimitError::Exhausted {
                retry_after_seconds: retry_seconds(remaining),
            });
        }
        let attempts = peers.entry(peer).or_default();
        if attempts.len() >= ATTEMPTS_PER_MINUTE {
            let remaining = attempts
                .front()
                .map(|first| WINDOW.saturating_sub(now.saturating_duration_since(*first)))
                .unwrap_or(WINDOW);
            return Err(LimitError::Exhausted {
                retry_after_seconds: retry_seconds(remaining),
            });
        }
        attempts.push_back(now);
        Ok(())
    }
}

fn retry_seconds(remaining: Duration) -> u32 {
    remaining
        .as_secs()
        .saturating_add(u64::from(remaining.subsec_nanos() != 0))
        .clamp(1, WINDOW.as_secs()) as u32
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use super::*;

    #[test]
    fn enforces_a_sliding_window_with_an_accurate_rounded_retry_delay() {
        let limiter = RateLimiter::default();
        let peer = Ipv4Addr::LOCALHOST.into();
        let start = Instant::now();
        for index in 0..ATTEMPTS_PER_MINUTE {
            assert!(
                limiter
                    .admit_at(peer, start + Duration::from_secs(index as u64))
                    .is_ok()
            );
        }
        assert_eq!(
            limiter.admit_at(peer, start + Duration::from_millis(20_500)),
            Err(LimitError::Exhausted {
                retry_after_seconds: 40
            })
        );
        assert!(limiter.admit_at(peer, start + WINDOW).is_ok());
        assert_eq!(
            limiter.admit_at(peer, start + WINDOW),
            Err(LimitError::Exhausted {
                retry_after_seconds: 1
            })
        );
        assert!(
            limiter
                .admit_at(peer, start + WINDOW + Duration::from_secs(1))
                .is_ok()
        );
    }

    #[test]
    fn bounds_cardinality_without_evicting_live_limits_and_reclaims_expired_peers() {
        let limiter = RateLimiter::default();
        let start = Instant::now();
        for index in 0..MAX_PEERS {
            let peer = IpAddr::V6(Ipv6Addr::from(index as u128 + 1));
            assert!(limiter.admit_at(peer, start).is_ok());
        }
        let next = IpAddr::V4(Ipv4Addr::LOCALHOST);
        assert_eq!(
            limiter.admit_at(next, start + Duration::from_millis(1)),
            Err(LimitError::Exhausted {
                retry_after_seconds: 60
            })
        );
        assert_eq!(limiter.peers.lock().unwrap().len(), MAX_PEERS);
        assert!(limiter.admit_at(next, start + WINDOW).is_ok());
        assert_eq!(limiter.peers.lock().unwrap().len(), 1);
    }

    #[test]
    fn peers_are_independent_and_rejections_do_not_extend_the_window() {
        let limiter = RateLimiter::default();
        let start = Instant::now();
        let first = Ipv4Addr::LOCALHOST.into();
        let second = Ipv4Addr::new(127, 0, 0, 2).into();
        for _ in 0..ATTEMPTS_PER_MINUTE {
            limiter.admit_at(first, start).unwrap();
        }
        assert!(limiter.admit_at(second, start).is_ok());
        assert!(
            limiter
                .admit_at(first, start + Duration::from_secs(59))
                .is_err()
        );
        assert!(limiter.admit_at(first, start + WINDOW).is_ok());
    }
}
