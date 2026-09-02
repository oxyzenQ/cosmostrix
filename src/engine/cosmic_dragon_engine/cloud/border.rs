// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Border-cell utilities for the message overlay.
//!
//! Extracted from `cloud/mod.rs` to keep `mod.rs` under the
//! 800-LOC cap. The logic is unchanged — only the location
//! has moved.
//!
//! v80.0.0-alpha.1 (S-master-HUNT-3, owner bug: message dash swallowed): the old
//! glyph-based `is_border_char(ch)` helper (matching ' ', '+', '-',
//! '|', and the box-drawing set) was REMOVED. Border membership is
//! POSITIONAL now — `MsgChr.is_border`, stamped by `reset_message`
//! when the layout itself places a border glyph at a perimeter cell.
//! The glyph test could not tell a layout-placed border from user
//! text that happens to contain '-', '+', '|' or a box-drawing
//! glyph, so those content characters were classified as border
//! cells: never revealed as content (drawn blank) and, in the
//! `-m` no-border mode, able to fabricate a border order out of
//! user text. `MsgChr.is_border` is the single source of truth.

use super::state::MsgChr;

/// Build clockwise-ordered list of border cell indices: top-left → top →
/// top-right → right → bottom-right → bottom → bottom-left → left.
///
/// v80.0.0-alpha.1 (S-master-HUNT-3): membership is positional (`mc.is_border` — layout-
/// stamped), not glyph-based, so user text glyphs can never join the
/// border order (or distort its bounding box).
pub(crate) fn build_border_order(message: &[MsgChr]) -> Vec<usize> {
    if message.is_empty() {
        return Vec::new();
    }
    // Find bounding box of border cells.
    let mut min_line = u16::MAX;
    let mut max_line = 0u16;
    let mut min_col = u16::MAX;
    let mut max_col = 0u16;
    for mc in message {
        if mc.is_border {
            min_line = min_line.min(mc.line);
            max_line = max_line.max(mc.line);
            min_col = min_col.min(mc.col);
            max_col = max_col.max(mc.col);
        }
    }
    if min_line == u16::MAX {
        return Vec::new();
    }

    // Collect border cells in clockwise order.
    let mut order: Vec<usize> = Vec::new();
    // 1. Top edge: left→right (includes corners)
    for col in min_col..=max_col {
        for (idx, mc) in message.iter().enumerate() {
            if mc.line == min_line && mc.col == col && mc.is_border {
                order.push(idx);
            }
        }
    }
    // 2. Right edge: top+1 to bottom-1 (corners already added)
    for line in (min_line + 1)..max_line {
        for (idx, mc) in message.iter().enumerate() {
            if mc.line == line && mc.col == max_col && mc.is_border {
                order.push(idx);
            }
        }
    }
    // 3. Bottom edge: left→right (includes corners)
    for col in min_col..=max_col {
        for (idx, mc) in message.iter().enumerate() {
            if mc.line == max_line && mc.col == col && mc.is_border {
                order.push(idx);
            }
        }
    }
    // 4. Left edge: bottom-1 to top+1 (corners already added)
    for line in ((min_line + 1)..max_line).rev() {
        for (idx, mc) in message.iter().enumerate() {
            if mc.line == line && mc.col == min_col && mc.is_border {
                order.push(idx);
            }
        }
    }
    order
}
