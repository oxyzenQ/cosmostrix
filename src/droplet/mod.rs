// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Individual droplet (rain stream) simulation.
//!
//! Each droplet represents a single column of falling rain — a vertical
//! stream of characters with a bright head, fading trail, and optional
//! tail. Droplets are recycled via an object pool (`Vec<Droplet>` in Cloud)
//! to avoid per-spawn allocations.
//!
//! ## Physics
//!
//! Droplets accelerate under gravity toward a terminal velocity (configurable
//! via `--speed`). A sinusoidal turbulence overlay adds organic velocity
//! variation so streams don't move at perfectly constant speed.
//!
//! ## Visual Effects Pipeline
//!
//! During `draw()`, each cell's foreground color passes through a stack of
//! composable effects applied in order:
//! 1. Transition energy glow (new-palette streams)
//! 2. Head bloom (cells near the stream head)
//! 3. Parallax layer brightness (far layers dimmer)
//! 4. Atmospheric glyph dimming (far layer simplification)
//! 5. Depth fog vignette (top/bottom edge dimming)
//! 6. Cursor glow (mouse proximity brightness)
//! 7. Click flash (expanding ring from click point)
//! 8. Head brightness modulation
//! 9. Head self-bloom (layer-scaled 55% white blend)
//! 10. Rain shadow (bottom 20% quadratic fade)
//! 11. Viewport edge fade (top/bottom cinematic dissolve)
//! 12. Cinematic radial vignette (corner darkening, applied last)
//!
//! Each effect reads from `DrawCtx` and modifies the color via the palette
//! blending functions in `palette.rs`.

use std::time::{Duration, Instant};

use crate::constants::{
    ADVANCE_REMAINDER_CAP, DROPLET_GRAVITY, DROPLET_TERMINAL_VELOCITY_MULT,
    FRACTIONAL_HEAD_BRIGHTNESS_AMP, HEAD_LINGER_BRIGHTNESS_MS, STARTUP_EASE_TAU,
    STARTUP_VELOCITY_FRACTION, TURBULENCE_AMPLITUDE, TURBULENCE_FREQ,
};
// DH-05: PARALLAX_LAYERS is only used by compounded_brightness which is
// #[cfg(test)]-gated. Import it conditionally to avoid unused-import warnings.
#[cfg(test)]
use crate::constants::PARALLAX_LAYERS;
#[cfg(test)]
use crate::constants::RAIN_SHADOW_LAYER_MULT;
// Re-export brightness factors so existing `crate::droplet::viewport_edge_fade`
// etc. references continue to resolve after extraction to brightness_factors.rs.
// viewport_edge_fade + crt_vignette_factor are used by production code
// (cloud/spawn.rs, droplet draw path). rain_shadow_factor + vignette_factor
// are used by tests via `crate::droplet::*` — gate those with #[cfg(test)].
pub(crate) use crate::brightness_factors::{crt_vignette_factor, viewport_edge_fade};
#[cfg(test)]
pub(crate) use crate::brightness_factors::{rain_shadow_factor, vignette_factor};
#[cfg(test)]
use crate::constants::VIGNETTE_LAYER_MULT;

#[cfg(test)]
pub(crate) fn compounded_brightness(
    col: u16,
    line: u16,
    cols: u16,
    lines: u16,
    layer: usize,
) -> f32 {
    let layer = layer.min(PARALLAX_LAYERS - 1);
    let shadow_mult = RAIN_SHADOW_LAYER_MULT
        .get(layer)
        .copied()
        .filter(|m| *m > 0.0)
        .unwrap_or(0.0);
    let vignette_mult = VIGNETTE_LAYER_MULT
        .get(layer)
        .copied()
        .filter(|m| *m > 0.0)
        .unwrap_or(0.0);

    // Per-layer scaling mirrors the render path's
    // `1.0 - (1.0 - raw) * LAYER_MULT[layer]` formula: when LAYER_MULT=0.0
    // (front layer), the effect is fully suppressed (factor=1.0); when
    // LAYER_MULT=1.0 (mid/back), the raw effect applies unchanged.
    let shadow_raw = crate::brightness_factors::rain_shadow_factor(line, lines);
    let shadow = 1.0 - (1.0 - shadow_raw) * shadow_mult;

    let edge = crate::brightness_factors::viewport_edge_fade(line, lines);

    let vignette_raw = crate::brightness_factors::vignette_factor(col, line, cols, lines);
    let radial = 1.0 - (1.0 - vignette_raw) * vignette_mult;

    let crt = crate::brightness_factors::crt_vignette_factor(line, lines);

    shadow * edge * radial * crt
}

