// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Charset-aware glyph mapping for Monolith Rain.

use super::monolith::SegmentKind;
use super::render::DrawCtx;

const SPINE_PERIOD: u16 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MonolithGlyphSet {
    Binary,
    Minimal,
    Code,
    Blocks,
    Tech,
    Dense,
}

pub(super) fn segment_char(
    ctx: &DrawCtx<'_>,
    line: u16,
    col: u16,
    kind: SegmentKind,
    pos_from_bottom: u8,
) -> char {
    match glyph_set_for_cell(ctx, line, col) {
        MonolithGlyphSet::Binary => binary_segment_char(kind, pos_from_bottom),
        MonolithGlyphSet::Minimal => minimal_segment_char(kind, pos_from_bottom),
        MonolithGlyphSet::Code => code_segment_char(kind, pos_from_bottom),
        MonolithGlyphSet::Blocks => blocks_segment_char(kind, pos_from_bottom),
        MonolithGlyphSet::Tech => tech_segment_char(kind, pos_from_bottom),
        MonolithGlyphSet::Dense => dense_segment_char(kind, pos_from_bottom),
    }
}

pub(super) fn spine_char(ctx: &DrawCtx<'_>, line: u16, col: u16) -> char {
    match glyph_set_for_cell(ctx, line, col) {
        MonolithGlyphSet::Binary => {
            if ((line / SPINE_PERIOD) + col) % 2 == 0 {
                '.'
            } else {
                '-'
            }
        }
        MonolithGlyphSet::Code | MonolithGlyphSet::Tech => '-',
        MonolithGlyphSet::Blocks => '.',
        MonolithGlyphSet::Minimal | MonolithGlyphSet::Dense => {
            if ((line / SPINE_PERIOD) + col) % 2 == 0 {
                ':'
            } else {
                '.'
            }
        }
    }
}

fn binary_segment_char(kind: SegmentKind, pos_from_bottom: u8) -> char {
    match kind {
        SegmentKind::Micro => '.',
        SegmentKind::Short => {
            if pos_from_bottom == 0 {
                '1'
            } else {
                '.'
            }
        }
        SegmentKind::Medium => {
            if pos_from_bottom == 0 {
                '='
            } else {
                '0'
            }
        }
        SegmentKind::Hero => match pos_from_bottom {
            0 => '+',
            1 | 2 => '=',
            _ => '-',
        },
    }
}

fn minimal_segment_char(kind: SegmentKind, pos_from_bottom: u8) -> char {
    match kind {
        SegmentKind::Micro => '.',
        SegmentKind::Short => {
            if pos_from_bottom == 0 {
                '-'
            } else {
                '.'
            }
        }
        SegmentKind::Medium => {
            if pos_from_bottom == 0 {
                '='
            } else {
                '-'
            }
        }
        SegmentKind::Hero => match pos_from_bottom {
            0 => '+',
            1 | 2 => '=',
            _ => '-',
        },
    }
}

fn code_segment_char(kind: SegmentKind, pos_from_bottom: u8) -> char {
    match kind {
        SegmentKind::Micro => ':',
        SegmentKind::Short => {
            if pos_from_bottom == 0 {
                '+'
            } else {
                '.'
            }
        }
        SegmentKind::Medium => {
            if pos_from_bottom == 0 {
                '='
            } else {
                '-'
            }
        }
        SegmentKind::Hero => match pos_from_bottom {
            0 => '+',
            1 | 2 => '=',
            _ => '-',
        },
    }
}

fn blocks_segment_char(kind: SegmentKind, pos_from_bottom: u8) -> char {
    match kind {
        SegmentKind::Micro => '.',
        SegmentKind::Short => {
            if pos_from_bottom == 0 {
                '+'
            } else {
                '-'
            }
        }
        SegmentKind::Medium => {
            if pos_from_bottom == 0 {
                '='
            } else {
                '+'
            }
        }
        SegmentKind::Hero => match pos_from_bottom {
            0 => '+',
            1 | 2 => '=',
            _ => '+',
        },
    }
}

fn tech_segment_char(kind: SegmentKind, pos_from_bottom: u8) -> char {
    match kind {
        SegmentKind::Micro => '.',
        SegmentKind::Short => {
            if pos_from_bottom == 0 {
                '*'
            } else {
                '-'
            }
        }
        SegmentKind::Medium => {
            if pos_from_bottom == 0 {
                '+'
            } else {
                '='
            }
        }
        SegmentKind::Hero => match pos_from_bottom {
            0 => '+',
            1 | 2 => '*',
            _ => '=',
        },
    }
}

fn dense_segment_char(kind: SegmentKind, pos_from_bottom: u8) -> char {
    match kind {
        SegmentKind::Micro => '.',
        SegmentKind::Short => {
            if pos_from_bottom == 0 {
                '+'
            } else {
                ':'
            }
        }
        SegmentKind::Medium => {
            if pos_from_bottom == 0 {
                '='
            } else {
                '+'
            }
        }
        SegmentKind::Hero => match pos_from_bottom {
            0 => '+',
            1 | 2 => '=',
            _ => '+',
        },
    }
}

fn glyph_set_for_cell(ctx: &DrawCtx<'_>, line: u16, col: u16) -> MonolithGlyphSet {
    let sample = [
        ctx.get_char(line, col, 0),
        ctx.get_char(line, col, 1),
        ctx.get_char(line, col, 5),
    ];
    if sample.iter().all(|ch| matches!(ch, '0' | '1')) {
        return MonolithGlyphSet::Binary;
    }
    if sample.iter().any(|ch| (*ch).is_ascii_hexdigit()) {
        return MonolithGlyphSet::Tech;
    }
    if sample.iter().any(|ch| {
        matches!(
            ch,
            '.' | ':' | '-' | '=' | '+' | '*' | '·' | '•' | '○' | '●'
        )
    }) {
        return MonolithGlyphSet::Minimal;
    }
    if sample.iter().any(|ch| matches!(ch, '▀'..='▟' | '─'..='╿')) {
        return MonolithGlyphSet::Blocks;
    }
    if sample
        .iter()
        .any(|ch| ch.is_ascii_alphanumeric() || ch.is_ascii_punctuation())
    {
        return MonolithGlyphSet::Code;
    }
    MonolithGlyphSet::Dense
}
