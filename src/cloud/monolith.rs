// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Structured segmented rain for the monolith scene.

use std::time::{Duration, Instant};

use crossterm::style::Color;
use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::cell::Cell;
use crate::cinematic::{
    monolith_breathing_factor, monolith_hero_pulse, monolith_motion_factor, monolith_spine_cadence,
};
use crate::constants::EDGE_FADE_BOLD_THRESHOLD;
use crate::constants::MAX_PALETTE_SLOTS;
use crate::constants::MONOLITH_LAYER_BRIGHTNESS;
use crate::constants::SPAWN_REMAINDER_CAP;
use crate::frame::Frame;
use crate::palette;
use crate::runtime::{BoldMode, ColorMode, MonolithSize};
use crate::terminal::blank_cell;

use super::monolith_glyphs::{segment_char, spine_char};
use super::render::DrawCtx;

const MAX_SEGMENTS: usize = 9;
const MIN_STREAM_SPAN: u16 = 14;
const MAX_STREAM_SPAN: u16 = 30;
const ACTIVE_BASE: f32 = 0.06;
const ACTIVE_DENSITY_MULT: f32 = 0.28;
const ACTIVE_MAX: f32 = 0.35;
const SPAWN_RATE_MULT: f32 = 1.4;
const SPAWN_RATE_FLOOR: f32 = 2.0;
const SPINE_PERIOD: u16 = 3;
const SPINE_BRIGHTNESS: f32 = 0.07;
const DRAWN_CELLS_PER_LANE_RESERVE: usize = 32;