#[derive(Clone, Debug)]
pub(crate) struct Droplet {
    pub is_alive: bool,
    pub is_head_crawling: bool,
    pub is_tail_crawling: bool,

    /// Column this droplet is bound to; `u16::MAX` when inactive (recycled).
    pub bound_col: u16,
    pub head_put_line: u16,
    pub head_cur_line: u16,

    /// RAIN_BORDER_TOUCH_GLOW: snapshot of `head_put_line` from the previous
    /// frame, used to detect the transition (prev < top && now >= top) when
    /// the head crosses the message overlay's top border. Reset to 0 on
    /// `activate()` so a freshly-spawned droplet doesn't fire a spurious
    /// touch on its first frame.
    pub prev_head_put_line: u16,

    pub tail_put_line: Option<u16>,
    pub tail_cur_line: u16,

    /// Line at which the head stops; `u16::MAX` sentinel when inactive.
    pub end_line: u16,
    /// Index into the char_pool; `u16::MAX` sentinel when inactive.
    pub char_pool_idx: u16,
    /// Visual length of the droplet trail; `u16::MAX` sentinel when inactive.
    pub length: u16,
    pub chars_per_sec: f32,

    pub advance_remainder: f32,

    /// Current velocity (chars/sec), increases with gravity.
    pub velocity: f32,

    /// Which parallax layer this droplet belongs to (0=far, 1=mid, 2=near).
    pub layer: u8,

    /// Number of tail cells for this droplet. For front layer (2), this is
    /// a dynamic value in [1, 3] set at spawn time via random variation —
    /// creates organic tail length rhythm. For mid/back layers, this is 1
    /// (preserving the existing single-cell tail behavior).
    ///
    /// Used in draw() to assign CharLoc::TailN(i) for the first `tail_cells`
    /// cells of the visible trail, mapping them to palette tail color stops.
    pub tail_cells: u8,

    /// Which palette generation slot this droplet was born with.
    /// Streams retain their birth palette for their entire lifecycle;
    /// the new palette propagates only through newly spawned streams.
    pub palette_slot: u8,

    /// Turbulence phase offset (determines unique oscillation pattern).
    pub turb_phase: f32,
    /// Turbulence accumulator (elapsed time for this droplet's oscillation).
    pub turb_time: f32,

    pub last_time: Option<Instant>,
    pub head_stop_time: Option<Instant>,
    pub time_to_linger: Duration,
    /// Birth timestamp for cinematic startup easing (set once in activate).
    birth_time: Option<Instant>,
}

impl Droplet {
    pub(crate) fn new() -> Self {
        Self {
            is_alive: false,
            is_head_crawling: false,
            is_tail_crawling: false,
            bound_col: u16::MAX,
            head_put_line: 0,
            head_cur_line: 0,
            prev_head_put_line: 0,
            tail_put_line: None,
            tail_cur_line: 0,
            end_line: u16::MAX,
            char_pool_idx: u16::MAX,
            length: u16::MAX,
            chars_per_sec: 0.0,

            advance_remainder: 0.0,
            velocity: 0.0,
            layer: 0,
            tail_cells: 1,
            palette_slot: 0,
            turb_phase: 0.0,
            turb_time: 0.0,

            last_time: None,
            head_stop_time: None,
            time_to_linger: Duration::from_millis(0),
            birth_time: None,
        }
    }

    pub(crate) fn activate(&mut self, now: Instant) {
        self.is_alive = true;
        self.is_head_crawling = true;
        self.is_tail_crawling = true;
        // When SPAWN_PHASE_JITTER is enabled, advance_remainder is set to a
        // random value by the caller (Cloud::spawn_droplets) AFTER activate()
        // resets it to 0.0. This ordering ensures activate() always produces
        // a consistent initial state, and jitter is layered on top.
        self.advance_remainder = 0.0;
        // Cinematic startup: begin at a low fraction and ease into full speed
        // via exponential approach in advance(). This eliminates the jarring
        // instant-snap from the old 0.3× initial velocity.
        self.velocity = self.chars_per_sec * STARTUP_VELOCITY_FRACTION;
        self.turb_time = 0.0;
        self.last_time = Some(now);
        self.birth_time = Some(now);
    }

