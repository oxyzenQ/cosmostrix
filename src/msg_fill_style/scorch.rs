// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! msg-fill-style `scorch` — burnt-in text with ember tint and smoke.
//!
//! The third candidate from the post-engrave expansion family
//! (see `docs/research/MSG_FILL_STYLE_EXPANSION_RESEARCH.md` §3.C):
//! chars appear in an ember tint (orange/red) at the head, cooling
//! to the palette color over ~400 ms; occasional smoke particles
//! drift upward from the head; the cell behind the head briefly
//! chars (dimmer + bold). Highest visual impact in the family —
//! the "wow option" per the research doc.
//!
//! The reveal math stays stateless (pure function of elapsed time);
//! the smoke lives in a dedicated 16-slot pool (pre-allocated once,
//! `O(active)`/frame, `O(1)` early-out when idle — the same pattern
//! as `engrave.rs`'s spark sidecar, cloned with scorch-specific
//! tuning).
//!
//! ## Reveal math (stateless)
//!
//! Chars burn in at `SCORCH_CHAR_MS` (80 ms) pacing — same as
//! typewriter/engrave/hologram/glitch. A char appears in an EMBER
//! TINT (`SCORCH_EMBER_RGB` = (255, 100, 30) — a hot ember orange)
//! at full blend (1.0) with a bright head factor
//! (1.0 + `SCORCH_HEAD_BOOST` = 1.5). Over `SCORCH_COOL_MS` (400 ms):
//!
//! 1. **Early cool** (age 0..200 ms): ember blend 1.0 → 0.5 linear,
//!    factor 1.5 → `SCORCH_CHAR_DIP` (0.8). The char dims as it
//!    "chars" — the research doc's "dimmer" sub-effect.
//! 2. **Late cool** (age 200..400 ms): ember blend 0.5 → 0.0 linear,
//!    factor 0.8 → 1.0. The char "recovers" to the palette color.
//! 3. **Settled** (age >= 400 ms): tint = `None`, factor = 1.0 —
//!    bit-identical to every other settled style.
//!
//! Without a timeline (`elapsed_ms = None`), every cell settles
//! instantly (`tint = None`, factor = 1.0) — same `usize::MAX`
//! reveal_count semantics every stateless style uses for bench
//! and edge paths.
//!
//! ## Smoke sidecar (stateful)
//!
//! The smoke burst needs one usize of bookkeeping ("which head char
//! was last smoke-emitting") plus a particle pool. Both live here,
//! behind a single `Cloud::scorch_smoke_pass` entry point called at
//! the END of `draw_message` (after the halo row, alongside the
//! `engrave_spark_pass` and `hologram_scanline_pass` — only one is
//! wired per style) so smoke renders ON TOP of the overlay text:
//! the "scorching head throws smoke" look.
//!
//! ## Why a dedicated pool instead of reusing the engrave spark pool
//!
//! Same reasoning as engrave: the shared quantum pool renders in
//! `apply_quantum_ripple` BEFORE `draw_message`, and `draw_message`
//! `set_force`-paints every overlay cell — a quantum-pool smoke
//! particle inside the box would be overdrawn. A dedicated pool also
//! keeps scorch smoke from competing with engrave sparks for the 48
//! shared slots, and vice versa.
//!
//! ## Bounds and cost (LTS contract)
//!
//! - Pool: `SCORCH_SMOKE_POOL_SIZE` (16) slots pre-allocated once at
//!   `Cloud::new` — zero per-frame allocation, `O(active)` per frame.
//! - Spawn cadence: one puff of `SCORCH_SMOKE_PER_HEAD` (1) per
//!   NEWLY revealed char — never per frame. Frame-rate independent,
//!   no double-spawn after skipped frames (a jumped head fires ONE
//!   puff at its new position), and puffs stop automatically when
//!   the reveal completes, is paused, or sits in the intro lead.
//! - Steady state: puffs every 80 ms with 700 ms lifetime →
//!   ~9 concurrent smoke particles, well under the 16-slot cap.
//! - `--no-effects` (PERF-4): spawning is gated on `effects_enabled`
//!   (same contract as every particle subsystem); already-active
//!   smoke decays out naturally.
//! - Bench mode: never runs — `draw_message` itself is skipped in
//!   bench mode (Z-6), so the whole pass is dead code on that path.
//!
//! Border: lags text with the shared `t^1.5` ease-out curve.

