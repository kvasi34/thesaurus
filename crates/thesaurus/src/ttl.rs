use std::time::{Duration, Instant};

use log::{info, trace};

use crate::store::Store;

const N: usize = 20;

/// Background daemon that periodically samples the expiry index and evicts expired keys.
pub struct TtlEvictionDaemon {
    store: Store,
}

impl TtlEvictionDaemon {
    /// Spawns the TTL daemon and starts the loop sampling key-value pairs for eviction.
    /// The `hz` parameter controls the loop interval in milliseconds.
    pub async fn spawn(hz: u64, store: Store) {
        info!(
            "Starting TTL eviction daemon task with an eviction interval of {} ms",
            hz
        );
        let daemon = Self { store };

        let mut interval = tokio::time::interval(Duration::from_millis(hz));
        loop {
            interval.tick().await;
            daemon.evict_expired();
        }
    }

    /// Samples `N` keys and evicts the expired ones from the store.
    pub fn evict_expired(&self) {
        let sample_pairs = self.store.sample_eviction_index(N);
        let now = Instant::now();
        let mut keys_to_delete: Vec<String> = Vec::new();
        for (key, expiration_instant) in sample_pairs {
            if now >= expiration_instant {
                keys_to_delete.push(key);
            }
        }

        trace!(
            "Evicting {} keys: {:?}",
            keys_to_delete.len(),
            keys_to_delete
        );
        self.store.delete_bulk(&keys_to_delete);
    }
}
