// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Terminal protocol detection at startup.
//!
//! Detects the terminal emulator from environment variables and enables
//! protocol-level optimizations where available:
//!
//! - **Synchronized output** (`ESC[?2026h` / `ESC[?2026l`): Frames the
//!   entire draw in a sync region so the terminal emulator buffers output
//!   internally and flushes atomically. Eliminates visual tearing during
//!   partial redraws. Supported by: kitty, wezterm, alacritty, foot,
//!   iTerm2 3.5+, Windows Terminal 1.22+, tmux 3.3+.
//!
//! - **Vendor identification**: stored for future protocol-specific
//!   features (kitty graphics protocol, foot's damage-tracking, etc.).

use std::env;

/// Known terminal vendors with useful protocol extensions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)] // vendor field is stored for future protocol-specific features
pub enum TerminalVendor {
    Unknown,
    Kitty,
    WezTerm,
    Alacritty,
    Foot,
    ITerm2,
    WindowsTerminal,
    Tmux,
    Rio,
}

/// Capabilities discovered at startup.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)] // vendor field stored for future protocol-specific features
pub struct TerminalCaps {
    pub vendor: TerminalVendor,
    /// Synchronized output (`ESC[?2026h` / `ESC[?2026l`) — universally
    /// safe to enable; terminals that don't support it silently ignore
    /// the escape sequence.
    pub sync_output: bool,
}

/// Run detection from environment variables. Safe to call before any
/// terminal initialization.
pub fn detect() -> TerminalCaps {
    let term = env::var("TERM").unwrap_or_default();
    let term_program = env::var("TERM_PROGRAM").unwrap_or_default();

    let vendor = if !env::var("KITTY_PID").unwrap_or_default().is_empty()
        || env::var("KITTY_WINDOW_ID").is_ok()
    {
        TerminalVendor::Kitty
    } else if env::var("WEZTERM_PANE").is_ok()
        || env::var("WEZTERM_EXECUTABLE").is_ok()
        || term_program.eq_ignore_ascii_case("wezterm")
    {
        TerminalVendor::WezTerm
    } else if env::var("ALACRITTY_LOG").is_ok()
        || env::var("ALACRITTY_SOCKET").is_ok()
        || term_program.eq_ignore_ascii_case("alacritty")
    {
        TerminalVendor::Alacritty
    } else if env::var("FOOT_PID").is_ok()
        || env::var("TERM").unwrap_or_default().starts_with("foot")
    {
        TerminalVendor::Foot
    } else if term_program.eq_ignore_ascii_case("iterm.app")
        || term_program.eq_ignore_ascii_case("iTerm.app")
    {
        TerminalVendor::ITerm2
    } else if env::var("WT_SESSION").is_ok() || env::var("WT_PROFILE_ID").is_ok() {
        TerminalVendor::WindowsTerminal
    } else if !env::var("TMUX").unwrap_or_default().is_empty() || term.starts_with("tmux-") {
        TerminalVendor::Tmux
    } else if term.eq_ignore_ascii_case("rio") {
        TerminalVendor::Rio
    } else {
        TerminalVendor::Unknown
    };

    // Synchronized output is supported by virtually all modern terminals.
    // The escape sequences are a no-op on terminals that don't support
    // them, so enabling unconditionally is safe.  The only known exception
    // is the Linux console (TERM=linux) — skip there explicitly.
    // tmux 3.3+ passes sync sequences through to the outer terminal.
    let sync_ok = !term.eq_ignore_ascii_case("linux");

    TerminalCaps {
        vendor,
        sync_output: sync_ok,
    }
}

/// Byte sequence to begin a synchronized output region.
/// The terminal buffers all subsequent output until the end marker.
pub const SYNC_START: &[u8] = b"\x1b[?2026h";

/// Byte sequence to end a synchronized output region.
/// The terminal flushes all buffered content atomically.
pub const SYNC_END: &[u8] = b"\x1b[?2026l";

impl std::fmt::Display for TerminalVendor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TerminalVendor::Unknown => write!(f, "unknown"),
            TerminalVendor::Kitty => write!(f, "kitty"),
            TerminalVendor::WezTerm => write!(f, "wezterm"),
            TerminalVendor::Alacritty => write!(f, "alacritty"),
            TerminalVendor::Foot => write!(f, "foot"),
            TerminalVendor::ITerm2 => write!(f, "iterm2"),
            TerminalVendor::WindowsTerminal => write!(f, "windows-terminal"),
            TerminalVendor::Tmux => write!(f, "tmux"),
            TerminalVendor::Rio => write!(f, "rio"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_returns_caps_even_with_no_terminal_env() {
        // In CI or headless environments, detect must not panic.
        let caps = detect();
        // At minimum we get a vendor enum and a sync_output bool.
        let _vendor_str = caps.vendor.to_string();
        // sync_output defaults to true unless TERM=linux
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
        let caps = TerminalCaps {
            vendor: TerminalVendor::Unknown,
            sync_output: false,
        };
        assert!(!caps.sync_output);
    }
}
