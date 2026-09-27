//! Rate-limited progress reporting to avoid overwhelming the UI with updates.

use crate::protocol::{Emitter, Stage};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MIN_UPDATE_INTERVAL: Duration = Duration::from_millis(400);

#[derive(Clone)]
pub struct ProgressThrottle {
    inner: Arc<Mutex<Inner>>,
    emitter: Arc<Emitter>,
}

struct Inner {
    last_update: Instant,
    last_completed: u64,
    last_total: u64,
    pending_update: bool,
}

impl ProgressThrottle {
    pub fn new(emitter: Arc<Emitter>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                last_update: Instant::now() - MIN_UPDATE_INTERVAL,
                last_completed: 0,
                last_total: 0,
                pending_update: false,
            })),
            emitter,
        }
    }

    pub fn report(
        &self,
        stage: Stage,
        completed: u64,
        total: u64,
        parallelism: Option<u32>,
        resource_wait: bool,
    ) {
        let mut inner = self.inner.lock().unwrap();
        let now = Instant::now();
        let elapsed = now.duration_since(inner.last_update);
        
        let changed = completed != inner.last_completed || total != inner.last_total;
        
        if changed && elapsed >= MIN_UPDATE_INTERVAL {
            inner.last_update = now;
            inner.last_completed = completed;
            inner.last_total = total;
            inner.pending_update = false;
            drop(inner);
            
            self.emitter.progress_with_detail(stage, completed, total, parallelism, resource_wait);
        } else if changed {
            inner.pending_update = true;
        }
    }

    pub fn force_flush(&self, stage: Stage) {
        let mut inner = self.inner.lock().unwrap();
        if inner.pending_update {
            let completed = inner.last_completed;
            let total = inner.last_total;
            inner.last_update = Instant::now();
            inner.pending_update = false;
            drop(inner);
            
            self.emitter.progress(stage, completed, total);
        }
    }
}
