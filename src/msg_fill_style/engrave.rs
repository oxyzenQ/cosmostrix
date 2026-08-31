// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! msg-fill-style `engrave` — laser engraving: burn-in + hot head +
//! heat trail + spark bursts.
//!
//! Everything about the engrave style lives in this one file: the
//! stateless reveal math (constants + `reveal` + budget + border +
//! text-progress hooks) AND the stateful spark sidecar (the only
//! stateful member of the style family).
//!
//! ## Reveal math (stateless, like every other style)
//!
//! Chars burn in at `ENGRAVE_CHAR_MS` (80 ms) pacing. A char appears
//! at FULL brightness the instant the head reaches it (no 30%
//! fade-in — a laser burns text in, it does not fade it in), then
//! cools from `(1 + ENGRAVE_BOOST)` (2x) back to 1.0 over
//! `ENGRAVE_HEAT_MS` (300 ms). The last ~4 chars are always cooling
//! at any moment, forming the heat trail behind the engraving head.
//!
//! ## Spark sidecar (stateful)
//!
//! The spark burst needs one usize of bookkeeping ("which head char
//! was last spark-emitting") plus a particle pool. Both live here,
//! behind a single `Cloud::engrave_spark_pass` entry point called at
//! the END of `draw_message` (after the halo row) so sparks render ON
//! TOP of the overlay text: the "engraving head throws debris" look.
//!
//! ## Why a dedicated pool instead of reusing the quantum pool
//!
//! The obvious reuse — `spawn_border_spark` into the shared
//! `quantum_particles` pool — is architecturally wrong here:
//! `apply_quantum_ripple` runs in `rain_at` BEFORE
//! `post_rain_processing` → `draw_message`, and `draw_message`
//! `set_force`-paints every overlay cell (glyph or space) on top. A
//! pool-shared spark would be overdrawn inside the message box —
//! invisible exactly where the engraving head lives. The border-touch
//! spark does not hit this because its particles spawn ON the top
//! border and immediately fly UP, out of the overlay region.
//!
//! A dedicated pool also keeps the engrave effect from competing with
//! mouse-click ripples for the 96 shared slots, and vice versa.
//!
//! ## Bounds and cost (LTS contract)
//!
//! - Pool: `ENGRAVE_SPARK_POOL_SIZE` (48) slots pre-allocated once at
//!   `Cloud::new` — zero per-frame allocation, `O(active)` per frame.
//! - Spawn cadence: one burst of `ENGRAVE_SPARKS_PER_HEAD` (3) per
//!   NEWLY revealed char — never per frame. Frame-rate independent,
//!   no double-spawn after skipped frames (a jumped head fires ONE
//!   burst at its new position), and bursts stop automatically when
//!   the reveal completes, is paused, or sits in the intro lead
//!   (MESSAGE_INTRO_LEAD, armed only while a cinematic plays; elapsed
//!   does not advance → the head does not move → no burst).
//! - Steady state: bursts every 80 ms with 200 ms lifetime →
//!   9-12 concurrent sparks, 4x under the pool cap.
//! - `--no-effects` (PERF-4): spawning is gated on `effects_enabled`
//!   (same contract as every particle subsystem); already-active
//!   sparks decay out naturally.
//! - Bench mode: never runs — `draw_message` itself is skipped in
//!   bench mode (Z-6), so the whole pass is dead code on that path.
//!
//! Border: lags text with the shared `t^1.5` ease-out curve.

use std::time::Instant;

use crossterm::style::Color;
use rand::distr::Distribution;

use super::{index_fraction, index_pacing, lagged_border, CellReveal};
use crate::cell::Cell;
use crate::frame::Frame;

// ── Reveal math (stateless) ────────────────────────────────────────────────

