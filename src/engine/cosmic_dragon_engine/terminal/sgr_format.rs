// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! SGR (Select Graphic Rendition) byte formatting helpers.
//!
//! Extracted from `terminal.rs` to keep that file under its 800-LOC guard.
//! These are pure functions that format ANSI escape sequences directly into
//! a byte buffer, bypassing crossterm's trait dispatch + fmt machinery +
//! heap String allocation. Used by the hot render path in `terminal.rs`
//! when the `ColorCache` misses (non-palette color or non-palette bg).
//!
//! task-17: named base-16 colors now format as their classic SGR codes
//! (`30-37`/`90-97` fg, `40-47`/`100-107` bg) instead of being skipped.
//! This is the Color16 wire contract from the capability table in
//! `output/mod.rs` — before task-17 a cache-miss cell with a named fg
//! emitted a bg-only escape (no foreground at all), and the 16-color
//! rain palettes never reached the wire because `color_cache.rs`
//! decoded them back to `38;2` truecolor.

use crossterm::style::Color;

use crate::palette::quantize::named16_slot;

/// Push a u8 as ASCII decimal digits into buf (no heap alloc, no format!).
///
/// BOLT: delegates to `bolt::push_u8` (branchless table lookup via
/// `U8_PADDED` + `U8_LEN`). The original branchy cascade (n<10, n<100,
/// else) is gone — see `src/bolt/mod.rs` for the table layout and the
/// projected production-path gain rationale.
#[inline]
pub(crate) fn push_u8(buf: &mut Vec<u8>, n: u8) {
    crate::bolt::push_u8(buf, n);
}

/// Push a u16 as ASCII decimal digits into buf (no heap alloc, no format!).
///
/// BOLT: delegates to `bolt::push_u16` which routes 0..=255 through the
/// branchless `bolt::push_u8` and falls back to a digit-extraction loop
/// only for 256..=65535 (cursor row/col values > 255, rare).
#[inline]
pub(crate) fn push_u16(buf: &mut Vec<u8>, n: u16) {
    crate::bolt::push_u16(buf, n);
}

/// Push the classic SGR code for base-16 slot `slot` (0..=15) into buf.
///
/// fg: `30-37` (normal) / `90-97` (bright). bg: `40-47` / `100-107`.
/// These are the sequences every ANSI terminal honors — including
/// true 16-color environments like the linux console that drop
/// `38;5;N` and `38;2;R;G;B` entirely.
#[inline]
fn push_classic_code(buf: &mut Vec<u8>, slot: u8, bg: bool) {
    debug_assert!(slot < 16, "base-16 slot out of range: {slot}");
    if bg {
        if slot < 8 {
            buf.push(b'4');
            buf.push(b'0' + slot);
        } else {
            buf.push(b'1');
            buf.push(b'0');
            buf.push(b'0' + (slot - 8));
        }
    } else if slot < 8 {
        buf.push(b'3');
        buf.push(b'0' + slot);
    } else {
        buf.push(b'9');
        buf.push(b'0' + (slot - 8));
    }
}

/// Write combined fg+bg SGR escape sequence directly into buf.
/// Produces `\x1b[38;2;r;g;b;48;2;r;g;bm` (or subset for Reset/None).
/// Bypasses crossterm trait dispatch + fmt machinery + heap String alloc.
///
/// task-17: `Color::AnsiValue` emits `38;5;N`/`48;5;N` and named base-16
/// colors emit their classic `3x`/`9x` (fg) and `4x`/`10x` (bg) codes —
/// callers on non-truecolor sessions pass emission-boundary-quantized
/// colors (see `palette::quantize`), so the bytes leaving this function
/// always match the session's resolved color mode.
#[inline]
pub(crate) fn write_sgr_colors_buf(buf: &mut Vec<u8>, fg: Option<Color>, bg: Option<Color>) {
    buf.extend_from_slice(b"\x1b[");
    let mut first = true;
    match fg {
        Some(Color::Rgb { r, g, b }) => {
            buf.extend_from_slice(b"38;2;");
            push_u8(buf, r);
            buf.push(b';');
            push_u8(buf, g);
            buf.push(b';');
            push_u8(buf, b);
            first = false;
        }
        Some(Color::AnsiValue(v)) => {
            buf.extend_from_slice(b"38;5;");
            push_u8(buf, v);
            first = false;
        }
        Some(Color::Reset) | None => {
            buf.extend_from_slice(b"39");
            first = false;
        }
        Some(named) => {
            // task-17: classic base-16 codes for named colors (previously
            // skipped — a named fg emitted no foreground at all).
            // The binding pattern also catches any future crossterm
            // variant; named16_slot returns None there → skip, as before.
            if let Some(slot) = named16_slot(named) {
                push_classic_code(buf, slot, false);
                first = false;
            }
        }
    }
    match bg {
        Some(Color::Rgb { r, g, b }) => {
            if !first {
                buf.push(b';');
            }
            buf.extend_from_slice(b"48;2;");
            push_u8(buf, r);
            buf.push(b';');
            push_u8(buf, g);
            buf.push(b';');
            push_u8(buf, b);
        }
        Some(Color::AnsiValue(v)) => {
            if !first {
                buf.push(b';');
            }
            buf.extend_from_slice(b"48;5;");
            push_u8(buf, v);
        }
        Some(Color::Reset) | None => {
            if !first {
                buf.push(b';');
            }
            buf.extend_from_slice(b"49");
        }
        Some(named) => {
            // task-17: classic bg codes for named colors (previously
            // skipped — a named bg emitted no background at all).
            if let Some(slot) = named16_slot(named) {
                if !first {
                    buf.push(b';');
                }
                push_classic_code(buf, slot, true);
            }
        }
    }
    buf.extend_from_slice(b"m");
}
