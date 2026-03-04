//! Thread-safe progress reporting wrapper for LOD generation.

use std::ffi::CString;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// C-compatible progress callback function pointer.
pub type ProgressFn = extern "C" fn(message: *const std::os::raw::c_char, progress: f64);

/// Thread-safe progress tracker that reports to an optional C callback.
pub struct Progress {
    callback: Option<ProgressFn>,
    cancelled: &'static AtomicBool,
    total: AtomicU64,
    done: AtomicU64,
}

impl Progress {
    pub fn new(callback: Option<ProgressFn>, cancelled: &'static AtomicBool) -> Self {
        Self {
            callback,
            cancelled,
            total: AtomicU64::new(0),
            done: AtomicU64::new(0),
        }
    }

    /// Set the total number of work items for progress fraction calculation.
    pub fn set_total(&self, total: u64) {
        self.total.store(total, Ordering::Relaxed);
        self.done.store(0, Ordering::Relaxed);
    }

    /// Increment the done counter by one and update progress fraction.
    /// Only sends the fraction update (no message text change).
    pub fn increment(&self) {
        let done = self.done.fetch_add(1, Ordering::Relaxed) + 1;
        let total = self.total.load(Ordering::Relaxed);
        if total > 0 {
            let fraction = done as f64 / total as f64;
            if let Some(cb) = self.callback {
                // Send null message pointer to update fraction without changing message text
                cb(std::ptr::null(), fraction);
            }
        }
    }

    /// Report a text message with current progress fraction.
    pub fn report(&self, msg: &str) {
        if let Some(cb) = self.callback {
            let total = self.total.load(Ordering::Relaxed);
            let done = self.done.load(Ordering::Relaxed);
            let fraction = if total > 0 {
                done as f64 / total as f64
            } else {
                0.0
            };
            if let Ok(c_msg) = CString::new(msg) {
                cb(c_msg.as_ptr(), fraction);
            }
        }
    }

    /// Check if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}