#[derive(Clone, Copy, Debug)]
pub(super) enum SegmentKind {
    Micro,
    Short,
    Medium,
    Hero,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum BrightnessLevel {
    Ghost,
    Dim,
    Mid,
    Hot,
    Core,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DrawnCellKind {
    Segment,
    Spine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DrawnCell {
    pub(super) col: u16,
    pub(super) line: u16,
    pub(super) kind: DrawnCellKind,
}

#[derive(Clone, Copy, Debug)]
struct Segment {
    offset: u16,
    len: u8,
    kind: SegmentKind,
}

#[derive(Clone, Copy, Debug)]
struct ActivationParams {
    now: Instant,
    lines: u16,
    size: MonolithSize,
    palette_slot: u8,
}

#[derive(Clone, Copy, Debug)]
struct SpineTone {
    breath: f32,
    cadence: u16,
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
struct MonolithStream {
    active: bool,
    col: u16,
    head: f32,
    speed_mult: f32,
    phase: f32,
    span: u16,
    palette_slot: u8,
    layer: u8,
    segments: [Segment; MAX_SEGMENTS],
    segment_count: u8,
    last_time: Option<Instant>,
}

impl MonolithStream {
    fn new(col: u16) -> Self {
        Self {
            active: false,
            col,
            head: 0.0,
            speed_mult: 1.0,
            phase: 0.0,
            span: MIN_STREAM_SPAN,
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
        self.span = MIN_STREAM_SPAN;
        self.palette_slot = 0;
        self.layer = 0;
        self.segment_count = 0;
        self.last_time = None;
    }
}

pub(super) struct MonolithRain {
    streams: Vec<MonolithStream>,
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

pub(super) struct MonolithSpawnParams {
    pub(super) cols: u16,
    pub(super) lines: u16,
    pub(super) density: f32,
    pub(super) size: MonolithSize,
    pub(super) active_palette_slot: u8,
    pub(super) spawn_scale: f32,
    pub(super) mouse_enabled: bool,
    pub(super) mouse_col: u16,
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
    pub(super) density_map: Option<&'static [f64]>,
}

pub(super) struct MonolithRandom<'a> {
    pub(super) rng: &'a mut StdRng,
    pub(super) rand_chance: &'a Uniform<f32>,
    pub(super) rand_col: &'a Uniform<u16>,
}

pub(super) struct MonolithCleanup<'a> {
    pub(super) lines: u16,
    pub(super) bg: Option<Color>,
    pub(super) phosphor: &'a mut [u8],
    pub(super) phosphor_base_fg: &'a mut [Option<Color>],
    pub(super) phosphor_base_ch: &'a mut [char],
    pub(super) phosphor_layer: &'a mut [u8],
}

impl MonolithRain {
    pub(super) fn new() -> Self {
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

    pub(super) fn reset(&mut self, cols: u16) {
        let lane_count = lane_count(cols);
        if self.streams.len() != lane_count {
            self.streams.clear();
            self.streams.reserve(lane_count);
            for lane in 0..lane_count {
                self.streams.push(MonolithStream::new(lane_col(lane)));
            }
            let reserve = lane_count.saturating_mul(DRAWN_CELLS_PER_LANE_RESERVE);
            self.previous_cells = Vec::with_capacity(reserve);
            self.current_cells = Vec::with_capacity(reserve);
        } else {
            for (lane, stream) in self.streams.iter_mut().enumerate() {
                stream.reset_for_lane(lane_col(lane));
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
    pub(super) fn active_count(&self) -> usize {
        self.active_count
    }

    pub(super) fn adopt_palette_slot(&mut self, palette_slot: u8) {
        for stream in &mut self.streams {
            if stream.active {
                stream.palette_slot = palette_slot;
            }
        }
    }

    /// v30.2 §H10: shift `last_time` of every active stream forward by the
    /// pause duration so the first post-resume frame computes a small dt
    /// (not the full pause duration). Previously this was "safe by accident"
    /// because `resume_blend ≈ 0` on the first frame zeroed the motion
    /// delta — but if a callsite ever computes a non-trivial dt before
    /// `resume_blend` ramps up, streams would teleport. Explicit shift
    /// removes that reliance.
    pub(super) fn shift_active_streams_last_time(&mut self, elapsed: std::time::Duration) {
        for stream in &mut self.streams {
            if stream.active {
                if let Some(t) = stream.last_time.as_mut() {
                    *t += elapsed;
                }
            }
        }
    }

    pub(super) fn clear_draw_history(&mut self) {
        self.previous_cells.clear();
        self.current_cells.clear();
    }

    #[cfg(test)]
    pub(super) fn deactivate_all_for_test(&mut self) {
        for stream in &mut self.streams {
            stream.active = false;
        }
        self.active_count = 0;
    }

    #[cfg(test)]
    pub(super) fn draw_history_count_for_test(&self) -> usize {
        self.previous_cells.len() + self.current_cells.len()
    }

    #[cfg(test)]
    pub(super) fn drawn_cells_for_test(&self) -> &[DrawnCell] {
        &self.previous_cells
    }

    #[cfg(test)]
    pub(super) fn active_heads_for_test(&self) -> Vec<f32> {
        self.streams
            .iter()
            .filter(|stream| stream.active)
            .map(|stream| stream.head)
            .collect()
    }

    pub(super) fn clear_spine_phosphor(&self, cleanup: &mut MonolithCleanup<'_>) {
        for cell in &self.previous_cells {
            if matches!(cell.kind, DrawnCellKind::Spine) {
                clear_phosphor_metadata(cleanup, cell.col, cell.line);
            }
        }
    }

    pub(super) fn spawn(
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
        let spawn_rate = (target as f32 * SPAWN_RATE_MULT + SPAWN_RATE_FLOOR) * params.spawn_scale;
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

    pub(super) fn advance(
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

    pub(super) fn draw(
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

    pub(super) fn find_inactive_lane(
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

fn activate_stream(
    stream: &mut MonolithStream,
    params: ActivationParams,
    rand_chance: &Uniform<f32>,
    rng: &mut StdRng,
) {
    stream.active = true;
    stream.head = 0.0;
    stream.speed_mult = varied_speed_mult(rand_chance.sample(rng));
    stream.phase = rand_chance.sample(rng);
    stream.span = varied_span(params.lines, rand_chance.sample(rng));
    stream.palette_slot = params.palette_slot;
    stream.layer = layer_from_roll(rand_chance.sample(rng));
    stream.last_time = Some(params.now);
    build_segments(stream, params.size, rand_chance, rng);
}

fn build_segments(
    stream: &mut MonolithStream,
    size: MonolithSize,
    rand_chance: &Uniform<f32>,
    rng: &mut StdRng,
) {
    let mut cursor = 0u16;
    let mut count = 0usize;
    while cursor < stream.span && count < MAX_SEGMENTS {
        let roll = rand_chance.sample(rng);
        let kind = if roll < 0.36 {
            SegmentKind::Micro
        } else if roll < 0.70 {
            SegmentKind::Short
        } else if roll < 0.93 {
            SegmentKind::Medium
        } else {
            SegmentKind::Hero
        };
        let len = segment_len(kind, size, rand_chance.sample(rng));

        stream.segments[count] = Segment {
            offset: cursor,
            len,
            kind,
        };
        count += 1;

        let gap = segment_gap(size, rand_chance.sample(rng));
        cursor = cursor.saturating_add(len as u16).saturating_add(gap);
    }
    stream.segment_count = count as u8;
}

fn segment_len(kind: SegmentKind, size: MonolithSize, roll: f32) -> u8 {
    let extra = roll.clamp(0.0, 1.0);
    match (size, kind) {
        (MonolithSize::Small, SegmentKind::Micro) => 1,
        (MonolithSize::Small, SegmentKind::Short) => 2,
        (MonolithSize::Small, SegmentKind::Medium) => 3 + (extra * 2.0) as u8,
        (MonolithSize::Small, SegmentKind::Hero) => 5 + (extra * 3.0) as u8,
        (MonolithSize::Normal, SegmentKind::Micro) => 1,
        (MonolithSize::Normal, SegmentKind::Short) => 2 + (extra * 2.0) as u8,
        (MonolithSize::Normal, SegmentKind::Medium) => 4 + (extra * 2.0) as u8,
        (MonolithSize::Normal, SegmentKind::Hero) => 6 + (extra * 3.0) as u8,
        (MonolithSize::Large, SegmentKind::Micro) => 2,
        (MonolithSize::Large, SegmentKind::Short) => 3 + (extra * 2.0) as u8,
        (MonolithSize::Large, SegmentKind::Medium) => 5 + (extra * 3.0) as u8,
        (MonolithSize::Large, SegmentKind::Hero) => 8 + (extra * 3.0) as u8,
    }
}

fn segment_gap(size: MonolithSize, roll: f32) -> u16 {
    let roll = roll.clamp(0.0, 1.0);
    match size {
        MonolithSize::Small => 3 + (roll * 6.0) as u16,
        MonolithSize::Normal => 2 + (roll * 5.0) as u16,
        MonolithSize::Large => 2 + (roll * 4.0) as u16,
    }
}

fn draw_spine(
    stream: &MonolithStream,
    ctx: &DrawCtx<'_>,
    frame: &mut Frame,
    drawn_cells: &mut Vec<DrawnCell>,
    tone: SpineTone,
) {
    let head_line = stream.head.floor() as i32;
    for idx in 0..stream.segment_count as usize {
        let segment = stream.segments[idx];
        let bottom = head_line - segment.offset as i32;
        let top = bottom - segment.len as i32 + 1;
        let envelope = spine_envelope(segment.kind);

        for line_i in (top - envelope)..top {
            draw_spine_cell(
                stream,
                ctx,
                frame,
                drawn_cells,
                line_i,
                segment.offset,
                tone,
            );
        }
        for line_i in (bottom + 1)..=(bottom + envelope) {
            draw_spine_cell(
                stream,
                ctx,
                frame,
                drawn_cells,
                line_i,
                segment.offset,
                tone,
            );
        }
    }
}

fn draw_spine_cell(
    stream: &MonolithStream,
    ctx: &DrawCtx<'_>,
    frame: &mut Frame,
    drawn_cells: &mut Vec<DrawnCell>,
    line_i: i32,
    segment_offset: u16,
    tone: SpineTone,
) {
    if line_i < 0 || line_i >= ctx.lines as i32 {
        return;
    }
    let line = line_i as u16;
    let cadence = tone.cadence.max(SPINE_PERIOD);
    if (line + stream.col + segment_offset) % cadence != 0 {
        return;
    }

    let edge_fade = ctx.edge_fade(line);
    let fg = color_for_level(
        ctx,
        stream.palette_slot,
        line,
        stream.col,
        BrightnessLevel::Ghost,
        edge_fade * SPINE_BRIGHTNESS * layer_brightness(stream.layer) * 0.72 * tone.breath,
    );
    frame.set(
        stream.col,
        line,
        Cell {
            ch: spine_char(ctx, line, stream.col),
            fg,
            bg: ctx.bg,
            bold: false,
        },
    );
    drawn_cells.push(DrawnCell {
        col: stream.col,
        line,
        kind: DrawnCellKind::Spine,
    });
}

fn draw_segments(
    stream: &MonolithStream,
    ctx: &DrawCtx<'_>,
    frame: &mut Frame,
    drawn_cells: &mut Vec<DrawnCell>,
    breath: f32,
) {
    let head_line = stream.head.floor() as i32;
    let frac = stream.head.fract().clamp(0.0, 1.0);
    for idx in 0..stream.segment_count as usize {
        let segment = stream.segments[idx];
        let bottom = head_line - segment.offset as i32;
        let top = bottom - segment.len as i32 + 1;

        // F8: hoist hero_pulse per segment (all args are segment-invariant)
        let hero_pulse = if matches!(segment.kind, SegmentKind::Medium | SegmentKind::Hero) {
            monolith_hero_pulse(stream.phase, segment.offset, frac)
        } else {
            1.0
        };

        for line_i in top..=bottom {
            if line_i < 0 || line_i >= ctx.lines as i32 {
                continue;
            }
            let line = line_i as u16;
            let pos_from_bottom = (bottom - line_i) as u8;
            let level = segment_level(segment.kind, pos_from_bottom);
            let edge_fade = ctx.edge_fade(line);
            let pulse = if matches!(level, BrightnessLevel::Hot | BrightnessLevel::Core) {
                hero_pulse
            } else {
                1.0
            };
            let fg = color_for_level(
                ctx,
                stream.palette_slot,
                line,
                stream.col,
                level,
                edge_fade * layer_brightness(stream.layer) * breath * pulse,
            );
            let bold = bold_for_level(ctx.bold_mode, level, line, stream.col)
                && edge_fade >= EDGE_FADE_BOLD_THRESHOLD;
            let ch = segment_char(ctx, line, stream.col, segment.kind, pos_from_bottom);

            frame.set(
                stream.col,
                line,
                Cell {
                    ch,
                    fg,
                    bg: ctx.bg,
                    bold,
                },
            );
            drawn_cells.push(DrawnCell {
                col: stream.col,
                line,
                kind: DrawnCellKind::Segment,
            });
        }
    }
}

fn spine_envelope(kind: SegmentKind) -> i32 {
    match kind {
        SegmentKind::Micro => 0,
        SegmentKind::Short | SegmentKind::Medium => 1,
        SegmentKind::Hero => 2,
    }
}

fn segment_level(kind: SegmentKind, pos_from_bottom: u8) -> BrightnessLevel {
    match kind {
        SegmentKind::Micro => BrightnessLevel::Dim,
        SegmentKind::Short => {
            if pos_from_bottom == 0 {
                BrightnessLevel::Mid
            } else {
                BrightnessLevel::Dim
            }
        }
        SegmentKind::Medium => {
            if pos_from_bottom == 0 {
                BrightnessLevel::Hot
            } else {
                BrightnessLevel::Mid
            }
        }
        SegmentKind::Hero => match pos_from_bottom {
            0 => BrightnessLevel::Core,
            1 | 2 => BrightnessLevel::Hot,
            _ => BrightnessLevel::Mid,
        },
    }
}

pub(super) fn color_for_level(
    ctx: &DrawCtx<'_>,
    palette_slot: u8,
    line: u16,
    col: u16,
    level: BrightnessLevel,
    factor: f32,
) -> Option<Color> {
    if ctx.color_mode == ColorMode::Mono {
        return None;
    }

    let effective_slot = if ctx.color_uses_previous_palette(palette_slot, line, col) {
        palette_slot
    } else {
        ctx.active_palette_slot
    };
    // Cosmic Dragon egg #18: direct indexing with bounds check instead of .get().copied().unwrap_or().
    // palette_slices is a fixed array [T; MAX_PALETTE_SLOTS] (4 elements).
    let slot_idx = effective_slot as usize;
    let mut colors = if slot_idx < MAX_PALETTE_SLOTS {
        ctx.palette_slices[slot_idx]
    } else {
        &[]
    };
    if colors.is_empty() {
        let active_idx = ctx.active_palette_slot as usize;
        colors = if active_idx < MAX_PALETTE_SLOTS {
            ctx.palette_slices[active_idx]
        } else {
            &[]
        };
    }
    if colors.is_empty() {
        return None;
    }

    let last = colors.len().saturating_sub(1);
    let first_visible = usize::from(last > 0);
    // v17 mastery: raised palette indices for vivid high-contrast rain.
    // Old values were too dim — body cells (Ghost/Dim/Mid) were at 20-40%
    // of palette brightness, making the rain look dark/dim.
    // New values: Ghost/Dim at 33%, Mid at 60%, Hot at 85%, Core at 100%.
    let ghost_idx = (last / 3).max(first_visible); // visible trace at 33%
    let idx = match level {
        BrightnessLevel::Ghost => ghost_idx,
        BrightnessLevel::Dim => ghost_idx,
        // Mid: raised from 40% to 60% for clear body visibility
        BrightnessLevel::Mid => (last * 3) / 5,
        // Hot: raised from 80% to 85% for sharper afterglow contrast
        BrightnessLevel::Hot => (last * 17) / 20,
        // Core: always brightest
        BrightnessLevel::Core => last,
    };
    let base_color = colors[idx];
    let factor = factor.max(0.0);

    // Optimized hot path: decode color to RGB once, then chain all
    // blend operations on the raw (r, g, b) tuple without re-decoding.
    // This eliminates 2-4 color_to_rgb() calls per cell per frame.
    let (mut r, mut g, mut b) = palette::decode_color(base_color)?;

    // v30.3 (chroma audit, A10): the monolith color pipeline has three
    // brightness/blend stages. Each routes through the chroma engine
    // when active, falls back to chroma::legacy otherwise. All paths
    // use the same equations as the original inline math -- the
    // migration is a pure auditability refactor.
    //
    // Stage 1: factor < 1.0 -- brightness scale (dim).
    // Stage 2: factor > 1.0 -- blend toward white (boost).
    // Stage 3: Core level -- extra blend toward white (CORE_WF = 0.55).
    if factor < 1.0 {
        let (nr, ng, nb) = if ctx.color_pipeline.is_chroma() {
            let scaled = crate::chroma::palette::apply_brightness_rgb(r, g, b, factor);
            palette::decode_color(scaled).unwrap_or((r, g, b))
        } else {
            crate::chroma::legacy::scale_rgb(r, g, b, factor)
        };
        r = nr;
        g = ng;
        b = nb;
    }
    if factor > 1.0 {
        // v17 mastery: raised white_factor cap from 0.12 to 0.20 for
        // stronger pulse/breath brightness boost on Core/Hot cells.
        let white_factor = (factor - 1.0).min(0.20);
        let (nr, ng, nb) = if ctx.color_pipeline.is_chroma() {
            crate::chroma::palette::blend_toward_white_rgb(r, g, b, white_factor)
        } else {
            crate::chroma::legacy::blend_toward_white(r, g, b, white_factor)
        };
        r = nr;
        g = ng;
        b = nb;
    }
    if matches!(level, BrightnessLevel::Core) {
        // v17 mastery: CORE_WF = 140 (0.55 white blend). Was 115 (0.45).
        // Head/core cell is dramatically brighter than body/tail —
        // the high-contrast vivid hierarchy the owner wants.
        const CORE_WF: f32 = 140.0 / 256.0; // 0.546875 — was i32=140
        let (nr, ng, nb) = if ctx.color_pipeline.is_chroma() {
            crate::chroma::palette::blend_toward_white_rgb(r, g, b, CORE_WF)
        } else {
            crate::chroma::legacy::blend_toward_white(r, g, b, CORE_WF)
        };
        r = nr;
        g = ng;
        b = nb;
    }

    Some(Color::Rgb { r, g, b })
}

fn bold_for_level(mode: BoldMode, level: BrightnessLevel, line: u16, col: u16) -> bool {
    match mode {
        BoldMode::Off => false,
        BoldMode::All => !matches!(level, BrightnessLevel::Ghost | BrightnessLevel::Dim),
        BoldMode::Random => {
            matches!(level, BrightnessLevel::Core)
                || (matches!(level, BrightnessLevel::Hot) && ((line ^ col) & 1) == 0)
        }
    }
}

/// Per-layer brightness multiplier for the Monolith scene.
///
/// v30.0.0 centralization: values moved to
/// `central_control_rains.rs::MONOLITH_LAYER_BRIGHTNESS` so future
/// tuning requires editing only that single file. This wrapper now
/// just reads from the constant array.
///
/// Tracks the rain field's visibility floor (PARALLAX_BRIGHTNESS_MULT).
/// Mid is set slightly under the rain's mid value so monolith glyph
/// streams read as half-a-step behind the rain front, preserving depth
/// cue without the rain "disappearing" behind a too-dim monolith. Back
/// matches the rain back value so the monolith's distant body sits in
/// the same atmospheric haze as the distant rain. Front kept at 1.0
/// (monolith hero pulse stays the brightest glyph element — front rain
/// at 1.05 is still slightly brighter but the monolith's solid glyph
/// mass keeps it visually dominant as the focal anchor).
fn layer_brightness(layer: u8) -> f32 {
    MONOLITH_LAYER_BRIGHTNESS[layer as usize]
}

fn clear_cell(frame: &mut Frame, cleanup: &mut MonolithCleanup<'_>, col: u16, line: u16) {
    clear_phosphor_metadata(cleanup, col, line);
    // Use set_force: previous_cells are known-drawn from the last frame,
    // so the equality check in set() is almost always wasted work.
    frame.set_force(col, line, blank_cell(cleanup.bg));
}

fn clear_phosphor_metadata(cleanup: &mut MonolithCleanup<'_>, col: u16, line: u16) {
    if line >= cleanup.lines {
        return;
    }
    let pidx = col as usize * cleanup.lines as usize + line as usize;
    // F9: all 4 arrays are co-sized (allocated together in reset()).
    // Single bounds check suffices; skip 3 redundant get_mut() checks.
    if pidx >= cleanup.phosphor.len() {
        return;
    }
    cleanup.phosphor[pidx] = 0;
    cleanup.phosphor_base_fg[pidx] = None;
    cleanup.phosphor_base_ch[pidx] = '\0';
    cleanup.phosphor_layer[pidx] = 0;
}

fn visible_range(stream: &MonolithStream, lines: u16) -> Option<(u16, u16)> {
    if lines == 0 {
        return None;
    }
    let head = stream.head.floor() as i32;
    let min = (head - stream.span as i32).max(0) as u16;
    let max = head.min(lines as i32 - 1);
    if max < 0 || min > max as u16 {
        None
    } else {
        Some((min, max as u16))
    }
}

pub(super) fn target_active_count(lanes: usize, density: f32) -> usize {
    if lanes == 0 {
        return 0;
    }
    let ratio =
        (ACTIVE_BASE + density.clamp(0.01, 5.0) * ACTIVE_DENSITY_MULT).clamp(0.02, ACTIVE_MAX);
    ((lanes as f32 * ratio).round() as usize).clamp(1, lanes)
}

fn lane_count(cols: u16) -> usize {
    cols.max(1) as usize
}

fn lane_col(lane: usize) -> u16 {
    lane as u16
}

fn varied_speed_mult(roll: f32) -> f32 {
    0.78 + roll.clamp(0.0, 1.0) * 0.58
}

fn varied_span(lines: u16, roll: f32) -> u16 {
    let max = MAX_STREAM_SPAN
        .min(lines.saturating_add(8))
        .max(MIN_STREAM_SPAN);
    let span = MIN_STREAM_SPAN as f32 + roll.clamp(0.0, 1.0) * (max - MIN_STREAM_SPAN) as f32;
    span.round() as u16
}

fn layer_from_roll(roll: f32) -> u8 {
    if roll < 0.45 {
        0
    } else if roll < 0.85 {
        1
    } else {
        2
    }
}
