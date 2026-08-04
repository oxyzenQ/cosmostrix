// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Keyboard input handling and paste burst detection.
//!
//! Processes key events through the keybinding dispatch table and filters
//! out rapid printable key events that occur during bracketed paste
//! sequences (which arrive as individual Key events on some terminals).

use std::time::{Duration, Instant};

#[cfg(unix)]
use std::sync::atomic::AtomicBool;
#[cfg(unix)]
use std::sync::Arc;

use crate::charset::{build_chars, charset_from_str};
use crate::cloud::Cloud;
use crate::constants::*;
use crate::frame::Frame;
use crate::rain_style::RainStyle;

use crate::scene;

use super::super::{cycle_charset_preset, cycle_color_scheme, CloudConfig};

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
    /// printable key press. That made `c` (color cycle), `s` (charset),
    /// `p` (pause), etc. unreachable on those terminals.
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
//
// `scene_generation` is bumped on every reassignment of `scene_name` so the
// event loop can detect "scene changed during this frame" with a u64 compare
// instead of cloning the String per frame (~60 allocs/sec saved).
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_keybinding(
    cloud: &mut Cloud,
    frame: &mut Frame,
    k: &crossterm::event::KeyEvent,
    charset_preset: &mut String,
    scene_name: &mut String,
    scene_generation: &mut u64,
    user_ranges: &[(char, char)],
    def_ascii: bool,
    _cfg: &CloudConfig,
    #[cfg(unix)] _term_reinit: &Arc<AtomicBool>,
) -> bool {
    use crossterm::event::KeyCode;

    // Quit policy: only 'q' exits. Esc, Ctrl+C (SIGINT is deprecated),
    // Ctrl+Z (in-app suspend removed v30: terminal-driven SIGTSTP still
    // works via signal_handlers.rs), Tab/BackTab, and any other
    // unrecognized key are silently ignored (fall through to the
    // `_ => {}` arm at the end of this match). This prevents accidental
    // exits from terminal menu Esc, Ctrl+C muscle memory, or stray
    // function keys. The user must press 'q' deliberately to quit.
    // v25.13: SIGINT (Ctrl+C) is no longer in the graceful-shutdown
    // signal list — see signal_handlers.rs.
    //
    // Historical note: Tab previously had an explicit arm here that
    // toggled shading mode, which triggered a phosphor ghost-flood bug.
    // The arm was removed v30; Tab now falls through to `_ => {}`. The
    // regression suite in tests.rs::tab_* documents the historical bug
    // and verifies Tab remains a no-op.
    match (k.code, k.modifiers) {
        (KeyCode::Char('q'), _) => cloud.raining = false,
        (KeyCode::Char(' '), _) => {
            cloud.reset(frame.width, frame.height);
            cloud.force_draw_everything();
            // Restart message typewriter so Space gives a full cinematic
            // replay — rain reseed + message types out from scratch.
            cloud.restart_message_typewriter();
        }
        // Color cycle: 'c' forward, 'C' (shift+c) reverse.
        // v30 simplify had removed uppercase 'C'/'S' for consistency; owner
        // restored them as reverse-cycle bindings (shift+c/s is simple and
        // matches the c/C, s/S convention). See audit task flags-audit-4.
        (KeyCode::Char('c'), _) => {
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

        (KeyCode::Char('p'), _) => {
            return cloud.toggle_pause();
        }
        (KeyCode::Char('x'), _) => {
            let next = scene::cycle_scene(scene_name, 1);
            *scene_name = next.to_string();
            *scene_generation = scene_generation.wrapping_add(1);
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
        // Density: '[' decreases, ']' increases. Simplified from the
        // legacy alias set (-/_ for down, +/=Shift for up) — those were
        // carried over from an older keymap and never documented in the
        // --help reference, so they only caused confusion. '[' and ']'
        // are the canonical density keys and the only ones documented.
        (KeyCode::Char('['), _) => {
            let d = (cloud.droplet_density - DENSITY_STEP).max(0.01);
            cloud.set_droplet_density(d);
        }
        (KeyCode::Char(']'), _) => {
            let d = (cloud.droplet_density + DENSITY_STEP).min(5.0);
            cloud.set_droplet_density(d);
        }

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
