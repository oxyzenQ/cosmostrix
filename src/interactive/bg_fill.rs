// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Terminal background fill helper (v16).
//!
//! Originally filled the entire terminal screen with a custom background
//! color using spaces or Clear(All). Now a no-op for scrollback safety.
//!
//! ## Why no-op
//!
//! Writing to every cell of the alternate screen (whether via \x1b[2J
//! or a space-fill loop) can set an internal flag on VTE-based terminals
//! (GNOME Terminal, xfce4-terminal, Alacritty <0.12) that causes the
//! main screen's scrollback to be cleared when LeaveAlternateScreen is
//! later called. The renderer's full redraw already paints every cell
//! with the correct background color on the first frame and after resize,
//! making this pre-fill redundant and a scrollback hazard.

use crossterm::style::Color;

/// Fill the entire terminal screen with a background color.
///
/// **Scrollback-safe no-op**: The renderer's full redraw already sets
/// the correct background on every cell. Pre-filling the alternate screen
/// (via Clear(All) or space-write) risks clearing the main screen's
/// scrollback on VTE-based terminals when LeaveAlternateScreen is called.
#[inline]
pub(crate) fn fill_terminal_bg(_bg: Option<Color>) {
    // No-op: renderer handles background fill via full redraw.
}
