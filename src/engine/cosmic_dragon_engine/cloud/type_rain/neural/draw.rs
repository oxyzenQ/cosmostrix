// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The neural draw pass (NIGHT-research-9) — the presentation
//! half of the orchestration.
//!
//! Order of the passes (painter's order, dimmest first): the
//! wiring (the dotted idle wires, the glow rung on recent
//! traffic, the growth prefix, the retire fade), the pulses (the
//! signal heads and their one-deep wakes — the brightest movers),
//! the neurons (the potential ladder, the fired flash, the output
//! rung), the streamers (the data streaks — the rain, in front of
//! the machine it feeds). Then the monolith drawn-cell diff
//! cleanup (generation-tagged).
//!
//! The wire budget is deliberately constant: the idle wire draws
//! every third path cell (the dashed-line read), the events raise
//! the RUNG, never the cell count (the burst lights the whole
//! wiring one step brighter instead of painting more cells — the
//! dirty-cell budget survives the drama). The pulses and their
//! wakes carry the motion read.
//!
//! Glyphs re-roll matrix-style on cell crossing (the family
//! shimmer — motion-gated mutation, never a timer) for the moving
//! cells; the static cells roll at first draw and re-roll on
//! their events (a neuron on fire, a wire on its successor —
//! event-gated).
//!
//! The genesis's brightness cap (law 0): the machine reads
//! nothing through the signal phase, at most Dim while the layers
//! build and the wiring sweeps, then the cap ramps with the
//! luminosity to the full law at the first thought. The
//! streamers are NOT capped — the data is the scene while the
//! machine is dark (the DNA soup precedent).

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::constants::{NEUR_SHIMMER_CHANCE, NEUR_THRESHOLD};

use crate::frame::Frame;

use super::super::super::render::DrawCtx;
use super::super::monolith::monolith_helpers::{
    bold_for_level, clear_cell, clear_phosphor_metadata, color_for_level, pick_pool_char,
};
use super::super::monolith::{BrightnessLevel, MonolithCleanup};
use super::genesis::GenesisPhase;
use super::network::{Pulse, Streamer};
use super::neural::{NeurCell, NeuralRain};

/// The cell painter: the per-cell render contract (palette-aware
/// color + bold, the motion-gated shimmer, the drawn-cell
/// bookkeeping) bundled so the per-cell signature stays under
/// clippy's 7-arg threshold (the family's params-struct pattern).
struct NeurPainter<'a, 'b, 'c> {
    ctx: &'a DrawCtx<'b>,
    frame: &'a mut Frame,
    cells: &'c mut Vec<NeurCell>,
    rand_chance: &'a Uniform<f32>,
    rng: &'a mut StdRng,
}

impl NeurPainter<'_, '_, '_> {
    /// Paint a pulse head: the first draw picks the glyph outright
    /// (the lorenz contract), the cell crossings roll the family
    /// shimmer, the trail fields double as the last-drawn cell
    /// (u16::MAX means "never drawn").
    fn pulse_head(&mut self, p: &mut Pulse, x: f32, y: f32, level: BrightnessLevel, factor: f32) {
        let (col, line) = match cell_of(x, y, self.ctx.cols as f32, self.ctx.lines as f32) {
            Some(c) => c,
            None => return,
        };
        let first = p.trail_col == u16::MAX;
        let crossed = first || p.trail_col != col || p.trail_line != line;
        if first || (crossed && self.rand_chance.sample(self.rng) < NEUR_SHIMMER_CHANCE) {
            p.ch = pick_pool_char(self.ctx.char_pool, self.rand_chance, self.rng);
        }
        p.set_trail(col, line);
        self.paint(col, line, p.ch, p.palette_slot, level, factor);
    }

    /// Paint a streamer head: the same first-draw + shimmer
    /// contract on the falling data cell.
    fn streamer_head(
        &mut self,
        d: &mut Streamer,
        x: f32,
        y: f32,
        level: BrightnessLevel,
        factor: f32,
    ) {
        let (col, line) = match cell_of(x, y, self.ctx.cols as f32, self.ctx.lines as f32) {
            Some(c) => c,
            None => return,
        };
        let first = d.trail_col == u16::MAX;
        let crossed = first || d.trail_col != col || d.trail_line != line;
        if first || (crossed && self.rand_chance.sample(self.rng) < NEUR_SHIMMER_CHANCE) {
            d.ch = pick_pool_char(self.ctx.char_pool, self.rand_chance, self.rng);
        }
        d.set_trail(col, line);
        self.paint(col, line, d.ch, d.palette_slot, level, factor);
    }

    /// Paint the wake cell (the previous head's glyph, one rung
    /// dimmer — the streak's dimmer half).
    fn wake(&mut self, ch: char, palette_slot: u8, col: u16, line: u16, level: BrightnessLevel) {
        if col >= self.ctx.cols || line >= self.ctx.lines {
            return;
        }
        self.paint(col, line, ch, palette_slot, level, 1.0);
    }

    /// Paint one static cell (a neuron or a wire cell — the
    /// position is precomputed, the bounds re-checked against the
    /// live viewport).
    fn static_cell(
        &mut self,
        col: u16,
        line: u16,
        ch: char,
        palette_slot: u8,
        level: BrightnessLevel,
        factor: f32,
    ) {
        if col >= self.ctx.cols || line >= self.ctx.lines {
            return;
        }
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
        self.cells.push(NeurCell { col, line });
    }
}

