use moka::future::Cache as MokaCache;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::protocol::CacheStats;

pub struct Cache {
    inner: MokaCache<String, String>,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl Cache {
    pub fn new(max_entries: u64, ttl_seconds: u64) -> Self {
        let inner = MokaCache::builder()
            .max_capacity(max_entries)
            .time_to_live(Duration::from_secs(ttl_seconds))
            .build();

        Self {
            inner,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    pub async fn get(&self, key: &str) -> Option<String> {
        let result = self.inner.get(key).await;

        if result.is_some() {
            self.hits.fetch_add(1, Ordering::Relaxed);
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
        }

        result
    }

    pub async fn insert(&self, key: &str, value: String) {
        self.inner.insert(key.to_string(), value).await;
    }

    pub fn clear(&self) {
        self.inner.invalidate_all();
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
    }

    pub fn stats(&self) -> CacheStats {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;

        CacheStats {
            entries: self.inner.entry_count(),
            hits,
            misses,
            hit_rate: if total > 0 {
                hits as f64 / total as f64
            } else {
                0.0
            },
        }
    }
}
