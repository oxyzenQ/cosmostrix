// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Terminal background fill helper (v16).
//!
//! Ensures the entire terminal screen is filled with a custom background
//! color, including edges and margins that the frame doesn't cover.

use crossterm::style::Color;

/// Fill the entire terminal screen with a background color.
///
/// Sets the SGR bg color, then writes spaces to every cell on the screen.
/// This is more reliable than Clear(All) alone — some terminals don't
/// fill cleared cells with the current bg color. By actively writing
/// spaces with the bg SGR set, every cell is guaranteed to get the
/// correct background, including edges, margins, and status lines.
pub(crate) fn fill_terminal_bg(bg: Option<Color>) {
    let Some(bg) = bg else { return };
    use crossterm::cursor::MoveTo;
    use crossterm::execute;
    use crossterm::style::SetBackgroundColor;
    use std::io::Write;
    let mut out = std::io::stdout();
    // Set bg color, then write spaces to every cell.
    // Scrollback-safe: avoid Clear(ClearType::All) here — issuing \x1b[2J
    // inside the alternate screen can set an internal flag on VTE-based
    // terminals that causes the main screen's scrollback to be cleared
    // when LeaveAlternateScreen is later called. The space-fill loop
    // below already covers every cell, so Clear(All) is redundant.
    let _ = execute!(out, SetBackgroundColor(bg));
    // Write spaces to every row to guarantee coverage.
    let (w, h) = crossterm::terminal::size().unwrap_or((80, 24));
    let spaces = " ".repeat(w as usize);
    for y in 0..h {
        let _ = execute!(out, MoveTo(0, y));
        let _ = out.write_all(spaces.as_bytes());
    }
    let _ = out.flush();
}
