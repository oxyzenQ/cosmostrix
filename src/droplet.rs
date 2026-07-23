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

use crossterm::style::Color;

use crate::cloud::{CharLoc, DrawCtx};
use crate::constants::{
    ADVANCE_REMAINDER_CAP, DROPLET_GRAVITY, DROPLET_TERMINAL_VELOCITY_MULT,
    EDGE_FADE_BOLD_THRESHOLD, EDGE_FADE_BOTTOM_LIP, EDGE_FADE_BOTTOM_MIN, EDGE_FADE_BOTTOM_ROWS,
    EDGE_FADE_ROWS, EDGE_FADE_TOP_MIN, FOG_MIN_FACTOR, FOG_ROWS, FRACTIONAL_BLOOM_AMP,
    FRACTIONAL_HEAD_BRIGHTNESS_AMP, HEAD_BLOOM_CELLS, HEAD_BLOOM_INTENSITY, HEAD_BLOOM_SIGMA,
    HEAD_LINGER_BRIGHTNESS_MS, HEAD_SHIMMER_PERIOD_SECS, MOUSE_FLASH_DURATION_SECS,
    MOUSE_FLASH_INTENSITY, MOUSE_FLASH_RING_WIDTH, MOUSE_FLASH_SECONDARY_FRAC,
    MOUSE_FLASH_SECONDARY_SPEED_FRAC, MOUSE_FLASH_SPEED, MOUSE_GLOW_INTENSITY,
    MOUSE_GLOW_RADIUS_COLS, MOUSE_GLOW_RADIUS_LINES, PARALLAX_BRIGHTNESS_MULT,
    PARALLAX_CONTRAST_REDUCTION, PARALLAX_GLYPH_DIM, PARALLAX_HEAD_BLOOM_MULT,
    PARALLAX_HEAD_SELFBLOOM_MULT, PARALLAX_SATURATION_MULT, RAIN_SHADOW_LAYER_MULT,
    RAIN_SHADOW_PCT, STARTUP_EASE_TAU, STARTUP_VELOCITY_FRACTION, TRANSITION_ENERGY_DURATION_SECS,
    TRANSITION_ENERGY_SATURATION_BOOST, TRANSITION_HEAD_GLOW_BOOST, TURBULENCE_AMPLITUDE,
    TURBULENCE_FREQ, VIGNETTE_INNER_RADIUS, VIGNETTE_INTENSITY, VIGNETTE_LAYER_MULT,
};
use crate::frame::Frame;
use crate::palette;

