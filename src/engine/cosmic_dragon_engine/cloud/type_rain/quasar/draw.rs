// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The quasar draw pass (NIGHT-research-8) — the presentation
//! half of the orchestration.
//!
//! Order of the passes (painter's order, dimmest first): the
//! halo (the host glow, breathing with the core's pulse), the
//! disk (the radial temperature ladder x the doppler asymmetry x
//! the fresh-feed charge), the jets (the energy ladder + the
//! knot's traveling pulse, riding the precessing helix), the
//! infall (the fuel streaks — the rain, in front of the engine
//! it feeds), then the core (the engine's cell + its glow ring,
//! topmost — the brightest object owns the frame). Then the
//! monolith drawn-cell diff cleanup (generation-tagged).
//!
//! Glyphs re-roll matrix-style on cell crossing (the family
//! shimmer — motion-gated mutation, never a timer); the core's
//! glyph re-rolls only on first light and flare fire
//! (event-gated).
//!
//! The ignition's brightness cap (law 0): every engine population
//! reads at most Ghost through the dark cloud, at most Dim while
//! the disk condenses, then the cap ramps with the luminosity to
//! the full law at first light. The infall is NOT capped — the
//! cold cloud is the scene while the engine is dark (the DNA
//! soup precedent).

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::constants::{
    QUAS_CHARGE_RUNG, QUAS_DISK_INNER, QUAS_DOPPLER_RUNG, QUAS_DOPPLER_W, QUAS_KNOT_W,
    QUAS_SHIMMER_CHANCE,
};
use crate::frame::Frame;

use super::super::super::render::DrawCtx;
use super::super::monolith::monolith_helpers::{
    bold_for_level, clear_cell, color_for_level, pick_pool_char,
};
use super::super::monolith::{BrightnessLevel, MonolithCleanup};
use super::ignition::IgnitionPhase;
use super::particles::{los_of, Particle, HALO_ROUND};
use super::quasar::{QuasCell, QuasarRain};

/// The cell painter: the per-cell render contract (palette-aware
/// color + bold, the motion-gated shimmer, the drawn-cell
/// bookkeeping) bundled so the per-cell signature stays under
/// clippy's 7-arg threshold (the family's params-struct pattern).
struct QuasPainter<'a, 'b, 'c> {
    ctx: &'a DrawCtx<'b>,
    frame: &'a mut Frame,
    cells: &'c mut Vec<QuasCell>,
    rand_chance: &'a Uniform<f32>,
    rng: &'a mut StdRng,
}

impl QuasPainter<'_, '_, '_> {
    /// Paint one particle cell at a projected position: the
    /// shimmer gate (the glyph re-rolls when the head lands on a
    /// new cell), then the palette ladder.
    fn cell(&mut self, p: &mut Particle, x: f32, y: f32, level: BrightnessLevel, factor: f32) {
        let (col, line) = match cell_of(x, y, self.ctx.cols as f32, self.ctx.lines as f32) {
            Some(c) => c,
            None => return,
        };
        // Matrix shimmer: mutate the glyph when the head lands on
        // a new cell (the trail fields double as the last-drawn
        // cell — u16::MAX means "never drawn").
        let crossed = p.trail_col == u16::MAX || p.trail_col != col || p.trail_line != line;
        if crossed && self.rand_chance.sample(self.rng) < QUAS_SHIMMER_CHANCE {
            p.ch = pick_pool_char(self.ctx.char_pool, self.rand_chance, self.rng);
        }
        p.set_trail(col, line);
        self.paint(col, line, p.ch, p.palette_slot, level, factor);
    }

    /// Paint the infall streak's tail cell (the head's glyph, one
    /// rung dimmer — no shimmer roll: the streak is the head's
    /// wake, not its own body).
    fn streak(&mut self, p: &Particle, col: u16, line: u16, level: BrightnessLevel) {
        self.paint(col, line, p.ch, p.palette_slot, level, 1.0);
    }

    /// Paint one literal-glyph cell (the core + its glow ring —
    /// the glyph is the engine's own, event-gated).
    fn literal(
        &mut self,
        x: f32,
        y: f32,
        ch: char,
        palette_slot: u8,
        level: BrightnessLevel,
        factor: f32,
    ) {
        let Some((col, line)) = cell_of(x, y, self.ctx.cols as f32, self.ctx.lines as f32) else {
            return;
        };
        self.paint(col, line, ch, palette_slot, level, factor);
    }

    fn paint(
        &mut self,
        col: u16,
        line: u16,
        ch: char,
        palette_slot: u8,
        level: BrightnessLevel,
        factor: f32,
    ) {
        let fg = color_for_level(self.ctx, palette_slot, line, col, level, factor);
        let bold = bold_for_level(self.ctx.bold_mode, level, line, col);
        let cell = crate::cell::Cell {
            ch,
            fg,
            bg: self.ctx.bg,
            bold,
        };
        self.frame.set(col, line, cell);
        self.cells.push(QuasCell { col, line });
    }
}

