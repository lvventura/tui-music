use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Default)]
pub struct Metrics {
    pub cache_hits: AtomicU64,
    pub cache_misses: AtomicU64,
    pub provider_requests: AtomicU64,
    pub provider_failures: AtomicU64,
    pub retries: AtomicU64,
}

impl Metrics {
    pub fn snapshot(&self) -> [u64; 5] {
        [
            self.cache_hits.load(Ordering::Relaxed),
            self.cache_misses.load(Ordering::Relaxed),
            self.provider_requests.load(Ordering::Relaxed),
            self.provider_failures.load(Ordering::Relaxed),
            self.retries.load(Ordering::Relaxed),
        ]
    }
}
