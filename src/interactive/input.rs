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
        //
        // v35 ambient harmony: 'c'/'C' clears `ambient_palette_locked` (user
        // is taking ownership of color) and sets `user_override_since_ambient`
        // (so the next ambient fire isn't deduped). See
        // docs/audits/AMBIENT_SCHEDULER_AUDIT.md §2.3.
        (KeyCode::Char('c'), _) => {
            let next = cycle_color_scheme(cloud.color_scheme(), 1);
            cloud.set_color_scheme(next);
            cloud.user_override_since_ambient = true;
            cloud.ambient_palette_locked = false;
        }
        (KeyCode::Char('C'), _) => {
            let prev = cycle_color_scheme(cloud.color_scheme(), -1);
            cloud.set_color_scheme(prev);
            cloud.user_override_since_ambient = true;
            cloud.ambient_palette_locked = false;
        }
        (KeyCode::Char('s'), _) => {
            let next = cycle_charset_preset(charset_preset, 1);
            *charset_preset = next.to_string();
            if let Ok(cs) = charset_from_str(charset_preset, def_ascii) {
                let chars = build_chars(cs, user_ranges, def_ascii);
                cloud.transition_chars(chars);
            }
            // v35: charset change is a user override — flag it so the next
            // ambient fire (which resets charset via apply_ambient_entry)
            // isn't deduped.
            cloud.user_override_since_ambient = true;
        }
        (KeyCode::Char('S'), _) => {
            let prev = cycle_charset_preset(charset_preset, -1);
            *charset_preset = prev.to_string();
            if let Ok(cs) = charset_from_str(charset_preset, def_ascii) {
                let chars = build_chars(cs, user_ranges, def_ascii);
                cloud.transition_chars(chars);
            }
            cloud.user_override_since_ambient = true;
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
            // v35: scene change is a user override — flag both. The palette
            // lock is cleared because the new scene may bring its own color
            // (and auto-drift should be free to drift from there until the
            // next ambient fire re-locks).
            cloud.user_override_since_ambient = true;
            cloud.ambient_palette_locked = false;
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

/// v35: Apply the current ambient phase to the cloud immediately.
///
/// Called when the user presses `a` (ambient snap-back). Returns `true` if
/// an entry was applied (caller should redraw — rebuild ColorCache, Frame,
/// fill terminal bg), `false` if no schedule is active or no current phase
/// exists at this minute (silent no-op).
///
/// This is the manual "return to ambient" command. Without it, after the
/// user presses `x`/`c`/`s` to override the ambient scene, the only way to
/// return to ambient is to wait for the next boundary fire (which for a
/// single-entry schedule is up to 24h away) or restart cosmostrix.
///
/// After applying, both v35 harmony flags are updated:
/// - `user_override_since_ambient = false` — ambient just re-asserted.
/// - `ambient_palette_locked = true` — auto-drift palette drift is suppressed
///   until the user overrides again.
///
/// The caller (event_loop) is responsible for post-apply side effects:
/// `term.set_color_cache(ColorCache::new(&cloud.palette))`,
/// `frame = Frame::new(w, h, cloud.palette.bg)`,
/// `fill_terminal_bg(cloud.palette.bg)`. We don't do them here because we
/// don't have access to `term` / `frame` / `w` / `h`.
///
/// See `docs/audits/AMBIENT_SCHEDULER_AUDIT.md` §2.2.
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_ambient_snapback(
    cloud: &mut Cloud,
    charset_preset: &mut String,
    scene_name: &mut String,
    scene_generation: &mut u64,
    last_applied_ambient_entry: &mut Option<crate::ambient::AmbientEntry>,
    schedule: &crate::ambient::AmbientSchedule,
    cfg_map: &std::collections::HashMap<String, String>,
    user_ranges: &[(char, char)],
    def_ascii: bool,
) -> bool {
    let now_min = crate::ambient::current_minute_of_day();
    let Some(entry) = schedule.current_phase(now_min).cloned() else {
        // No active phase at this minute (schedule is empty, or `now_min`
        // is before the first entry of the day with no wrap-around). Silent
        // no-op — pressing 'a' with no ambient schedule is a no-op, not an
        // error.
        crate::lr_trace!(
            "ambient: 'a' key pressed but no current phase at minute {} — no-op",
            now_min
        );
        return false;
    };
    crate::lr_trace!(
        "ambient: 'a' key snap-back — applying phase {:02}:{:02} (scene={})",
        entry.hour,
        entry.minute,
        entry.scene
    );
    *charset_preset = cloud.apply_ambient_entry(
        &entry,
        charset_preset,
        user_ranges,
        def_ascii,
        cfg_map,
    );
    *last_applied_ambient_entry = Some(entry.clone());
    *scene_name = entry.scene.clone();
    *scene_generation = scene_generation.wrapping_add(1);
    // v35 harmony: ambient just re-asserted — clear user override and lock
    // the palette against auto-drift.
    cloud.user_override_since_ambient = false;
    cloud.ambient_palette_locked = true;
    true
}
