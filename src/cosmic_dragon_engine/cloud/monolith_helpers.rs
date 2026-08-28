// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Monolith drawing helpers — extracted from `cloud/monolith.rs` to
//! keep that file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns 19 free functions used by MonolithRain's spawn/advance/draw
//! methods: activate_stream, build_segments, segment_len, segment_gap,
//! draw_spine, draw_spine_cell, draw_segments, spine_envelope,
//! segment_level, color_for_level, bold_for_level, clear_cell,
//! clear_phosphor_metadata, visible_range, target_active_count,
//! lane_count, varied_speed_mult, varied_span, layer_from_roll.

use crossterm::style::Color;
use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::cell::Cell;
use crate::cinematic::monolith_hero_pulse;
use crate::constants::*;
use crate::frame::Frame;
use crate::palette;
use crate::runtime::{BoldMode, ColorMode, MonolithSize};
use crate::terminal::blank_cell;

use super::monolith::{
    ActivationParams, BrightnessLevel, DrawnCell, DrawnCellKind, MonolithCleanup, MonolithStream,
    Segment, SegmentKind, SpineTone, MAX_SEGMENTS,
};
use super::monolith_glyphs::{segment_char, spine_char};
use super::render::DrawCtx;

pub(super) fn activate_stream(
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

pub(super) fn build_segments(
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
    debug_assert!(count <= u8::MAX as usize, "segment_count must fit u8");
    stream.segment_count = count as u8;
}

pub(super) fn segment_len(kind: SegmentKind, size: MonolithSize, roll: f32) -> u8 {
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

pub(super) fn segment_gap(size: MonolithSize, roll: f32) -> u16 {
    let roll = roll.clamp(0.0, 1.0);
    match size {
        MonolithSize::Small => 3 + (roll * 6.0) as u16,
        MonolithSize::Normal => 2 + (roll * 5.0) as u16,
        MonolithSize::Large => 2 + (roll * 4.0) as u16,
    }
}

pub(super) fn draw_spine(
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

pub(super) fn draw_spine_cell(
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
    let cadence = tone.cadence.max(MONOLITH_SPINE_PERIOD);
    if !(line + stream.col + segment_offset).is_multiple_of(cadence) {
        return;
    }

    let edge_fade = ctx.edge_fade(line);
    let fg = color_for_level(
        ctx,
        stream.palette_slot,
        line,
        stream.col,
        BrightnessLevel::Ghost,
        edge_fade
            * MONOLITH_SPINE_BRIGHTNESS
            * MONOLITH_LAYER_BRIGHTNESS[stream.layer as usize]
            * 0.72
            * tone.breath,
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

pub(super) fn draw_segments(
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
            let pos_from_bottom = {
                let v = bottom - line_i;
                debug_assert!(v <= 255, "pos_from_bottom must fit u8");
                v as u8
            };
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
                edge_fade * MONOLITH_LAYER_BRIGHTNESS[stream.layer as usize] * breath * pulse,
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

pub(super) fn spine_envelope(kind: SegmentKind) -> i32 {
    match kind {
        SegmentKind::Micro => 0,
        SegmentKind::Short | SegmentKind::Medium => 1,
        SegmentKind::Hero => 2,
    }
}

pub(super) fn segment_level(kind: SegmentKind, pos_from_bottom: u8) -> BrightnessLevel {
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

pub(crate) fn color_for_level(
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

    // (chroma audit, A10): the monolith color pipeline has three
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
            let scaled =
                crate::chroma_dragon_engine::palette::apply_brightness_rgb(r, g, b, factor);
            palette::decode_color(scaled).unwrap_or((r, g, b))
        } else {
            crate::chroma_dragon_engine::legacy::scale_rgb(r, g, b, factor)
        };
        r = nr;
        g = ng;
        b = nb;
    }
    if factor > 1.0 {
        // v17 mastery: raised white_factor cap from 0.12 to 0.20 for
        // stronger pulse/breath brightness boost on Core/Hot cells.
        let white_factor = (factor - 1.0).min(MONOLITH_WHITE_BOOST_CAP);
        let (nr, ng, nb) = if ctx.color_pipeline.is_chroma() {
            crate::chroma_dragon_engine::palette::blend_toward_white_rgb(r, g, b, white_factor)
        } else {
            crate::chroma_dragon_engine::legacy::blend_toward_white(r, g, b, white_factor)
        };
        r = nr;
        g = ng;
        b = nb;
    }
    if matches!(level, BrightnessLevel::Core) {
        // CORE_WF centralized as MONOLITH_CORE_WHITE_BLEND.
        let (nr, ng, nb) = if ctx.color_pipeline.is_chroma() {
            crate::chroma_dragon_engine::palette::blend_toward_white_rgb(
                r,
                g,
                b,
                MONOLITH_CORE_WHITE_BLEND,
            )
        } else {
            crate::chroma_dragon_engine::legacy::blend_toward_white(
                r,
                g,
                b,
                MONOLITH_CORE_WHITE_BLEND,
            )
        };
        r = nr;
        g = ng;
        b = nb;
    }

    Some(Color::Rgb { r, g, b })
}

pub(crate) fn bold_for_level(mode: BoldMode, level: BrightnessLevel, line: u16, col: u16) -> bool {
    match mode {
        BoldMode::Off => false,
        BoldMode::All => !matches!(level, BrightnessLevel::Ghost | BrightnessLevel::Dim),
        BoldMode::Random => {
            matches!(level, BrightnessLevel::Core)
                || (matches!(level, BrightnessLevel::Hot) && ((line ^ col) & 1) == 0)
        }
    }
}

pub(super) fn clear_cell(
    frame: &mut Frame,
    cleanup: &mut MonolithCleanup<'_>,
    col: u16,
    line: u16,
) {
    clear_phosphor_metadata(cleanup, col, line);
    // Use set_force: previous_cells are known-drawn from the last frame,
    // so the equality check in set() is almost always wasted work.
    frame.set_force(col, line, blank_cell(cleanup.bg));
}

pub(super) fn clear_phosphor_metadata(cleanup: &mut MonolithCleanup<'_>, col: u16, line: u16) {
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

pub(super) fn visible_range(stream: &MonolithStream, lines: u16) -> Option<(u16, u16)> {
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

pub(crate) fn target_active_count(lanes: usize, density: f32) -> usize {
    if lanes == 0 {
        return 0;
    }
    let ratio = (MONOLITH_ACTIVE_BASE + density.clamp(0.01, 5.0) * MONOLITH_ACTIVE_DENSITY_MULT)
        .clamp(0.02, MONOLITH_ACTIVE_MAX);
    ((lanes as f32 * ratio).round() as usize).clamp(1, lanes)
}

pub(super) fn lane_count(cols: u16) -> usize {
    cols.max(1) as usize
}

pub(super) fn varied_speed_mult(roll: f32) -> f32 {
    0.78 + roll.clamp(0.0, 1.0) * 0.58
}

pub(super) fn varied_span(lines: u16, roll: f32) -> u16 {
    let max = MONOLITH_MAX_STREAM_SPAN
        .min(lines.saturating_add(8))
        .max(MONOLITH_MIN_STREAM_SPAN);
    let span = MONOLITH_MIN_STREAM_SPAN as f32
        + roll.clamp(0.0, 1.0) * (max - MONOLITH_MIN_STREAM_SPAN) as f32;
    span.round() as u16
}

pub(super) fn layer_from_roll(roll: f32) -> u8 {
    if roll < 0.45 {
        0
    } else if roll < 0.85 {
        1
    } else {
        2
    }
}
