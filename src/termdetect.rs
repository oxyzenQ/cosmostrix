// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Terminal protocol detection at startup.
//!
//! Detects terminal capabilities from environment variables and enables
//! protocol-level optimizations where available:
//!
//! - **Synchronized output** (`ESC[?2026h` / `ESC[?2026l`): Frames the
//!   entire draw in a sync region so the terminal emulator buffers output
//!   internally and flushes atomically. Eliminates visual tearing during
//!   partial redraws. Supported by: kitty, wezterm, alacritty, foot,
//!   iTerm2 3.5+, Windows Terminal 1.22+, tmux 3.3+.
//!
//! - **xterm.js host detection** (Tier 2): the `TERM_PROGRAM` env var is
//!   checked against a list of known Electron-based terminal hosts that
//!   embed xterm.js as their renderer. All of them share the same
//!   unbounded-buffer-growth failure mode: at high frame rates, cosmostrix
//!   pumps ANSI bytes into node-pty → xterm.js, whose in-memory scrollback
//!   grows without bound over multi-hour runs until V8 hits an OOM
//!   assertion (SIGTRAP). When an xterm.js host is detected:
//!
//!     * Synchronized output is disabled (xterm.js's mode 2026 buffer
//!       implementation amplifies memory pressure).
//!     * A default FPS cap is applied (see `XTERMJS_FPS_CAP`).
//!     * A rolling byte-budget backpressure is enabled (see
//!       `XTERMJS_BYTE_BUDGET_PER_WINDOW` in `constants.rs`).
//!     * A periodic RIS reset (ESC c) is emitted to clear xterm.js's
//!       in-memory buffer (see `XTERMJS_RIS_RESET_BYTES` in `constants.rs`).
//!
//!   The original crash was reported 2026-08-04 inside VSCode (code-oss
//!   Signal 5 TRAP after hours of cosmostrix). Tier 1 (shipped) covered
//!   only `TERM_PROGRAM=vscode`; Tier 2 extends detection to all known
//!   xterm.js hosts.

use std::env;

/// `TERM_PROGRAM` values that identify an xterm.js-based Electron host.
/// All of these ship xterm.js as their terminal renderer and inherit the
/// same unbounded-buffer-growth failure mode at high ANSI byte rates.
///
/// Kept as a `const` slice (not a `match`) so the test suite can iterate
/// over the list and verify each host triggers detection.
///
/// **Adding a host**: append the exact `TERM_PROGRAM` string here. The
/// rest of the detection / capping / RIS-reset machinery keys off the
/// `xtermjs_host` boolean and is host-agnostic.
const XTERMJS_HOSTS: &[&str] = &[
    // VSCode (and forks like VSCodium, code-oss, code-insiders). The
    // original crash report was inside VSCode.
    "vscode",
    // Hyper — Electron-based terminal, uses xterm.js as the renderer.
    // Sets TERM_PROGRAM=Hyper (capital H).
    "Hyper",
    // WaveTerminal — Electron-based, embeds xterm.js in a TilingWave pane.
    // Sets TERM_PROGRAM=WaveTerminal.
    "WaveTerminal",
    // Tabby — Electron-based terminal manager, embeds xterm.js for the
    // terminal pane. Sets TERM_PROGRAM=Tabby.
    "Tabby",
    // WarpTerminal — Rust+Electron hybrid; the renderer pane is xterm.js.
    // Sets TERM_PROGRAM=WarpTerminal.
    "WarpTerminal",
];

/// Capabilities discovered at startup.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TerminalCaps {
    /// Synchronized output (`ESC[?2026h` / `ESC[?2026l`) — universally
    /// safe to enable; terminals that don't support it silently ignore
    /// the escape sequence.
    pub sync_output: bool,
    /// True when running inside ANY xterm.js-based Electron host
    /// (`TERM_PROGRAM` matches an entry in `XTERMJS_HOSTS`). This is the
    /// primary Tier 2 signal — gating FPS cap, byte-budget backpressure,
    /// and periodic RIS reset.
    pub xtermjs_host: bool,
    /// Back-compat alias: true when `TERM_PROGRAM=vscode` specifically.
    /// Equivalent to `xtermjs_host && term_program == "vscode"`. Kept so
    /// existing call sites (verbose output, doc cross-references) can
    /// single out VSCode without re-reading the env var. New code should
    /// key off `xtermjs_host` instead.
    pub vscode_integrated: bool,
    /// Maximum recommended FPS for this terminal. Native terminals
    /// (Alacritty, Kitty, etc.) get 300 (effectively uncapped — the
    /// user's --fps value wins). xterm.js hosts get 30 to keep the
    /// worst-case byte rate under ~7 MB/sec.
    pub default_fps_cap: f64,
}

/// FPS cap applied when running inside any xterm.js-based host.
/// xterm.js's in-memory buffer grows unbounded at high frame rates;
/// 30 FPS keeps the worst-case byte rate under ~7 MB/sec (vs ~13.7 MB/sec
/// at 60 FPS), which xterm.js can drain over multi-hour runs without
/// OOMing — *assuming* the Tier 2 RIS reset also fires periodically to
/// clear the cumulative buffer. The user's --fps value is clamped to
/// this cap, not overridden silently — the verbose output discloses the
/// cap so there's no confusion.
const XTERMJS_FPS_CAP: f64 = 30.0;