use std::time::Instant;

use crossterm::style::Color;
use rand::distr::Distribution;

use super::{index_fraction, index_pacing, lagged_border, CellReveal};
use crate::cell::Cell;
use crate::constants::PARTICLE_MAX_FRAME_DT_SECS;
use crate::frame::Frame;

// ── Reveal math constants (stateless) ───────────────────────────────────────

/// Per-character reveal stagger (same 80 ms pacing as
/// typewriter/engrave/hologram/glitch).
pub(crate) const SCORCH_CHAR_MS: usize = 80;

/// Cooling window: ember tint fades to zero and factor settles to
/// 1.0 over this duration. 400 ms = ~5 chars cooling at any moment
/// (at 80 ms/char pacing), forming the "scorch trail" behind the
/// active head.
pub(crate) const SCORCH_COOL_MS: usize = 400;

/// Head boost above settled brightness (factor = 1.0 + 0.5 = 1.5
/// at the freshly-scorched head, routed through the unclamped boost
/// path like pulse/engrave).
pub(crate) const SCORCH_HEAD_BOOST: f32 = 0.5;

/// Char-dip floor: the factor dips to 0.8 at mid-cool (the
/// "charred" dim sub-effect), then recovers to 1.0.
pub(crate) const SCORCH_CHAR_DIP: f32 = 0.8;

/// Ember tint RGB: a hot ember orange. The renderer blends the
/// palette fg color toward this by the per-cell blend factor (1.0
/// at the head → 0.0 at settled). Distinct from pure red so the
/// "ember" read is warm, not alarm-like.
pub(crate) const SCORCH_EMBER_RGB: (u8, u8, u8) = (255, 100, 30);

// ── Smoke sidecar constants (stateful) ──────────────────────────────────────

/// Scorch smoke pool size. Puffs fire once per newly revealed char
/// (every SCORCH_CHAR_MS = 80 ms) and live
/// SCORCH_SMOKE_LIFETIME_SECS (0.70 s), so steady-state concurrency
/// is ~9 active. 16 slots = comfortable headroom (the wow-option
/// cap per the research doc §3.C).
pub(crate) const SCORCH_SMOKE_POOL_SIZE: usize = 16;

/// Smoke puffs per newly scorch'd char. 1 = sparse, slow-rising
/// ash (the research doc's "occasional smoke particles" spec).
pub(crate) const SCORCH_SMOKE_PER_HEAD: usize = 1;

/// Scorch smoke lifetime in seconds. 700 ms = slow, lazy drift
/// (smoke, not sparks — the engrave spark's 200 ms would read as
/// debris, not smoke).
pub(crate) const SCORCH_SMOKE_LIFETIME_SECS: f32 = 0.70;

/// Scorch smoke upward speed in cells/second. 2.5 = slow rise
/// (smoke buoyancy, not ballistic debris). With the ±20% variance
/// band → ~1.75 cells of travel per puff lifetime — a gentle
/// vertical wisp above the scorching head.
pub(crate) const SCORCH_SMOKE_SPEED: f32 = 2.5;

/// Scorch smoke gray base color (all channels equal). Dimmed by
/// the smoothstep envelope over the puff lifetime so the smoke
/// fades to nothing rather than popping out.
pub(crate) const SCORCH_SMOKE_GRAY: u8 = 128;

// ── Reveal math (stateless) ────────────────────────────────────────────────

/// Per-cell reveal: ember tint + factor curve (hot → char → settle).
pub(super) fn reveal(
    content_idx: usize,
    elapsed_ms: Option<usize>,
    reveal_count: usize,
) -> CellReveal {
    if content_idx >= reveal_count {
        return CellReveal::hidden();
    }
    let reveal_at = content_idx * SCORCH_CHAR_MS;
    let (factor, tint) = match elapsed_ms {
        None => (1.0, None),
        Some(ms) => {
            let age = ms.saturating_sub(reveal_at);
            if age >= SCORCH_COOL_MS {
                (1.0, None)
            } else {
                let progress = age as f32 / SCORCH_COOL_MS as f32;
                // Ember blend: 1.0 → 0.0 linear across the full cool.
                let ember_blend = 1.0 - progress;
                // Factor: 1.5 → 0.8 (early cool) → 1.0 (late cool).
                let half = SCORCH_COOL_MS / 2;
                let factor = if age < half {
                    let p = age as f32 / half as f32;
                    1.0 + SCORCH_HEAD_BOOST - p * (1.0 + SCORCH_HEAD_BOOST - SCORCH_CHAR_DIP)
                } else {
                    let p = (age - half) as f32 / half as f32;
                    SCORCH_CHAR_DIP + p * (1.0 - SCORCH_CHAR_DIP)
                };
                (
                    factor,
                    Some((
                        SCORCH_EMBER_RGB.0,
                        SCORCH_EMBER_RGB.1,
                        SCORCH_EMBER_RGB.2,
                        ember_blend,
                    )),
                )
            }
        }
    };
    CellReveal {
        visible: true,
        factor,
        slide_rows: 0,
        glyph_override: None,
        tint,
    }
}

