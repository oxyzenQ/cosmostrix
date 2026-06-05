// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

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
    if pool_is_binary(ctx) {
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

fn pool_is_binary(ctx: &DrawCtx<'_>) -> bool {
    let pool = if ctx.char_pool.is_empty() {
        ctx.previous_char_pool
    } else {
        ctx.char_pool
    };
    !pool.is_empty() && pool.iter().all(|ch| matches!(ch, '0' | '1'))
}