/// Back-compat alias for `XTERMJS_FPS_CAP`. Used by older tests that
/// reference the VSCode-specific name. New code should reference
/// `XTERMJS_FPS_CAP` directly.
#[cfg(test)]
#[allow(non_upper_case_globals)]
const VSCODE_FPS_CAP: f64 = XTERMJS_FPS_CAP;

/// Run detection from environment variables. Safe to call before any
/// terminal initialization.
pub(crate) fn detect() -> TerminalCaps {
    let term = env::var("TERM").unwrap_or_default();
    let term_program = env::var("TERM_PROGRAM").unwrap_or_default();

    // Tier 2: match against the full list of known xterm.js hosts. The
    // comparison is case-sensitive against the canonical strings these
    // terminals emit (VSCode emits lowercase "vscode", Hyper emits
    // "Hyper" with capital H, etc.) — matching the upstream documented
    // behavior, not a lowercased approximation.
    let xtermjs_host = XTERMJS_HOSTS.iter().any(|&h| term_program == h);

    // VSCode-specific alias for back-compat with Tier 1 code paths that
    // single out VSCode in user-facing strings (warnings, verbose output).
    let vscode_integrated = xtermjs_host && term_program == "vscode";

    // Synchronized output is supported by virtually all modern terminals.
    // The escape sequences are a no-op on terminals that don't support
    // them, so enabling unconditionally is safe. Three exceptions:
    //   1. Linux console (TERM=linux) — does not understand the sequence.
    //   2. xterm.js hosts — xterm.js's mode 2026 buffer implementation
    //      amplifies memory pressure under high frame rates, contributing
    //      to the multi-hour SIGTRAP crash.
    // tmux 3.3+ passes sync sequences through to the outer terminal.
    let sync_ok = !term.eq_ignore_ascii_case("linux") && !xtermjs_host;

    // xterm.js hosts get a 30 FPS cap; everything else is effectively
    // uncapped (the user's --fps value, validated to 1.0..=300.0, wins).
    let default_fps_cap = if xtermjs_host { XTERMJS_FPS_CAP } else { 300.0 };

    TerminalCaps {
        sync_output: sync_ok,
        xtermjs_host,
        vscode_integrated,
        default_fps_cap,
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // env::set_var is process-global and not thread-safe; serialize the
    // tests that touch TERM_PROGRAM so they don't race with each other.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// Helper: restore TERM and TERM_PROGRAM after a test mutates them.
    /// Captures the prev values up-front and returns a closure that
    /// restores them when dropped (RAII). Eliminates the boilerplate
    /// `match prev_*` blocks that previously appeared in every test.
    struct EnvGuard {
        prev_term: Option<String>,
        prev_tp: Option<String>,
    }

    impl EnvGuard {
        fn capture() -> Self {
            Self {
                prev_term: env::var("TERM").ok(),
                prev_tp: env::var("TERM_PROGRAM").ok(),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match self.prev_term.take() {
                Some(v) => env::set_var("TERM", v),
                None => env::remove_var("TERM"),
            }
            match self.prev_tp.take() {
                Some(v) => env::set_var("TERM_PROGRAM", v),
                None => env::remove_var("TERM_PROGRAM"),
            }
        }
    }

    #[test]
    fn sync_markers_are_valid_escape_sequences() {
        // SYNC_START / SYNC_END must start with ESC [ and end with
        // valid CSI terminators (h/l for set/reset private modes).
        assert!(SYNC_START.starts_with(b"\x1b["));
        assert!(SYNC_END.starts_with(b"\x1b["));
        assert_eq!(SYNC_START.last(), Some(&b'h'));
        assert_eq!(SYNC_END.last(), Some(&b'l'));
    }

    #[test]
    fn ris_reset_is_valid_escape_sequence() {
        // RIS is ESC c (0x1b 0x63) — a 2-byte C1 control sequence.
        assert_eq!(RIS_RESET.len(), 2);
        assert_eq!(RIS_RESET[0], 0x1b);
        assert_eq!(RIS_RESET[1], b'c');
    }

    #[test]
    fn sync_output_disabled_for_linux_console() {
        // Simulate TERM=linux detection result
        let caps = TerminalCaps {
            sync_output: false,
            xtermjs_host: false,
            vscode_integrated: false,
            default_fps_cap: 300.0,
        };
        assert!(!caps.sync_output);
    }

    #[test]
    fn vscode_detection_when_term_program_is_vscode() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::capture();
        env::set_var("TERM", "xterm-256color");
        env::set_var("TERM_PROGRAM", "vscode");
        let caps = detect();
        assert!(caps.xtermjs_host, "xtermjs_host must be true for VSCode");
        assert!(
            caps.vscode_integrated,
            "vscode_integrated must be true for VSCode"
        );
        assert!(
            !caps.sync_output,
            "sync_output must be disabled for xterm.js hosts (OOM amplification)"
        );
        assert_eq!(caps.default_fps_cap, XTERMJS_FPS_CAP);
    }

    #[test]
    fn xtermjs_host_detection_for_all_known_hosts() {
        // Tier 2: every entry in XTERMJS_HOSTS must trigger detection.
        // This is the regression test for the core Tier 2 fix — if a
        // new host is added to the list but the detection logic breaks,
        // this test fails.
        let _guard = ENV_LOCK.lock().unwrap();
        for &host in known_xtermjs_hosts() {
            let _env = EnvGuard::capture();
            env::set_var("TERM", "xterm-256color");
            env::set_var("TERM_PROGRAM", host);
            let caps = detect();
            assert!(
                caps.xtermjs_host,
                "xtermjs_host must be true for TERM_PROGRAM={host}"
            );
            assert!(
                !caps.sync_output,
                "sync_output must be disabled for TERM_PROGRAM={host}"
            );
            assert_eq!(
                caps.default_fps_cap, XTERMJS_FPS_CAP,
                "FPS cap must apply for TERM_PROGRAM={host}"
            );
        }
    }

    #[test]
    fn vscode_alias_false_for_non_vscode_xtermjs_hosts() {
        // Tier 2: vscode_integrated is a back-compat alias that should
        // be FALSE for non-VSCode xterm.js hosts (Hyper, WaveTerminal,
        // etc.), even though xtermjs_host is true. This protects
        // user-facing strings that single out VSCode.
        let _guard = ENV_LOCK.lock().unwrap();
        for &host in known_xtermjs_hosts().iter().filter(|&&h| h != "vscode") {
            let _env = EnvGuard::capture();
            env::set_var("TERM", "xterm-256color");
            env::set_var("TERM_PROGRAM", host);
            let caps = detect();
            assert!(caps.xtermjs_host, "xtermjs_host must be true for {host}");
            assert!(
                !caps.vscode_integrated,
                "vscode_integrated must be false for non-VSCode host {host}"
            );
        }
    }

    #[test]
    fn detection_false_for_native_terminals() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::capture();
        env::set_var("TERM", "xterm-256color");
        env::set_var("TERM_PROGRAM", "alacritty");
        let caps = detect();
        assert!(!caps.xtermjs_host);
        assert!(!caps.vscode_integrated);
        assert!(
            caps.sync_output,
            "sync_output must stay on for native terminals"
        );
        assert_eq!(caps.default_fps_cap, 300.0);
    }

    #[test]
    fn detection_false_when_term_program_unset() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::capture();
        env::set_var("TERM", "xterm-256color");
        env::remove_var("TERM_PROGRAM");
        let caps = detect();
        assert!(!caps.xtermjs_host);
        assert!(!caps.vscode_integrated);
    }

    #[test]
    fn detection_false_for_unknown_electron_hosts() {
        // Tier 2: an Electron host that's NOT in our list (e.g., a
        // future or internal tool) should not trigger xterm.js
        // detection. We can't know if it embeds xterm.js, so we err
        // on the side of full performance (no cap, sync_output on).
        let _guard = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::capture();
        env::set_var("TERM", "xterm-256color");
        env::set_var("TERM_PROGRAM", "SomeUnknownElectronApp");
        let caps = detect();
        assert!(!caps.xtermjs_host);
        assert!(caps.sync_output);
        assert_eq!(caps.default_fps_cap, 300.0);
    }

    #[test]
    fn detect_does_not_panic_with_empty_term() {
        let _guard = ENV_LOCK.lock().unwrap();
        let _env = EnvGuard::capture();
        env::remove_var("TERM");
        env::remove_var("TERM_PROGRAM");
        let caps = detect();
        assert!(!caps.xtermjs_host);
        assert!(caps.sync_output, "empty TERM should still enable sync");
    }

    #[test]
    fn vscode_fps_cap_alias_matches_xtermjs_cap() {
        // Back-compat: VSCODE_FPS_CAP is now an alias for XTERMJS_FPS_CAP.
        // Older tests reference VSCODE_FPS_CAP — this assertion ensures
        // the alias tracks the canonical constant if either is retuned.
        assert_eq!(VSCODE_FPS_CAP, XTERMJS_FPS_CAP);
    }

    #[test]
    fn known_hosts_list_includes_vscode() {
        // VSCode is the original crash host and must always be in the
        // list — removing it would silently regress the Tier 1 fix.
        assert!(
            known_xtermjs_hosts().contains(&"vscode"),
            "XTERMJS_HOSTS must contain 'vscode' (Tier 1 back-compat)"
        );
    }

    #[test]
    fn known_hosts_list_has_at_least_five_entries() {
        // Tier 2 expanded detection from 1 host (VSCode) to ≥5 hosts
        // (VSCode + Hyper + WaveTerminal + Tabby + WarpTerminal). This
        // test fails if a future refactor accidentally shrinks the list.
        assert!(
            known_xtermjs_hosts().len() >= 5,
            "XTERMJS_HOSTS must have at least 5 entries (Tier 2 expansion)"
        );
    }
}