impl NeuralRain {
    /// Draw pass — the wiring, the pulses, the neurons, the
    /// streamers, then the generation-tagged diff cleanup.
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
        let cap_rank = genesis_cap_rank(phase, lum);
        let flaring = self.burst_age.is_some();
        let signal_dark = matches!(phase, GenesisPhase::Signal);

        // Pass A — the wiring: the dotted idle wires (every third
        // path cell), the glow rung on recent traffic, the growth
        // prefix (the dendrite's uncovered cells), the retire
        // fade (the dimming wire). The event raises the rung,
        // never the cell count.
        for s in &mut self.synapses {
            if !s.active || s.fade <= 0.0 {
                continue;
            }
            if s.ch == '\0' {
                s.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
            }
            let n_cells = s.grown_cells().min(s.path.len());
            if n_cells == 0 {
                continue;
            }
            let mut level = BrightnessLevel::Ghost;
            if s.glow > 0.35 {
                level = BrightnessLevel::Dim;
            }
            if flaring {
                level = step_up_level(level);
            }
            if s.fade < 0.5 {
                level = step_down_level(level);
            }
            level = cap_level(level, cap_rank);
            let factor = (0.65 + 0.35 * s.glow) * s.fade.max(0.0);
            let mut painter = NeurPainter {
                ctx,
                frame,
                cells: &mut self.current_cells,
                rand_chance,
                rng,
            };
            for i in (0..n_cells).step_by(3) {
                let (col, line) = s.path[i];
                painter.static_cell(col, line, s.ch, s.palette_slot, level, factor);
            }
        }

        // Pass B — the pulses: the signal heads (the warm Hot
        // ceiling, NIGHT-research-22 — the retired flaring branch
        // lifted every head to Core for the whole 1.8 s burst
        // window, about ten times the black hole's whip flash) and
        // their one-deep wakes (Mid — the streak's dimmer half).
        for p in &mut self.pulses {
            if !p.active {
                continue;
            }
            let idx = p.pos.floor().max(0.0) as usize;
            let head = self.synapses.get(p.syn).and_then(|s| s.path.get(idx));
            let Some((col, line)) = head else {
                continue;
            };
            let level = pulse_head_level(cap_rank);
            let mut painter = NeurPainter {
                ctx,
                frame,
                cells: &mut self.current_cells,
                rand_chance,
                rng,
            };
            // The wake: the previous head cell, one rung dimmer.
            if let Some((tc, tl)) = p.trail_cell() {
                painter.wake(p.ch, p.palette_slot, tc, tl, step_down_level(level));
            }
            painter.pulse_head(p, *col as f32, *line as f32, level, 1.0);
        }

        // Pass C — the neurons: the potential ladder, the fired
        // flash, the output rung — under the genesis cap.
        for n in &mut self.nodes {
            if !n.active {
                continue;
            }
            // The glyph: rolled at first draw, re-rolled on fire
            // (the flash's peak — event-gated mutation).
            if n.ch == '\0' || n.flash > 0.9 {
                n.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
            }
            let mut level = if n.flash > 0.66 {
                BrightnessLevel::Core
            } else if n.flash > 0.33 || n.potential >= 0.75 {
                BrightnessLevel::Hot
            } else if n.potential >= 0.5 {
                BrightnessLevel::Mid
            } else if n.potential >= 0.25 {
                BrightnessLevel::Dim
            } else {
                BrightnessLevel::Ghost
            };
            if n.is_output {
                // The answer reads one rung hotter than the
                // question (the output band is the machine's
                // voice).
                level = step_up_level(level);
            }
            level = cap_level(level, cap_rank);
            let factor = 0.7 + 0.3 * (n.potential / NEUR_THRESHOLD).min(1.0);
            NeurPainter {
                ctx,
                frame,
                cells: &mut self.current_cells,
                rand_chance,
                rng,
            }
            .static_cell(n.x, n.y, n.ch, n.palette_slot, level, factor);
        }