/// Compute the viewport edge fade factor for a cell at the given line.
/// Returns a value in `[min(top, bottom)..1.0]` depending on proximity
/// to the viewport edges. Interior rows return 1.0 (no dimming).
///
/// This fade is applied AFTER all other visual effects (including head
/// self-bloom and head brightness modulation) so it takes priority at
/// viewport edges, creating:
/// - Smooth rain emergence at the top (rain appears to enter from beyond)
/// - Smooth rain exit at the bottom (tails fade out before the terminal border)
/// - Prevention of bright head tips lingering on the bottom border
///
/// The asymmetric min values (EDGE_FADE_TOP_MIN=0.55 vs
/// EDGE_FADE_BOTTOM_MIN=0.20) ensure the bottom fade is more aggressive
/// to prevent the phosphor ghost residue artifact where dying droplet
/// heads burn into the bottom row.
#[inline]
pub(crate) fn viewport_edge_fade(line: u16, lines: u16) -> f32 {
    if lines == 0 || EDGE_FADE_ROWS == 0 {
        return 1.0;
    }
    // Top edge: linear fade over EDGE_FADE_ROWS rows.
    let top_fade = if line < EDGE_FADE_ROWS {
        EDGE_FADE_TOP_MIN + (1.0 - EDGE_FADE_TOP_MIN) * (line as f32 / EDGE_FADE_ROWS as f32)
    } else {
        1.0
    };
    // v17: Bottom edge — 2-zone cinematic dissolve.
    //
    // Zone 1 (gentle pre-fade): rows [lines-EDGE_FADE_BOTTOM_ROWS .. lines-EDGE_FADE_ROWS]
    //   smoothstep from 1.0 down to EDGE_FADE_BOTTOM_LIP. Subtle — rain still
    //   clearly visible but starting to darken.
    //
    // Zone 2 (sharp lip): rows [lines-EDGE_FADE_ROWS .. lines-1]
    //   linear from EDGE_FADE_BOTTOM_LIP down to EDGE_FADE_BOTTOM_MIN. Heavy
    //   fade — rain dissolves into shadow before the border.
    //
    // The 2-zone design produces a film-like vignette where rain gradually
    // fades across the bottom 30% of the screen (on a 40-line terminal),
    // eliminating the "concrete wall" artifact where dying heads pile up.
    let bottom_dist = lines.saturating_sub(line).saturating_sub(1);
    let bottom_fade = if bottom_dist < EDGE_FADE_ROWS {
        // Zone 2: sharp lip fade. bottom_dist in [0, EDGE_FADE_ROWS).
        // Linear from EDGE_FADE_BOTTOM_MIN (at bottom_dist=0) to
        // EDGE_FADE_BOTTOM_LIP (at bottom_dist=EDGE_FADE_ROWS).
        let t = bottom_dist as f32 / EDGE_FADE_ROWS as f32;
        EDGE_FADE_BOTTOM_MIN + (EDGE_FADE_BOTTOM_LIP - EDGE_FADE_BOTTOM_MIN) * t
    } else if bottom_dist < EDGE_FADE_BOTTOM_ROWS {
        // Zone 1: gentle pre-fade. bottom_dist in [EDGE_FADE_ROWS, EDGE_FADE_BOTTOM_ROWS).
        // Smoothstep from EDGE_FADE_BOTTOM_LIP (at bottom_dist=EDGE_FADE_ROWS)
        // up to 1.0 (at bottom_dist=EDGE_FADE_BOTTOM_ROWS).
        let span = (EDGE_FADE_BOTTOM_ROWS - EDGE_FADE_ROWS) as f32;
        let t = (bottom_dist - EDGE_FADE_ROWS) as f32 / span;
        // Smoothstep: 3t² - 2t³ (slow start, fast middle, slow end).
        let smooth = t * t * (3.0 - 2.0 * t);
        EDGE_FADE_BOTTOM_LIP + (1.0 - EDGE_FADE_BOTTOM_LIP) * smooth
    } else {
        1.0
    };
    top_fade.min(bottom_fade)
}

/// Cinematic radial vignette: darkens cells based on Euclidean distance
/// from the screen center. Cells inside VIGNETTE_INNER_RADIUS are
/// unmodified; cells from there to the corner are dimmed smoothly via
/// smoothstep up to VIGNETTE_INTENSITY.
///
/// This is a pure photographic vignette — it does NOT replace the
/// top/bottom edge fade (which is a directional cinematic dissolve).
/// The vignette adds a soft "lens" darkening on top of all other
/// effects, drawing the eye toward the focused center of the frame.
///
/// O(1) per cell: 2 subtractions, 2 multiplications, 1 sqrt, 1
/// smoothstep, 1 multiply. Called once per cell in the draw loop.
#[inline]
pub(crate) fn vignette_factor(col: u16, line: u16, cols: u16, lines: u16) -> f32 {
    if cols == 0 || lines == 0 || VIGNETTE_INTENSITY <= 0.0 {
        return 1.0;
    }
    // Normalize to [-1, 1] centered on screen midpoint.
    let nx = (col as f32 - cols as f32 * 0.5) / (cols as f32 * 0.5);
    let ny = (line as f32 - lines as f32 * 0.5) / (lines as f32 * 0.5);
    // Euclidean distance from center, normalized so corner = sqrt(2)/2 ≈ 0.707
    // for a non-square screen. We rescale to make corner ≈ 1.0 by dividing by
    // the diagonal half-length, but a simpler approach: just use raw Euclidean
    // and treat the diagonal half-length as 1.0. To keep the inner-radius
    // semantics intuitive (0.7 = 70% of the way to the corner), we normalize
    // by max(nx², ny²) → corner = 1.0 in Chebyshev distance, which matches
    // the perceived "corners are darkest" intuition better than Euclidean
    // for non-square terminal cells (which are ~2:1 tall).
    let dist_sq = nx * nx + ny * ny;
    let dist = dist_sq.sqrt();
    // Corner of a square screen is at dist = sqrt(2) ≈ 1.414; of a typical
    // wide terminal (cols=2*lines), it's sqrt(1 + 0.25) ≈ 1.118. We
    // normalize so the *corner of a square* maps to 1.0, which keeps the
    // inner-radius cutoff intuitive on standard terminals.
    let normalized = dist * std::f32::consts::FRAC_1_SQRT_2;
    if normalized <= VIGNETTE_INNER_RADIUS {
        return 1.0;
    }
    // Smoothstep from VIGNETTE_INNER_RADIUS (factor=1.0) to 1.0 (factor=1-VIGNETTE_INTENSITY).
    let t = ((normalized - VIGNETTE_INNER_RADIUS) / (1.0 - VIGNETTE_INNER_RADIUS)).clamp(0.0, 1.0);
    let smooth = t * t * (3.0 - 2.0 * t);
    1.0 - VIGNETTE_INTENSITY * smooth
}

