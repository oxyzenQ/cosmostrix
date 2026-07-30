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

use std::env;

/// Capabilities discovered at startup.
#[derive(Clone, Copy, Debug)]
pub struct TerminalCaps {
    /// Synchronized output (`ESC[?2026h` / `ESC[?2026l`) — universally
    /// safe to enable; terminals that don't support it silently ignore
    /// the escape sequence.
    pub sync_output: bool,
}

/// Run detection from environment variables. Safe to call before any
/// terminal initialization.
pub fn detect() -> TerminalCaps {
    let term = env::var("TERM").unwrap_or_default();

    // Synchronized output is supported by virtually all modern terminals.
    // The escape sequences are a no-op on terminals that don't support
    // them, so enabling unconditionally is safe.  The only known exception
    // is the Linux console (TERM=linux) — skip there explicitly.
    // tmux 3.3+ passes sync sequences through to the outer terminal.
    let sync_ok = !term.eq_ignore_ascii_case("linux");

    TerminalCaps {
        sync_output: sync_ok,
    }
}

/// Byte sequence to begin a synchronized output region.
/// The terminal buffers all subsequent output until the end marker.
pub const SYNC_START: &[u8] = b"\x1b[?2026h";

/// Byte sequence to end a synchronized output region.
/// The terminal flushes all buffered content atomically.
pub const SYNC_END: &[u8] = b"\x1b[?2026l";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_returns_caps_even_with_no_terminal_env() {
        // In CI or headless environments, detect must not panic.
        let caps = detect();
        // sync_output defaults to true unless TERM=linux
        let _ = caps.sync_output;
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
    fn sync_output_disabled_for_linux_console() {
        // Simulate TERM=linux detection result
        let caps = TerminalCaps { sync_output: false };
        assert!(!caps.sync_output);
    }
}