/// Per-character reveal stagger (same 80 ms pacing as typewriter,
/// kept as its own constant so the two can diverge).
pub(crate) const ENGRAVE_CHAR_MS: usize = 80;
/// Head boost above settled brightness (1.0 → peak factor 2.0,
/// routed through the unclamped boost path like pulse).
pub(crate) const ENGRAVE_BOOST: f32 = 1.0;
/// Heat-glow decay window behind the engraving head.
pub(crate) const ENGRAVE_HEAT_MS: usize = 300;

// ── Spark sidecar constants (moved from `types/constants.rs` in the
// msg_fill_style directory refactor — they are engrave-only tuning,
// not shared engine tuning) ─────────────────────────────────────────────────

/// Engrave spark pool size. Bursts fire once per newly revealed char
/// (every ENGRAVE_CHAR_MS = 80 ms) and live
/// ENGRAVE_SPARK_LIFETIME_SECS (0.20 s), so steady-state concurrency is
/// ~3 bursts x 3 particles = 9-12 active. 48 slots = 4x headroom (also
/// the owner-advisor-requested cap: "cap concurrent sparks at 48").
pub(crate) const ENGRAVE_SPARK_POOL_SIZE: usize = 48;

/// Particles per engrave burst (one burst per newly engraved char).
pub(crate) const ENGRAVE_SPARKS_PER_HEAD: usize = 3;

/// Engrave spark lifetime in seconds. 200 ms = tight, snappy debris.
pub(crate) const ENGRAVE_SPARK_LIFETIME_SECS: f32 = 0.20;

/// Engrave spark speed in cells/second. 10.0 with the masterclass
/// ±10% variance → ~2 cells of travel per spark lifetime — a spray
/// zone around the head without polluting the rest of the overlay.
pub(crate) const ENGRAVE_SPARK_SPEED: f32 = 10.0;

/// Per-cell reveal: burn-in at full brightness, then heat decay.
pub(super) fn reveal(
    content_idx: usize,
    elapsed_ms: Option<usize>,
    reveal_count: usize,
) -> CellReveal {
    // Burn-in reveal: a char appears at FULL brightness the instant
    // the head reaches it (no 30% fade-in — a laser burns text in, it
    // does not fade it in), then cools from (1 + ENGRAVE_BOOST) back
    // to 1.0 over ENGRAVE_HEAT_MS. The last ~4 chars are always
    // cooling at any moment, forming the heat trail behind the
    // engraving head.
    if content_idx < reveal_count {
        let reveal_at = content_idx * ENGRAVE_CHAR_MS;
        let heat = match elapsed_ms {
            None => 1.0,
            Some(ms) => {
                let age = ms.saturating_sub(reveal_at);
                if age >= ENGRAVE_HEAT_MS {
                    1.0
                } else {
                    let decay = 1.0 - age as f32 / ENGRAVE_HEAT_MS as f32;
                    1.0 + ENGRAVE_BOOST * decay
                }
            }
        };
        CellReveal {
            visible: true,
            factor: heat,
            slide_rows: 0,
            glyph_override: None,
            tint: None,
        }
    } else {
        CellReveal::hidden()
    }
}

/// Index budget: 80 ms/char with the pre-v51 `.max(1)` floor.
pub(super) fn reveal_budget(elapsed_ms: Option<usize>, total_text: usize) -> usize {
    index_pacing(ENGRAVE_CHAR_MS, elapsed_ms, total_text)
}

/// Border lags text (t^1.5) — the pre-v51 cinematic curve.
pub(super) fn border_progress(text_progress: f32) -> f32 {
    lagged_border(text_progress)
}

/// Text progress: revealed-cell fraction.
pub(super) fn text_progress(reveal_count: usize, total_text: usize) -> f32 {
    index_fraction(reveal_count, total_text)
}

// ── Spark sidecar (stateful) ───────────────────────────────────────────────