/// Rain shadow: quadratic fade-out across the bottom RAIN_SHADOW_PCT of
/// the screen. Cells above the threshold are unmodified; cells from the
/// threshold to the bottom row fade smoothly to 0.0 (full dark).
///
/// Distinct from EDGE_FADE_BOTTOM: the edge fade is a sharp 12-row lip
/// that prevents bright head pile-up at the very last row. The rain
/// shadow is a wider, softer 20%-of-screen quadratic that gives the
/// frame perceptual "depth" — rain appears to dissipate into shadow at
/// the ground rather than hitting a wall.
///
/// Applied BEFORE phosphor decay so the captured phosphor energy is
/// already dimmed — the afterglow trail fades in sync with the shadow.
#[inline]
pub(crate) fn rain_shadow_factor(line: u16, lines: u16) -> f32 {
    if lines == 0 || RAIN_SHADOW_PCT <= 0.0 {
        return 1.0;
    }
    let threshold = ((1.0 - RAIN_SHADOW_PCT) * lines as f32) as u16;
    if line < threshold {
        return 1.0;
    }
    let span = (lines.saturating_sub(threshold)).max(1) as f32;
    let t = ((line - threshold) as f32 / span).clamp(0.0, 1.0);
    // Quadratic fade: 1.0 → 0.0 as t goes 0 → 1, with slow start and
    // accelerating fade. Reads as natural depth shadow.
    1.0 - t * t
}

#[derive(Clone, Debug)]
pub struct Droplet {
    pub is_alive: bool,
    pub is_head_crawling: bool,
    pub is_tail_crawling: bool,

