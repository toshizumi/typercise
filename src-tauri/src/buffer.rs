use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;

use crate::store::Store;

pub struct Buffer {
    keys: AtomicU64,
    corrections: AtomicU64,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            keys: AtomicU64::new(0),
            corrections: AtomicU64::new(0),
        }
    }

    pub fn inc_key(&self) {
        self.keys.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_correction(&self) {
        self.corrections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn take(&self) -> (u64, u64) {
        (
            self.keys.swap(0, Ordering::Relaxed),
            self.corrections.swap(0, Ordering::Relaxed),
        )
    }
}

pub fn spawn_flush_task(buf: Arc<Buffer>, store: Arc<Store>) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(10));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            let (k, c) = buf.take();
            if k == 0 && c == 0 {
                continue;
            }
            let minute_ts = Utc::now().timestamp() / 60;
            if let Err(e) = store.add_minute(minute_ts, k as i64, c as i64) {
                tracing::warn!(error = ?e, "flush to sqlite failed; will retry next tick");
                buf.keys.fetch_add(k, Ordering::Relaxed);
                buf.corrections.fetch_add(c, Ordering::Relaxed);
            }
        }
    });
}