/// One engrave spark particle. Plain data — no trail ring buffer
/// (sparks live 200 ms and travel ~2 cells; a trail would be noise),
/// no per-particle lifetime field (all sparks share
/// `ENGRAVE_SPARK_LIFETIME_SECS`).
#[derive(Clone, Copy)]
pub(crate) struct EngraveSpark {
    pub(crate) active: bool,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) vx: f32,
    pub(crate) vy: f32,
    pub(crate) birth: Instant,
    /// Glyph: '·' (debris, ~70%) or '*' (bright fleck, ~30%).
    pub(crate) ch: char,
    /// Palette head color snapshotted at burst time (white-hot on most
    /// schemes; follows the active theme like the border spark does).
    pub(crate) r: u8,
    pub(crate) g: u8,
    pub(crate) b: u8,
}

/// All mutable engrave state, grouped so `Cloud` grows by a single
/// field (the struct lives in the cloud state namespace; see
/// `cloud/mod.rs`). `last_head` doubles as the movement detector:
/// `usize::MAX` = "no burst fired yet" (fresh overlay / fresh reveal).
pub(crate) struct EngraveState {
    pub(crate) sparks: Vec<EngraveSpark>,
    /// Active spark count (incremental, O(1) early-out in the pass).
    pub(crate) active_count: usize,
    /// Content index of the last head char that fired a burst.
    pub(crate) last_head: usize,
    /// Timestamp of the previous physics update (dt clamping).
    pub(crate) last_update: Instant,
}

impl EngraveState {
    pub(crate) fn new(now: Instant) -> Self {
        Self {
            sparks: vec![
                EngraveSpark {
                    active: false,
                    x: 0.0,
                    y: 0.0,
                    vx: 0.0,
                    vy: 0.0,
                    birth: now,
                    ch: '·',
                    r: 255,
                    g: 255,
                    b: 255,
                };
                ENGRAVE_SPARK_POOL_SIZE
            ],
            active_count: 0,
            last_head: usize::MAX,
            last_update: now,
        }
    }

    /// Drop everything and re-arm the movement detector. Called from
    /// `reset_message` (new overlay layout) and from the reveal
    /// restart paths (`restart_message_typewriter`, style change) so
    /// the first char of the fresh reveal fires its burst.
    pub(crate) fn reset(&mut self) {
        for s in &mut self.sparks {
            s.active = false;
        }
        self.active_count = 0;
        self.last_head = usize::MAX;
    }
}

impl crate::cloud::Cloud {
    /// Engrave spark pass — one entry point, called at the very end of
    /// `draw_message` (after the halo row) so sparks render ON TOP of
    /// the overlay text: the "engraving head throws debris" look.
    ///
    /// `head_pos`/`head_idx` describe the most recently revealed
    /// content cell (captured in the main draw loop); `elapsed_ms` is
    /// the same reveal timeline the text uses. Both `None`/sentinel
    /// cases simply skip spawning — the render half still runs so
    /// already-airborne sparks decay out.
    pub(crate) fn engrave_spark_pass(
        &mut self,
        frame: &mut Frame,
        now: Instant,
        head_pos: Option<(u16, u16)>,
        head_idx: usize,
        elapsed_ms: Option<usize>,
    ) {
        // Spawn on head MOVEMENT only (one burst per newly engraved
        // char). While paused or inside the intro lead the elapsed
        // timeline stalls, the head stops advancing, and `head_idx`
        // stops changing — bursts stop with it. No timeline (None)
        // means bench/edge paths: never spawn, keep decaying.
        if let (Some((col, line)), Some(_)) = (head_pos, elapsed_ms) {
            if head_idx != self.engrave.last_head {
                self.engrave.last_head = head_idx;
                if self.effects_enabled {
                    // Palette head color (near-white on most schemes):
                    // white-hot sparks that follow the active theme.
                    // Same panic-safe snapshot chain as
                    // detect_border_touch — never `.last().unwrap()`.
                    let head_rgb = self
                        .palette
                        .colors
                        .last()
                        .copied()
                        .and_then(crate::palette::decode_color)
                        .unwrap_or((255, 255, 255));
                    self.spawn_engrave_burst(col, line, head_rgb, now);
                }
            }
        }
        self.draw_engrave_sparks(frame, now);
    }