    /// Column this droplet is bound to; `u16::MAX` when inactive (recycled).
    pub bound_col: u16,
    pub head_put_line: u16,
    pub head_cur_line: u16,

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
    pub fn new() -> Self {
        Self {
            is_alive: false,
            is_head_crawling: false,
            is_tail_crawling: false,
            bound_col: u16::MAX,
            head_put_line: 0,
            head_cur_line: 0,
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

    pub fn activate(&mut self, now: Instant) {
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
    pub fn apply_phase_jitter(&mut self, offset: f32) {
        self.advance_remainder = offset.clamp(0.0, 1.0);
    }

    pub fn increment_time(&mut self, delta: Duration) {
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
    pub fn advance(&mut self, now: Instant, lines: u16, time_scale: f32) -> bool {
        let Some(last) = self.last_time else {
            self.last_time = Some(now);
            return false;
        };

        let elapsed = now.saturating_duration_since(last);
        let elapsed_sec = elapsed.as_secs_f32();
        // Apply resume time-scale: the simulation clock runs in slow motion
        // during the smoothstep transition. Gravity, turbulence, and position
        // all advance at the scaled rate, producing genuine inertia recovery
        // rather than a frozen-then-unfrozen snap.
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
    pub fn fractional_progress(&self) -> f32 {
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

    /// Legacy binary helper kept for CharLoc::Head classification threshold.
    /// Unused after the head_brightness hoisting optimization, but retained
    /// as a thin wrapper for any future caller that needs the bool form.
    #[inline]
    #[allow(dead_code)]
    fn is_head_bright(&self, now: Instant) -> bool {
        self.head_brightness(now) > 0.3
    }

    pub fn draw(
        &mut self,
        ctx: &DrawCtx<'_>,
        frame: &mut Frame,
        now: Instant,
        draw_everything: bool,
    ) {
        let bg = ctx.bg;

        let mut start_line = 0u16;
        if let Some(tp) = self.tail_put_line {
            let blank = crate::terminal::blank_cell(bg);
            for line in self.tail_cur_line..=tp {
                frame.set_force(self.bound_col, line, blank);
            }
            self.tail_cur_line = tp;
            start_line = tp.saturating_add(1);
        }

        // PERF: head_brightness() depends only on `self` and `now`, NOT on
        // `line`. Previously it was called once per line inside the loop
        // via is_head_bright() and again at line 440 via head_brightness() —
        // 2× redundant Instant::elapsed() + exp() per line per droplet.
        // Hoist both computations out of the loop.
        let head_bright = self.head_brightness(now);
        let is_head_bright_cached = head_bright > 0.3;

        // F6: hoist loop-invariant transition energy + F7: fractional_progress
        let is_new_generation = self.palette_slot == ctx.active_palette_slot && ctx.transitioning;
        let transition_wf: Option<i32> = if is_new_generation {
            self.last_time.and_then(|birth| {
                let age = now.saturating_duration_since(birth).as_secs_f32();
                if age < TRANSITION_ENERGY_DURATION_SECS {
                    let t = 1.0 - (age / TRANSITION_ENERGY_DURATION_SECS);
                    Some((t * TRANSITION_ENERGY_SATURATION_BOOST * 256.0) as i32)
                } else {
                    None
                }
            })
        } else {
            None
        };
        let frac_progress = self.fractional_progress();

        for line in start_line..=self.head_put_line {
            if line >= ctx.lines {
                break;
            }

            let is_glitched = ctx.is_glitched(line, self.bound_col);
            // Head glyph shimmer: periodically cycle the head character to create
            // subtle "churn" that makes active cells feel alive without flicker.
            // The shimmer uses a time-based offset into the char_pool, so the
            // character changes smoothly at HEAD_SHIMMER_PERIOD_SECS intervals.
            let is_head = line == self.head_put_line && is_head_bright_cached;
            let val = if is_head && self.is_head_crawling {
                let birth = self.birth_time.unwrap_or(now);
                let age = now.saturating_duration_since(birth).as_secs_f32();
                let shimmer_idx = (age / HEAD_SHIMMER_PERIOD_SECS) as u16;
                ctx.get_char(
                    line,
                    self.bound_col,
                    self.char_pool_idx.wrapping_add(shimmer_idx),
                )
            } else {
                ctx.get_char(line, self.bound_col, self.char_pool_idx)
            };

            let mut loc = CharLoc::Middle;
            // Front-layer dynamic tail: for layer 2 droplets with tail_cells > 1,
            // assign the first `tail_cells` cells of the visible trail to
            // CharLoc::TailN(i), mapping them to palette tail color stops
            // (0=darkest/furthest, up to FRONT_LAYER_MAX_TAIL_STOPS-1). This
            // restores visible multi-cell tails that were missing — previously
            // front-layer droplets showed only head+body with no tail.
            //
            // Mid/back layers (tail_cells == 1) retain the existing single-cell
            // CharLoc::Tail assignment to preserve the 3-2-2 distribution.
            let visible_start = self.tail_put_line.map_or(0, |tp| tp.saturating_add(1));
            if line < self.head_put_line && line >= visible_start {
                let dist_from_tail = line.saturating_sub(visible_start);
                if self.tail_cells > 1 && dist_from_tail < self.tail_cells as u16 {
                    loc = CharLoc::TailN {
                        seg: dist_from_tail as u8,
                        total: self.tail_cells,
                    };
                } else if self.tail_put_line.is_some() && dist_from_tail == 0 {
                    loc = CharLoc::Tail;
                }
            }
            if is_head {
                loc = CharLoc::Head;
            }

            if matches!(loc, CharLoc::Middle)
                && line < self.head_cur_line
                && !is_glitched
                && line != self.end_line
                && !ctx.shading_distance
                && !ctx.transitioning
                && !ctx.charset_transitioning()
                && !draw_everything
            {
                continue;
            }

            let (fg, bold) = ctx.get_attr(
                self.palette_slot,
                line,
                self.bound_col,
                val,
                loc,
                self.head_put_line,
                self.length,
            );

            // head_bright was hoisted out of the loop above — reuse cached value.

            // Apply visual effects to foreground color
            let edge_fade = ctx.edge_fade(line);

            let fg = fg.and_then(|c| {
                // Decode color to RGB once; chain all effects on raw tuples.
                let (mut r, mut g, mut b) = palette::decode_color(c)?;

                // F6: transition energy uses hoisted transition_wf
                if let Some(wf) = transition_wf {
                    r = (r as i32 + ((255 - r as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                    g = (g as i32 + ((255 - g as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                    b = (b as i32 + ((255 - b as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                }

                // Head bloom: exponential gaussian falloff for natural glow.
                if matches!(loc, CharLoc::Middle) {
                    let dist_from_head = self.head_put_line.saturating_sub(line);
                    if dist_from_head > 0 && dist_from_head < HEAD_BLOOM_CELLS {
                        let d = dist_from_head as f32;
                        let gaussian = (-d * d / (2.0 * HEAD_BLOOM_SIGMA * HEAD_BLOOM_SIGMA)).exp();
                        let bloom = if is_new_generation {
                            HEAD_BLOOM_INTENSITY + TRANSITION_HEAD_GLOW_BOOST
                        } else {
                            HEAD_BLOOM_INTENSITY
                        };
                        // Depth-of-field: scale head bloom by layer so back-layer
                        // heads don't out-bloom front-layer bodies. Without this,
                        // a short back-layer droplet (head + 1 body cell) shows as
                        // a bright bloom spot against the dark background.
                        let layer_bloom = PARALLAX_HEAD_BLOOM_MULT[self.layer as usize];
                        let frac_bloom = 1.0 + frac_progress * FRACTIONAL_BLOOM_AMP;
                        let factor = gaussian * bloom * frac_bloom * layer_bloom;
                        let wf = (factor * 256.0) as i32;
                        r = (r as i32 + ((255 - r as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                        g = (g as i32 + ((255 - g as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                        b = (b as i32 + ((255 - b as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                    }
                }

                // Parallax layer brightness + glyph dim: combine into one multiply
                let layer_brightness = PARALLAX_BRIGHTNESS_MULT[self.layer as usize];
                let glyph_dim = PARALLAX_GLYPH_DIM[self.layer as usize];
                let combined_layer = layer_brightness * glyph_dim;
                if combined_layer < 1.0 {
                    let fi = (combined_layer * 256.0) as i32;
                    r = ((r as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                    g = ((g as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                    b = ((b as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                }

                // Depth-of-field saturation: blend toward luminance (gray) by
                // `1.0 - saturation_mult`. Back layers lose color vividness so
                // they read as "atmospheric haze" instead of "same rain but
                // dimmer". This is what kills the bright-spot effect most
                // decisively — even an unsuppressed bright head becomes pale
                // gray instead of vivid color, so it no longer pops as a hot
                // pixel against the dark background.
                //
                // Luminance is computed via the standard Rec. 601 weighting
                // (0.299R + 0.587G + 0.114B) using integer math.
                let saturation_mult = PARALLAX_SATURATION_MULT[self.layer as usize];
                if saturation_mult < 1.0 {
                    let lum = (r as u32 * 77 + g as u32 * 150 + b as u32 * 29 + 128) >> 8;
                    let lum = lum.min(255) as u8;
                    // Blend: out = color * sat + lum * (1 - sat)
                    // Equivalent to: out = color - (color - lum) * (1 - sat)
                    let inv_sat = ((1.0 - saturation_mult) * 256.0) as i32;
                    let dr = (r as i32 - lum as i32) * inv_sat;
                    let dg = (g as i32 - lum as i32) * inv_sat;
                    let db = (b as i32 - lum as i32) * inv_sat;
                    r = (r as i32 - (dr + 128) / 256).clamp(0, 255) as u8;
                    g = (g as i32 - (dg + 128) / 256).clamp(0, 255) as u8;
                    b = (b as i32 - (db + 128) / 256).clamp(0, 255) as u8;
                }

                // Depth-of-field: reduce fg-bg contrast for background layer.
                // Blends the foreground color toward black (background) by
                // PARALLAX_CONTRAST_REDUCTION[layer]. This creates a "foggy"
                // perceptual blur — the terminal equivalent of depth-of-field.
                // Only layer 0 (background) is affected; layers 1-2 stay sharp.
                let contrast_reduction = PARALLAX_CONTRAST_REDUCTION[self.layer as usize];
                if contrast_reduction > 0.0 {
                    let cr = (contrast_reduction * 256.0) as i32;
                    r = ((r as i32 * (256 - cr) + 128) >> 8).clamp(0, 255) as u8;
                    g = ((g as i32 * (256 - cr) + 128) >> 8).clamp(0, 255) as u8;
                    b = ((b as i32 * (256 - cr) + 128) >> 8).clamp(0, 255) as u8;
                }

                // Depth fog: dim top and bottom rows
                let fog_factor = if line < FOG_ROWS {
                    FOG_MIN_FACTOR + (1.0 - FOG_MIN_FACTOR) * (line as f32 / FOG_ROWS as f32)
                } else {
                    let bottom_dist = ctx.lines.saturating_sub(line).saturating_sub(1);
                    if bottom_dist < FOG_ROWS {
                        FOG_MIN_FACTOR
                            + (1.0 - FOG_MIN_FACTOR) * (bottom_dist as f32 / FOG_ROWS as f32)
                    } else {
                        1.0
                    }
                };
                if fog_factor < 1.0 {
                    let fi = (fog_factor * 256.0) as i32;
                    r = ((r as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                    g = ((g as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                    b = ((b as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                }

                // Cursor glow: cells near mouse cursor get brighter (elliptical falloff)
                if ctx.mouse_col != u16::MAX {
                    let col_dist = if self.bound_col > ctx.mouse_col {
                        (self.bound_col - ctx.mouse_col) as f32
                    } else {
                        (ctx.mouse_col - self.bound_col) as f32
                    };
                    let line_dist = if line > ctx.mouse_line {
                        (line - ctx.mouse_line) as f32
                    } else {
                        (ctx.mouse_line - line) as f32
                    };
                    let norm_col = col_dist / MOUSE_GLOW_RADIUS_COLS;
                    let norm_line = line_dist / MOUSE_GLOW_RADIUS_LINES;
                    let dist_sq = norm_col * norm_col + norm_line * norm_line;
                    if dist_sq < 1.0 {
                        let glow = (1.0 - dist_sq) * MOUSE_GLOW_INTENSITY;
                        let wf = (glow * 256.0) as i32;
                        r = (r as i32 + ((255 - r as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                        g = (g as i32 + ((255 - g as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                        b = (b as i32 + ((255 - b as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                    }
                }

                // Click flash: expanding glow wave from click point (v17 mastery).
                // F4: use cached flash_elapsed instead of per-cell flash_time.elapsed()
                //
                // v17 mastery: dual-ring water-drop ripple. A primary bright ring
                // expands outward at 60 cells/s, followed by a secondary dimmer
                // ring at half speed — creating a layered "stone in water"
                // cinematic ripple that propagates to the screen edge.
                //
                // The fade uses a quadratic curve (fade^1.5) for natural energy
                // dissipation — the wave starts strong and decays gradually like
                // a real water ripple, not a linear cutoff.
                if let Some(elapsed) = ctx.flash_elapsed {
                    let col_dist = if self.bound_col > ctx.flash_col {
                        (self.bound_col - ctx.flash_col) as f32
                    } else {
                        (ctx.flash_col - self.bound_col) as f32
                    };
                    let line_dist = if line > ctx.flash_line {
                        (line - ctx.flash_line) as f32
                    } else {
                        (ctx.flash_line - line) as f32
                    };
                    let euclidean = (col_dist * col_dist + line_dist * line_dist).sqrt();
                    // Quadratic fade: natural energy dissipation (fade^1.5).
                    // The wave starts strong and decays gradually like a real
                    // water ripple, rather than a linear cutoff.
                    let raw_fade = (1.0 - elapsed / MOUSE_FLASH_DURATION_SECS).max(0.0);
                    let fade = raw_fade * raw_fade.sqrt();

                    // Primary ring: fast, bright, full intensity.
                    let primary_radius = elapsed * MOUSE_FLASH_SPEED;
                    let primary_dist = (euclidean - primary_radius).abs();
                    let mut factor = 0.0;
                    if primary_dist < MOUSE_FLASH_RING_WIDTH {
                        // Sharp leading edge, soft trailing tail (squared falloff).
                        let t = 1.0 - primary_dist / MOUSE_FLASH_RING_WIDTH;
                        let t_smooth = t * t;
                        factor = t_smooth * MOUSE_FLASH_INTENSITY * fade;
                    }

                    // Secondary ring: slower, dimmer, layered echo.
                    let secondary_radius =
                        elapsed * MOUSE_FLASH_SPEED * MOUSE_FLASH_SECONDARY_SPEED_FRAC;
                    let secondary_dist = (euclidean - secondary_radius).abs();
                    if secondary_dist < MOUSE_FLASH_RING_WIDTH {
                        let t = 1.0 - secondary_dist / MOUSE_FLASH_RING_WIDTH;
                        let t_smooth = t * t;
                        factor +=
                            t_smooth * MOUSE_FLASH_INTENSITY * MOUSE_FLASH_SECONDARY_FRAC * fade;
                    }

                    if factor > 0.0 {
                        let wf = (factor * 256.0) as i32;
                        r = (r as i32 + ((255 - r as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                        g = (g as i32 + ((255 - g as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                        b = (b as i32 + ((255 - b as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                    }
                }

                // Head brightness modulation
                if matches!(loc, CharLoc::Head) && head_bright < 1.0 {
                    let factor = 0.7 + 0.3 * head_bright;
                    let fi = (factor * 256.0) as i32;
                    r = ((r as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                    g = ((g as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                    b = ((b as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                }

                // Head self-bloom: 45% white blend toward white.
                // Cinematic head pop — head is OBVIOUSLY brighter than body.
                // Was 12% (subtle), raised to 45% for film-quality head glow.
                //
                // Cinematic final polish: scale HEAD_WF by per-layer multiplier
                // so back-layer heads don't get re-brightened after dimming.
                // Without this, the layer brightness dimming (25% for back
                // layer) was being undone by the 55% white blend, popping the
                // head back up to ~66% brightness — visible as a "white dot".
                // With PARALLAX_HEAD_SELFBLOOM_MULT[0] = 0.30, the effective
                // self-bloom for back-layer heads is ~17%, keeping them
                // firmly below the front-layer body visibility floor.
                if matches!(loc, CharLoc::Head) {
                    // v17 mastery: HEAD_WF = 140 (0.55 white blend) — head is
                    // the brightest cell, high contrast vs body/tail. Was 115 (0.45).
                    const HEAD_WF: i32 = 140; // 0.55 * 256 ≈ 140
                    let layer_selfbloom = PARALLAX_HEAD_SELFBLOOM_MULT[self.layer as usize] as i32;
                    let wf = (HEAD_WF * layer_selfbloom) / 256;
                    r = (r as i32 + ((255 - r as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                    g = (g as i32 + ((255 - g as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                    b = (b as i32 + ((255 - b as i32) * wf + 128) / 256).clamp(0, 255) as u8;
                }

                // Rain shadow: quadratic fade-out across bottom 15% of screen.
                // Applied BEFORE the edge_fade (which is a sharper lip) and
                // BEFORE the vignette (which is a radial effect). The shadow
                // is the broadest, softest bottom dim — gives the frame
                // perceptual depth ("rain dissipating into shadow at ground")
                // rather than "rain hitting a wall". Applied here in the
                // droplet color pipeline so phosphor captures the already-
                // dimmed color (afterglow fades in sync with shadow).
                //
                // Front-layer exclusion: RAIN_SHADOW_LAYER_MULT[2] = 0.0 means
                // front-layer neon is NOT dimmed by the shadow — it stays at
                // full fidelity across the entire screen height. Mid/back
                // layers (mult=1.0) get the full shadow for depth.
                let shadow_raw = rain_shadow_factor(line, ctx.lines);
                let shadow = 1.0 - (1.0 - shadow_raw) * RAIN_SHADOW_LAYER_MULT[self.layer as usize];
                if shadow < 1.0 {
                    let fi = (shadow * 256.0) as i32;
                    r = ((r as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                    g = ((g as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                    b = ((b as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                }

                // PERF(v10): Viewport edge fade applied on raw RGB tuples
                // before wrapping into Color::Rgb.  This eliminates one
                // decode_color() match + destructure + apply_brightness_rgb()
                // call + extra .map() closure per cell — the color is already
                // (r, g, b) here, so we just multiply in-place.
                if edge_fade < 1.0 {
                    let fi = (edge_fade * 256.0) as i32;
                    r = ((r as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                    g = ((g as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                    b = ((b as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                }

                // Cinematic radial vignette — applied LAST, AFTER all other
                // effects (including edge_fade). This is the photographic
                // "lens darkening" that frames the image: corners dimmed
                // smoothly toward 70% of their post-effects brightness,
                // drawing the eye to the focused center. O(1) per cell.
                //
                // Front-layer exclusion: VIGNETTE_LAYER_MULT[2] = 0.0 means
                // front-layer neon is NOT dimmed by the vignette — it stays at
                // full fidelity even at screen corners. Mid/back layers
                // (mult=1.0) get the full vignette for depth.
                let vignette_raw = vignette_factor(self.bound_col, line, ctx.cols, ctx.lines);
                let vignette =
                    1.0 - (1.0 - vignette_raw) * VIGNETTE_LAYER_MULT[self.layer as usize];
                if vignette < 1.0 {
                    let fi = (vignette * 256.0) as i32;
                    r = ((r as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                    g = ((g as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                    b = ((b as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
                }
                Some(Color::Rgb { r, g, b })
            });
            // Suppress bold at viewport edges to prevent harsh bright spots
            // right at the border where the fade should create smooth dimming.
            let bold = bold && edge_fade >= EDGE_FADE_BOLD_THRESHOLD;

            frame.set_force(
                self.bound_col,
                line,
                crate::cell::Cell {
                    ch: val,
                    fg,
                    bg,
                    bold,
                },
            );

            if ctx.full_width && self.bound_col + 1 < frame.width {
                frame.set_force(
                    self.bound_col + 1,
                    line,
                    crate::cell::Cell {
                        ch: ' ',
                        fg: None,
                        bg,
                        bold: false,
                    },
                );
            }
        }

        self.head_cur_line = self.head_put_line;
    }
}
