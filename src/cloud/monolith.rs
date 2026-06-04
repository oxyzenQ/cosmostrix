// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

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
use crate::droplet::viewport_edge_fade;
use crate::frame::Frame;
use crate::palette;
use crate::runtime::{BoldMode, ColorMode};
use crate::terminal::blank_cell;

use super::render::DrawCtx;

const MAX_SEGMENTS: usize = 9;
const MIN_STREAM_SPAN: u16 = 14;
const MAX_STREAM_SPAN: u16 = 30;
const ACTIVE_BASE: f32 = 0.06;
const ACTIVE_DENSITY_MULT: f32 = 0.28;
const ACTIVE_MAX: f32 = 0.35;
const SPAWN_RATE_MULT: f32 = 1.4;
const SPAWN_RATE_FLOOR: f32 = 2.0;
const SPINE_PERIOD: u16 = 4;
const SPINE_BRIGHTNESS: f32 = 0.22;

#[derive(Clone, Copy, Debug)]
enum SegmentKind {
    Micro,
    Short,
    Medium,
    Hero,
}

#[derive(Clone, Copy, Debug)]
enum BrightnessLevel {
    Ghost,
    Dim,
    Mid,
    Hot,
    Core,
}

#[derive(Clone, Copy, Debug)]
struct Segment {
    offset: u16,
    len: u8,
    kind: SegmentKind,
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
    speed: f32,
    span: u16,
    char_seed: u16,
    palette_slot: u8,
    layer: u8,
    segments: [Segment; MAX_SEGMENTS],
    segment_count: u8,
    last_time: Option<Instant>,
    last_draw_min: u16,
    last_draw_max: u16,
    had_last_draw: bool,
}

impl MonolithStream {
    fn new(col: u16) -> Self {
        Self {
            active: false,
            col,
            head: 0.0,
            speed: 0.0,
            span: MIN_STREAM_SPAN,
            char_seed: 0,
            palette_slot: 0,
            layer: 0,
            segments: [Segment::empty(); MAX_SEGMENTS],
            segment_count: 0,
            last_time: None,
            last_draw_min: 0,
            last_draw_max: 0,
            had_last_draw: false,
        }
    }

    fn reset_for_lane(&mut self, col: u16) {
        self.active = false;
        self.col = col;
        self.head = 0.0;
        self.speed = 0.0;
        self.span = MIN_STREAM_SPAN;
        self.char_seed = 0;
        self.palette_slot = 0;
        self.layer = 0;
        self.segment_count = 0;
        self.last_time = None;
        self.had_last_draw = false;
    }
}

pub(super) struct MonolithRain {
    streams: Vec<MonolithStream>,
    spawn_scan_idx: usize,
    active_count: usize,
}

pub(super) struct MonolithSpawnParams {
    pub(super) cols: u16,
    pub(super) lines: u16,
    pub(super) full_width: bool,
    pub(super) density: f32,
    pub(super) chars_per_sec: f32,
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

impl MonolithRain {
    pub(super) fn new() -> Self {
        Self {
            streams: Vec::new(),
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
        } else {
            for (lane, stream) in self.streams.iter_mut().enumerate() {
                stream.reset_for_lane(lane_col(lane, full_width));
            }
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
                now,
                params.lines,
                params.chars_per_sec,
                params.active_palette_slot,
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
        max_sim_delta: Duration,
        resume_blend: f32,
    ) {
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
            let delta = elapsed.as_secs_f32() * stream.speed * resume_blend;
            stream.head += delta.max(0.0);
            stream.last_time = Some(now);

            if stream.head - stream.span as f32 > lines as f32 + 1.0 {
                stream.active = false;
                self.active_count = self.active_count.saturating_sub(1);
            }
        }
    }