impl QuasarRain {
    /// Draw pass — the halo, the disk, the jets, the infall, the
    /// core, then the generation-tagged diff cleanup.
    pub(crate) fn draw(
        &mut self,
        ctx: &DrawCtx<'_>,
        frame: &mut Frame,
        cleanup: &mut MonolithCleanup<'_>,
        rng: &mut StdRng,
        rand_chance: &Uniform<f32>,
    ) {
        let lines_us = ctx.lines as usize;
        self.current_cells.clear();

        let phase = self.phase();
        let lum = self.luminosity();
        let pulse = self.pulse_display();
        let knot = self.knot();
        let cap_rank = ignition_cap_rank(phase, lum);
        let helix_amp = crate::constants::QUAS_HELIX_AMP
            .min(self.geom.cx * 0.12)
            .max(0.0);
        let core_visible = matches!(
            phase,
            IgnitionPhase::Light | IgnitionPhase::Jets | IgnitionPhase::Steady
        );
        let dark = matches!(phase, IgnitionPhase::Dark | IgnitionPhase::Disk);
        let flaring = self.flare_age.is_some();

        // Pass A — the halo: the host glow, Ghost at rest, a rung
        // brighter when the pulse peaks or the flare burns.
        for p in &mut self.halo {
            if !p.active {
                continue;
            }
            let (x, y) = self.geom.project(p.f, p.theta, HALO_ROUND);
            let mut level = BrightnessLevel::Ghost;
            if pulse > 0.8 {
                level = BrightnessLevel::Dim;
            }
            level = cap_level(level, cap_rank);
            let factor = 0.7 + 0.3 * pulse * lum;
            QuasPainter {
                ctx,
                frame,
                cells: &mut self.current_cells,
                rand_chance,
                rng,
            }
            .cell(p, x, y, level, factor);
        }

        // Pass B — the disk: the radial temperature ladder, the
        // doppler rung swing, the fresh-feed charge boost — all
        // under the ignition cap.
        for p in &mut self.disk {
            if !p.active {
                continue;
            }
            let (x, y) = self.geom.project(p.f, p.theta, 1.0);
            let los = los_of(p.theta);
            let t = (p.f - QUAS_DISK_INNER) / (1.0 - QUAS_DISK_INNER);
            let mut level = if t < 0.14 {
                BrightnessLevel::Core
            } else if t < 0.30 {
                BrightnessLevel::Hot
            } else if t < 0.52 {
                BrightnessLevel::Mid
            } else if t < 0.75 {
                BrightnessLevel::Dim
            } else {
                BrightnessLevel::Ghost
            };
            if los > QUAS_DOPPLER_RUNG {
                level = step_up_level(level);
            } else if los < -QUAS_DOPPLER_RUNG {
                level = step_down_level(level);
            }
            if p.charge > QUAS_CHARGE_RUNG {
                level = step_up_level(level);
            }
            level = cap_level(level, cap_rank);
            // The doppler brightness factor swings with the
            // line-of-sight velocity, riding in with the light.
            let factor = (1.0 + QUAS_DOPPLER_W * los * lum).max(0.1);
            QuasPainter {
                ctx,
                frame,
                cells: &mut self.current_cells,
                rand_chance,
                rng,
            }
            .cell(p, x, y, level, factor);
        }

        // Pass C — the jets: the energy ladder (hottest at the
        // launch collar, dim at the tip), the knot's traveling
        // pulse, the precessing helix.
        if self.jets_fired {
            let prec_phase = self.prec_phase;
            for p in &mut self.jets {
                if !p.active {
                    continue;
                }
                let (x, y) = self.geom.project_jet(p.s, p.side, prec_phase, helix_amp);
                let mut level = if p.s < 0.18 {
                    BrightnessLevel::Core
                } else if p.s < 0.45 {
                    BrightnessLevel::Hot
                } else if p.s < 0.70 {
                    BrightnessLevel::Mid
                } else {
                    BrightnessLevel::Ghost
                };
                if let Some(k) = knot {
                    if (p.s - k).abs() < QUAS_KNOT_W {
                        level = step_up_level(level);
                    }
                }
                QuasPainter {
                    ctx,
                    frame,
                    cells: &mut self.current_cells,
                    rand_chance,
                    rng,
                }
                .cell(p, x, y, level, 1.0);
            }
        }

        // Pass D — the infall: the fuel streaks (head + the
        // one-deep wake), Ghost at rest, Dim through the dark
        // cloud (the cold gas IS the scene), a rung brighter
        // while the flare's clump burns in. Uncapped — the rain
        // is visible from the first frame.
        for p in &mut self.infall {
            if !p.active {
                continue;
            }
            let (x, y) = self.geom.project(p.f, p.theta, 1.0);
            let mut level = if dark {
                BrightnessLevel::Dim
            } else {
                BrightnessLevel::Ghost
            };
            if flaring {
                level = step_up_level(level);
            }
            let mut painter = QuasPainter {
                ctx,
                frame,
                cells: &mut self.current_cells,
                rand_chance,
                rng,
            };
            // The wake: the previous head cell, one rung dimmer.
            if let Some((tc, tl)) = p.trail_cell() {
                if tc < ctx.cols && tl < ctx.lines {
                    painter.streak(p, tc, tl, step_down_level(level));
                }
            }
            painter.cell(p, x, y, level, 1.0);
        }

        // Pass E — the core: the engine's cell (Core-bright, its
        // glyph re-rolled on first light and every flare fire)
        // wrapped in the pulse-following glow ring.
        if core_visible {
            if self.core_roll {
                self.core_ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                self.core_roll = false;
            }
            let glow = if pulse > 0.6 {
                BrightnessLevel::Hot
            } else {
                BrightnessLevel::Mid
            };
            let glow_factor = 0.7 + 0.3 * pulse;
            let (cx, cy) = (self.geom.cx, self.geom.cy);
            let core = self.core_ch;
            let mut painter = QuasPainter {
                ctx,
                frame,
                cells: &mut self.current_cells,
                rand_chance,
                rng,
            };
            painter.literal(
                cx,
                cy,
                core,
                self.field_palette_slot,
                BrightnessLevel::Core,
                1.0,
            );
            for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                painter.literal(
                    cx + dx,
                    cy + dy,
                    core,
                    self.field_palette_slot,
                    glow,
                    glow_factor,
                );
            }
        }

