// Copyright (c) 2026 rezky_nightky

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
//!
//! Each effect reads from `DrawCtx` and modifies the color via the palette
//! blending functions in `palette.rs`.

use std::time::{Duration, Instant};

use crate::cloud::{CharLoc, DrawCtx};
use crate::constants::{
    DROPLET_GRAVITY, DROPLET_TERMINAL_VELOCITY_MULT, FOG_MIN_FACTOR, FOG_ROWS, HEAD_BLOOM_CELLS,
    HEAD_BLOOM_INTENSITY, HEAD_BLOOM_SIGMA, HEAD_LINGER_BRIGHTNESS_MS, MOUSE_FLASH_DURATION_SECS,
    MOUSE_FLASH_INTENSITY, MOUSE_FLASH_RING_WIDTH, MOUSE_FLASH_SPEED, MOUSE_GLOW_INTENSITY,
    MOUSE_GLOW_RADIUS_COLS, MOUSE_GLOW_RADIUS_LINES, PARALLAX_BRIGHTNESS_MULT, PARALLAX_GLYPH_DIM,
    STARTUP_EASE_TAU, STARTUP_VELOCITY_FRACTION, TRANSITION_ENERGY_DURATION_SECS,
    TRANSITION_ENERGY_SATURATION_BOOST, TRANSITION_HEAD_GLOW_BOOST, TURBULENCE_AMPLITUDE,
    TURBULENCE_FREQ,
};
use crate::frame::Frame;
use crate::palette;

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
        self.advance_remainder = 0.0;
        // Cinematic startup: begin at a low fraction and ease into full speed
        // via exponential approach in advance(). This eliminates the jarring
        // instant-snap from the old 0.3× initial velocity.
        self.velocity = self.chars_per_sec * STARTUP_VELOCITY_FRACTION;
        self.turb_time = 0.0;
        self.last_time = Some(now);
        self.birth_time = Some(now);
    }

    pub fn increment_time(&mut self, delta: Duration) {
        if let Some(t) = self.last_time.as_mut() {
            *t += delta;
        }
        if let Some(t) = self.head_stop_time.as_mut() {
            *t += delta;
        }
    }

    #[inline]
    pub fn advance(&mut self, now: Instant, lines: u16) -> bool {
        let Some(last) = self.last_time else {
            self.last_time = Some(now);
            return false;
        };

        let elapsed = now.saturating_duration_since(last);
        let elapsed_sec = elapsed.as_secs_f32();

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
            self.velocity = (self.velocity + DROPLET_GRAVITY * elapsed_sec).min(terminal_vel);
        }

        // Subtle velocity turbulence: smooth sinusoidal drift
        self.turb_time += elapsed_sec;
        let turb_drift =
            (self.turb_time * TURBULENCE_FREQ * std::f32::consts::TAU + self.turb_phase).sin()
                * TURBULENCE_AMPLITUDE
                * self.chars_per_sec;
        let turb_velocity = (self.velocity + turb_drift).max(0.0);

        let delta = (turb_velocity * elapsed_sec).max(0.0);
        let total = self.advance_remainder + delta;
        let whole = total.floor();
        self.advance_remainder = total - whole;
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

    /// Returns 0.0–1.0 indicating how "bright" the head cell should appear.
    /// During crawling: 1.0. After head stops: exponential decay from 1.0→0.0
    /// over HEAD_LINGER_BRIGHTNESS_MS, producing a smooth fade instead of
    /// the old binary on/off switch.
    #[inline]
    fn head_brightness(&self, now: Instant) -> f32 {
        if self.is_head_crawling {
            return 1.0;
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
    #[inline]
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
            for line in self.tail_cur_line..=tp {
                frame.set(self.bound_col, line, crate::terminal::blank_cell(bg));
            }
            self.tail_cur_line = tp;
            start_line = tp.saturating_add(1);
        }

        for line in start_line..=self.head_put_line {
            if line >= ctx.lines {
                break;
            }

            let is_glitched = ctx.is_glitched(line, self.bound_col);
            let val = ctx.get_char(line, self.char_pool_idx);

            let mut loc = CharLoc::Middle;
            if self.tail_put_line.is_some() && Some(line) == self.tail_put_line.map(|v| v + 1) {
                loc = CharLoc::Tail;
            }
            if line == self.head_put_line && self.is_head_bright(now) {
                loc = CharLoc::Head;
            }

            if matches!(loc, CharLoc::Middle)
                && line < self.head_cur_line
                && !is_glitched
                && line != self.end_line
                && !ctx.shading_distance
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
                now,
                self.head_put_line,
                self.length,
            );

            // Smooth head brightness: fade head glow exponentially after stop
            let head_bright = self.head_brightness(now);

            // Apply visual effects to foreground color
            let is_new_generation =
                self.palette_slot == ctx.active_palette_slot && ctx.transitioning;

            let fg = fg.map(|c| {
                let mut c = c;

                // Transition energy: new-palette streams glow brighter when fresh
                if is_new_generation {
                    if let Some(birth) = self.last_time {
                        let age = now.saturating_duration_since(birth).as_secs_f32();
                        if age < TRANSITION_ENERGY_DURATION_SECS {
                            let t = 1.0 - (age / TRANSITION_ENERGY_DURATION_SECS);
                            c = palette::blend_toward_white(
                                c,
                                t * TRANSITION_ENERGY_SATURATION_BOOST,
                            );
                        }
                    }
                }

                // Head bloom: exponential gaussian falloff for natural glow.
                // New-generation streams get enhanced bloom for energetic leading edge.
                if matches!(loc, CharLoc::Middle) {
                    let dist_from_head = self.head_put_line.saturating_sub(line);
                    if dist_from_head > 0 && dist_from_head < HEAD_BLOOM_CELLS {
                        // Gaussian: intensity × e^(-d²/2σ²) — softer center, faster edge falloff
                        let d = dist_from_head as f32;
                        let gaussian = (-d * d / (2.0 * HEAD_BLOOM_SIGMA * HEAD_BLOOM_SIGMA)).exp();
                        let bloom = if is_new_generation {
                            HEAD_BLOOM_INTENSITY + TRANSITION_HEAD_GLOW_BOOST
                        } else {
                            HEAD_BLOOM_INTENSITY
                        };
                        c = palette::blend_toward_white(c, gaussian * bloom);
                    }
                }

                // Parallax layer brightness
                let layer_brightness = PARALLAX_BRIGHTNESS_MULT[self.layer as usize];
                if layer_brightness < 1.0 {
                    c = palette::apply_brightness(c, layer_brightness);
                }

                // Atmospheric depth: per-layer glyph dimming (far layer = simpler glyphs)
                let glyph_dim = PARALLAX_GLYPH_DIM[self.layer as usize];
                if glyph_dim < 1.0 {
                    c = palette::apply_brightness(c, glyph_dim);
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
                    c = palette::apply_brightness(c, fog_factor);
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
                        c = palette::blend_toward_white(c, glow);
                    }
                }

                // Click flash: expanding ring of brightness from click point
                if let Some(flash_time) = ctx.flash_time {
                    let elapsed = flash_time.elapsed().as_secs_f32();
                    if elapsed < MOUSE_FLASH_DURATION_SECS {
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
                        let ring_radius = elapsed * MOUSE_FLASH_SPEED;
                        let ring_dist = (euclidean - ring_radius).abs();
                        if ring_dist < MOUSE_FLASH_RING_WIDTH {
                            let t = 1.0 - ring_dist / MOUSE_FLASH_RING_WIDTH;
                            let fade = 1.0 - elapsed / MOUSE_FLASH_DURATION_SECS;
                            c = palette::blend_toward_white(c, t * MOUSE_FLASH_INTENSITY * fade);
                        }
                    }
                }

                // Head brightness modulation: smoothly fade the head cell's
                // brightness after it stops (exponential decay). This replaces
                // the old binary bright/dim switch with a perceptually smooth
                // transition that makes stream endings feel less abrupt.
                if matches!(loc, CharLoc::Head) && head_bright < 1.0 {
                    c = palette::apply_brightness(c, 0.5 + 0.5 * head_bright);
                }

                c
            });

            frame.set(
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
                frame.set(
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
