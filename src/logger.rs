// ============================================================================
// [SOVEREIGN ENGINE CONSTITUTION - INVIOLABLE SAFETY DIRECTIVE]
// STRICT ARCHITECTURAL RULE:
// It is strictly forbidden, under any circumstances or for any reason whatsoever,
// to remove, comment out, bypass, or weaken `#![deny(unused_must_use)]` from
// this file or ANY other .rs file in this codebase.
// All fallible operations, Results, and Options must be explicitly handled.
// Zero silent error swallowing under the rug. This engine is built to endure for decades.
// ============================================================================
#![deny(unused_must_use)]

use parking_lot::Mutex;
use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Clone)]
pub struct LogBuffer {
    logs: Arc<Mutex<VecDeque<String>>>,
}

impl LogBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            logs: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
        }
    }

    pub fn push(&self, msg: String) {
        let mut guard = self.logs.lock();
        if guard.len() >= 500 {
            guard.pop_front();
        }
        let timestamp = chrono_timestamp();
        guard.push_back(format!("[{}] {}", timestamp, msg));
    }

    pub fn get_logs(&self) -> Vec<String> {
        let guard = self.logs.lock();
        guard.iter().cloned().collect()
    }
}


fn chrono_timestamp() -> String {
    let now = std::time::SystemTime::now();
    let since_epoch = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = since_epoch.as_secs() % 86400;
    let hours = (secs / 3600 + 3) % 24; // UTC+3 approx
    let mins = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, s)
}
