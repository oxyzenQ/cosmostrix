// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! SGR (Select Graphic Rendition) byte formatting helpers.
//!
//! Extracted from `terminal.rs` to keep that file under its 1000-LOC guard.
//! These are pure functions that format ANSI escape sequences directly into
//! a byte buffer, bypassing crossterm's trait dispatch + fmt machinery +
//! heap String allocation. Used by the hot render path in `terminal.rs`
//! when the `ColorCache` misses (non-palette color or non-palette bg).

use crossterm::style::Color;

/// Push a u8 as ASCII decimal digits into buf (no heap alloc, no format!).
#[inline]
pub(crate) fn push_u8(buf: &mut Vec<u8>, n: u8) {
    if n < 10 {
        buf.push(b'0' + n);
    } else if n < 100 {
        buf.push(b'0' + n / 10);
        buf.push(b'0' + n % 10);
    } else {
        buf.push(b'0' + n / 100);
        buf.push(b'0' + (n / 10) % 10);
        buf.push(b'0' + n % 10);
    }
}

/// Push a u16 as ASCII decimal digits into buf (no heap alloc, no format!).
#[inline]
pub(crate) fn push_u16(buf: &mut Vec<u8>, n: u16) {
    if n < 256 {
        push_u8(buf, n as u8);
    } else {
        // 256..=65535: up to 5 digits
        let mut tmp = [0u8; 5];
        let mut val = n;
        let mut len = 0;
        while val > 0 {
            tmp[len] = b'0' + (val % 10) as u8;
            val /= 10;
            len += 1;
        }
        for i in (0..len).rev() {
            buf.push(tmp[i]);
        }
    }
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
