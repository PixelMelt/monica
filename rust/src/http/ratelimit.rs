//! Fixed-window rate limits kept in memory (X005), like Laravel's
//! `RateLimiter::for(...)` with `Limit::perMinute(n)->by(key)`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::error::{ApiError, Code};

#[derive(Clone, Default)]
pub struct RateLimiter {
    hits: Arc<Mutex<HashMap<String, (Instant, u32)>>>,
}

impl RateLimiter {
    /// Counts one hit for `key`; 429 with `Retry-After` past `max` per `window`.
    pub fn hit(&self, key: &str, max: u32, window: Duration) -> Result<(), ApiError> {
        let now = Instant::now();
        let mut hits = self.hits.lock().unwrap();
        if hits.len() > 10_000 {
            hits.retain(|_, (start, _)| now.duration_since(*start) < window);
        }
        let entry = hits.entry(key.to_owned()).or_insert((now, 0));
        if now.duration_since(entry.0) >= window {
            *entry = (now, 0);
        }
        if entry.1 >= max {
            let mut e = ApiError::new(Code::RateLimited);
            e.retry_after = Some((window - now.duration_since(entry.0)).as_secs().max(1));
            return Err(e);
        }
        entry.1 += 1;
        Ok(())
    }

    /// Forgets the hits for `key` (successful login clears its counter).
    pub fn clear(&self, key: &str) {
        self.hits.lock().unwrap().remove(key);
    }

    pub fn per_minute(&self, key: &str, max: u32) -> Result<(), ApiError> {
        self.hit(key, max, Duration::from_secs(60))
    }
}
