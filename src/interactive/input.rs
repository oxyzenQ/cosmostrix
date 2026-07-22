// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

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
use crate::rain_style::RainStyle;

use crate::scene;
#[cfg(unix)]
use crate::terminal::restore_terminal_best_effort;

use super::super::{cycle_charset_preset, cycle_color_scheme, CloudConfig};
#[cfg(unix)]
use super::watchdog::MOUSE_CAPTURE_ACTIVE;

const PASTE_BURST_SUPPRESS_MS: u64 = 50;

#[derive(Default)]
pub(super) struct PasteBurstGuard {
    suppress_until: Option<Instant>,
}

impl PasteBurstGuard {
    /// Returns true if the given plain-printable key should be silently
    /// dropped because we are inside a bracketed-paste suppression window.
    ///
    /// Only the bracketed-paste signal (`note_bracketed_paste`) arms the
    /// suppression window. We deliberately do NOT inspect the OS event queue
    /// for "another event is ready" — on modern terminals that emit
    /// Press+Release pairs (kitty / foot / wezterm / alacritty / contour /
    /// Windows Console), the Release event is always queued immediately
    /// after the Press, so a queue-ready check would drop every single
    /// printable key press. That made `L` (storm mode), `c` (color cycle),
    /// `s` (charset), `p` (pause), etc. unreachable on those terminals.
    pub(super) fn ignore_plain_key(
        &mut self,
        key: &crossterm::event::KeyEvent,
        now: Instant,
    ) -> bool {
        if !is_plain_printable_key(key) {
            return false;
        }

        if self.suppress_until.is_some_and(|until| now <= until) {
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

// Runtime key handling coordinates cloud, frame, scene, charset, and terminal
// recovery state in one dispatch point; splitting would obscure side effects.
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_keybinding(
    cloud: &mut Cloud,
    frame: &mut Frame,
    k: &crossterm::event::KeyEvent,
    charset_preset: &mut String,
    scene_name: &mut String,
    user_ranges: &[(char, char)],
    def_ascii: bool,
    _cfg: &CloudConfig,
    #[cfg(unix)] term_reinit: &Arc<AtomicBool>,
) -> bool {
    use crossterm::event::KeyCode;
    use crossterm::event::KeyModifiers;

    // Quit policy: only 'q' exits. Esc, Ctrl+C, and any other unrecognized
    // key are silently ignored (fall through to the `_ => {}` arm at the
    // end of this match). This prevents accidental exits from terminal
    // menu Esc, Ctrl+C muscle memory, or stray function keys. The user
    // must press 'q' deliberately to quit.
    match (k.code, k.modifiers) {
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
            // Restart message typewriter so Space gives a full cinematic
            // replay — rain reseed + message types out from scratch.
            cloud.restart_message_typewriter();
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
        // v17: 'a' (toggle async) REMOVED. Async is always on; use --uniform
        // to disable. The 'a' key now falls through to the _ => {} catch-all
        // (silently ignored, like all other unrecognized keys).
        (KeyCode::Char('g'), _) => {
            // v18: cycle glitch intensity (off → default → intense → off).
            // The old toggle (glitchy on/off) was invisible at Subtle level
            // (3%). Cycling through visible levels makes the key useful.
            let pct = cloud.glitch_pct();
            if !cloud.glitchy || pct < 0.05 {
                // off or subtle → default (10%)
                cloud.set_glitchy(true);
                cloud.set_glitch_pct(0.10);
            } else if pct < 0.20 {
                // default (10%) → intense (25%)
                cloud.set_glitchy(true);
                cloud.set_glitch_pct(0.25);
            } else {
                // intense (25%) → off
                cloud.set_glitchy(false);
            }
        }
        (KeyCode::Char('p'), _) => {
            return cloud.toggle_pause();
        }
        (KeyCode::Char('x' | 'X'), _) => {
            let next = scene::cycle_scene(scene_name, 1);
            *scene_name = next.to_string();
            *charset_preset =
                cloud.apply_scene_runtime(next, charset_preset, user_ranges, def_ascii);
        }
        (KeyCode::Up, _) => {
            let mut cps = cloud.chars_per_sec;
            if cps <= 0.5 {
                cps *= 2.0;
            } else {
                cps += 1.0;
            }
            cloud.set_chars_per_sec(runtime_speed_clamp(cps, cloud.rain_style()));
        }
        (KeyCode::Down, _) => {
            let mut cps = cloud.chars_per_sec;
            if cps <= 1.0 {
                cps /= 2.0;
            } else {
                cps -= 1.0;
            }
            cloud.set_chars_per_sec(runtime_speed_clamp(cps, cloud.rain_style()));
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
        // v16: Digit-key color shortcuts (1-0, !@#$%) removed.
        // Use 'c'/'C' to cycle through all 43 themes instead — it's
        // more discoverable and doesn't require memorizing a mapping.
        // --colors-custom is also available for user-defined palettes.
        _ => {}
    }

    false
}

pub(super) fn runtime_speed_clamp(cps: f32, rain_style: RainStyle) -> f32 {
    let max = if matches!(rain_style, RainStyle::Monolith) {
        MONOLITH_EFFECTIVE_SPEED_MAX
    } else {
        RUNTIME_SPEED_MAX
    };
    if cps.is_finite() {
        cps.clamp(RUNTIME_SPEED_MIN, max)
    } else {
        RUNTIME_SPEED_MIN
    }
}
