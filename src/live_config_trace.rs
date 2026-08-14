// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Opt-in debug tracing for the live-reload subsystem.
//!
//! Set `COSMOSTRIX_LIVE_RELOAD_DEBUG=1` in the environment to emit a
//! step-by-step trace of every live-reload event: file-change detection,
//! mtime dedup decisions, parse counts, strict-validation result, per-
//! field apply/skip decisions in `rebuild_cloud_config` (with reason —
//! CLI-explicit vs parse failure), and the final Cloud rebuild summary
//! emitted from the render thread.
//!
//! Default off — zero cost when the env var is unset. When enabled, trace
//! lines are buffered to `LIVE_RELOAD_DEBUG_TRACES` and drained by
//! main.rs AFTER `run_interactive` returns and `Terminal::drop` restores
//! the main screen — otherwise they would leak into the alt-screen rain
//! matrix (AB-10 rain-screen cleanliness).
//!
//! Split into its own module so `live_config.rs` stays under the
//! 1500-LOC source cap enforced by `loc_tests`.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;

/// Cap for the debug trace buffer. Traces are emitted from the watcher
/// thread, polling heartbeat, and render thread during the rain loop.
/// A single live-reload cycle can produce ~10-20 trace lines, and a
/// typical debug session inspects ~20-50 reloads, so 1000 entries is
/// generous while still bounded against runaway logs.
const MAX_DEBUG_TRACE_LOG: usize = 1000;

/// AB-10: buffered debug traces emitted while `COSMOSTRIX_LIVE_RELOAD_DEBUG=1`.
///
/// All `lr_trace!` and `debug_trace` calls append here instead of writing
/// directly to stderr. main.rs drains this buffer after Terminal::drop so
/// the trace lines land on the main screen, not in the alt-screen rain matrix.
pub(crate) static LIVE_RELOAD_DEBUG_TRACES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// True when `COSMOSTRIX_LIVE_RELOAD_DEBUG` is set to a truthy value.
///
/// Read once at first call and cached in an AtomicU8
/// (0 = unknown, 1 = off, 2 = on) so repeated checks are branch-predicted
/// and never touch the env-var lookup after the first call.
pub(crate) fn live_reload_debug_enabled() -> bool {
    static STATE: AtomicU8 = AtomicU8::new(0);
    match STATE.load(Ordering::Acquire) {
        1 => false,
        2 => true,
        _ => {
            let on = matches!(
                std::env::var("COSMOSTRIX_LIVE_RELOAD_DEBUG")
                    .ok()
                    .as_deref(),
                Some("1") | Some("true") | Some("TRUE") | Some("yes")
            );
            STATE.store(if on { 2 } else { 1 }, Ordering::Release);
            on
        }
    }
}

/// Buffer a `[live-reload-trace]` line if tracing is enabled. No-op
/// otherwise. Bulletproof — never panics on poisoned mutex.
pub(crate) fn debug_trace(args: std::fmt::Arguments<'_>) {
    if !live_reload_debug_enabled() {
        return;
    }
    // Format the args into a String and buffer. We can't write directly to
    // stderr here because the watcher/render threads run while the alt screen
    // is active — AB-10 rain-screen cleanliness. main.rs drains post-exit.
    if let Ok(mut g) = LIVE_RELOAD_DEBUG_TRACES.lock() {
        if g.len() < MAX_DEBUG_TRACE_LOG {
            g.push(format!("[live-reload-trace] {}", args));
        }
    }
}

/// Drain the debug trace buffer. Empty if disabled, no traces, or mutex
/// poisoned. Production exit path (main.rs) calls this after Terminal::drop
/// so the traces land on the main screen, not in the rain matrix.
pub(crate) fn drain_debug_traces() -> Vec<String> {
    LIVE_RELOAD_DEBUG_TRACES
        .lock()
        .map(|mut g| std::mem::take(&mut *g))
        .unwrap_or_default()
}

/// Emit a debug trace line if `COSMOSTRIX_LIVE_RELOAD_DEBUG=1`. No-op
/// otherwise. Intended for use inside `live_config.rs` only.
#[macro_export]
macro_rules! lr_trace {
    ($($arg:tt)*) => {
        if $crate::live_config_trace::live_reload_debug_enabled() {
            $crate::live_config_trace::debug_trace(format_args!($($arg)*));
        }
    };
}

/// Public trace hook for callers outside `live_config.rs` (e.g. the
/// render thread in `event_loop.rs`) to confirm a rebuild was actually
/// applied to the live Cloud. Same env-gated path as `lr_trace!` —
/// zero cost when `COSMOSTRIX_LIVE_RELOAD_DEBUG` is unset.
///
/// Accepts the resolved field values (not the CloudConfig itself) so
/// the trace line shows what the user actually sees post-rebuild:
/// `color=?`, `charset=?`, `speed`, `density`, `fps`.
pub(crate) fn trace_rebuild_applied(
    color_scheme: &crate::runtime::ColorScheme,
    charset_preset: &str,
    speed: f32,
    density: f32,
    fps: f64,
) {
    if live_reload_debug_enabled() {
        debug_trace(format_args!(
            "Cloud rebuilt — color={:?} charset='{}' speed={:.2} density={:.3} fps={:.2}",
            color_scheme, charset_preset, speed, density, fps
        ));
    }
}

/// field-level config diff trace.
///
/// Emits a structured diff between the previously-applied config map and
/// the newly-received one. `old = None` triggers the "[initial]" trace
/// (first-ever apply). Otherwise emits "[changed N]", "[added N]", and
/// "[removed N]" lines as needed, plus a "no field-level changes" line
/// if the diff is empty (whitespace/comment-only edit).
///
/// Extracted from `event_loop.rs` to keep the file under the 1200-LOC cap.
/// No-op when `COSMOSTRIX_LIVE_RELOAD_DEBUG` is unset.
pub(crate) fn trace_config_diff(
    old: Option<&std::collections::HashMap<String, String>>,
    new: &std::collections::HashMap<String, String>,
) {
    if !live_reload_debug_enabled() {
        return;
    }
    match old {
        None => {
            let mut keys: Vec<&String> = new.keys().collect();
            keys.sort();
            for k in keys {
                crate::lr_trace!("config diff [initial]: {} = {}", k, new[k]);
            }
        }
        Some(old_map) => {
            let all_keys: std::collections::BTreeSet<&String> =
                old_map.keys().chain(new.keys()).collect();
            let mut changed: Vec<String> = Vec::new();
            let mut added: Vec<String> = Vec::new();
            let mut removed: Vec<String> = Vec::new();
            for k in &all_keys {
                match (old_map.get(*k), new.get(*k)) {
                    (Some(o), Some(n)) => {
                        if o != n {
                            changed.push(format!("{}: {} → {}", k, o, n));
                        }
                    }
                    (None, Some(n)) => added.push(format!("{}: {}", k, n)),
                    (Some(o), None) => removed.push(format!("{}: {}", k, o)),
                    (None, None) => unreachable!(),
                }
            }
            if !changed.is_empty() {
                crate::lr_trace!(
                    "config diff [changed {}]: {}",
                    changed.len(),
                    changed.join(", ")
                );
            }
            if !added.is_empty() {
                crate::lr_trace!("config diff [added {}]: {}", added.len(), added.join(", "));
            }
            if !removed.is_empty() {
                crate::lr_trace!(
                    "config diff [removed {}]: {}",
                    removed.len(),
                    removed.join(", ")
                );
            }
            if changed.is_empty() && added.is_empty() && removed.is_empty() {
                crate::lr_trace!("config diff: no field-level changes (whitespace/comment edit)");
            }
        }
    }
}