        // Pass F — the generation-tagged diff cleanup (monolith
        // pattern: u32 counter bump instead of clearing the array).
        self.drawn_gen_counter = self.drawn_gen_counter.wrapping_add(1);
        let gen = self.drawn_gen_counter;
        let total = self.disk.len() + self.jets.len() + self.halo.len() + self.infall.len();
        let need_len = total.saturating_mul(lines_us.max(1)).max(8);
        if self.drawn_gen.len() != need_len {
            self.drawn_gen.resize(need_len, 0);
        }
        for cell in &self.current_cells {
            let idx = cell.col as usize * lines_us + cell.line as usize;
            if idx < self.drawn_gen.len() {
                self.drawn_gen[idx] = gen;
            }
        }

        // Clear previous cells NOT redrawn this frame.
        let drawn_gen = &self.drawn_gen[..];
        for cell in &self.previous_cells {
            let idx = cell.col as usize * lines_us + cell.line as usize;
            if idx < drawn_gen.len() && drawn_gen[idx] == gen {
                continue;
            }
            clear_cell(frame, cleanup, cell.col, cell.line);
        }

        std::mem::swap(&mut self.previous_cells, &mut self.current_cells);
    }
}

/// The ignition's brightness cap as a ladder rank (0 = Ghost ..
/// 4 = Core): Ghost through the dark cloud, Dim while the disk
/// condenses, ramping to the full law with the luminosity.
fn ignition_cap_rank(phase: IgnitionPhase, lum: f32) -> u8 {
    match phase {
        IgnitionPhase::Dark => 0,
        IgnitionPhase::Disk => 1,
        IgnitionPhase::Light => (1.0 + lum * 3.0) as u8,
        IgnitionPhase::Jets | IgnitionPhase::Steady => 4,
    }
}

