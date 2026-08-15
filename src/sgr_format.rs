// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! SGR (Select Graphic Rendition) byte formatting helpers.
//!
//! Extracted from `terminal.rs` to keep that file under its 1500-LOC guard.
//! These are pure functions that format ANSI escape sequences directly into
//! a byte buffer, bypassing crossterm's trait dispatch + fmt machinery +
//! heap String allocation. Used by the hot render path in `terminal.rs`
//! when the `ColorCache` misses (non-palette color or non-palette bg).

use crossterm::style::Color;

/// Push a u8 as ASCII decimal digits into buf (no heap alloc, no format!).
///
/// BOLT: delegates to `bolt::push_u8` (branchless table lookup via
/// `U8_PADDED` + `U8_LEN`). The original branchy cascade (n<10, n<100,
/// else) is gone — see `src/bolt.rs` for the table layout and the
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

/// Write combined fg+bg SGR escape sequence directly into buf.
/// Produces `\x1b[38;2;r;g;b;48;2;r;g;bm` (or subset for Reset/None).
/// Bypasses crossterm trait dispatch + fmt machinery + heap String alloc.
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
        _ => {} // named colors: skip (rare in production TrueColor mode)
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
        _ => {} // named colors: skip
    }
    buf.extend_from_slice(b"m");
}