    /// Apply spawn phase jitter: set a random fractional advance offset so
    /// this droplet's row advances are staggered relative to other droplets.
    /// Without jitter, all droplets start at advance_remainder=0 and advance
    /// on the same frame cadence, creating a robotic synchronized march.
    /// With jitter, each droplet's head brightens and advances at a different
    /// phase, making the rain feel organic and alive.
    #[inline]
    pub(crate) fn apply_phase_jitter(&mut self, offset: f32) {
        self.advance_remainder = offset.clamp(0.0, 1.0);
    }

    pub(crate) fn increment_time(&mut self, delta: Duration) {
        if let Some(t) = self.last_time.as_mut() {
            *t += delta;
        }
        if let Some(t) = self.head_stop_time.as_mut() {
            *t += delta;
        }
        if let Some(t) = self.birth_time.as_mut() {
            *t += delta;
        }
    }

    #[inline]
    pub(crate) fn advance(&mut self, now: Instant, lines: u16, time_scale: f32) -> bool {
        let Some(last) = self.last_time else {
            self.last_time = Some(now);
            return false;
        };

        let elapsed = now.saturating_duration_since(last);
        // defense-in-depth clamp — the caller (rain.rs:281-294) already
        // clamps via max_sim_delta, but if max_sim_delta is ever disabled or
        // a future callsite bypasses it, this prevents position teleport on
        // frame timing spikes (GC pause, OS stall).
        let elapsed_sec = elapsed.as_secs_f32().min(1.0 / 30.0);
        // Apply resume time-scale: simulation clock runs in slow motion
        // during the smoothstep transition. Gravity, turbulence, and position
        // all advance at the scaled rate.
        let effective_sec = elapsed_sec * time_scale;

        // Apply gravity: accelerate toward terminal velocity.
        // During startup (first ~0.5s), use exponential ease-in for a
        // cinematic ramp instead of linear gravity. After startup,
        // standard linear gravity takes over for natural feel.
        let terminal_vel = self.chars_per_sec * DROPLET_TERMINAL_VELOCITY_MULT;
        let stream_age = self
            .birth_time
            .map(|bt| now.saturating_duration_since(bt).as_secs_f32())
            .unwrap_or(1.0); // fallback: skip easing if no birth_time
        if stream_age < STARTUP_EASE_TAU * 3.0 {
            // Exponential ease: v → target × (1 - e^(-t/τ))
            // After 3τ, we're at 95% and switch to linear gravity.
            let eased_target = terminal_vel * (1.0 - (-stream_age / STARTUP_EASE_TAU).exp());
            self.velocity = self.velocity.max(eased_target);
        } else {
            // Gravity accumulates at time-scaled rate for smooth velocity ramp.
            self.velocity = (self.velocity + DROPLET_GRAVITY * effective_sec).min(terminal_vel);
        }

        // Subtle velocity turbulence: smooth sinusoidal drift (time-scaled).
        self.turb_time += effective_sec;
        let turb_drift =
            (self.turb_time * TURBULENCE_FREQ * std::f32::consts::TAU + self.turb_phase).sin()
                * TURBULENCE_AMPLITUDE
                * self.chars_per_sec;
        let turb_velocity = (self.velocity + turb_drift).max(0.0);

        // Position delta uses effective (time-scaled) elapsed time.
        // When time_scale=0.0 (just resumed), no movement occurs.
        // When time_scale=1.0 (fully active), full speed is restored.
        let delta = (turb_velocity * effective_sec).max(0.0);
        // Clamp the accumulated remainder to prevent high-speed droplets
        // from advancing too many rows in one frame, which dumps cells
        // into bottom rows and creates permanent "concrete wall" residue.
        let clamped_remainder = self.advance_remainder.min(ADVANCE_REMAINDER_CAP);
        let total = clamped_remainder + delta;
        let whole = total.floor();
        self.advance_remainder = (total - whole).min(ADVANCE_REMAINDER_CAP);
        let chars_advanced = whole as u16;
        if chars_advanced == 0 {
            self.last_time = Some(now);
            return false;
        }

        if self.is_head_crawling {
            self.head_put_line = self.head_put_line.saturating_add(chars_advanced);
            if self.head_put_line > self.end_line {
                self.head_put_line = self.end_line;
            }

            if self.head_put_line == self.end_line {
                self.is_head_crawling = false;
                if self.head_stop_time.is_none() {
                    self.head_stop_time = Some(now);
                    if self.time_to_linger > Duration::from_millis(0) {
                        self.is_tail_crawling = false;
                    }
                }
            }
        }

        if self.is_tail_crawling
            && (self.head_put_line >= self.length || self.head_put_line >= self.end_line)
        {
            let next_tail = match self.tail_put_line {
                Some(v) => v.saturating_add(chars_advanced),
                None => chars_advanced,
            };

            let mut next_tail = next_tail;
            if next_tail > self.end_line {
                next_tail = self.end_line;
            }
            self.tail_put_line = Some(next_tail);

            let thresh_line = lines / 4;
            if self.tail_cur_line <= thresh_line && next_tail > thresh_line {
                self.last_time = Some(now);
                return true;
            }
        }

        if !self.is_tail_crawling {
            if let Some(stop) = self.head_stop_time {
                if now.saturating_duration_since(stop) >= self.time_to_linger {
                    self.is_tail_crawling = true;
                }
            }
        }

        if self.tail_put_line == Some(self.head_put_line) {
            self.is_alive = false;
        }

        self.last_time = Some(now);
        false
    }