    /// Activate up to `ENGRAVE_SPARKS_PER_HEAD` inactive pool slots at
    /// `(col, line)`, flying outward on a full circle (engraving
    /// ejects debris in all directions — downward embers included).
    /// Pool-full → burst silently truncated (same pattern as
    /// `spawn_quantum_ripple` / `spawn_border_spark`).
    fn spawn_engrave_burst(&mut self, col: u16, line: u16, head_rgb: (u8, u8, u8), now: Instant) {
        let cx = col as f32 + 0.5;
        let cy = line as f32 + 0.5;
        let mut spawned = 0usize;
        for p in &mut self.engrave.sparks {
            if spawned >= ENGRAVE_SPARKS_PER_HEAD {
                break;
            }
            if p.active {
                continue;
            }
            // Full-circle fan + masterclass ±10% speed variance (the
            // same 0.9..1.1 band quantum ripples and border sparks use
            // — enough for organic feel, no visible stratification).
            let angle: f32 = self.rand_chance.sample(&mut self.mt) * std::f32::consts::TAU;
            let speed = ENGRAVE_SPARK_SPEED * (0.9 + self.rand_chance.sample(&mut self.mt) * 0.2);
            // Debris mix: mostly middle dots, occasional bright flecks.
            let ch = if self.rand_chance.sample(&mut self.mt) < 0.7 {
                '·'
            } else {
                '*'
            };
            p.active = true;
            p.x = cx;
            p.y = cy;
            p.vx = speed * angle.cos();
            p.vy = speed * angle.sin();
            p.birth = now;
            p.ch = ch;
            p.r = head_rgb.0;
            p.g = head_rgb.1;
            p.b = head_rgb.2;
            spawned += 1;
        }
        self.engrave.active_count = self.engrave.active_count.saturating_add(spawned);
    }

    /// Physics update + render of the active sparks. O(active) with an
    /// O(1) early-out when the pool is idle — the common case for the
    /// other six styles (this pass is only wired up for `engrave`, but
    /// the early-out also covers engrave runs between bursts once the
    /// reveal completes and the last sparks expire).
    fn draw_engrave_sparks(&mut self, frame: &mut Frame, now: Instant) {
        if self.engrave.active_count == 0 {
            // Keep the timestamp fresh so the first frame after a burst
            // does not integrate a huge dt (mirrors
            // apply_quantum_ripple's early-out).
            self.engrave.last_update = now;
            return;
        }

        // Frame-rate-independent motion: real dt since the last
        // update, clamped to 1/30 s, scaled by resume_blend so sparks
        // decelerate together with the rain during the pause
        // coast-down (same contract as apply_quantum_ripple). While
        // FULLY paused draw_message is not called at all (rain_at
        // early-returns), so sparks simply freeze mid-air.
        let dt = now
            .saturating_duration_since(self.engrave.last_update)
            .as_secs_f32()
            .min(1.0 / 30.0)
            * self.resume_blend.clamp(0.0, 1.0);
        self.engrave.last_update = now;

        let bg = self.palette.bg;
        // Clamp target, not bounce: sparks live 200 ms and travel ~2
        // cells from a centered overlay — they never reach the screen
        // edge in practice; the clamp is pure defense-in-depth for
        // degenerate tiny terminals (saturating_sub guards 0×0).
        let max_x = self.cols.saturating_sub(1) as f32;
        let max_y = self.lines.saturating_sub(1) as f32;

        let mut still_active = 0usize;
        for s in &mut self.engrave.sparks {
            if !s.active {
                continue;
            }
            let age = now.saturating_duration_since(s.birth).as_secs_f32();
            if age >= ENGRAVE_SPARK_LIFETIME_SECS {
                s.active = false;
                continue;
            }
            // Ballistic debris: constant velocity, no decay/bounce —
            // 200 ms is too short for either to be visible.
            s.x += s.vx * dt;
            s.y += s.vy * dt;
            s.x = s.x.clamp(0.0, max_x);
            s.y = s.y.clamp(0.0, max_y);

            // Smoothstep-down envelope (1 → 0 over the lifetime) — the
            // same decay curve as the border-touch pulse, so every
            // transient overlay effect shares one visual language.
            let t = age / ENGRAVE_SPARK_LIFETIME_SECS;
            let u = 1.0 - t;
            let env = u * u * (3.0 - 2.0 * u);
            let (r, g, b) = crate::chroma_dragon_engine::legacy::scale_rgb(s.r, s.g, s.b, env);
            frame.set_force(
                s.x as u16,
                s.y as u16,
                Cell {
                    ch: s.ch,
                    fg: Some(Color::Rgb { r, g, b }),
                    bg,
                    bold: false,
                },
            );
            still_active += 1;
        }
        self.engrave.active_count = still_active;
    }
}

