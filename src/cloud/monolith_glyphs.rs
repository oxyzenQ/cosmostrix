// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Charset-aware glyph mapping for Monolith Rain.

use super::monolith::SegmentKind;
use super::render::DrawCtx;

const SPINE_PERIOD: u16 = 3;

pub(super) fn segment_char(
    ctx: &DrawCtx<'_>,
    line: u16,
    col: u16,
    kind: SegmentKind,
    pos_from_bottom: u8,
) -> char {
    let salt = segment_salt(kind, pos_from_bottom);
    safe_pool_char(ctx, line, col, salt)
}

pub(super) fn spine_char(ctx: &DrawCtx<'_>, line: u16, col: u16) -> char {
    let phase = ((line / SPINE_PERIOD) + col) % 3;
    // Use cached pool_is_binary from DrawCtx instead of iterating the
    // entire char pool on every call. This was the #2 per-cell bottleneck
    // in the monolith render path (after color_to_rgb).
    if ctx.pool_is_binary {
        return if phase == 0 { '0' } else { '1' };
    }

    match phase {
        0 => safe_pool_char(ctx, line, col, 1),
        1 => '.',
        _ => '-',
    }
}

fn segment_salt(kind: SegmentKind, pos_from_bottom: u8) -> u16 {
    let base = match kind {
        SegmentKind::Micro => 1,
        SegmentKind::Short => 7,
        SegmentKind::Medium => 19,
        SegmentKind::Hero => 37,
    };
    base + pos_from_bottom as u16 * 11
}

fn safe_pool_char(ctx: &DrawCtx<'_>, line: u16, col: u16, salt: u16) -> char {
    let ch = ctx.get_char(line, col, salt);
    match ch {
        '#' => '+',
        ch if ch.is_control() => '.',
        ch => ch,
    }
}