/// Index budget: 80 ms/char with the pre-v80.0.0-beta.1 `.max(1)` floor.
pub(super) fn reveal_budget(elapsed_ms: Option<usize>, total_text: usize) -> usize {
    index_pacing(SCORCH_CHAR_MS, elapsed_ms, total_text)
}

/// Border lags text (t^1.5) — the pre-v80.0.0-beta.1 cinematic curve.
pub(super) fn border_progress(text_progress: f32) -> f32 {
    lagged_border(text_progress)
}

/// Text progress: revealed-cell fraction.
pub(super) fn text_progress(reveal_count: usize, total_text: usize) -> f32 {
    index_fraction(reveal_count, total_text)
}

// ── Smoke sidecar (stateful) ───────────────────────────────────────────────

/// One scorch smoke particle. Plain data — no trail ring buffer
/// (smoke lives 700 ms and drifts ~1.75 cells; a trail would be
/// noise), no per-particle lifetime field (all puffs share
/// `SCORCH_SMOKE_LIFETIME_SECS`).
#[derive(Clone, Copy)]
pub(crate) struct ScorchSmoke {
    pub(crate) active: bool,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) vx: f32,
    pub(crate) vy: f32,
    pub(crate) birth: Instant,
    /// S-master-HUNT-21/22: accumulated simulation age (seconds). See
    /// QuantumParticle.sim_age for the full rationale — same
    /// real-time clock fix.
    pub(crate) sim_age: f32,
}

/// All mutable scorch state, grouped so `Cloud` grows by a single
/// field (the struct lives in the cloud state namespace; see
/// `cloud/mod.rs`). `last_head` doubles as the movement detector:
/// `usize::MAX` = "no puff fired yet" (fresh overlay / fresh
/// reveal).
pub(crate) struct ScorchState {
    pub(crate) smoke: Vec<ScorchSmoke>,
    /// Active smoke count (incremental, O(1) early-out in the pass).
    pub(crate) active_count: usize,
    /// Content index of the last head char that fired a puff.
    pub(crate) last_head: usize,
    /// Timestamp of the previous physics update (dt clamping).
    pub(crate) last_update: Instant,
}

impl ScorchState {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            smoke: vec![
                ScorchSmoke {
                    active: false,
                    x: 0.0,
                    y: 0.0,
                    vx: 0.0,
                    vy: 0.0,
                    birth: now,
                    sim_age: 0.0,
                };
                SCORCH_SMOKE_POOL_SIZE
            ],
            active_count: 0,
            last_head: usize::MAX,
            last_update: now,
        }
    }

    /// Drop everything and re-arm the movement detector. Called from
    /// `reset_message` (new overlay layout) and from the reveal
    /// restart paths (`restart_message_typewriter`, style change) so
    /// the first char of the fresh reveal fires its puff.
    pub(crate) fn reset(&mut self) {
        for s in &mut self.smoke {
            s.active = false;
        }
        self.active_count = 0;
        self.last_head = usize::MAX;
    }
}