#[cfg(test)]
mod tests {
    use super::super::{content_reveal, index_reveal_count, MsgFillStyle};
    use super::*;

    #[test]
    fn engrave_reveals_at_80ms_per_char() {
        // Same index pacing as typewriter: 319 ms → 3 chars, 320 ms → 4.
        let total = 40;
        let count = index_reveal_count(MsgFillStyle::Engrave, Some(319), total);
        assert_eq!(count, 3);
        let count = index_reveal_count(MsgFillStyle::Engrave, Some(320), total);
        assert_eq!(count, 4);
        let count = index_reveal_count(MsgFillStyle::Engrave, Some(0), total);
        assert_eq!(count, 1, "max(1) floor: first char at t=0");
    }

    #[test]
    fn engrave_chars_burn_in_hot_and_cool_off() {
        // Age 0: burned in at (1 + ENGRAVE_BOOST) = 2.0 — NOT the 30%
        // fade-in start the typewriter family uses.
        let head = content_reveal(MsgFillStyle::Engrave, 0, 1, Some(0), 10, 1.0);
        assert!(head.visible);
        assert!((head.factor - (1.0 + ENGRAVE_BOOST)).abs() < 1e-6);
        // Mid-decay: age 150 of 300 ms → 1 + 1.0 * 0.5 = 1.5.
        let mid = content_reveal(MsgFillStyle::Engrave, 0, 1, Some(150), 10, 1.0);
        assert!((mid.factor - 1.5).abs() < 1e-6);
        // Cooled: age >= ENGRAVE_HEAT_MS → settled at 1.0.
        let cooled = content_reveal(MsgFillStyle::Engrave, 0, 1, Some(ENGRAVE_HEAT_MS), 10, 1.0);
        assert!((cooled.factor - 1.0).abs() < 1e-6);
    }

    #[test]
    fn engrave_hidden_until_reveal_count_reaches_the_cell() {
        let r = content_reveal(MsgFillStyle::Engrave, 7, 1, Some(400), 7, 1.0);
        assert!(!r.visible, "cell 7 must stay hidden until reveal_count > 7");
        let r = content_reveal(MsgFillStyle::Engrave, 6, 1, Some(400), 7, 1.0);
        assert!(r.visible);
    }

    #[test]
    fn spark_pool_constants_hold_the_owner_contract() {
        // Owner-advisor contract: cap concurrent sparks at 48; one
        // burst of 3 per newly engraved char; 200 ms lifetime.
        assert_eq!(ENGRAVE_SPARK_POOL_SIZE, 48);
        assert_eq!(ENGRAVE_SPARKS_PER_HEAD, 3);
        assert!((ENGRAVE_SPARK_LIFETIME_SECS - 0.20).abs() < 1e-6);
    }
}
