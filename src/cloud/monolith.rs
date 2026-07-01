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
use crate::constants::EDGE_FADE_BOLD_THRESHOLD;
use crate::constants::SPAWN_REMAINDER_CAP;
use crate::frame::Frame;
use crate::palette;
use crate::runtime::{BoldMode, ColorMode, MonolithSize};
use crate::terminal::blank_cell;
use crate::zactrix_core::{
    monolith_breathing_factor, monolith_hero_pulse, monolith_motion_factor, monolith_spine_cadence,
};

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
    spawn_scan_idx: usize,
    active_count: usize,
}

pub(super) struct MonolithSpawnParams {
    pub(super) cols: u16,
    pub(super) lines: u16,
    pub(super) full_width: bool,
    pub(super) density: f32,
    pub(super) size: MonolithSize,
    pub(super) active_palette_slot: u8,
    pub(super) spawn_scale: f32,
    pub(super) mouse_enabled: bool,
    pub(super) mouse_col: u16,
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
            spawn_scan_idx: 0,
            active_count: 0,
        }
    }

    pub(super) fn reset(&mut self, cols: u16, full_width: bool) {
        let lane_count = lane_count(cols, full_width);
        if self.streams.len() != lane_count {
            self.streams.clear();
            self.streams.reserve(lane_count);
            for lane in 0..lane_count {
                self.streams
                    .push(MonolithStream::new(lane_col(lane, full_width)));
            }
            let reserve = lane_count.saturating_mul(DRAWN_CELLS_PER_LANE_RESERVE);
            self.previous_cells = Vec::with_capacity(reserve);
            self.current_cells = Vec::with_capacity(reserve);
        } else {
            for (lane, stream) in self.streams.iter_mut().enumerate() {
                stream.reset_for_lane(lane_col(lane, full_width));
            }
            self.previous_cells.clear();
            self.current_cells.clear();
        }
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
                params.full_width,
                params.mouse_enabled,
                params.mouse_col,
                random.rand_col,
                random.rng,
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
        for cell in &self.previous_cells {
            clear_cell(frame, cleanup, cell.col, cell.line);
            if ctx.full_width && cell.col + 1 < frame.width {
                clear_cell(frame, cleanup, cell.col + 1, cell.line);
            }
        }
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

        std::mem::swap(&mut self.previous_cells, &mut self.current_cells);
    }

    fn refresh_active_count(&mut self) {
        self.active_count = self.streams.iter().filter(|stream| stream.active).count();
    }

    fn find_inactive_lane(
        &mut self,
        full_width: bool,
        mouse_enabled: bool,
        mouse_col: u16,
        rand_col: &Uniform<u16>,
        rng: &mut StdRng,
    ) -> Option<usize> {
        let len = self.streams.len();
        for _ in 0..len.min(16) {
            let lane = (rand_col.sample(rng) as usize) % len;
            if self.lane_is_available(lane, full_width, mouse_enabled, mouse_col) {
                return Some(lane);
            }
        }

        let start = self.spawn_scan_idx.min(len.saturating_sub(1));
        for offset in 0..len {
            let lane = (start + offset) % len;
            if self.lane_is_available(lane, full_width, mouse_enabled, mouse_col) {
                return Some(lane);
            }
        }
        None
    }

    fn lane_is_available(
        &self,
        lane: usize,
        full_width: bool,
        mouse_enabled: bool,
        mouse_col: u16,
    ) -> bool {
        if self.streams[lane].active {
            return false;
        }
        if !mouse_enabled || mouse_col == u16::MAX {
            return true;
        }
        let col = lane_col(lane, full_width);
        col.abs_diff(mouse_col) > crate::constants::MOUSE_AVOID_RADIUS_COLS
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
            if ctx.full_width && stream.col + 1 < frame.width {
                frame.set(stream.col + 1, line, blank_cell(ctx.bg));
            }
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
    let mut colors = ctx
        .palette_slices
        .get(effective_slot as usize)
        .copied()
        .unwrap_or(&[]);
    if colors.is_empty() {
        colors = ctx
            .palette_slices
            .get(ctx.active_palette_slot as usize)
            .copied()
            .unwrap_or(&[]);
    }
    if colors.is_empty() {
        return None;
    }

    let last = colors.len().saturating_sub(1);
    let first_visible = usize::from(last > 0);
    let idx = match level {
        // Ghost: use first visible for clean zero-line distinction
        // This ensures ghost spine cells are the faintest possible
        // non-invisible color, creating clear visual separation
        // between "empty space" and "spine trace."
        BrightnessLevel::Ghost => first_visible,
        // Dim: lowered from last/4 to first_visible for deeper separation
        // When bg is None (transparent), this keeps Dim cells barely visible
        // rather than muddy mid-range values.
        BrightnessLevel::Dim => first_visible,
        // Mid: slightly raised from last/2 for clearer body readability
        // The body segment is the most common visual element and
        // benefits from slightly higher contrast.
        BrightnessLevel::Mid => (last * 2) / 5,
        // Hot: raised from last*3/4 for sharper afterglow contrast
        // The hot zone marks the bottom of hero segments and
        // benefits from stronger contrast to separate from body.
        BrightnessLevel::Hot => (last * 4) / 5,
        // Core: unchanged — always the brightest
        BrightnessLevel::Core => last,
    };
    let base_color = colors[idx];
    let factor = factor.max(0.0);

    // Optimized hot path: decode color to RGB once, then chain all
    // blend operations on the raw (r, g, b) tuple without re-decoding.
    // This eliminates 2-4 color_to_rgb() calls per cell per frame.
    let (mut r, mut g, mut b) = palette::decode_color(base_color)?;

    if factor < 1.0 {
        let fi = (factor * 256.0) as i32;
        r = ((r as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
        g = ((g as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
        b = ((b as i32 * fi + 128) >> 8).clamp(0, 255) as u8;
    }
    if factor > 1.0 {
        let white_factor = (factor - 1.0).min(0.12);
        let wf = (white_factor * 256.0) as i32;
        r = (r as i32 + ((255 - r as i32) * wf + 128) / 256).clamp(0, 255) as u8;
        g = (g as i32 + ((255 - g as i32) * wf + 128) / 256).clamp(0, 255) as u8;
        b = (b as i32 + ((255 - b as i32) * wf + 128) / 256).clamp(0, 255) as u8;
    }
    if matches!(level, BrightnessLevel::Core) {
        // Core gets an additional 10% white blend
        const CORE_WF: i32 = 26; // 0.10 * 256 ≈ 26
        r = (r as i32 + ((255 - r as i32) * CORE_WF + 128) / 256).clamp(0, 255) as u8;
        g = (g as i32 + ((255 - g as i32) * CORE_WF + 128) / 256).clamp(0, 255) as u8;
        b = (b as i32 + ((255 - b as i32) * CORE_WF + 128) / 256).clamp(0, 255) as u8;
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

fn layer_brightness(layer: u8) -> f32 {
    match layer {
        0 => 0.62,
        1 => 0.84,
        _ => 1.0,
    }
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

fn target_active_count(lanes: usize, density: f32) -> usize {
    if lanes == 0 {
        return 0;
    }
    let ratio =
        (ACTIVE_BASE + density.clamp(0.01, 5.0) * ACTIVE_DENSITY_MULT).clamp(0.02, ACTIVE_MAX);
    ((lanes as f32 * ratio).round() as usize).clamp(1, lanes)
}

fn lane_count(cols: u16, full_width: bool) -> usize {
    if full_width {
        (cols / 2).max(1) as usize
    } else {
        cols.max(1) as usize
    }
}

fn lane_col(lane: usize, full_width: bool) -> u16 {
    if full_width {
        (lane as u16).saturating_mul(2)
    } else {
        lane as u16
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_density_targets_sparse_lane_count() {
        let target = target_active_count(100, 0.75);
        assert!((20..=35).contains(&target));
    }
}
