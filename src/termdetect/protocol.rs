// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

#[cfg(test)]
use crate::termdetect::hosts::XTERMJS_HOSTS;

/// Byte sequence to begin a synchronized output region.
/// The terminal buffers all subsequent output until the end marker.
pub(crate) const SYNC_START: &[u8] = b"\x1b[?2026h";

/// Byte sequence to end a synchronized output region.
/// The terminal flushes all buffered content atomically.
pub(crate) const SYNC_END: &[u8] = b"\x1b[?2026l";

/// RIS (Reset to Initial State) — `ESC c`.
///
/// Tier 2: emitted periodically when running inside an xterm.js host to
/// force xterm.js to clear its in-memory scrollback buffer, preventing
/// the unbounded growth that leads to V8 OOM (SIGTRAP). The next frame
/// after a RIS performs a full redraw, so the user sees a brief
/// (single-frame) blanking — far less disruptive than the multi-second
/// hang of an OOM crash.
///
/// RIS is a hard reset in the ANSI spec, but xterm.js's implementation
/// is more lenient than hardware terminals: it preserves the current
/// TTY mode (raw mode, alternate screen, etc.) and only flushes the
/// buffer + scrollback. We still re-issue the alternate-screen sequence
/// in `Terminal::emit_ris_reset` to be safe across hosts.
///
/// This constant is exposed for tests that verify the byte sequence. The
/// runtime path uses a richer `RIS_RECOVERY` sequence (RIS + re-enter
/// alternate screen + cursor hide + SGR mouse mode) defined locally in
/// `Terminal::emit_ris_reset`.
#[cfg(test)]
pub(crate) const RIS_RESET: &[u8] = b"\x1bc";

/// Returns the canonical list of xterm.js host `TERM_PROGRAM` strings.
/// Used by the test suite to verify every entry in `XTERMJS_HOSTS`
/// triggers detection. Not used in production code paths.
#[cfg(test)]
pub(crate) fn known_xtermjs_hosts() -> &'static [&'static str] {
    XTERMJS_HOSTS
}

/// FPS cap applied when running inside any xterm.js-based host.
/// xterm.js's in-memory buffer grows unbounded at high frame rates;
/// 30 FPS keeps the worst-case byte rate under ~7 MB/sec (vs ~13.7 MB/sec
/// at 60 FPS), which xterm.js can drain over multi-hour runs without
/// OOMing — *assuming* the Tier 2 RIS reset also fires periodically to
/// clear the cumulative buffer. The user's --fps value is clamped to
/// this cap, not overridden silently — the verbose output discloses the
/// cap so there's no confusion.
pub(super) const XTERMJS_FPS_CAP: f64 = 30.0;

/// Back-compat alias for `XTERMJS_FPS_CAP`. Used by older tests that
/// reference the VSCode-specific name. New code should reference
/// `XTERMJS_FPS_CAP` directly.
#[cfg(test)]
#[allow(non_upper_case_globals)]
pub(crate) const VSCODE_FPS_CAP: f64 = XTERMJS_FPS_CAP;

/// Dynamic default FPS for high-performance terminals when the user
/// doesn't specify `--fps` or `fps =`. 144 Hz matches the most common
/// high-refresh monitor rate (between 120 and 165). The user's explicit
/// value always wins over this default.
pub(super) const HIGH_PERF_DEFAULT_FPS: f64 = 144.0;

/// Dynamic default FPS for standard/unknown terminals when the user
/// doesn't specify `--fps` or `fps =`. 60 FPS is the universal safe
/// default that every terminal can sustain.
pub(super) const STANDARD_DEFAULT_FPS: f64 = 60.0;
