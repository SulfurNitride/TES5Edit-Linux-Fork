//! Thread-safe progress reporting for LOD generation.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Function pointer type for progress callbacks (matches C FFI).
pub type ProgressFn = unsafe extern "C" fn(*const std::os::raw::c_char, f64);

/// Thread-safe progress reporter.
#[derive(Clone)]
pub struct Progress {
    callback: Option<ProgressFn>,
    message: Arc<Mutex<String>>,
    fraction: Arc<Mutex<f64>>,
    cancel: Arc<AtomicBool>,
}

impl Progress {
    pub fn new(callback: Option<ProgressFn>, cancel: Arc<AtomicBool>) -> Self {
        Self {
            callback,
            message: Arc::new(Mutex::new(String::new())),
            fraction: Arc::new(Mutex::new(0.0)),
            cancel,
        }
    }

    /// Report progress with a message and fraction (0.0 to 1.0).
    pub fn report(&self, msg: &str, fraction: f64) {
        if let Ok(mut m) = self.message.lock() {
            *m = msg.to_string();
        }
        if let Ok(mut f) = self.fraction.lock() {
            *f = fraction;
        }
        if let Some(cb) = self.callback {
            let c_msg = std::ffi::CString::new(msg).unwrap_or_default();
            unsafe { cb(c_msg.as_ptr(), fraction) };
        }
    }

    /// Check if cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Create a no-op progress (for tests).
    pub fn noop() -> Self {
        Self {
            callback: None,
            message: Arc::new(Mutex::new(String::new())),
            fraction: Arc::new(Mutex::new(0.0)),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
}
