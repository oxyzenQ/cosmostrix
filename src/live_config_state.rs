// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Session-wide buffered state for the live-reload subsystem.
//!
//! Holds the statics that survive across the rain loop:
//! - `LIVE_RELOAD_EXIT_CODE` / `LIVE_RELOAD_ERROR` — fatal reload failure
//!   (set by watcher, drained by main.rs post-exit).
//! - `LIVE_RELOAD_VALIDATION_REJECTIONS` — accumulated silent rejections
//!   (drained by post-exit verbose summary).
//! - `LIVE_RELOAD_RUNTIME_WARNINGS` — non-fatal runtime warnings emitted
//!   from the live-reload path (e.g. deprecated `.stops` alias). Buffered
//!   during the rain loop to avoid alt-screen leak (AB-10), drained post-exit.

use std::sync::atomic::AtomicU8;
use std::sync::Mutex;

/// Global exit code set by live-reload when invalid config is detected.
/// 0 = no error (default), 2 = live-reload validation failure.
/// Main.rs checks this after run_interactive() returns and exits accordingly.
pub(crate) static LIVE_RELOAD_EXIT_CODE: AtomicU8 = AtomicU8::new(0);

/// Global error message captured during live-reload failure.
/// Printed to stderr AFTER terminal restoration so the user can see it.
pub(crate) static LIVE_RELOAD_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// (bug #14): Accumulated validation rejections during the session.
///
/// Each entry is one rejected config reload (timestamp + error). Drained in
/// the post-exit verbose summary so the user sees EVERY silent rejection
/// that happened while editing config.toml mid-session. Without this, OOR
/// values like `color.tune.tail = 5.0` get silently rejected by
/// `validate_config_strictly`, the watcher continues, the rain runs on the
/// last valid config — and the user has no idea their edit was rejected.
///
/// Cap at 64 entries (defense against misbehaving editors that save 1000×/s).
pub(crate) static LIVE_RELOAD_VALIDATION_REJECTIONS: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub(crate) const MAX_REJECTION_LOG: usize = 64;

/// AB-10 (rain-screen cleanliness): non-fatal runtime warnings emitted from
/// the live-reload path (e.g. deprecated `.stops` alias in colors-custom).
///
/// Previously these were `eprintln!`'d directly from `colors_custom::collect_colors_custom`,
/// which is called by `rebuild_cloud_config` on every config save. The
/// eprintln fired while the alternate screen was active, leaking the
/// warning line into the rain matrix.
///
/// Now they are buffered here and drained by main.rs AFTER `run_interactive`
/// returns and `Terminal::drop` restores the main screen.
pub(crate) static LIVE_RELOAD_RUNTIME_WARNINGS: Mutex<Vec<String>> = Mutex::new(Vec::new());

const MAX_RUNTIME_WARNING_LOG: usize = 64;

/// Append a non-fatal runtime warning to the session log. Bulletproof —
/// never panics on poisoned mutex. Called from the live-reload path only.
pub(crate) fn push_runtime_warning(msg: &str) {
    if let Ok(mut guard) = LIVE_RELOAD_RUNTIME_WARNINGS.lock() {
        if guard.len() < MAX_RUNTIME_WARNING_LOG {
            guard.push(msg.to_string());
        }
    }
}

/// Drain the runtime warning log. Empty if no warnings or mutex poisoned.
/// Production exit path (main.rs) calls this after Terminal::drop so the
/// warnings land on the main screen, not in the rain matrix.
pub(crate) fn drain_runtime_warnings() -> Vec<String> {
    LIVE_RELOAD_RUNTIME_WARNINGS
        .lock()
        .map(|mut g| std::mem::take(&mut *g))
        .unwrap_or_default()
}

/// Append a validation rejection to the session log.
/// Called from `validate_and_send` when `validate_config_strictly` rejects
/// the new config. Bulletproof — never panics on poisoned mutex.
pub(crate) fn push_validation_rejection(msg: &str) {
    let ts = crate::output::now_hhmm();
    let entry = format!("{ts} {msg}");
    if let Ok(mut guard) = LIVE_RELOAD_VALIDATION_REJECTIONS.lock() {
        if guard.len() < MAX_REJECTION_LOG {
            guard.push(entry);
        }
    }
}

/// Drain the session rejection log (test-only utility). Empty if no
/// rejections or mutex poisoned. Production exit path (bug #15)
/// prints the first rejection via LIVE_RELOAD_EXIT_CODE and exits, so the
/// log is never drained in production.
#[cfg(test)]
pub fn drain_validation_rejections() -> Vec<String> {
    LIVE_RELOAD_VALIDATION_REJECTIONS
        .lock()
        .map(|mut guard| std::mem::take(&mut *guard))
        .unwrap_or_default()
}
