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
//! - **VSCode integrated terminal detection** (`TERM_PROGRAM=vscode`):
//!   VSCode's xterm.js terminal emulator cannot sustain high frame rates
//!   indefinitely. At 60 FPS, cosmostrix pumps 0.3-13.7 MB/sec of ANSI
//!   bytes into node-pty → xterm.js, whose in-memory buffer grows without
//!   bound over multi-hour runs until V8 hits an OOM assertion (SIGTRAP).
//!   When VSCode is detected, synchronized output is disabled (xterm.js's
//!   mode 2026 implementation amplifies memory pressure) and a default
//!   FPS cap is applied (see `vscode_fps_cap`). The crash was reported
//!   2026-08-04 (code-oss Signal 5 TRAP after hours of cosmostrix).

use std::env;

/// Capabilities discovered at startup.
#[derive(Clone, Copy, Debug)]
pub(crate) struct TerminalCaps {
    /// Synchronized output (`ESC[?2026h` / `ESC[?2026l`) — universally
    /// safe to enable; terminals that don't support it silently ignore
    /// the escape sequence.
    pub sync_output: bool,
    /// True when running inside VSCode's integrated terminal
    /// (`TERM_PROGRAM=vscode`). Disables sync_output and applies a
    /// default FPS cap to prevent xterm.js OOM crashes over long runs.
    pub vscode_integrated: bool,
    /// Maximum recommended FPS for this terminal. Native terminals
    /// (Alacritty, Kitty, etc.) get 240 (effectively uncapped — the
    /// user's --fps value wins). VSCode's xterm.js gets 30 to keep
    /// the ANSI byte rate under ~7 MB/sec worst case.
    pub default_fps_cap: f64,
}

/// FPS cap applied when running inside VSCode's integrated terminal.
/// xterm.js's in-memory buffer grows unbounded at high frame rates;
/// 30 FPS keeps the worst-case byte rate under ~7 MB/sec (vs ~13.7 MB/sec
/// at 60 FPS), which xterm.js can drain over multi-hour runs without
/// OOMing. The user's --fps value is clamped to this cap, not overridden
/// silently — the verbose output discloses the cap so there's no confusion.
const VSCODE_FPS_CAP: f64 = 30.0;

/// Run detection from environment variables. Safe to call before any
/// terminal initialization.
pub(crate) fn detect() -> TerminalCaps {
    let term = env::var("TERM").unwrap_or_default();
    let term_program = env::var("TERM_PROGRAM").unwrap_or_default();

    // Detect VSCode's integrated terminal. VSCode sets TERM_PROGRAM=vscode.
    // (Other Electron-based editors like Hyper set TERM_PROGRAM=Hyper;
    // those are not capped because the crash report is VSCode-specific.)
    let vscode_integrated = term_program == "vscode";

    // Synchronized output is supported by virtually all modern terminals.
    // The escape sequences are a no-op on terminals that don't support
    // them, so enabling unconditionally is safe. Three exceptions:
    //   1. Linux console (TERM=linux) — does not understand the sequence.
    //   2. VSCode integrated terminal — xterm.js's mode 2026 buffer
    //      implementation amplifies memory pressure under high frame
    //      rates, contributing to the multi-hour SIGTRAP crash.
    // tmux 3.3+ passes sync sequences through to the outer terminal.
    let sync_ok = !term.eq_ignore_ascii_case("linux") && !vscode_integrated;

    // VSCode gets a 30 FPS cap; everything else is effectively uncapped
    // (the user's --fps value, validated to 1.0..=240.0, wins).
    let default_fps_cap = if vscode_integrated {
        VSCODE_FPS_CAP
    } else {
        240.0
    };

    TerminalCaps {
        sync_output: sync_ok,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // env::set_var is process-global and not thread-safe; serialize the
    // tests that touch TERM_PROGRAM so they don't race with each other.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

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
    fn sync_output_disabled_for_linux_console() {
        // Simulate TERM=linux detection result
        let caps = TerminalCaps {
            sync_output: false,
            vscode_integrated: false,
            default_fps_cap: 240.0,
        };
        assert!(!caps.sync_output);
    }

    #[test]
    fn vscode_detection_when_term_program_is_vscode() {
        let _guard = ENV_LOCK.lock().unwrap();
        let prev_term = env::var("TERM").ok();
        let prev_tp = env::var("TERM_PROGRAM").ok();
        env::set_var("TERM", "xterm-256color");
        env::set_var("TERM_PROGRAM", "vscode");
        let caps = detect();
        assert!(caps.vscode_integrated, "vscode_integrated must be true");
        assert!(
            !caps.sync_output,
            "sync_output must be disabled for VSCode (xterm.js OOM amplification)"
        );
        assert_eq!(caps.default_fps_cap, VSCODE_FPS_CAP);
        // Restore
        match prev_term {
            Some(v) => env::set_var("TERM", v),
            None => env::remove_var("TERM"),
        }
        match prev_tp {
            Some(v) => env::set_var("TERM_PROGRAM", v),
            None => env::remove_var("TERM_PROGRAM"),
        }
    }

    #[test]
    fn vscode_detection_false_for_other_terminals() {
        let _guard = ENV_LOCK.lock().unwrap();
        let prev_term = env::var("TERM").ok();
        let prev_tp = env::var("TERM_PROGRAM").ok();
        env::set_var("TERM", "xterm-256color");
        env::set_var("TERM_PROGRAM", "alacritty");
        let caps = detect();
        assert!(!caps.vscode_integrated);
        assert!(caps.sync_output, "sync_output must stay on for non-VSCode");
        assert_eq!(caps.default_fps_cap, 240.0);
        match prev_term {
            Some(v) => env::set_var("TERM", v),
            None => env::remove_var("TERM"),
        }
        match prev_tp {
            Some(v) => env::set_var("TERM_PROGRAM", v),
            None => env::remove_var("TERM_PROGRAM"),
        }
    }

    #[test]
    fn vscode_detection_false_when_term_program_unset() {
        let _guard = ENV_LOCK.lock().unwrap();
        let prev_term = env::var("TERM").ok();
        let prev_tp = env::var("TERM_PROGRAM").ok();
        env::set_var("TERM", "xterm-256color");
        env::remove_var("TERM_PROGRAM");
        let caps = detect();
        assert!(!caps.vscode_integrated);
        match prev_term {
            Some(v) => env::set_var("TERM", v),
            None => env::remove_var("TERM"),
        }
        match prev_tp {
            Some(v) => env::set_var("TERM_PROGRAM", v),
            None => env::remove_var("TERM_PROGRAM"),
        }
    }

    #[test]
    fn detect_does_not_panic_with_empty_term() {
        let _guard = ENV_LOCK.lock().unwrap();
        let prev_term = env::var("TERM").ok();
        let prev_tp = env::var("TERM_PROGRAM").ok();
        env::remove_var("TERM");
        env::remove_var("TERM_PROGRAM");
        let caps = detect();
        assert!(!caps.vscode_integrated);
        assert!(caps.sync_output, "empty TERM should still enable sync");
        match prev_term {
            Some(v) => env::set_var("TERM", v),
            None => env::remove_var("TERM"),
        }
        match prev_tp {
            Some(v) => env::set_var("TERM_PROGRAM", v),
            None => env::remove_var("TERM_PROGRAM"),
        }
    }
}
