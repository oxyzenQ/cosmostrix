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
use crate::runtime::ColorScheme;
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

    match (k.code, k.modifiers) {
        // Only 'q' quits. Esc and Ctrl+C are intentionally NOT exit keys
        // — owner wants a single, deliberate quit key to avoid accidental
        // exits (Esc from terminal menus, Ctrl+C from muscle memory).
        (KeyCode::Char('q'), _) => cloud.raining = false,
        (KeyCode::Esc, _) => {
            // Esc is ignored — use 'q' to quit.
        }
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
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            // Ctrl+C is ignored — use 'q' to quit.
            // This prevents accidental exits from Ctrl+C muscle memory.
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

/// Check if a key is "recognized" by cosmostrix's interactive keybindings.
///
/// This is the single source of truth for the screensaver exit logic: in
/// `--screensaver` mode, recognized keys are processed normally (cycle color,
/// toggle HUD, etc.) and do NOT exit; unrecognized keys exit the screensaver.
///
/// The set mirrors `handle_keybinding`'s match arms plus the HUD toggle keys
/// (`i`/`I`/`h`/`H`) which are handled earlier in the event loop. Any new
/// keybinding added to `handle_keybinding` or the HUD handlers MUST be added
/// here too — otherwise the screensaver would exit when the user presses the
/// new key, which is surprising and broken behavior.
///
/// Keys that are intentionally ignored (Esc, Ctrl+C, Tab) are listed as
/// recognized so they don't cause screensaver exit. The user must press a
/// genuinely unrecognized key (e.g. `z`, `F1`, `Home`) to exit screensaver.
pub(super) fn is_recognized_key(
    code: crossterm::event::KeyCode,
    modifiers: crossterm::event::KeyModifiers,
) -> bool {
    use crossterm::event::{KeyCode, KeyModifiers};

    match (code, modifiers) {
        // Quit key
        (KeyCode::Char('q'), _) => true,
        // Ignored-but-recognized (don't exit screensaver)
        (KeyCode::Esc, _) => true,
        (KeyCode::Tab, _) | (KeyCode::BackTab, _) => true,
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => true, // Ctrl+C ignored
        // Suspend (Ctrl+Z, Unix only — but recognize everywhere for consistency)
        (KeyCode::Char('z'), KeyModifiers::CONTROL) => true,
        // Reset
        (KeyCode::Char(' '), _) => true,
        // Color cycle
        (KeyCode::Char('c'), KeyModifiers::NONE) | (KeyCode::Char('C'), _) => true,
        // Charset cycle
        (KeyCode::Char('s'), _) | (KeyCode::Char('S'), _) => true,
        // Async toggle
        (KeyCode::Char('a'), _) => true,
        // Glitch toggle
        (KeyCode::Char('g'), _) => true,
        // Pause
        (KeyCode::Char('p'), _) => true,
        // Profile cycle
        (KeyCode::Char('m'), _) => true,
        // Scene cycle
        (KeyCode::Char('x'), _) | (KeyCode::Char('X'), _) => true,
        // HUD toggle + position (handled in event_loop before keybinding)
        (KeyCode::Char('i'), _) | (KeyCode::Char('I'), _) => true,
        (KeyCode::Char('h'), _) | (KeyCode::Char('H'), _) => true,
        // Speed
        (KeyCode::Up, _) | (KeyCode::Down, _) => true,
        // Glitch pct (only effective when glitchy, but always recognized)
        (KeyCode::Left, _) | (KeyCode::Right, _) => true,
        // Density
        (KeyCode::Char('['), _)
        | (KeyCode::Char(']'), _)
        | (KeyCode::Char('-'), _)
        | (KeyCode::Char('+'), _)
        | (KeyCode::Char('='), _)
        | (KeyCode::Char('_'), _) => true,
        // Direct color schemes (0-9)
        (KeyCode::Char('0'..='9'), _) => true,
        // Shifted direct color schemes (! @ # $ %)
        (KeyCode::Char('!'), _)
        | (KeyCode::Char('@'), _)
        | (KeyCode::Char('#'), _)
        | (KeyCode::Char('$'), _)
        | (KeyCode::Char('%'), _) => true,
        // Anything else is unrecognized → screensaver exits
        _ => false,
    }
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