    pub(super) fn draw(&mut self, ctx: &DrawCtx<'_>, frame: &mut Frame) {
        for stream in &mut self.streams {
            if stream.had_last_draw {
                for line in stream.last_draw_min..=stream.last_draw_max {
                    frame.set(stream.col, line, blank_cell(ctx.bg));
                    if ctx.full_width && stream.col + 1 < frame.width {
                        frame.set(stream.col + 1, line, blank_cell(ctx.bg));
                    }
                }
                stream.had_last_draw = false;
            }

            if !stream.active {
                continue;
            }

            let Some((min_line, max_line)) = visible_range(stream, ctx.lines) else {
                continue;
            };

            draw_spine(stream, ctx, frame, min_line, max_line);
            draw_segments(stream, ctx, frame);

            stream.last_draw_min = min_line;
            stream.last_draw_max = max_line;
            stream.had_last_draw = true;
        }
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
    now: Instant,
    lines: u16,
    chars_per_sec: f32,
    palette_slot: u8,
    rand_chance: &Uniform<f32>,
    rng: &mut StdRng,
) {
    stream.active = true;
    stream.head = 0.0;
    stream.speed = varied_speed(chars_per_sec, rand_chance.sample(rng));
    stream.span = varied_span(lines, rand_chance.sample(rng));
    stream.char_seed =
        (rand_chance.sample(rng) * crate::constants::MAX_CHAR_POOL_IDX as f32) as u16;
    stream.palette_slot = palette_slot;
    stream.layer = layer_from_roll(rand_chance.sample(rng));
    stream.last_time = Some(now);
    stream.had_last_draw = false;
    build_segments(stream, rand_chance, rng);
}

fn build_segments(stream: &mut MonolithStream, rand_chance: &Uniform<f32>, rng: &mut StdRng) {
    let mut cursor = 0u16;
    let mut count = 0usize;
    while cursor < stream.span && count < MAX_SEGMENTS {
        let roll = rand_chance.sample(rng);
        let (kind, len) = if roll < 0.36 {
            (SegmentKind::Micro, 1)
        } else if roll < 0.70 {
            (SegmentKind::Short, 2)
        } else if roll < 0.93 {
            (
                SegmentKind::Medium,
                3 + (rand_chance.sample(rng) * 2.0) as u8,
            )
        } else {
            (SegmentKind::Hero, 5 + (rand_chance.sample(rng) * 3.0) as u8)
        };

        stream.segments[count] = Segment {
            offset: cursor,
            len,
            kind,
        };
        count += 1;

        let gap = 2 + (rand_chance.sample(rng) * 6.0) as u16;
        cursor = cursor.saturating_add(len as u16).saturating_add(gap);
    }
    stream.segment_count = count as u8;
}

fn draw_spine(
    stream: &MonolithStream,
    ctx: &DrawCtx<'_>,
    frame: &mut Frame,
    min_line: u16,
    max_line: u16,
) {
    for line in min_line..=max_line {
        if (line + stream.col) % SPINE_PERIOD != 0 {
            continue;
        }
        let edge_fade = viewport_edge_fade(line, ctx.lines);
        let fg = color_for_level(
            ctx,
            stream.palette_slot,
            line,
            stream.col,
            BrightnessLevel::Ghost,
            edge_fade * SPINE_BRIGHTNESS * layer_brightness(stream.layer),
        );
        frame.set(
            stream.col,
            line,
            Cell {
                ch: '.',
                fg,
                bg: ctx.bg,
                bold: false,
            },
        );
    }
}

fn draw_segments(stream: &MonolithStream, ctx: &DrawCtx<'_>, frame: &mut Frame) {
    let head_line = stream.head.floor() as i32;
    let frac = stream.head.fract().clamp(0.0, 1.0);
    for idx in 0..stream.segment_count as usize {
        let segment = stream.segments[idx];
        let bottom = head_line - segment.offset as i32;
        let top = bottom - segment.len as i32 + 1;

        for line_i in top..=bottom {
            if line_i < 0 || line_i >= ctx.lines as i32 {
                continue;
            }
            let line = line_i as u16;
            let pos_from_bottom = (bottom - line_i) as u8;
            let level = segment_level(segment.kind, pos_from_bottom);
            let edge_fade = viewport_edge_fade(line, ctx.lines);
            let pulse = if matches!(level, BrightnessLevel::Hot | BrightnessLevel::Core) {
                1.0 + frac * 0.08
            } else {
                1.0
            };
            let fg = color_for_level(
                ctx,
                stream.palette_slot,
                line,
                stream.col,
                level,
                edge_fade * layer_brightness(stream.layer) * pulse,
            );
            let bold = bold_for_level(ctx.bold_mode, level, line, stream.col)
                && edge_fade >= EDGE_FADE_BOLD_THRESHOLD;
            let ch = segment_char(ctx, stream, segment, line, pos_from_bottom);

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
        }
    }
}

fn segment_char(
    ctx: &DrawCtx<'_>,
    stream: &MonolithStream,
    segment: Segment,
    line: u16,
    pos_from_bottom: u8,
) -> char {
    let base = stream
        .char_seed
        .wrapping_add(segment.offset)
        .wrapping_add(pos_from_bottom as u16);
    if matches!(segment.kind, SegmentKind::Hero) && pos_from_bottom == 0 {
        let slow_tick = (stream.head as u16) / 4;
        ctx.get_char(line, stream.col, base.wrapping_add(slow_tick))
    } else {
        ctx.get_char(line, stream.col, base)
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

fn color_for_level(
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
        BrightnessLevel::Ghost => first_visible,
        BrightnessLevel::Dim => (last / 4).max(first_visible),
        BrightnessLevel::Mid => last / 2,
        BrightnessLevel::Hot => (last * 3) / 4,
        BrightnessLevel::Core => last,
    };
    let mut color = colors[idx];
    let factor = factor.max(0.0);
    color = palette::apply_brightness(color, factor.min(1.0));
    if factor > 1.0 {
        color = palette::blend_toward_white(color, (factor - 1.0).min(0.12));
    }
    if matches!(level, BrightnessLevel::Core) {
        color = palette::blend_toward_white(color, 0.10);
    }
    Some(color)
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

fn varied_speed(chars_per_sec: f32, roll: f32) -> f32 {
    let lane_variation = 0.78 + roll.clamp(0.0, 1.0) * 0.58;
    (chars_per_sec * lane_variation).max(0.001)
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
