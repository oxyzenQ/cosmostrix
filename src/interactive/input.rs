// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Keyboard input handling and paste burst detection.
//!
//! Processes key events through the keybinding dispatch table and filters
//! out rapid printable key events that occur during bracketed paste
//! sequences (which arrive as individual Key events on some terminals).

use std::time::{Duration, Instant};

use crate::platform::TermReinit;

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

    // IN-03: KeyModifiers::NONE == empty bitflags, so `is_empty()` already
    // covers it. Dropped the redundant clause to avoid reader confusion
    // (the dead branch suggested the author intended something else).
    matches!(key.code, KeyCode::Char(_))
        && (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT)
}

/// Returns true if the key event has NO modifier bits set.
///
/// Use for non-cycle shortcuts (q, c, s, x, i, p, [, ], space,
/// arrows) that must only respond to bare keypresses — Shift+key
/// (e.g. Shift+q → 'Q') and any other modifier combination are rejected.
///
/// CapsLock is a keyboard state, NOT a KeyModifiers bit — it changes
/// which Char the terminal reports ('q' → 'Q' when CapsLock is on).
/// crossterm then tags that uppercase char with SHIFT
/// (`char_code_to_event` cannot distinguish CapsLock from a held
/// Shift in the byte stream), so the CapsLock-produced uppercase
/// still fails this check — which is correct: it does not match the
/// lowercase `KeyCode::Char('q')` arm either way.
pub(super) fn is_unmodified(modifiers: crossterm::event::KeyModifiers) -> bool {
    modifiers.is_empty()
}

/// Returns true if the key event's modifiers are in the "safe" allowlist:
/// only bare keys (KeyModifiers::NONE) or SHIFT (for capital S/C/X
/// reverse-cycle bindings). Rejects ALL other modifier bits: CONTROL, ALT,
/// SUPER, HYPER, META, FUNCTION.
///
/// Use ONLY for cycle shortcuts (uppercase S, C, X) where Shift is a
/// deliberate input (Shift+s → S = reverse charset cycle, Shift+c → C =
/// reverse color cycle, Shift+x → X = reverse scene cycle).
/// Non-cycle shortcuts must use `is_unmodified()` instead.
///
/// CapsLock is a keyboard state, NOT a KeyModifiers bit — it changes
/// which Char the terminal reports ('c' → 'C' when CapsLock is on).
/// crossterm tags that uppercase char with SHIFT, so CapsLock+C lands
/// in the same cycle arm as Shift+C — accepted, since both produce the
/// uppercase letter that means "reverse cycle".
pub(super) fn is_unmodified_or_shift(modifiers: crossterm::event::KeyModifiers) -> bool {
    modifiers.is_empty() || modifiers == crossterm::event::KeyModifiers::SHIFT
}

/// Normalize the kitty-protocol form of Shift+letter to the uppercase char.
///
/// Two real-world event shapes carry "Shift + letter":
///
///  * Legacy / plain-text path: the terminal sends the shifted byte
///    ('X'), and crossterm's `char_code_to_event` tags every uppercase
///    char with `KeyModifiers::SHIFT` — so the event arrives as
///    `Char('X') + SHIFT` (CapsLock-produced uppercase gets the same
///    tag, because the byte stream is indistinguishable).
///  * Kitty CSI-u path: kitty-keyboard-protocol terminals report the
///    BASE (lowercase) codepoint plus the SHIFT modifier bit —
///    `CSI 120;2u` for Shift+X arrives as `Char('x') + SHIFT`.
///    crossterm only substitutes the shifted codepoint when the
///    terminal also reports alternate keys (a flag cosmostrix does
///    not push), so the lowercase base survives as-is.
///
/// Without normalization the second shape matches NEITHER the
/// lowercase arm (modifiers are SHIFT, not NONE) NOR the uppercase arm
/// (code is 'x', not 'X') — Shift+X/C/S were silent no-ops on
/// kitty-protocol terminals (owner-reported bug). Normalizing to the
/// uppercase char routes both shapes into the same reverse-cycle arm.
///
/// Non-cycle keys are unaffected: Shift+q normalizes to `Char('Q')`,
/// which has no binding, so the bare-key-only policy still rejects it.
/// Non-ASCII, non-lowercase, and non-Char codes pass through unchanged.
pub(super) fn normalize_shifted_char(
    code: crossterm::event::KeyCode,
    modifiers: crossterm::event::KeyModifiers,
) -> crossterm::event::KeyCode {
    use crossterm::event::{KeyCode, KeyModifiers};
    if modifiers == KeyModifiers::SHIFT {
        if let KeyCode::Char(c) = code {
            if c.is_ascii_lowercase() {
                return KeyCode::Char(c.to_ascii_uppercase());
            }
        }
    }
    code
}

