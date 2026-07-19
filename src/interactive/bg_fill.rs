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
    use crossterm::terminal;
    use std::io::Write;
    let mut out = std::io::stdout();
    // Set bg + clear screen first (fast path for terminals that respect it)
    let _ = execute!(out, SetBackgroundColor(bg));
    let _ = execute!(out, terminal::Clear(terminal::ClearType::All));
    // Then actively write spaces to every row to guarantee coverage.
    let (w, h) = crossterm::terminal::size().unwrap_or((80, 24));
    let spaces = " ".repeat(w as usize);
    for y in 0..h {
        let _ = execute!(out, MoveTo(0, y));
        let _ = out.write_all(spaces.as_bytes());
    }
    let _ = out.flush();
}
