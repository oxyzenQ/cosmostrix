// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Terminal restore + reset + blank cell helpers — extracted from
//! `terminal/mod.rs` to keep that file under the 800-LOC hard cap
//! (see `src/RULES_LOC.md`).
//!
//! Owns:
//! - `restore_terminal_best_effort()`: graceful terminal restore
//!   (disable mouse capture, focus change, bracketed paste, reset
//!   attributes + color). Called on normal exit + panic hook.
//! - `reset_terminal_emergency()`: 5-layer nuclear reset for
//!   `--reset-terminal` (ANSI restore + ANSI reset + crossterm +
//!   stty sane + reset + tput).
//! - `blank_cell()`: empty Cell with optional bg.
//! - `TERMINAL_RESTORE_SEQUENCE` + `TERMINAL_RESET_SEQUENCE` constants.
//!
//! Re-exported from `terminal/mod.rs` via `pub(crate) use` so all
//! existing `crate::terminal::restore_terminal_best_effort()` +
//! `crate::terminal::reset_terminal_emergency()` + `crate::terminal::
//! blank_cell()` call sites resolve unchanged.

#[cfg(unix)]
use std::io::{stdin, IsTerminal};
use std::io::{stdout, Write};
#[cfg(unix)]
use std::process::Command;

use crossterm::cursor;
use crossterm::event;
use crossterm::style::{Attribute, Color, ResetColor, SetAttribute};
use crossterm::terminal as crossterm_terminal;
use crossterm::ExecutableCommand as _;

use crate::cell::Cell;

#[cold]
pub(crate) fn restore_terminal_best_effort() {
    let mut out = stdout();
    let _ = out.execute(event::DisableMouseCapture);
    let _ = out.execute(event::DisableFocusChange);
    let _ = out.execute(event::DisableBracketedPaste);
    let _ = out.write_all(TERMINAL_RESTORE_SEQUENCE.as_bytes());
    let _ = out.execute(SetAttribute(Attribute::Reset));
    let _ = out.execute(ResetColor);
    let _ = out.execute(cursor::Show);
    let _ = out.execute(crossterm_terminal::EnableLineWrap);
    // Only leave alternate screen if the terminal supports it.
    // The Linux virtual console (TERM=linux) DOES support the alt screen
    // buffer (\x1b[?1049l via vt.c, kernel 2.6.x+) — see termdetect.rs
    // for the full history. Only `dumb` terminals and an unset TERM
    // lack alt screen support. TERMINAL_RESTORE_SEQUENCE already includes
    // \x1b[?1049l, so crossterm's LeaveAlternateScreen is redundant in
    // most cases, but crossterm may do additional internal state cleanup.
    let term = std::env::var("TERM").unwrap_or_default();
    let has_alt = !term.eq_ignore_ascii_case("dumb") && !term.is_empty();
    if has_alt {
        let _ = out.execute(crossterm_terminal::LeaveAlternateScreen);
    }
    let _ = crossterm_terminal::disable_raw_mode();
    let _ = out.flush();
}

/// Best-effort terminal restore sequence.
///
/// Disables all optional terminal modes that cosmostrix may have enabled:
/// - Mouse reporting (1000, 1002, 1003, 1006, 1015)
/// - Bracketed paste (2004)
/// - Focus events (1004)
/// - Kitty keyboard protocol (`\x1b[<1u` — pop DISAMBIGUATE_ESCAPE_CODES)
///   Added v50: panic recovery must pop kitty keyboard flags, otherwise the
///   user's shell receives CSI-u sequences for every keypress (arrow keys,
///   Home/End, etc. break) until they reset their terminal. The pop is a
///   no-op on terminals that never had a push (kitty protocol is a stack —
///   popping an empty stack is well-defined as a no-op on compliant
///   terminals; on non-compliant terminals the bytes are inert control
///   sequences that get discarded).
/// - Alternate screen (1049)
/// - Synchronized output (2026) — added v15, prevents stuck sync mode
/// - Cursor hide (25 → show)
/// - SGR reset (0m)
///
/// Also resets:
/// - Scroll region to full screen (`\x1b[r`) — in case cosmostrix set one
/// - Character set to US ASCII (`\x1b(B`) — in case cosmostrix changed it
/// - Auto-wrap enabled (`\x1b[?7h`) — in case it was disabled
///
/// Does NOT clear screen or scrollback — that's the destructive
/// TERMINAL_RESET_SEQUENCE used only by --reset-terminal.
pub(crate) const TERMINAL_RESTORE_SEQUENCE: &str = "\x1b[0m\
     \x1b[?2026l\
     \x1b[<1u\
     \x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[?1015l\
     \x1b[?2004l\
     \x1b[?1004l\
     \x1b[?1049l\
     \x1b[r\
     \x1b(B\
     \x1b[?7h\
     \x1b[?25h\
     \x1b[0m";

