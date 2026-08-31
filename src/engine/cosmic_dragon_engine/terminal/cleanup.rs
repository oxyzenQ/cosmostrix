// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Terminal cleanup — extracted from `terminal/mod.rs` to keep that
//! file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns `Terminal::cleanup_terminal()` — the full teardown sequence
//! that disables mouse capture, leaves alt screen, resets cursor, etc.
//! Called from Drop + signal handlers.

use std::io::Write;

use crossterm::cursor;
use crossterm::event;
use crossterm::style::{Attribute, ResetColor, SetAttribute};
use crossterm::terminal as crossterm_terminal;
use crossterm::ExecutableCommand as _;

impl super::Terminal {
    pub(super) fn cleanup_terminal(&mut self) {
        if self.cleaned_up {
            return;
        }
        self.cleaned_up = true;

        let _ = self.disable_mouse_capture();
        if self.bracketed_paste_enabled {
            let _ = self.stdout.execute(event::DisableBracketedPaste);
            self.bracketed_paste_enabled = false;
        }
        // Kitty keyboard protocol pop: must happen before
        // LeaveAlternateScreen so the protocol stack is unwound while
        // still on the alt screen (matches the SYNC_END ordering pattern).
        // PopKeyboardEnhancementFlags sends `CSI <1u` which removes the
        // DISAMBIGUATE_ESCAPE_CODES flag pushed at init. If we forgot
        // this pop, the terminal would stay in enhanced-keyboard mode
        // AFTER cosmostrix exits — the user's shell would receive CSI-u
        // sequences for every keypress, breaking arrow keys / Home /
        // End / etc. in their shell.
        //
        // Safe to call unconditionally even if the push failed: kitty
        // protocol is a stack, popping an empty stack is a no-op on
        // compliant terminals. But we gate on `kitty_keyboard_enabled`
        // to avoid emitting `CSI <1u` to terminals that never got a
        // push (which could otherwise misinterpret the bytes).
        if self.kitty_keyboard_enabled {
            let _ = self.stdout.execute(event::PopKeyboardEnhancementFlags);
            self.kitty_keyboard_enabled = false;
        }
        let _ = self.stdout.execute(SetAttribute(Attribute::Reset));
        let _ = self.stdout.execute(ResetColor);
        if self.cursor_hidden {
            let _ = self.stdout.execute(cursor::Show);
            self.cursor_hidden = false;
        }
        if self.line_wrap_disabled {
            let _ = self.stdout.execute(crossterm_terminal::EnableLineWrap);
            self.line_wrap_disabled = false;
        }

        if self.alternate_screen_enabled {
            // v50 scrollback fix: emit SYNC_END BEFORE LeaveAlternateScreen.
            //
            // The previous order was: LeaveAlternateScreen → SYNC_END.
            // That meant \x1b[?2026l (sync end) landed on the MAIN screen
            // (after the switch back from alt). Combined with the init-time
            // SYNC_START that also landed on the main screen (before
            // EnterAlternateScreen), sync mode was open on the main screen
            // for the entire session. When SYNC_END finally arrived after
            // LeaveAlternateScreen, VTE-based terminals and Alacritty
            // interpreted the buffered leave-alt-screen switch as a content
            // update and "displayed" it — destroying the main screen's
            // scrollback.
            //
            // The correct order (matching restore_terminal_best_effort() /
            // TERMINAL_RESTORE_SEQUENCE) is: SYNC_END → LeaveAlternateScreen.
            // This closes sync mode on the ALT screen (where the last frame
            // opened it) before switching back to the (untouched) main screen.
            // The main screen never sees a sync open or close, so its
            // scrollback is preserved.
            //
            // REMOVED Clear(All) before LeaveAlternateScreen.
            //
            // \x1b[2J inside the alternate screen can clear the main
            // screen's scrollback on some terminal emulators (VTE-based,
            // some xterm-direct implementations). LeaveAlternateScreen
            // alone properly restores the main screen buffer. The alternate
            // screen content (including any rain residue) is swapped out
            // and becomes invisible. No pre-clear is needed.
            if self.term_caps.sync_output {
                // Belt-and-suspenders: if the last frame's SYNC_END was
                // lost (write failure / partial flush), sync mode is stuck
                // open on the alt screen. Close it here BEFORE leaving the
                // alt screen, so the main screen is never touched by a
                // sync-end sequence.
                let _ = self.stdout.write_all(crate::termdetect::SYNC_END);
            }
            // v50 TTY scrollback fix: ALWAYS flush before LeaveAlternateScreen,
            // not just when sync_output is true. On TTY terminals (and
            // terminals where sync_output is false), the BufWriter may have
            // pending rain content from the last frame's draw() that wasn't
            // fully flushed. Without this flush, the pending content gets
            // sent to the terminal AFTER LeaveAlternateScreen (in the final
            // flush at the end of cleanup_terminal), which means it lands on
            // the MAIN screen — overwriting the user's terminal history.
            //
            // Flushing here ensures ALL pending content is sent to the alt
            // screen BEFORE the screen switch, so the main screen is
            // untouched when LeaveAlternateScreen reveals it.
            let _ = self.stdout.flush();
            let _ = self
                .stdout
                .execute(crossterm_terminal::LeaveAlternateScreen);
            self.alternate_screen_enabled = false;

            // v50.0.0-beta.6: removed MoveTo(0, h-1) that was causing blank
            // lines after exit. LeaveAlternateScreen already restores the
            // cursor to where it was before entering the alt screen (right
            // after the shell prompt). The previous MoveTo(0, h-1) moved the
            // cursor to the BOTTOM of the terminal, creating a large blank
            // gap between the shell prompt and any post-exit output (perf
            // report, verbose summary). On TTY terminals where the screen is
            // cleared as a side effect of alt screen switch, the cursor is
            // already at the top — a single newline ensures we're on a fresh
            // line without creating a gap. The flush AFTER LeaveAlternateScreen
            // ensures the screen-switch sequence is actually sent to the
            // terminal BEFORE raw mode is disabled.
            let _ = self.stdout.flush();
        } else if !self.term_caps.has_alternate_screen {
            // No alternate screen was entered (terminal doesn't support it).
            // We ran on the main screen directly. Scrollback-safe exit:
            // do NOT issue Clear(All) — \x1b[2J can clear scrollback on
            // many terminals (VTE-based, Alacritty < 0.12, some xterm
            // configs). The rain rendering scrolls into scrollback naturally,
            // which is the expected behavior — previous command output is
            // preserved.
            //
            // v50.0.0-beta.6 LTS audit: removed MoveTo(0, h-1) that was
            // creating blank lines after exit on dumb/non-alt-screen terminals
            // (same bug class as the alt-screen path fixed earlier). On these
            // terminals cosmostrix ran on the main screen, so the cursor is
            // already at the correct position (bottom of the rain output).
            // Moving it to the terminal's bottom row created a gap between
            // the last rain line and the shell prompt. No MoveTo needed —
            // the cursor stays where it is, and the shell prompt appears on
            // the next line naturally.
        }
        if self.raw_mode_enabled {
            let _ = crossterm_terminal::disable_raw_mode();
            self.raw_mode_enabled = false;
        }
        let _ = self.stdout.flush();
    }
}
