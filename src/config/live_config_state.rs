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

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
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

/// v80.0.0-alpha.1 (S-master-HUNT-3, owner bug: self-heal diagnostics exposed without
/// --verbose): verbose-only runtime diagnostics — the self-heal family
/// ("predictive throttle", "sustained pressure", "throttle released",
/// "power-dragon off"). These report what the engine did AUTOMATICALLY
/// to itself; they are not user-actionable (that is the point of
/// self-healing), so surfacing them after every non-verbose session is
/// noise. They buffer exactly like `LIVE_RELOAD_RUNTIME_WARNINGS` (AB-10:
/// never eprintln during the alt screen) but drain ONLY when the session
/// ran with `--verbose`/`-v` — the "tell me everything" contract.
/// Actionable warnings (config validation, live-reload degradation) stay
/// on the always-drained channel.
pub(crate) static LIVE_RELOAD_RUNTIME_DIAGS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// v80.0.0-beta.1 killer-features hardening: true while the interactive rain session
/// (alternate screen) is active. Set once at the top of `run_interactive`,
/// never cleared — the process exits when the session ends. Warnings that
/// can fire on BOTH sides of the session boundary (charset-custom parse
/// notes, name-collision notices, ...) route through
/// `output::warn_runtime_or_now`, which checks this flag: direct stderr
/// before the session starts (immediate feedback), buffered
/// `push_runtime_warning` while the rain is on screen (AB-10 — no leak into
/// the alt screen), drained post-exit.
static INTERACTIVE_SESSION_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Mark the interactive rain session as active (called by `run_interactive`).
pub(crate) fn set_interactive_session_active() {
    INTERACTIVE_SESSION_ACTIVE.store(true, Ordering::Release);
}

/// Whether the interactive rain session (alt screen) is currently active.
#[must_use]
pub(crate) fn interactive_session_active() -> bool {
    INTERACTIVE_SESSION_ACTIVE.load(Ordering::Acquire)
}

/// Test-only reset so unit tests can exercise both routing sides.
#[cfg(test)]
pub(crate) fn reset_interactive_session_active() {
    INTERACTIVE_SESSION_ACTIVE.store(false, Ordering::Release);
}

/// Append a non-fatal runtime warning to the session log. Bulletproof —
/// never panics on poisoned mutex. Called from the live-reload path only.
///
/// v80.0.0-beta.1 killer-features hardening: identical messages are deduplicated.
/// Several killer-feature warnings re-fire on every scene change / config
/// save (the `.stops` deprecation, the scene-custom re-apply note); without
/// dedup a long editing session fills the 64-slot buffer with copies and
/// the post-exit summary becomes unreadable spam. First occurrence wins.
pub(crate) fn push_runtime_warning(msg: &str) {
    if let Ok(mut guard) = LIVE_RELOAD_RUNTIME_WARNINGS.lock() {
        if guard.iter().any(|m| m == msg) {
            return;
        }
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

/// Append a verbose-only runtime diagnostic to the session log (see
/// `LIVE_RELOAD_RUNTIME_DIAGS`). Bulletproof — never panics on a poisoned
/// mutex. Same dedup + 64-slot cap contract as `push_runtime_warning`
/// (self-heal messages re-fire per scene change; without dedup a long
/// session under pressure would spam the post-exit summary).
pub(crate) fn push_runtime_diag(msg: &str) {
    if let Ok(mut guard) = LIVE_RELOAD_RUNTIME_DIAGS.lock() {
        if guard.iter().any(|m| m == msg) {
            return;
        }
        if guard.len() < MAX_RUNTIME_WARNING_LOG {
            guard.push(msg.to_string());
        }
    }
}

/// Drain the verbose-only diagnostic log. Empty if no diags or mutex
/// poisoned. The exit path (output/post_exit.rs) calls this ONLY when the
/// session ran with `--verbose`/`-v` — non-verbose users never see
/// self-heal chatter (owner contract, v80.0.0-alpha.1 (S-master-HUNT-3)).
pub(crate) fn drain_runtime_diags() -> Vec<String> {
    LIVE_RELOAD_RUNTIME_DIAGS
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes tests that push/drain the shared warning buffer so they
    /// cannot steal each other's markers when running in parallel.
    static BUFFER_TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn push_runtime_warning_dedups_identical_messages() {
        let _guard = BUFFER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let marker = format!("dedup-test-{}", std::process::id());
        push_runtime_warning(&marker);
        push_runtime_warning(&marker);
        push_runtime_warning(&marker);
        let drained = drain_runtime_warnings();
        assert_eq!(
            drained.iter().filter(|m| m.as_str() == marker).count(),
            1,
            "identical warnings must dedup to one entry, got {drained:?}"
        );
    }

    #[test]
    fn warn_runtime_or_now_buffers_while_session_active() {
        let _guard = BUFFER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let marker = format!("routing-test-{}", std::process::id());
        set_interactive_session_active();
        crate::output::warn_runtime_or_now(&marker);
        let drained = drain_runtime_warnings();
        assert!(
            drained.iter().any(|m| m.contains(&marker)),
            "session-active warning must be buffered, got {drained:?}"
        );
        reset_interactive_session_active();
    }

    /// v80.0.0-alpha.1 (S-master-HUNT-3, owner bug: self-heal diagnostics exposed without
    /// --verbose): the DIAG channel is a separate buffer with the same
    /// dedup contract, and it never leaks into the always-drained
    /// warning channel.
    #[test]
    fn push_runtime_diag_buffers_and_dedups_separately() {
        let _guard = BUFFER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let marker = format!("diag-test-{}", std::process::id());
        push_runtime_diag(&marker);
        push_runtime_diag(&marker);
        let diags = drain_runtime_diags();
        assert_eq!(
            diags.iter().filter(|m| m.as_str() == marker).count(),
            1,
            "identical diags must dedup to one entry, got {diags:?}"
        );
        // Channel isolation: the diag must NOT have landed in the
        // always-drained warning log (that is what leaked the
        // self-heal line to non-verbose users before the fix).
        let warnings = drain_runtime_warnings();
        assert!(
            !warnings.iter().any(|m| m.contains(&marker)),
            "diag entries must never leak into the warning channel"
        );
    }

    /// v80.0.0-alpha.1 (S-master-HUNT-3) routing lock (source scan): every self-heal
    /// message must go through push_runtime_diag — a single
    /// push_runtime_warning in event_loop_self_heal.rs would re-expose
    /// the diagnostics to non-verbose users (the owner bug this channel
    /// exists to fix). Source-scan style (same family as the LOC test)
    /// because the routing lives in the interactive module, not here.
    #[test]
    fn self_heal_family_routes_through_diag_channel_only() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/interactive/event_loop_self_heal.rs");
        let src = std::fs::read_to_string(&path).expect("read event_loop_self_heal.rs");
        assert!(
            !src.contains("push_runtime_warning"),
            "self-heal messages must route through push_runtime_diag (verbose-only),              found a push_runtime_warning call"
        );
        assert_eq!(
            src.matches("push_runtime_diag").count(),
            4,
            "all four self-heal messages (power-dragon release, sustained, recovered,              predictive) must be on the diag channel"
        );
    }
}