    /// Returns 0.0–1.0 indicating how much fractional progress the droplet
    /// has made toward its next row advance. This is used to create per-frame
    /// visual variation (brightness ramp, bloom modulation) even when the
    /// head hasn't moved to a new row — the key to perceived smoothness.
    #[inline]
    pub(crate) fn fractional_progress(&self) -> f32 {
        self.advance_remainder.clamp(0.0, 1.0)
    }

    /// Returns 0.0–1.0 indicating how "bright" the head cell should appear.
    /// During crawling: 1.0 + fractional progress ramp. After head stops:
    /// exponential decay from 1.0→0.0 over HEAD_LINGER_BRIGHTNESS_MS.
    ///
    /// The fractional progress ramp makes the head progressively brighter
    /// as it approaches the next row advance, creating a subtle "energy
    /// building" pulse. This means every frame has a visible brightness
    /// change on the head cell, even when the row position hasn't changed —
    /// transforming the perceived update rate from ~8 FPS (row-quantized)
    /// to 60 FPS (brightness-interpolated).
    #[inline]
    fn head_brightness(&self, now: Instant) -> f32 {
        if self.is_head_crawling {
            // Fractional progress creates a subtle brightness ramp.
            // When advance_remainder is 0 (just advanced), brightness is 1.0.
            // When advance_remainder is ~1 (about to advance), brightness is
            // 1.0 + FRACTIONAL_HEAD_BRIGHTNESS_AMP (e.g., 1.15).
            // This "energy building" effect makes every frame feel different.
            return 1.0 + self.fractional_progress() * FRACTIONAL_HEAD_BRIGHTNESS_AMP;
        }
        if let Some(stop) = self.head_stop_time {
            let elapsed_ms = now.saturating_duration_since(stop).as_secs_f32() * 1000.0;
            let window = HEAD_LINGER_BRIGHTNESS_MS as f32;
            if elapsed_ms < window {
                // Exponential decay: e^(-3t/T) — at t=0: 1.0, at t=T: ~0.05
                return (-3.0 * elapsed_ms / window).exp();
            }
        }
        0.0
    }
}

// ─── Stabilization regression tests ─────────────────────────────────────────
//
// These tests lock in the three silent-override bug fixes.
// Each test replays the exact arithmetic the production pipeline performs
// on a per-pixel basis, with a multiplier drawn from the central control
// file. If anyone reverts a fix (or accidentally introduces a new
// `as i32` truncation on a fractional multiplier), the corresponding
// test fails immediately at CI time.
//
// The tests do not exercise the full draw pipeline — that would require
// building a complete DrawCtx and Frame. Instead they verify the
// mathematical invariant each fix restored: that a non-trivial
// multiplier produces a non-trivial delta on the output channel.

#[cfg(test)]
#[path = "../../test/droplet/tests.rs"]
mod tests;

// v50.0.0-beta.7 LOC refactor: Droplet::draw method extracted to draw.rs
// to keep this file under the 800-LOC hard cap.
mod draw;
