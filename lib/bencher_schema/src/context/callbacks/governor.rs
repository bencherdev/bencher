use std::{sync::Arc, time::Duration};

use bencher_callback::{CallbackAttempt, CallbackCredential, CallbackHost, RateLimit};
use dashmap::DashMap;
use http::StatusCode;
use tokio::{
    sync::{Mutex as AsyncMutex, OwnedMutexGuard, OwnedSemaphorePermit, Semaphore},
    time::Instant,
};

const HOST_SLOTS: usize = 64;
const SPACING: Duration = Duration::from_secs(1);
const FALLBACK_PAUSE: Duration = Duration::from_mins(1);
const MAX_PAUSE: Duration = Duration::from_hours(1);

/// Paces every send: one at a time per credential, a second apart and never while its rate limit
/// pauses it, and at most 64 in flight per host. State lives only in memory.
#[derive(Default)]
pub(super) struct Governor {
    credentials: DashMap<CallbackCredential, Arc<AsyncMutex<Pace>>>,
    hosts: DashMap<CallbackHost, Arc<Semaphore>>,
}

/// A credential's turn and a host slot, held for one send.
pub(super) struct Turn {
    pace: OwnedMutexGuard<Pace>,
    slot: OwnedSemaphorePermit,
}

#[derive(Default)]
struct Pace {
    /// The earliest the credential sends again.
    next: Option<Instant>,
    /// Rate limited answers since the credential's last 2xx.
    limited: u32,
}

/// Why an answer is a rate limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, derive_more::Display)]
pub(super) enum Signal {
    #[display("retry-after")]
    RetryAfter,
    #[display("remaining-zero")]
    RemainingZero,
    #[display("none")]
    Neither,
}

impl Governor {
    /// Wait for the credential's turn and its next allowed send, and only then for a host slot, so
    /// a paused credential never holds one. `None` only if the slots close, which they never do.
    pub(super) async fn turn(&self, credential: &CallbackCredential) -> Option<Turn> {
        let now = Instant::now();
        // `retain` and `entry` both hold the shard's lock, so an entry goes only while the map holds
        // its sole `Arc`.
        self.credentials.retain(|_, pace| {
            Arc::strong_count(pace) > 1 || pace.try_lock().is_ok_and(|pace| pace.in_force(now))
        });
        let pace = Arc::clone(
            self.credentials
                .entry(credential.clone())
                .or_default()
                .value(),
        );
        let pace = pace.lock_owned().await;
        if let Some(next) = pace.next {
            tokio::time::sleep_until(next).await;
        }
        self.hosts.retain(|_, slots| Arc::strong_count(slots) > 1);
        let slots = Arc::clone(
            self.hosts
                .entry(credential.host().clone())
                .or_insert_with(|| Arc::new(Semaphore::new(HOST_SLOTS)))
                .value(),
        );
        let slot = slots.acquire_owned().await.ok()?;
        Some(Turn { pace, slot })
    }
}

impl Turn {
    /// Free the host slot, and set the credential's next send a second after this answer, or after
    /// the pause it returns when the answer is a rate limit.
    pub(super) fn settle(self, attempt: &CallbackAttempt, now: i64) -> Option<Duration> {
        let Self { mut pace, slot } = self;
        drop(slot);
        let pause = if let Some((rate_limit, _)) = rate_limit(attempt) {
            pace.limited = pace.limited.saturating_add(1);
            Some(pause(rate_limit, pace.limited, now))
        } else {
            if let CallbackAttempt::Delivered(_) = attempt {
                pace.limited = 0;
            }
            None
        };
        pace.next = Some(Instant::now() + pause.unwrap_or_default().max(SPACING));
        pause
    }
}

impl Pace {
    fn in_force(&self, now: Instant) -> bool {
        self.next.is_some_and(|next| next > now)
    }
}

/// A 429, or a 403 with `retry-after` or `x-ratelimit-remaining: 0`, and what makes it one.
pub(super) fn rate_limit(attempt: &CallbackAttempt) -> Option<(&RateLimit, Signal)> {
    let CallbackAttempt::Refused(status, rate_limit) = attempt else {
        return None;
    };
    let signal = if rate_limit.retry_after.is_some() {
        Signal::RetryAfter
    } else if rate_limit.remaining == Some(0) {
        Signal::RemainingZero
    } else {
        Signal::Neither
    };
    let limited = *status == StatusCode::TOO_MANY_REQUESTS
        || (*status == StatusCode::FORBIDDEN && signal != Signal::Neither);
    limited.then_some((rate_limit, signal))
}

/// `retry-after`, else until the reset when no requests remain, else a minute that doubles with
/// each rate limit in a row; an hour at most.
fn pause(rate_limit: &RateLimit, limited: u32, now: i64) -> Duration {
    let RateLimit {
        retry_after,
        remaining,
        reset,
    } = *rate_limit;
    let pause = match (retry_after, remaining, reset) {
        (Some(seconds), _, _) => Duration::from_secs(seconds),
        (None, Some(0), Some(reset)) => {
            Duration::from_secs(u64::try_from(reset.saturating_sub(now)).unwrap_or_default())
        },
        _ => FALLBACK_PAUSE.saturating_mul(2u32.saturating_pow(limited.saturating_sub(1))),
    };
    pause.min(MAX_PAUSE)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bencher_callback::{CallbackAttempt, CallbackCredential, CallbackRequest, RateLimit};
    use http::{HeaderMap, StatusCode};

    use super::Governor;

    fn credential(url: &str) -> CallbackCredential {
        CallbackCredential::of(&CallbackRequest {
            url: url.parse().unwrap(),
            headers: HeaderMap::new(),
            body: "{}".to_owned(),
        })
    }

    fn tracked(governor: &Governor, credential: &CallbackCredential) -> (bool, bool) {
        (
            governor.credentials.contains_key(credential),
            governor.hosts.contains_key(credential.host()),
        )
    }

    #[tokio::test(start_paused = true)]
    async fn an_entry_goes_once_nothing_holds_it_and_no_pause_is_in_force() {
        let governor = Governor::default();
        let first = credential("https://first.example/hook");
        let second = credential("https://second.example/hook");
        let delivered = CallbackAttempt::Delivered(StatusCode::OK);
        let limited = CallbackAttempt::Refused(
            StatusCode::TOO_MANY_REQUESTS,
            RateLimit {
                retry_after: Some(600),
                ..RateLimit::default()
            },
        );

        governor.turn(&first).await.unwrap().settle(&delivered, 0);
        assert_eq!(tracked(&governor, &first), (true, true));

        // Its second has passed, so the next touch takes it away.
        tokio::time::advance(Duration::from_secs(1)).await;
        let turn = governor.turn(&second).await.unwrap();
        assert_eq!(tracked(&governor, &first), (false, false));
        turn.settle(&limited, 0);

        tokio::time::advance(Duration::from_secs(1)).await;
        governor.turn(&first).await.unwrap().settle(&delivered, 0);
        assert_eq!(
            tracked(&governor, &second),
            (true, false),
            "its pause holds it"
        );

        tokio::time::advance(Duration::from_mins(10)).await;
        governor.turn(&first).await.unwrap().settle(&delivered, 0);
        assert_eq!(tracked(&governor, &second), (false, false));
    }
}
