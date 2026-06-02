// Copyright (c) 2026 rezky_nightky

//! Keyboard input handling and paste burst detection.
//!
//! Processes key events through the keybinding dispatch table and filters
//! out rapid printable key events that occur during bracketed paste
//! sequences (which arrive as individual Key events on some terminals).

use std::time::{Duration, Instant};

#[cfg(unix)]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(unix)]
use std::sync::Arc;

#[cfg(unix)]
use signal_hook::consts::SIGSTOP;
#[cfg(unix)]
use signal_hook::low_level;

use crate::charset::{build_chars, charset_from_str};
use crate::cloud::Cloud;
use crate::constants::*;
use crate::frame::Frame;
use crate::runtime::ColorScheme;
#[cfg(unix)]
use crate::terminal::restore_terminal_best_effort;

use super::super::{cycle_charset_preset, cycle_color_scheme, CloudConfig};
use super::watchdog::MOUSE_CAPTURE_ACTIVE;

const PASTE_BURST_SUPPRESS_MS: u64 = 50;

#[derive(Default)]
pub(super) struct PasteBurstGuard {
    suppress_until: Option<Instant>,
}

impl PasteBurstGuard {
    pub(super) fn ignore_plain_key(
        &mut self,
        key: &crossterm::event::KeyEvent,
        now: Instant,
        queued_event_ready: bool,
    ) -> bool {
        if !is_plain_printable_key(key) {
            return false;
        }

        if self.suppress_until.is_some_and(|until| now <= until) || queued_event_ready {
            self.suppress_until = Some(now + Duration::from_millis(PASTE_BURST_SUPPRESS_MS));
            true
        } else {
            false
        }
    }

    pub(super) fn note_bracketed_paste(&mut self, now: Instant) {
        self.suppress_until = Some(now + Duration::from_millis(PASTE_BURST_SUPPRESS_MS));
    }
}

pub(super) fn is_plain_printable_key(key: &crossterm::event::KeyEvent) -> bool {
    use crossterm::event::{KeyCode, KeyModifiers};

    matches!(key.code, KeyCode::Char(_))
        && (key.modifiers.is_empty()
            || key.modifiers == KeyModifiers::SHIFT
            || key.modifiers == KeyModifiers::NONE)
}

#[cfg(test)]
#[allow(dead_code)]
pub(super) const PASTE_BURST_SUPPRESS_MS_FOR_TEST: u64 = PASTE_BURST_SUPPRESS_MS;