// Runtime key handling coordinates cloud, frame, scene, charset, and terminal
// recovery state in one dispatch point; splitting would obscure side effects.
//
// `scene_generation` is bumped on every reassignment of `scene_name` so the
// event loop can detect "scene changed during this frame" with a u64 compare
// instead of cloning the String per frame (~60 allocs/sec saved).
/// Aggregated context for `handle_keybinding()`.
///
/// Bundles the mutable references and values that the keybinding handler
/// needs, replacing the former 10-parameter list.
pub(super) struct KeybindingCtx<'a> {
    pub cloud: &'a mut Cloud,
    pub frame: &'a mut Frame,
    pub charset_preset: &'a mut String,
    pub scene_name: &'a mut String,
    pub scene_generation: &'a mut u64,
    pub user_ranges: &'a [(char, char)],
    pub def_ascii: bool,
    pub cfg: &'a CloudConfig,
    pub term_reinit: &'a TermReinit,
}

pub(super) fn handle_keybinding(ctx: &mut KeybindingCtx, k: &crossterm::event::KeyEvent) -> bool {
    let cloud = &mut *ctx.cloud;
    let frame = &mut *ctx.frame;
    let charset_preset = &mut *ctx.charset_preset;
    let scene_name = &mut *ctx.scene_name;
    let scene_generation = &mut *ctx.scene_generation;
    let user_ranges = ctx.user_ranges;
    let def_ascii = ctx.def_ascii;
    let _cfg = ctx.cfg;
    let _term_reinit = ctx.term_reinit;

    use crossterm::event::{KeyCode, KeyModifiers};

    // Normalize the kitty CSI-u Shift+letter shape (base lowercase
    // codepoint + SHIFT) to the uppercase char so both terminal
    // families hit the same match arms. See normalize_shifted_char()
    // for the full event-shape analysis.
    let code = normalize_shifted_char(k.code, k.modifiers);
    let modifiers = k.modifiers;

    // Pause guard: when paused OR decelerating toward pause, ONLY
    // 'p' (resume/cancel-decel) and 'q' (quit) are processed. All
    // other keys are silently ignored to prevent queued state changes
    // from accumulating during the pause/deceleration window and
    // causing "stuck particles" or visual glitches on resume.
    //
    // Must check `is_paused_or_decelerating()` (not just `pause`)
    // because the deceleration phase (pause_start.is_some()) is also
    // a pause-related state where user interactions should be
    // suppressed (owner-reported bug: rapid p-taps left effects
    // hanging).
    if cloud.is_paused_or_decelerating() {
        match (code, modifiers) {
            (KeyCode::Char('p'), KeyModifiers::NONE) => {
                return cloud.toggle_pause();
            }
            (KeyCode::Char('q'), KeyModifiers::NONE) => {
                // Allow quit during pause
            }
            _ => {
                // Silently ignore all other keys during pause
                return false;
            }
        }
    }

    // Modifier fast-reject: block any key event that carries modifier bits
    // beyond NONE and SHIFT. The per-arm match below then applies the final
    // policy:
    //  • Non-cycle shortcuts (lowercase q/c/s/x/p/[/]/space, arrows):
    //    match only KeyModifiers::NONE — Shift+key is rejected (owner
    //    requirement: only bare lowercase key, not uppercase variant).
    //  • Cycle shortcuts (uppercase C/S/X):
    //    match any modifier that passes this guard (NONE or SHIFT) — both
    //    are valid: SHIFT = legacy plain-text (crossterm tags uppercase
    //    chars with SHIFT) or kitty CSI-u base+SHIFT (normalized to the
    //    uppercase char above); NONE = kitty alternate-keys form where
    //    crossterm already substituted the shifted codepoint and cleared
    //    the SHIFT bit.
    //
    // Owner-reported bug (v50 alpha.3): Super+C still cycled colors on
    // modern terminals (kitty, wezterm, foot) that report the kitty
    // keyboard protocol's enhanced modifier bits. crossterm 0.29 exposes
    // SUPER (0b1000), HYPER (0b10000), and META (0b100000) as separate
    // KeyModifiers bits — the previous denylist only blocked CONTROL |
    // ALT, leaving SUPER/HYPER/META unguarded.
    //
    // Owner follow-up (v50 alpha.4): non-cycle shortcuts like 'q' must
    // only respond to the bare lowercase key, NOT the Shift-produced
    // uppercase variant. Previously Shift+q (CapsLock on) could trigger
    // quit because the match arm used `_` for modifiers.
    //
    // The allowlist approach is future-proof: if crossterm adds new
    // modifier bits (e.g. FUNCTION), they are rejected by default — no
    // silent passthrough.
    //
    // CapsLock is a keyboard state, NOT a KeyModifiers bit — it changes
    // which Char the terminal reports ('c' → 'C' when CapsLock is on).
    // The uppercase byte is then tagged with SHIFT by crossterm's
    // `char_code_to_event` (the byte stream cannot distinguish CapsLock
    // from a held Shift), so CapsLock+C lands in the same cycle arm as
    // Shift+C. That is accepted behavior: both produce the uppercase
    // letter, both reverse-cycle.
    if !is_unmodified_or_shift(modifiers) {
        return false;
    }

    // Quit policy: only 'q' exits. Esc, Ctrl+C (SIGINT is deprecated),
    // Ctrl+Z (in-app suspend removed v30: terminal-driven SIGTSTP still
    // works via signal_handlers.rs), Tab/BackTab, and any other
    // unrecognized key are silently ignored (fall through to the
    // `_ => {}` arm at the end of this match). This prevents accidental
    // exits from terminal menu Esc, Ctrl+C muscle memory, or stray
    // function keys. The user must press 'q' deliberately to quit.
    // SIGINT (Ctrl+C) is no longer in the graceful-shutdown
    // signal list — see signal_handlers.rs.
    //
    // Historical note: Tab previously had an explicit arm here that
    // toggled shading mode, which triggered a phosphor ghost-flood bug.
    // The arm was removed v30; Tab now falls through to `_ => {}`. The
    // regression suite in tests.rs::tab_* documents the historical bug
    // and verifies Tab remains a no-op.
    match (code, modifiers) {
        // Non-cycle shortcuts: KeyModifiers::NONE only.
        // Owner requirement: only bare lowercase key, not Shift-produced
        // uppercase. E.g. 'q' quits, 'Q' does nothing.
        (KeyCode::Char('q'), KeyModifiers::NONE) => cloud.raining = false,
        (KeyCode::Char(' '), KeyModifiers::NONE) => {
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
        // ambient harmony: 'c'/'C' clears `ambient_palette_locked` (user
        // is taking ownership of color) and sets `user_override_since_ambient`
        // (so the next ambient fire isn't deduped). See
        // docs/archive/audits/AMBIENT_SCHEDULER_AUDIT.md §2.3.
        (KeyCode::Char('c'), KeyModifiers::NONE) => {
            let next = cycle_color_scheme(cloud.color_scheme(), 1);
            cloud.set_color_scheme(next);
            cloud.user_override_since_ambient = true;
            cloud.ambient_palette_locked = false;
        }
        // Cycle shortcut: uppercase 'C' accepts NONE (CapsLock) or SHIFT.
        // The global guard already limits modifiers to NONE | SHIFT, so
        // `_` here is safe and concise.
        (KeyCode::Char('C'), _) => {
            let prev = cycle_color_scheme(cloud.color_scheme(), -1);
            cloud.set_color_scheme(prev);
            cloud.user_override_since_ambient = true;
            cloud.ambient_palette_locked = false;
        }
        (KeyCode::Char('s'), KeyModifiers::NONE) => {
            let next = cycle_charset_preset(charset_preset, 1);
            *charset_preset = next.to_string();
            if let Ok(cs) = charset_from_str(charset_preset, def_ascii) {
                let chars = build_chars(cs, user_ranges, def_ascii);
                cloud.transition_chars(chars);
            }
            // charset change is a user override — flag it so the next
            // ambient fire (which resets charset via apply_ambient_entry)
            // isn't deduped.
            cloud.user_override_since_ambient = true;
        }
        // Cycle shortcut: uppercase 'S' accepts NONE (CapsLock) or SHIFT.
        (KeyCode::Char('S'), _) => {
            let prev = cycle_charset_preset(charset_preset, -1);
            *charset_preset = prev.to_string();
            if let Ok(cs) = charset_from_str(charset_preset, def_ascii) {
                let chars = build_chars(cs, user_ranges, def_ascii);
                cloud.transition_chars(chars);
            }
            cloud.user_override_since_ambient = true;
        }

        (KeyCode::Char('p'), KeyModifiers::NONE) => {
            return cloud.toggle_pause();
        }
        (KeyCode::Char('x'), KeyModifiers::NONE) => {
            let next = scene::cycle_scene(scene_name, 1);
            *scene_name = next.to_string();
            *scene_generation = scene_generation.wrapping_add(1);
            *charset_preset =
                cloud.apply_scene_runtime(next, charset_preset, user_ranges, def_ascii);
            // scene change is a user override — flag both. The palette
            // lock is cleared because the new scene may bring its own color
            // (and Crystal Dragon should be free to drift from there until the
            // next ambient fire re-locks).
            cloud.user_override_since_ambient = true;
            cloud.ambient_palette_locked = false;
        }
        // Cycle shortcut: uppercase 'X' accepts NONE (CapsLock) or SHIFT.
        // Both terminal families reach this arm: legacy plain-text sends
        // 'X' (tagged SHIFT by crossterm), kitty CSI-u sends base 'x' +
        // SHIFT which normalize_shifted_char() maps to 'X' above.
        (KeyCode::Char('X'), _) => {
            let prev = scene::cycle_scene(scene_name, -1);
            *scene_name = prev.to_string();
            *scene_generation = scene_generation.wrapping_add(1);
            *charset_preset =
                cloud.apply_scene_runtime(prev, charset_preset, user_ranges, def_ascii);
            cloud.user_override_since_ambient = true;
            cloud.ambient_palette_locked = false;
        }
        (KeyCode::Up, KeyModifiers::NONE) => {
            let mut cps = cloud.chars_per_sec;
            if cps <= 0.5 {
                cps *= 2.0;
            } else {
                cps += 1.0;
            }
            cloud.set_chars_per_sec(runtime_speed_clamp(cps, cloud.rain_style()));
        }
        (KeyCode::Down, KeyModifiers::NONE) => {
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
        (KeyCode::Char('['), KeyModifiers::NONE) => {
            let d = (cloud.droplet_density - DENSITY_STEP).max(0.01);
            cloud.set_droplet_density(d);
        }
        (KeyCode::Char(']'), KeyModifiers::NONE) => {
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

/// Decide whether the ambient scheduler should auto-snapback.
///
/// Pure decision function — no side effects. The event loop calls this
/// every frame; when it returns `true`, the loop re-applies the current
/// ambient phase to the cloud (clearing `user_override_since_ambient` and
/// re-locking `ambient_palette_locked`).
///
/// This replaces the v35 'a' shortcut with a fully automatic mechanism:
/// after the user presses `x`/`c`/`s` (which sets `user_override_since_ambient
/// = true`), the scheduler waits `AUTO_SNAPBACK_DELAY` seconds of input
/// idle, then silently re-asserts the current ambient phase. No new
/// shortcut, no new CLI flag — the harmony flags already in Cloud drive
/// the behavior.
///
/// See `docs/archive/audits/AMBIENT_SCHEDULER_AUDIT.md` §2.2.
pub(super) fn should_auto_snapback(
    user_override_since_ambient: bool,
    idle_secs: f64,
    auto_snapback_delay_secs: f64,
) -> bool {
    user_override_since_ambient && idle_secs >= auto_snapback_delay_secs
}

/// Try to auto-snapback to the current ambient phase.
///
/// Called every frame from the event loop. Returns `true` if ambient was
/// re-applied (caller must redraw — rebuild ColorCache, Frame, fill bg).
/// Returns `false` if no snapback is needed (no override, idle time below
/// threshold, no active phase, or empty schedule).
///
/// This is the automatic replacement for the v35 'a' shortcut. The
/// harmony flags (`user_override_since_ambient`, `ambient_palette_locked`)
/// are updated on successful apply — same as a scheduler fire.
#[allow(clippy::too_many_arguments)]
pub(super) fn try_auto_snapback(
    cloud: &mut Cloud,
    charset_preset: &mut String,
    scene_name: &mut String,
    scene_generation: &mut u64,
    last_applied_ambient_entry: &mut Option<crate::crystal_dragon_engine::ambient::AmbientEntry>,
    schedule: &crate::crystal_dragon_engine::ambient::AmbientSchedule,
    last_cfg_map: &Option<std::collections::HashMap<String, String>>,
    user_ranges: &[(char, char)],
    def_ascii: bool,
    last_user_input_at: Instant,
    auto_snapback_delay_secs: f64,
) -> bool {
    // AB-04: explicit empty-schedule guard — never snapback when
    // the schedule is empty. current_phase() returns None for empty
    // schedules, but this guard is belt-and-suspenders: it avoids
    // even calling current_phase() and makes the intent explicit.
    if schedule.entries.is_empty() {
        return false;
    }
    // AB-05: no previous ambient entry → nothing to snapback to.
    // After schedule-empty reload clears last_applied_ambient_entry,
    // this prevents snapback from re-applying a stale ambient scene
    // even if last_ambient_schedule hasn't been updated yet (file
    // watcher latency window).
    if last_applied_ambient_entry.is_none() {
        return false;
    }
    let now = Instant::now();
    // v50.0.0-beta.7 masterclass state machine: snapback counts from
    // drift_start (when Crystal Dragon drift fired), NOT from last_user_input_at.
    // This gives drift exactly ambient-snapback-secs of visibility before
    // ambient reverts. When drift is NOT active (no drift has fired this
    // cycle), fall back to last_user_input_at for manual user overrides.
    let snapback_ref = cloud.drift_start.unwrap_or(last_user_input_at);
    let idle_secs = now.saturating_duration_since(snapback_ref).as_secs_f64();
    if !should_auto_snapback(
        cloud.user_override_since_ambient,
        idle_secs,
        auto_snapback_delay_secs,
    ) {
        return false;
    }
    let now_min = crate::crystal_dragon_engine::ambient::current_minute_of_day();
    let Some(entry) = schedule.current_phase(now_min).cloned() else {
        return false;
    };
    let cfg_map = last_cfg_map.clone().unwrap_or_default();
    crate::lr_trace!(
        "ambient: auto-snapback after {:.1}s (drift_active={}) — applying phase {:02}:{:02} (scene={})",
        idle_secs,
        cloud.drift_active,
        entry.hour,
        entry.minute,
        entry.scene
    );
    *charset_preset =
        cloud.apply_ambient_entry(&entry, charset_preset, user_ranges, def_ascii, &cfg_map);
    *last_applied_ambient_entry = Some(entry.clone());
    *scene_name = entry.scene.clone();
    *scene_generation = scene_generation.wrapping_add(1);
    cloud.user_override_since_ambient = false;
    cloud.ambient_palette_locked = true;
    // Clear drift state — cycle complete, next drift can fire on next poll.
    cloud.drift_active = false;
    cloud.drift_start = None;
    // Reset the drift poll timer so the next drift fires 60s from now
    // (not immediately). Without this, the poll is already "due" (60s+
    // elapsed since last poll) and drift would re-fire instantly,
    // preventing the ambient palette from being visible.
    cloud.crystal_dragon_last_poll = Some(now);
    crate::interactive::ambient_diag_snapback();
    crate::interactive::ambient_diag_scene_change(&format!("auto-snapback(scene={})", entry.scene));
    true
}