/// Destructive terminal reset sequence (used by --reset-terminal).
///
/// Includes everything in TERMINAL_RESTORE_SEQUENCE plus:
/// - Cursor home (`\x1b[H`)
/// - Clear screen (`\x1b[2J`)
/// - Clear scrollback (`\x1b[3J`)
/// - Cursor home again (after clear)
///
/// This is the "nuclear option" — it wipes the visible screen AND the
/// scrollback buffer. Use only when the terminal is in a broken state
/// (e.g., after SIGKILL left cosmostrix's alternate screen + raw mode
/// active and the user can't see their shell prompt).
pub(crate) const TERMINAL_RESET_SEQUENCE: &str = "\x1b[0m\
     \x1b[?2026l\
     \x1b[<1u\
     \x1b[?1000l\x1b[?1002l\x1b[?1003l\x1b[?1006l\x1b[?1015l\
     \x1b[?2004l\
     \x1b[?1004l\
     \x1b[?1049l\
     \x1b[r\
     \x1b(B\
     \x1b[?7h\
     \x1b[?25h\
     \x1b[H\x1b[2J\x1b[3J\x1b[H\
     \x1b[0m";

/// Emergency terminal reset — the "nuclear option" for broken terminals.
///
/// Called by `--reset-terminal`. This is the most aggressive recovery
/// path, used when cosmostrix (or any terminal app) was killed with
/// SIGKILL and left the terminal in a broken state:
/// - Alternate screen still active (rain visible, shell hidden)
/// - Raw mode still on (no echo, can't type commands)
/// - Mouse reporting still on (clicks do weird things)
/// - Cursor hidden
/// - Synchronized output stuck (terminal buffers output)
///
/// Recovery sequence (defense-in-depth, 5 layers):
///
/// 1. **ANSI restore sequence** — disables all optional modes
/// 2. **ANSI reset sequence** — clears screen + scrollback + cursor home
/// 3. **crossterm commands** — LeaveAlternateScreen, Clear, Show cursor
/// 4. **stty sane** — restores terminal line discipline (raw mode off)
/// 5. **reset** — external utility that does a full terminal reset
///    (clears screen, resets modes, restores tabs, etc.)
///
/// Each layer is best-effort — failures are silently ignored because
/// the terminal may be in a state where some operations don't work.
/// The goal is maximum recovery probability, not perfection.
pub(crate) fn reset_terminal_emergency() {
    // Layer 1: ANSI restore sequence (disable all optional modes)
    restore_terminal_best_effort();

    let mut out = stdout();

    // Layer 2: ANSI reset sequence (clear screen + scrollback)
    let _ = out.write_all(TERMINAL_RESET_SEQUENCE.as_bytes());
    let _ = out.flush();

    // Layer 3: crossterm commands (redundant with ANSI but belt-and-suspenders)
    let _ = out.execute(SetAttribute(Attribute::Reset));
    let _ = out.execute(ResetColor);
    let _ = out.execute(cursor::Show);
    let _ = out.execute(crossterm_terminal::LeaveAlternateScreen);
    let _ = out.execute(cursor::MoveTo(0, 0));
    let _ = out.execute(crossterm_terminal::Clear(
        crossterm_terminal::ClearType::All,
    ));
    let _ = out.execute(crossterm_terminal::Clear(
        crossterm_terminal::ClearType::Purge,
    ));
    let _ = out.execute(cursor::MoveTo(0, 0));
    let _ = out.execute(crossterm_terminal::EnableLineWrap);
    let _ = out.flush();

    // Layer 4+5: external utilities (Unix only)
    #[cfg(unix)]
    {
        if stdin().is_terminal() || stdout().is_terminal() {
            // stty sane: restores terminal line discipline (raw mode off,
            // echo on, canonical mode on, etc.). This is critical after
            // SIGKILL because raw mode persists in the kernel's termios
            // state — ANSI escapes alone cannot fix it.
            let _ = Command::new("stty").arg("sane").status();

            // reset: full external terminal reset utility. Clears screen,
            // resets modes, restores tab stops, sends terminal init string.
            // May not exist on all systems (embedded, minimal containers)
            // — failure is silently ignored.
            let _ = Command::new("reset").status();

            // tput reset: alternative if 'reset' not available. Uses
            // terminfo database to send the appropriate reset sequences.
            // Some systems have tput but not reset.
            let _ = Command::new("tput").arg("reset").status();
        }
    }
}

#[must_use]
pub(crate) fn blank_cell(bg: Option<Color>) -> Cell {
    Cell {
        ch: ' ',
        fg: None,
        bg,
        bold: false,
    }
}