#[allow(clippy::too_many_arguments)]
pub(super) fn handle_keybinding(
    cloud: &mut Cloud,
    frame: &mut Frame,
    k: &crossterm::event::KeyEvent,
    charset_preset: &mut String,
    user_ranges: &[(char, char)],
    def_ascii: bool,
    _cfg: &CloudConfig,
    #[cfg(unix)] term_reinit: &Arc<AtomicBool>,
) -> bool {
    use crossterm::event::KeyCode;
    use crossterm::event::KeyModifiers;

    match (k.code, k.modifiers) {
        (KeyCode::Esc, _) => cloud.raining = false,
        (KeyCode::Char('q'), _) => cloud.raining = false,
        (KeyCode::Char('z'), KeyModifiers::CONTROL) => {
            #[cfg(unix)]
            {
                // Disable mouse capture before suspending so the terminal
                // is not left with mouse reporting active while cosmostrix
                // is in the background.
                if MOUSE_CAPTURE_ACTIVE.load(Ordering::Acquire) {
                    use crossterm::ExecutableCommand;
                    let _ = std::io::stdout().execute(crossterm::event::DisableMouseCapture);
                    MOUSE_CAPTURE_ACTIVE.store(false, Ordering::Release);
                }
                restore_terminal_best_effort();
                term_reinit.store(true, Ordering::Release);
                let _ = low_level::raise(SIGSTOP);
            }
        }
        (KeyCode::Char(' '), _) => {
            cloud.reset(frame.width, frame.height);
            cloud.force_draw_everything();
        }
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            cloud.raining = false;
        }
        (KeyCode::Char('c'), KeyModifiers::NONE) => {
            let next = cycle_color_scheme(cloud.color_scheme(), 1);
            cloud.set_color_scheme(next);
        }
        (KeyCode::Char('C'), _) => {
            let prev = cycle_color_scheme(cloud.color_scheme(), -1);
            cloud.set_color_scheme(prev);
        }
        (KeyCode::Char('s'), _) => {
            let next = cycle_charset_preset(charset_preset, 1);
            *charset_preset = next.to_string();
            if let Ok(cs) = charset_from_str(charset_preset, def_ascii) {
                let chars = build_chars(cs, user_ranges, def_ascii);
                cloud.transition_chars(chars);
            }
        }
        (KeyCode::Char('S'), _) => {
            let prev = cycle_charset_preset(charset_preset, -1);
            *charset_preset = prev.to_string();
            if let Ok(cs) = charset_from_str(charset_preset, def_ascii) {
                let chars = build_chars(cs, user_ranges, def_ascii);
                cloud.transition_chars(chars);
            }
        }
        (KeyCode::Char('a'), _) => {
            cloud.set_async(!cloud.async_mode);
        }
        (KeyCode::Char('g'), _) => {
            cloud.set_glitchy(!cloud.glitchy);
        }
        (KeyCode::Char('p'), _) => {
            return cloud.toggle_pause();
        }
        (KeyCode::Char('m'), _) => {
            cloud.cycle_profile();
        }
        (KeyCode::Up, _) => {
            let mut cps = cloud.chars_per_sec;
            if cps <= 0.5 {
                cps *= 2.0;
            } else {
                cps += 1.0;
            }
            cloud.set_chars_per_sec(cps.min(1000.0));
        }
        (KeyCode::Down, _) => {
            let mut cps = cloud.chars_per_sec;
            if cps <= 1.0 {
                cps /= 2.0;
            } else {
                cps -= 1.0;
            }
            cloud.set_chars_per_sec(cps.max(0.001));
        }
        (KeyCode::Left, _) if cloud.glitchy => {
            let gp = (cloud.glitch_pct - GLITCH_PCT_STEP).max(0.0);
            cloud.set_glitch_pct(gp);
        }
        (KeyCode::Right, _) if cloud.glitchy => {
            let gp = (cloud.glitch_pct + GLITCH_PCT_STEP).min(1.0);
            cloud.set_glitch_pct(gp);
        }
        (KeyCode::Tab, _) | (KeyCode::BackTab, _) => {
            // Tab and Shift+Tab are explicitly ignored. Previously Tab
            // toggled shading mode, which called set_shading_mode() →
            // semantic_invalidate → invalidate_semantic() → frame clear
            // without clearing phosphor_base_ch, causing a ghost background
            // glyph flood. Tab is not a useful shortcut for a terminal rain
            // renderer, so it is safely ignored to prevent this class of bug.
        }
        (KeyCode::Char('-'), _) | (KeyCode::Char('['), _) | (KeyCode::Char('_'), _) => {
            let d = (cloud.droplet_density - DENSITY_STEP).max(0.01);
            cloud.set_droplet_density(d);
        }
        (KeyCode::Char('+'), _)
        | (KeyCode::Char('='), KeyModifiers::SHIFT)
        | (KeyCode::Char(']'), _) => {
            let d = (cloud.droplet_density + DENSITY_STEP).min(5.0);
            cloud.set_droplet_density(d);
        }
        (KeyCode::Char('1'), _) => cloud.set_color_scheme(ColorScheme::Green),
        (KeyCode::Char('2'), _) => cloud.set_color_scheme(ColorScheme::Green2),
        (KeyCode::Char('3'), _) => cloud.set_color_scheme(ColorScheme::Green3),
        (KeyCode::Char('4'), _) => cloud.set_color_scheme(ColorScheme::Gold),
        (KeyCode::Char('5'), _) => cloud.set_color_scheme(ColorScheme::Neon),
        (KeyCode::Char('6'), _) => cloud.set_color_scheme(ColorScheme::Red),
        (KeyCode::Char('7'), _) => cloud.set_color_scheme(ColorScheme::Blue),
        (KeyCode::Char('8'), _) => cloud.set_color_scheme(ColorScheme::Cyan),
        (KeyCode::Char('9'), _) => cloud.set_color_scheme(ColorScheme::Purple),
        (KeyCode::Char('0'), _) => cloud.set_color_scheme(ColorScheme::Gray),
        (KeyCode::Char('!'), _) => cloud.set_color_scheme(ColorScheme::Rainbow),
        (KeyCode::Char('@'), _) => cloud.set_color_scheme(ColorScheme::Yellow),
        (KeyCode::Char('#'), _) => cloud.set_color_scheme(ColorScheme::Orange),
        (KeyCode::Char('$'), _) => cloud.set_color_scheme(ColorScheme::Fire),
        (KeyCode::Char('%'), _) => cloud.set_color_scheme(ColorScheme::Vaporwave),
        _ => {}
    }

    false
}