        // Pass D — the streamers: the data streaks (head + the
        // one-deep wake), Ghost at rest, Dim through the signal
        // phase (the data IS the scene while the machine is
        // dark), a rung brighter while the burst's clump falls
        // in. Uncapped — the rain is visible from the first
        // frame.
        for d in &mut self.streamers {
            if !d.active {
                continue;
            }
            let mut level = if signal_dark {
                BrightnessLevel::Dim
            } else {
                BrightnessLevel::Ghost
            };
            if flaring {
                level = step_up_level(level);
            }
            let mut painter = NeurPainter {
                ctx,
                frame,
                cells: &mut self.current_cells,
                rand_chance,
                rng,
            };
            // The wake: the previous head cell, one rung dimmer.
            if let Some((tc, tl)) = d.trail_cell() {
                painter.wake(d.ch, d.palette_slot, tc, tl, step_down_level(level));
            }
            // The sway projection is read before the head borrow
            // (the drift phase rides the streamer's own clock).
            let x = d.display_x();
            let y = d.y;
            painter.streamer_head(d, x, y, level, 1.0);
        }

        // Pass E — the generation-tagged diff cleanup (monolith
        // pattern: u32 counter bump instead of clearing the
        // array).
        self.drawn_gen_counter = self.drawn_gen_counter.wrapping_add(1);
        let gen = self.drawn_gen_counter;
        let total = self.nodes.len() + self.synapses.len() + self.pulses.len();
        let need_len = total
            .saturating_mul(lines_us.max(1))
            .max(8)
            .saturating_add(self.synapses.len().saturating_mul(4));
        if self.drawn_gen.len() != need_len {
            self.drawn_gen.resize(need_len, 0);
        }
        for cell in &self.current_cells {
            let idx = cell.col as usize * lines_us + cell.line as usize;
            if idx < self.drawn_gen.len() {
                self.drawn_gen[idx] = gen;
            }
            // NIGHT-hunter-29 (the phosphor ownership rule — the
            // doc on clear_phosphor_metadata): a cell drawn this
            // frame is owned by the draw, so its phosphor state
            // is zeroed every frame — no ghost-vs-draw strobe at
            // the freeze/resume window (the black hole's owner
            // report generalized to the whole family tree).
            clear_phosphor_metadata(cleanup, cell.col, cell.line);
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

/// The genesis's brightness cap as a ladder rank (0 = Ghost ..
/// 4 = Core): nothing through the signal phase (the machine is
/// unbuilt), Dim while the layers build and the wiring sweeps,
/// ramping to the full law with the luminosity at the first
/// thought.
fn genesis_cap_rank(phase: GenesisPhase, lum: f32) -> u8 {
    match phase {
        GenesisPhase::Signal => 0,
        GenesisPhase::Layers | GenesisPhase::Wire => 1,
        GenesisPhase::Thought => (1.0 + lum * 3.0) as u8,
        GenesisPhase::Steady => 4,
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

/// The pulse-head ladder (Pass B's draw read): a riding signal's
/// head composes at the warm Hot ceiling under the genesis cap.
///
/// NIGHT-research-22 (the masterclass audit's soft-light ruling,
/// tier two): the retired flaring branch lifted every pulse head
/// to Core for the whole 1.8 s burst window — about ten times the
/// black hole's whip flash, the audit's standing-Core finding.
/// The machine's white lives on the neurons' fire flashes (the
/// 0.35 s fired-flash tau) and the genesis ramp; a riding signal
/// reads the warm ceiling, calm or flaring.
pub(crate) fn pulse_head_level(cap_rank: u8) -> BrightnessLevel {
    cap_level(BrightnessLevel::Hot, cap_rank)
}

/// Step a brightness level up by one rung (toward Core) — the
/// burst's flare rung on the wires, the output band's answer.
///
/// NIGHT-research-22: the step-up stops at the warm Hot ceiling —
/// the output band's standing Hot (a high-potential answer node)
/// no longer steps to Core (the audit's second standing site);
/// a fired flash (already Core from the fire moment) keeps its
/// white through the step.
pub(crate) fn step_up_level(level: BrightnessLevel) -> BrightnessLevel {
    match level {
        BrightnessLevel::Ghost => BrightnessLevel::Dim,
        BrightnessLevel::Dim => BrightnessLevel::Mid,
        BrightnessLevel::Mid => BrightnessLevel::Hot,
        BrightnessLevel::Hot | BrightnessLevel::Core => level,
    }
}

/// Step a brightness level down (toward Ghost) — the retiring
/// wire's fade, the wakes (mirrors the family's step-down).
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
