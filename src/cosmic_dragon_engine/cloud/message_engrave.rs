// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! msg-fill-style `engrave` spark sidecar — extracted from
//! `cloud/message_draw.rs` to keep that file lean.
//!
//! The engrave style's TEXT reveal (burn-in + hot head + heat glow)
//! is stateless like every other style (see `types/msg_fill_style.rs`).
//! The spark burst is not: it needs one usize of bookkeeping ("which
//! head char was last spark-emitting") plus a particle pool. Both live
//! here, behind a single `Cloud::engrave_spark_pass` entry point called
//! at the END of `draw_message`.
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
//!   the reveal completes, is paused, or sits in the 6 s intro delay
//!   (elapsed does not advance → the head does not move → no burst).
//! - Steady state: bursts every 80 ms with 200 ms lifetime →
//!   9-12 concurrent sparks, 4x under the pool cap.
//! - `--no-effects` (PERF-4): spawning is gated on `effects_enabled`
//!   (same contract as every particle subsystem); already-active
//!   sparks decay out naturally.
//! - Bench mode: never runs — `draw_message` itself is skipped in
//!   bench mode (Z-6), so the whole pass is dead code on that path.
//!
//! Implemented as a separate `impl Cloud` block (Rust allows multiple
//! impl blocks across files for the same type).

use std::time::Instant;

use crossterm::style::Color;
use rand::distr::Distribution;

use crate::cell::Cell;
use crate::constants::*;
use crate::frame::Frame;

use super::Cloud;

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

impl Cloud {
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
        // char). While paused or inside the 6 s intro delay the elapsed
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