fn level_rank(level: BrightnessLevel) -> u8 {
    match level {
        BrightnessLevel::Ghost => 0,
        BrightnessLevel::Dim => 1,
        BrightnessLevel::Mid => 2,
        BrightnessLevel::Hot => 3,
        BrightnessLevel::Core => 4,
    }
}

fn rank_level(rank: u8) -> BrightnessLevel {
    match rank {
        0 => BrightnessLevel::Ghost,
        1 => BrightnessLevel::Dim,
        2 => BrightnessLevel::Mid,
        3 => BrightnessLevel::Hot,
        _ => BrightnessLevel::Core,
    }
}

fn cap_level(level: BrightnessLevel, cap_rank: u8) -> BrightnessLevel {
    rank_level(level_rank(level).min(cap_rank))
}

/// Step a brightness level up (toward Core) — the doppler's
/// approaching limb, the charge's fresh feed, the knot's pulse.
fn step_up_level(level: BrightnessLevel) -> BrightnessLevel {
    match level {
        BrightnessLevel::Ghost => BrightnessLevel::Dim,
        BrightnessLevel::Dim => BrightnessLevel::Mid,
        BrightnessLevel::Mid => BrightnessLevel::Hot,
        BrightnessLevel::Hot | BrightnessLevel::Core => BrightnessLevel::Core,
    }
}

/// Step a brightness level down (toward Ghost) — the receding
/// limb, the streak's wake (mirrors the family's step-down).
fn step_down_level(level: BrightnessLevel) -> BrightnessLevel {
    match level {
        BrightnessLevel::Core => BrightnessLevel::Hot,
        BrightnessLevel::Hot => BrightnessLevel::Mid,
        BrightnessLevel::Mid => BrightnessLevel::Dim,
        BrightnessLevel::Dim | BrightnessLevel::Ghost => BrightnessLevel::Ghost,
    }
}

/// Round a projected position to a cell, if it lands inside the
/// viewport (the family's rounding contract: the cell nearest the
/// position, negative projections rejected).
fn cell_of(x: f32, y: f32, cols_f: f32, lines_f: f32) -> Option<(u16, u16)> {
    let col = x.round();
    let line = y.round();
    if col < 0.0 || line < 0.0 || col >= cols_f || line >= lines_f {
        return None;
    }
    Some((col as u16, line as u16))
}
