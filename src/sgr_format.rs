// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! SGR (Select Graphic Rendition) byte formatting helpers.
//!
//! Extracted from `terminal.rs` to keep that file under its 1000-LOC guard.
//! These are pure functions that format ANSI escape sequences directly into
//! a byte buffer, bypassing crossterm's trait dispatch + fmt machinery +
//! heap String allocation. Used by the hot render path in `terminal.rs`
//! when the `ColorCache` misses (non-palette color or non-palette bg).
//!
//! ## BOLT integration (v30 cosmic-dragon perf audit)
//!
//! `push_u8` and `push_u16` are now thin wrappers around the
//! [`bolt`](crate::bolt) module's branchless lookup tables. The legacy
//! 3-branch cascade (`n<10`, `n<100`, `else`) is replaced by 2 L1-cached
//! table lookups + 1 memcpy. See `src/bolt.rs` for the full design and
//! `docs/BOLT.md` for the calibration history.
//!
//! The public API is unchanged — `push_u8(buf, n)` and `push_u16(buf, n)`
//! still take a `&mut Vec<u8>` and produce the same bytes as before. Only
//! the implementation changed (branchy → branchless). Existing callers
//! (`terminal.rs`, `bench_io.rs` fallback path) need no changes.

use crossterm::style::Color;

use crate::bolt;

/// Push a u8 as ASCII decimal digits into buf (no heap alloc, no format!).
///
/// BOLT-backed: delegates to `bolt::push_u8` which uses the `U8_PADDED` +
/// `U8_LEN` lookup tables (branchless). See [`bolt`] for details.
#[inline]
pub(crate) fn push_u8(buf: &mut Vec<u8>, n: u8) {
    bolt::push_u8(buf, n);
}

/// Push a u16 as ASCII decimal digits into buf (no heap alloc, no format!).
///
/// BOLT-backed: delegates to `bolt::push_u16` which uses the `U8_PADDED` +
/// `U8_LEN` lookup tables for the common `n < 256` case (branchless), with
/// an unrolled divmod fallback for the rare 4-5 digit case. See [`bolt`].
#[inline]
pub(crate) fn push_u16(buf: &mut Vec<u8>, n: u16) {
    bolt::push_u16(buf, n);
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