impl crate::cloud::Cloud {
    /// Scorch smoke pass — one entry point, called at the very end of
    /// `draw_message` (after the halo row) so smoke renders ON TOP of
    /// the overlay text: the "scorching head throws smoke" look.
    ///
    /// `head_pos`/`head_idx` describe the most recently revealed
    /// content cell (captured in the main draw loop); `elapsed_ms` is
    /// the same reveal timeline the text uses. Both `None`/sentinel
    /// cases simply skip spawning — the render half still runs so
    /// already-airborne smoke decays out.
    pub(crate) fn scorch_smoke_pass(
        &mut self,
        frame: &mut Frame,
        now: Instant,
        head_pos: Option<(u16, u16)>,
        head_idx: usize,
        elapsed_ms: Option<usize>,
    ) {
        // Spawn on head MOVEMENT only (one puff per newly scorch'd
        // char). While paused or inside the intro lead the elapsed
        // timeline stalls, the head stops advancing, and `head_idx`
        // stops changing — puffs stop with it. No timeline (None)
        // means bench/edge paths: never spawn, keep decaying.
        if let (Some((col, line)), Some(_)) = (head_pos, elapsed_ms) {
            if head_idx != self.scorch.last_head {
                self.scorch.last_head = head_idx;
                if self.effects_enabled {
                    self.spawn_scorch_puff(col, line, now);
                }
            }
        }
        self.draw_scorch_smoke(frame, now);
    }

    /// Activate up to `SCORCH_SMOKE_PER_HEAD` inactive pool slots at
    /// `(col, line)`, drifting upward with slight horizontal
    /// variance. Pool-full → puff silently truncated (same pattern
    /// as `spawn_engrave_burst` / `spawn_quantum_ripple`).
    fn spawn_scorch_puff(&mut self, col: u16, line: u16, now: Instant) {
        let cx = col as f32 + 0.5;
        // Spawn the smoke half a cell ABOVE the head (cy = line - 0.5)
        // so it starts just outside the content row — the next frame's
        // physics update moves it further up. This prevents the smoke
        // from overwriting the freshly scorch'd char on the spawn frame
        // (same draw-order concern as engrave: the smoke pass runs at
        // the END of draw_message, so a smoke painted ON the head cell
        // would hide the char until the smoke drifts clear).
        let cy = line.saturating_sub(1) as f32 + 0.5;
        let mut spawned = 0usize;
        for p in &mut self.scorch.smoke {
            if spawned >= SCORCH_SMOKE_PER_HEAD {
                break;
            }
            if p.active {
                continue;
            }
            // Upward drift (vy negative) with ±20% speed variance +
            // small horizontal sway (±0.5 cells/sec) — the lazy
            // smoke-wisp feel, not ballistic debris.
            let speed = SCORCH_SMOKE_SPEED * (0.8 + self.rand_chance.sample(&mut self.mt) * 0.4);
            let sway = (self.rand_chance.sample(&mut self.mt) - 0.5) * 1.0;
            p.active = true;
            p.x = cx;
            p.y = cy;
            p.vx = sway;
            p.vy = -speed;
            p.birth = now;
            p.sim_age = 0.0;
            spawned += 1;
        }
        self.scorch.active_count = self.scorch.active_count.saturating_add(spawned);
    }

