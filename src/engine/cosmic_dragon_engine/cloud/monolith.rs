// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Structured segmented rain for the monolith scene.

use std::time::{Duration, Instant};

use crossterm::style::Color;
use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::cinematic::{monolith_breathing_factor, monolith_motion_factor, monolith_spine_cadence};
use crate::constants::MONOLITH_DRAWN_CELLS_PER_LANE_RESERVE;
use crate::constants::MONOLITH_MIN_STREAM_SPAN;
use crate::constants::MONOLITH_SPAWN_RATE_FLOOR;
use crate::constants::MONOLITH_SPAWN_RATE_MULT;
use crate::constants::SPAWN_REMAINDER_CAP;
use crate::frame::Frame;
use crate::runtime::MonolithSize;

use super::render::DrawCtx;

// All tuning constants now centralized in central_control_rains.rs.
// MAX_SEGMENTS is a local structural const — Rust requires a concrete
// integer literal in [expr; N] repeat expressions within struct defaults.
pub(super) const MAX_SEGMENTS: usize = 9;

#[derive(Clone, Copy, Debug)]
pub(crate) enum SegmentKind {
    Micro,
    Short,
    Medium,
    Hero,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum BrightnessLevel {
    Ghost,
    Dim,
    Mid,
    Hot,
    Core,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DrawnCellKind {
    Segment,
    Spine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DrawnCell {
    pub(crate) col: u16,
    pub(crate) line: u16,
    pub(crate) kind: DrawnCellKind,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Segment {
    pub(super) offset: u16,
    pub(super) len: u8,
    pub(super) kind: SegmentKind,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ActivationParams {
    pub(super) now: Instant,
    pub(super) lines: u16,
    pub(super) size: MonolithSize,
    pub(super) palette_slot: u8,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SpineTone {
    pub(super) breath: f32,
    pub(super) cadence: u16,
}

impl Segment {
    const fn empty() -> Self {
        Self {
            offset: 0,
            len: 0,
            kind: SegmentKind::Micro,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct MonolithStream {
    pub(crate) active: bool,
    pub(crate) col: u16,
    pub(crate) head: f32,
    pub(crate) speed_mult: f32,
    pub(crate) phase: f32,
    pub(crate) span: u16,
    pub(crate) palette_slot: u8,
    pub(crate) layer: u8,
    pub(crate) segments: [Segment; MAX_SEGMENTS],
    pub(crate) segment_count: u8,
    pub(crate) last_time: Option<Instant>,
}

impl MonolithStream {
    fn new(col: u16) -> Self {
        Self {
            active: false,
            col,
            head: 0.0,
            speed_mult: 1.0,
            phase: 0.0,
            span: MONOLITH_MIN_STREAM_SPAN,
            palette_slot: 0,
            layer: 0,
            segments: [Segment::empty(); MAX_SEGMENTS],
            segment_count: 0,
            last_time: None,
        }
    }

    fn reset_for_lane(&mut self, col: u16) {
        self.active = false;
        self.col = col;
        self.head = 0.0;
        self.speed_mult = 1.0;
        self.phase = 0.0;
        self.span = MONOLITH_MIN_STREAM_SPAN;
        self.palette_slot = 0;
        self.layer = 0;
        self.segment_count = 0;
        self.last_time = None;
    }
}

pub(crate) struct MonolithRain {
    pub(crate) streams: Vec<MonolithStream>,
    previous_cells: Vec<DrawnCell>,
    current_cells: Vec<DrawnCell>,
    /// Per-cell generation tag, indexed `col * lines + line`. Sized to
    /// `streams.len() * lines` (i.e. cols × lines). When
    /// `drawn_gen_counter` is bumped each frame, cells drawn this frame
    /// are tagged with the new counter value. The clear pass then skips
    /// any `previous_cells` whose tag matches — those cells are about to
    /// be overwritten by the new draw, so clearing them first is wasted
    /// work (clear_phosphor_metadata + frame.set_force that gets
    /// immediately overwritten by the new draw's frame.set).
    ///
    /// Saves ~60-80% of clear_cell calls in the monolith draw pass at
    /// typical redraw ratios (most cells are redrawn at the same
    /// position frame-to-frame because streams move by 1 row per frame).
    drawn_gen: Vec<u32>,
    drawn_gen_counter: u32,
    spawn_scan_idx: usize,
    active_count: usize,
}

pub(crate) struct MonolithSpawnParams {
    pub(crate) cols: u16,
    pub(crate) lines: u16,
    pub(crate) density: f32,
    pub(crate) size: MonolithSize,
    pub(crate) active_palette_slot: u8,
    pub(crate) spawn_scale: f32,
    pub(crate) mouse_enabled: bool,
    pub(crate) mouse_col: u16,
    /// Optional per-column spawn probability weights (0.0..=1.0).
    ///
    /// When `Some`, the monolith spawner uses rejection sampling: a candidate
    /// lane is only accepted if a uniform random draw is `<= map[lane]`. Lanes
    /// with map value `1.0` always pass; lanes with `0.0` never spawn. When
    /// `None`, spawn distribution is uniform (default).
    ///
    /// The slice length should match the lane count; if shorter, missing lanes
    /// are treated as `1.0` (always available). If longer, extra entries are
    /// ignored.
    pub(crate) density_map: Option<&'static [f64]>,
}

pub(crate) struct MonolithRandom<'a> {
    pub(crate) rng: &'a mut StdRng,
    pub(crate) rand_chance: &'a Uniform<f32>,
    pub(crate) rand_col: &'a Uniform<u16>,
}

pub(crate) struct MonolithCleanup<'a> {
    pub(crate) lines: u16,
    pub(crate) bg: Option<Color>,
    pub(crate) phosphor: &'a mut [u8],
    pub(crate) phosphor_base_fg: &'a mut [Option<Color>],
    pub(crate) phosphor_base_ch: &'a mut [char],
    pub(crate) phosphor_layer: &'a mut [u8],
}

impl MonolithRain {
    pub(crate) fn new() -> Self {
        Self {
            streams: Vec::new(),
            previous_cells: Vec::new(),
            current_cells: Vec::new(),
            drawn_gen: Vec::new(),
            drawn_gen_counter: 0,
            spawn_scan_idx: 0,
            active_count: 0,
        }
    }

    pub(crate) fn reset(&mut self, cols: u16) {
        let lane_count = lane_count(cols);
        if self.streams.len() != lane_count {
            self.streams.clear();
            self.streams.reserve(lane_count);
            for lane in 0..lane_count {
                self.streams.push(MonolithStream::new(lane as u16));
            }
            let reserve = lane_count.saturating_mul(MONOLITH_DRAWN_CELLS_PER_LANE_RESERVE);
            self.previous_cells = Vec::with_capacity(reserve);
            self.current_cells = Vec::with_capacity(reserve);
        } else {
            for (lane, stream) in self.streams.iter_mut().enumerate() {
                stream.reset_for_lane(lane as u16);
            }
            self.previous_cells.clear();
            self.current_cells.clear();
        }
        self.drawn_gen.clear();
        self.drawn_gen_counter = 0;
        self.spawn_scan_idx = 0;
        self.active_count = 0;
    }

    #[must_use]
    pub(crate) fn active_count(&self) -> usize {
        self.active_count
    }

    pub(crate) fn adopt_palette_slot(&mut self, palette_slot: u8) {
        for stream in &mut self.streams {
            if stream.active {
                stream.palette_slot = palette_slot;
            }
        }
    }

    /// §H10: shift `last_time` of every active stream forward by the
    /// pause duration so the first post-resume frame computes a small dt
    /// (not the full pause duration). Previously this was "safe by accident"
    /// because `resume_blend ≈ 0` on the first frame zeroed the motion
    /// delta — but if a callsite ever computes a non-trivial dt before
    /// `resume_blend` ramps up, streams would teleport. Explicit shift
    /// removes that reliance.
    pub(crate) fn shift_active_streams_last_time(&mut self, elapsed: std::time::Duration) {
        for stream in &mut self.streams {
            if stream.active {
                if let Some(t) = stream.last_time.as_mut() {
                    *t += elapsed;
                }
            }
        }
    }

    pub(crate) fn clear_draw_history(&mut self) {
        self.previous_cells.clear();
        self.current_cells.clear();
    }

    #[cfg(test)]
    pub(crate) fn deactivate_all_for_test(&mut self) {
        for stream in &mut self.streams {
            stream.active = false;
        }
        self.active_count = 0;
    }

    #[cfg(test)]
    pub(crate) fn draw_history_count_for_test(&self) -> usize {
        self.previous_cells.len() + self.current_cells.len()
    }

    #[cfg(test)]
    pub(crate) fn drawn_cells_for_test(&self) -> &[DrawnCell] {
        &self.previous_cells
    }

    #[cfg(test)]
    pub(crate) fn active_heads_for_test(&self) -> Vec<f32> {
        self.streams
            .iter()
            .filter(|stream| stream.active)
            .map(|stream| stream.head)
            .collect()
    }

    pub(crate) fn clear_spine_phosphor(&self, cleanup: &mut MonolithCleanup<'_>) {
        for cell in &self.previous_cells {
            if matches!(cell.kind, DrawnCellKind::Spine) {
                clear_phosphor_metadata(cleanup, cell.col, cell.line);
            }
        }
    }

    pub(crate) fn spawn(
        &mut self,
        now: Instant,
        elapsed: Duration,
        spawn_remainder: &mut f32,
        params: MonolithSpawnParams,
        random: &mut MonolithRandom<'_>,
    ) {
        if params.cols == 0 || params.lines == 0 || self.streams.is_empty() {
            *spawn_remainder = 0.0;
            return;
        }

        self.refresh_active_count();
        let target = target_active_count(self.streams.len(), params.density);
        if self.active_count >= target {
            *spawn_remainder = (*spawn_remainder).min(SPAWN_REMAINDER_CAP);
            return;
        }

        let deficit = target - self.active_count;
        let spawn_rate = (target as f32 * MONOLITH_SPAWN_RATE_MULT + MONOLITH_SPAWN_RATE_FLOOR)
            * params.spawn_scale;
        let budget =
            elapsed.as_secs_f32() * spawn_rate + (*spawn_remainder).min(SPAWN_REMAINDER_CAP);
        if !budget.is_finite() || budget <= 0.0 {
            *spawn_remainder = 0.0;
            return;
        }

        let to_spawn = (budget.floor() as usize).min(deficit);
        *spawn_remainder = (budget - to_spawn as f32).min(SPAWN_REMAINDER_CAP);
        if to_spawn == 0 {
            return;
        }

        for _ in 0..to_spawn {
            let Some(idx) = self.find_inactive_lane(
                params.mouse_enabled,
                params.mouse_col,
                random.rand_col,
                random.rng,
                params.density_map,
                random.rand_chance,
            ) else {
                break;
            };

            activate_stream(
                &mut self.streams[idx],
                ActivationParams {
                    now,
                    lines: params.lines,
                    size: params.size,
                    palette_slot: params.active_palette_slot,
                },
                random.rand_chance,
                random.rng,
            );
            self.active_count += 1;
            self.spawn_scan_idx = (idx + 1) % self.streams.len();
        }
    }

    pub(crate) fn advance(
        &mut self,
        now: Instant,
        lines: u16,
        chars_per_sec: f32,
        max_sim_delta: Duration,
        resume_blend: f32,
    ) {
        let speed = chars_per_sec.max(0.0);
        for stream in &mut self.streams {
            if !stream.active {
                continue;
            }

            let Some(last) = stream.last_time else {
                stream.last_time = Some(now);
                continue;
            };
            let mut elapsed = now.saturating_duration_since(last);
            if max_sim_delta > Duration::from_millis(0) {
                elapsed = elapsed.min(max_sim_delta);
            }
            let motion = monolith_motion_factor(stream.phase, stream.head);
            let delta = elapsed.as_secs_f32() * speed * stream.speed_mult * motion * resume_blend;
            stream.head += delta.max(0.0);
            stream.last_time = Some(now);

            if stream.head - stream.span as f32 > lines as f32 + 1.0 {
                stream.active = false;
                self.active_count = self.active_count.saturating_sub(1);
            }
        }
    }

    pub(crate) fn draw(
        &mut self,
        ctx: &DrawCtx<'_>,
        frame: &mut Frame,
        cleanup: &mut MonolithCleanup<'_>,
    ) {
        let lines_us = ctx.lines as usize;

        // Pass 1: Draw all active streams into current_cells.
        // This populates the new frame state via frame.set (with equality
        // check) and records each drawn position in current_cells.
        self.current_cells.clear();
        for stream in &mut self.streams {
            if !stream.active {
                continue;
            }

            if visible_range(stream, ctx.lines).is_none() {
                continue;
            }

            // Compute cinematic breath/cadence once per stream and pass to
            // both draw_spine and draw_segments. Without this, each function
            // independently recomputes monolith_breathing_factor — wasting
            // one cross-module call per active stream per frame.
            let tone = SpineTone {
                breath: monolith_breathing_factor(stream.phase, stream.head, stream.layer),
                cadence: monolith_spine_cadence(stream.phase, stream.layer),
            };
            draw_spine(stream, ctx, frame, &mut self.current_cells, tone);
            draw_segments(stream, ctx, frame, &mut self.current_cells, tone.breath);
        }

        // Pass 2: Tag every position drawn this frame in O(current_cells.len()).
        // The drawn_gen array is indexed `col * lines + line` and sized to
        // `streams.len() * lines` (= cols × lines). Each frame bumps the
        // counter, so a single u32 write marks "drawn this frame" without
        // needing to clear the array.
        self.drawn_gen_counter = self.drawn_gen_counter.wrapping_add(1);
        let gen = self.drawn_gen_counter;
        let need_len = self.streams.len().saturating_mul(lines_us);
        if self.drawn_gen.len() != need_len {
            self.drawn_gen.resize(need_len, 0);
        }
        for cell in &self.current_cells {
            let idx = cell.col as usize * lines_us + cell.line as usize;
            // Direct index is safe: col < cols (checked at stream creation)
            // and line < lines (checked in draw_spine_cell / draw_segments).
            // The bounds check is defensive against resize races.
            if idx < self.drawn_gen.len() {
                self.drawn_gen[idx] = gen;
            }
        }

        // Pass 3: Clear only previous_cells NOT redrawn this frame.
        // For cells that WILL be redrawn, the new draw's frame.set already
        // overwrote the cell state — clearing first would be pure waste
        // (clear_phosphor_metadata zeroes 4 arrays, then frame.set_force
        // writes the blank cell, then the new draw's frame.set writes the
        // actual cell — 2 redundant writes per cell skipped).
        //
        // Typical monolith redraw ratio is ~60-80% (streams move 1 row/frame,
        // so most segments overlap their previous position). Skipping those
        // saves ~100-130 clear_cell calls per frame at 60-column density.
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

    fn refresh_active_count(&mut self) {
        self.active_count = self.streams.iter().filter(|stream| stream.active).count();
    }

    pub(crate) fn find_inactive_lane(
        &mut self,
        mouse_enabled: bool,
        mouse_col: u16,
        rand_col: &Uniform<u16>,
        rng: &mut StdRng,
        density_map: Option<&'static [f64]>,
        rand_chance: &Uniform<f32>,
    ) -> Option<usize> {
        let len = self.streams.len();
        // Try random selection up to 16 times. With a density map, apply
        // rejection sampling: a candidate lane must pass both the availability
        // check AND a probability draw against map[lane].
        for _ in 0..len.min(16) {
            let lane = (rand_col.sample(rng) as usize) % len;
            if !self.lane_is_available(lane, mouse_enabled, mouse_col) {
                continue;
            }
            // Density map gate: skip lanes with low spawn probability.
            if let Some(map) = density_map {
                let weight = map.get(lane).copied().unwrap_or(1.0);
                if weight < 1.0 {
                    // Draw a uniform f32 in [0.0, 1.0) and accept if <= weight.
                    // rand_chance is Uniform<f32> in [0.0, 1.0).
                    if rand_chance.sample(rng) > weight as f32 {
                        continue;
                    }
                }
            }
            return Some(lane);
        }

        // Fallback: linear scan for any available lane. Skip density map here
        // — if we've exhausted random tries, we'd rather spawn somewhere than
        // starve the renderer. Density map is a preference, not a hard rule.
        let start = self.spawn_scan_idx.min(len.saturating_sub(1));
        for offset in 0..len {
            let lane = (start + offset) % len;
            if self.lane_is_available(lane, mouse_enabled, mouse_col) {
                return Some(lane);
            }
        }
        None
    }

    fn lane_is_available(&self, lane: usize, _mouse_enabled: bool, _mouse_col: u16) -> bool {
        if self.streams[lane].active {
            return false;
        }
        // v17 mastery: mouse spawn avoidance REMOVED. Owner reported rain
        // becoming empty under the cursor. The old MOUSE_AVOID_RADIUS_COLS
        // check created a moving empty zone. Removed for visual continuity.
        true
    }
}

// v50.0.0-beta.7 LOC refactor: free functions extracted to
// monolith_helpers.rs. Only the functions this file actually calls are
// imported (S-master-1-v2: the former `#[allow(unused_imports)]` masked
// 9 dead names — build_segments, draw_spine_cell, layer_from_roll,
// segment_gap, segment_len, segment_level, spine_envelope, varied_span,
// varied_speed_mult are used only inside monolith_helpers itself).
// color_for_level + bold_for_level are re-exported for tests.
use super::monolith_helpers::{
    activate_stream, clear_cell, clear_phosphor_metadata, draw_segments, draw_spine, lane_count,
    target_active_count, visible_range,
};
#[allow(unused_imports)]
pub(crate) use super::monolith_helpers::{bold_for_level, color_for_level};
