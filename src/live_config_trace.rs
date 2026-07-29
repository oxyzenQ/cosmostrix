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
//! Default off — zero cost when the env var is unset. All trace writes
//! go through the bulletproof `write_fmt` path so they cannot panic on
//! broken stderr (terminal closed mid-session).
//!
//! Split into its own module so `live_config.rs` stays under the
//! 1500-LOC source cap enforced by `loc_tests`.

use std::io::Write;
use std::sync::atomic::{AtomicU8, Ordering};

/// True when `COSMOSTRIX_LIVE_RELOAD_DEBUG` is set to a truthy value.
///
/// Read once at first call and cached in an AtomicU8
/// (0 = unknown, 1 = off, 2 = on) so repeated checks are branch-predicted
/// and never touch the env-var lookup after the first call.
pub fn live_reload_debug_enabled() -> bool {
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

/// Emit a `[live-reload-trace]` line to stderr if tracing is enabled.
/// No-op otherwise. Bulletproof — never panics on broken stderr.
pub fn debug_trace(args: std::fmt::Arguments<'_>) {
    let _ = std::io::stderr().write_fmt(args);
}

/// Emit a debug trace line if `COSMOSTRIX_LIVE_RELOAD_DEBUG=1`. No-op
/// otherwise. Intended for use inside `live_config.rs` only.
#[macro_export]
macro_rules! lr_trace {
    ($($arg:tt)*) => {
        if $crate::live_config_trace::live_reload_debug_enabled() {
            $crate::live_config_trace::debug_trace(format_args!(
                "[live-reload-trace] {}\n",
                format_args!($($arg)*)
            ));
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
pub fn trace_rebuild_applied(
    color_scheme: &crate::runtime::ColorScheme,
    charset_preset: &str,
    speed: f32,
    density: f32,
    fps: f64,
) {
    if live_reload_debug_enabled() {
        debug_trace(format_args!(
            "[live-reload-trace] Cloud rebuilt — color={:?} charset='{}' speed={:.2} density={:.3} fps={:.2}\n",
            color_scheme, charset_preset, speed, density, fps
        ));
    }
}