    /// Physics update + render of the active smoke. O(active) with
    /// an O(1) early-out when the pool is idle — the common case for
    /// the other eight styles (this pass is only wired up for
    /// `scorch`, but the early-out also covers scorch runs between
    /// puffs once the reveal completes and the last smoke expires).
    fn draw_scorch_smoke(&mut self, frame: &mut Frame, now: Instant) {
        if self.scorch.active_count == 0 {
            // Keep the timestamp fresh so the first frame after a puff
            // does not integrate a huge dt (mirrors
            // apply_quantum_ripple's early-out).
            self.scorch.last_update = now;
            return;
        }

        // S-master-HUNT-22: real-time particle physics (same contract as
        // apply_quantum_ripple). Smoke integrates the ACTUAL delta since
        // the last update, bounded only by the
        // PARTICLE_MAX_FRAME_DT_SECS anti-teleport cap, and scaled by
        // resume_blend so smoke decelerates together with the rain
        // during the pause coast-down. While FULLY paused draw_message
        // is not called at all (rain_at early-returns), so smoke
        // simply freezes mid-air. The old `min(1/30)` clamp diluted
        // smoke time on slow terminals (10 FPS VTE advanced only
        // 33ms per 100ms frame), stretching the 700ms plume into
        // ~2.1s of drift.
        let dt = now
            .saturating_duration_since(self.scorch.last_update)
            .as_secs_f32()
            .min(PARTICLE_MAX_FRAME_DT_SECS)
            * self.resume_blend.clamp(0.0, 1.0);
        self.scorch.last_update = now;

        let bg = self.palette.bg;
        // Clamp target, not bounce: smoke lives 700 ms and drifts
        // ~1.75 cells upward from a centered overlay — it never
        // reaches the screen edge in practice; the clamp is pure
        // defense-in-depth for degenerate tiny terminals.
        let max_x = self.cols.saturating_sub(1) as f32;
        let max_y = self.lines.saturating_sub(1) as f32;

        let mut still_active = 0usize;
        for s in &mut self.scorch.smoke {
            if !s.active {
                continue;
            }
            // S-master-HUNT-21/22: use accumulated simulation age
            // instead of real-time age (now - birth). Same real-time
            // clock as apply_quantum_ripple — motion and aging stay
            // in lockstep, and the shared dt is real time (HUNT-22),
            // so smoke completes its drift in the intended wall-clock
            // duration on slow terminals.
            s.sim_age += dt;
            if s.sim_age >= SCORCH_SMOKE_LIFETIME_SECS {
                s.active = false;
                continue;
            }
            let age = s.sim_age;
            // Slow buoyant drift: constant velocity, no bounce —
            // 700 ms is short enough that neither is visible.
            s.x += s.vx * dt;
            s.y += s.vy * dt;
            s.x = s.x.clamp(0.0, max_x);
            s.y = s.y.clamp(0.0, max_y);

            // Smoothstep-down envelope (1 → 0 over the lifetime) —
            // the same decay curve as the engrave spark and the
            // border-touch pulse, so every transient overlay effect
            // shares one visual language. Applied to the gray
            // channel so smoke dims to nothing instead of popping.
            let t = age / SCORCH_SMOKE_LIFETIME_SECS;
            let u = 1.0 - t;
            let env = u * u * (3.0 - 2.0 * u);
            let gray = (SCORCH_SMOKE_GRAY as f32 * env) as u8;
            frame.set_force(
                s.x as u16,
                s.y as u16,
                Cell {
                    ch: '░',
                    fg: Some(Color::Rgb {
                        r: gray,
                        g: gray,
                        b: gray,
                    }),
                    bg,
                    bold: false,
                },
            );
            still_active += 1;
        }
        self.scorch.active_count = still_active;
    }
}

#[cfg(test)]
mod tests {
    use super::super::{content_reveal, index_reveal_count, MsgFillStyle};
    use super::*;

    #[test]
    fn scorch_reveals_at_80ms_per_char() {
        // Same index pacing as typewriter/engrave/hologram/glitch:
        // 319 ms → 3, 320 ms → 4. The `.max(1)` floor keeps the
        // first char visible at t=0.
        let total = 40;
        let count = index_reveal_count(MsgFillStyle::Scorch, Some(319), total);
        assert_eq!(count, 3);
        let count = index_reveal_count(MsgFillStyle::Scorch, Some(320), total);
        assert_eq!(count, 4);
        let count = index_reveal_count(MsgFillStyle::Scorch, Some(0), total);
        assert_eq!(count, 1, "max(1) floor: first char at t=0");
    }

    #[test]
    fn scorch_settles_to_palette_color_after_cool_window() {
        // At age >= SCORCH_COOL_MS (400 ms): tint = None, factor = 1.0.
        let r = content_reveal(MsgFillStyle::Scorch, 0, 1, Some(SCORCH_COOL_MS), 10, 1.0);
        assert!(r.visible);
        assert!(r.tint.is_none(), "tint must be None after cool window");
        assert!(
            (r.factor - 1.0).abs() < 1e-6,
            "factor must be 1.0 after cool window"
        );
    }

