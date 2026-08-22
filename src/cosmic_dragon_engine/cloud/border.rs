// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Border-cell utilities for the message overlay.
//!
//! Extracted from `cloud/mod.rs` to keep `mod.rs` under the
//! 1500-LOC cap after `cargo fmt` expanded `is_border_char`'s `matches!`
//! call across multiple lines. The logic is unchanged — only the
//! location has moved.

use super::state::MsgChr;

/// Check if a character is a border character (not content).
/// v25: includes rounded box-drawing chars.
#[inline]
pub(crate) fn is_border_char(ch: char) -> bool {
    matches!(
        ch,
        ' ' | '+' | '-' | '|' | '╭' | '╮' | '╰' | '╯' | '─' | '│'
    )
}

/// Build clockwise-ordered list of border cell indices: top-left → top →
/// top-right → right → bottom-right → bottom → bottom-left → left.
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
        if is_border_char(mc.val) && mc.val != ' ' {
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
            if mc.line == min_line && mc.col == col && is_border_char(mc.val) && mc.val != ' ' {
                order.push(idx);
            }
        }
    }
    // 2. Right edge: top+1 to bottom-1 (corners already added)
    for line in (min_line + 1)..max_line {
        for (idx, mc) in message.iter().enumerate() {
            if mc.line == line && mc.col == max_col && is_border_char(mc.val) && mc.val != ' ' {
                order.push(idx);
            }
        }
    }
    // 3. Bottom edge: left→right (includes corners)
    for col in min_col..=max_col {
        for (idx, mc) in message.iter().enumerate() {
            if mc.line == max_line && mc.col == col && is_border_char(mc.val) && mc.val != ' ' {
                order.push(idx);
            }
        }
    }
    // 4. Left edge: bottom-1 to top+1 (corners already added)
    for line in ((min_line + 1)..max_line).rev() {
        for (idx, mc) in message.iter().enumerate() {
            if mc.line == line && mc.col == min_col && is_border_char(mc.val) && mc.val != ' ' {
                order.push(idx);
            }
        }
    }
    order
}