    #[test]
    fn scorch_settles_without_timeline() {
        // No timeline (bench/edge): settled immediately — no tint,
        // factor 1.0, no override (same as settled() helper).
        let r = content_reveal(MsgFillStyle::Scorch, 0, 1, None, 10, 1.0);
        assert!(r.visible);
        assert!(r.tint.is_none());
        assert!(r.glyph_override.is_none());
        assert!((r.factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn scorch_head_burns_hot_with_full_ember_tint() {
        // At age 0: factor = 1.0 + SCORCH_HEAD_BOOST = 1.5, ember
        // blend = 1.0 (full ember tint).
        let r = content_reveal(MsgFillStyle::Scorch, 0, 1, Some(0), 10, 1.0);
        assert!(r.visible);
        assert!((r.factor - (1.0 + SCORCH_HEAD_BOOST)).abs() < 1e-6);
        let (_, _, _, blend) = r.tint.expect("head must have an ember tint at age 0");
        assert!(
            (blend - 1.0).abs() < 1e-6,
            "ember blend must be 1.0 at head"
        );
    }

    #[test]
    fn scorch_chars_dim_at_mid_cool_then_recovers() {
        // At age SCORCH_COOL_MS/2 (200 ms): factor = SCORCH_CHAR_DIP
        // (0.8), ember blend = 0.5. The "charred" dim sub-effect.
        let mid = content_reveal(
            MsgFillStyle::Scorch,
            0,
            1,
            Some(SCORCH_COOL_MS / 2),
            10,
            1.0,
        );
        assert!(mid.visible);
        assert!(
            (mid.factor - SCORCH_CHAR_DIP).abs() < 1e-6,
            "factor must be SCORCH_CHAR_DIP at mid-cool"
        );
        let (_, _, _, blend) = mid.tint.expect("mid-cool must still have an ember tint");
        assert!(
            (blend - 0.5).abs() < 1e-6,
            "ember blend must be 0.5 at mid-cool"
        );

        // Just before settle: factor approaching 1.0, blend
        // approaching 0.0.
        let late = content_reveal(
            MsgFillStyle::Scorch,
            0,
            1,
            Some(SCORCH_COOL_MS - 1),
            10,
            1.0,
        );
        assert!(late.visible);
        assert!(
            late.factor > SCORCH_CHAR_DIP,
            "factor must recover past mid-cool"
        );
        let (_, _, _, blend) = late.tint.expect("late-cool must still have a tint");
        assert!(
            blend < 0.5 && blend > 0.0,
            "ember blend must be < 0.5 and > 0 at late cool"
        );
    }

    #[test]
    fn scorch_hidden_until_reveal_count_reaches_the_cell() {
        // Same visibility gate as engrave/hologram/glitch: cell N
        // stays hidden until reveal_count > N. The ember tint must
        // never resurrect an unrevealed cell.
        let r = content_reveal(MsgFillStyle::Scorch, 7, 1, Some(400), 7, 1.0);
        assert!(!r.visible, "cell 7 must stay hidden until reveal_count > 7");
        let r = content_reveal(MsgFillStyle::Scorch, 6, 1, Some(400), 7, 1.0);
        assert!(r.visible, "cell 6 must be visible when reveal_count == 7");
    }

    #[test]
    fn scorch_ember_rgb_is_warm_orange_not_pure_red() {
        // The ember color must be a warm orange (R > G > B with G
        // and B both nonzero), not pure red — pure red reads as
        // "alarm", not "ember". Lock the exact value so a future
        // tuning round can't drift it silently.
        let (r, g, b) = SCORCH_EMBER_RGB;
        assert_eq!(r, 255, "ember R must be full-bright (hot)");
        assert!(
            g > 0 && g < r,
            "ember G must be nonzero but dimmer than R (warm, not red)"
        );
        assert!(
            b > 0 && b < g,
            "ember B must be nonzero but dimmer than G (warm, not red)"
        );
    }

    #[test]
    fn scorch_smoke_constants_hold_research_doc_contract() {
        // Lock the values called out in
        // MSG_FILL_STYLE_EXPANSION_RESEARCH.md §3.C so a future
        // tuning round can't drift them silently.
        assert_eq!(SCORCH_CHAR_MS, 80);
        assert_eq!(SCORCH_COOL_MS, 400);
        assert!((SCORCH_HEAD_BOOST - 0.5).abs() < 1e-6);
        assert!((SCORCH_CHAR_DIP - 0.8).abs() < 1e-6);
        assert_eq!(SCORCH_EMBER_RGB, (255, 100, 30));
        assert_eq!(SCORCH_SMOKE_POOL_SIZE, 16);
        assert_eq!(SCORCH_SMOKE_PER_HEAD, 1);
        assert!((SCORCH_SMOKE_LIFETIME_SECS - 0.70).abs() < 1e-6);
        assert!((SCORCH_SMOKE_SPEED - 2.5).abs() < 1e-6);
        assert_eq!(SCORCH_SMOKE_GRAY, 128);
    }
}
