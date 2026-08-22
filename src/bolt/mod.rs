// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! BOLT — Branchless Optimized Lookup Tables.
//!
//! Project-wide module hosting the table-driven u8/u16 → ASCII decimal
//! formatters and the branchless bold-escape selector. Extracted from
//! `bench_io.rs` (where the tables originally lived as inline consts
//! since commit `bd11095`) and `sgr_format.rs` (where the branchy
//! `push_u8` / `push_u16` helpers lived) so that every hot path —
//! `bench_io::emit_cell_lean`, `sgr_format::write_sgr_colors_buf`,
//! `Terminal::draw` — shares a single branchless implementation.
//!
//! ## Performance claim
//!
//! BOLT is a *projected production-path gain*; it is **not bench-measurable**
//! via `--bench-frames` because `bench_io::emit_cell_lean` was already
//! table-driven (inline `U8_PADDED` / `U8_LEN` / `BOLD_ESCAPES`) since
//! `bd11095`. Promoting the tables to a project-wide module and wiring them
//! into `sgr_format` + `terminal::draw` closes the gap so the production
//! render path now enjoys the same branchless formatting that the bench
//! I/O path already had. To measure the production path itself, use
//! `--bench-scene production-draw` (added alongside this module).
//!
//! ## Tables
//!
//! - `U8_PADDED` (256×3 bytes): ASCII digits left-aligned. For n=5,
//!   `[b'5', 0, 0]`; for n=42, `[b'4', b'2', 0]`; for n=255,
//!   `[b'2', b'5', b'5']`. The padding bytes are always overwritten by
//!   the caller's next sequential write — the SGR fast path builds
//!   `prefix → R → ';' → G → ';' → B → ';49m'` strictly left-to-right
//!   with no gaps, so a 3-byte memcpy is always safe regardless of the
//!   actual digit count.
//! - `U8_LEN` (256 bytes): digit count (1, 2, or 3) for each u8 value.
//! - `BOLD_ESCAPES` (`[&[u8]; 2]`): `BOLD_ESCAPES[0]` = bold OFF
//!   (`\x1b[22m`, 5 bytes); `BOLD_ESCAPES[1]` = bold ON (`\x1b[1m`,
//!   4 bytes). Selected via `cell.bold as usize` (branchless bool→int
//!   via `setne` on x86).
//! - `BOLD_ESCAPE_LENS` (`[usize; 2]`): byte lengths paired with
//!   `BOLD_ESCAPES` for the `copy_from_slice` call.
//!
//! Total table size: 1024 + 16 + 16 = 1056 bytes. Fits comfortably in
//! L1 cache (typically 32 KB). All lookups are `O(1)` with no branches.

/// Branchless u8 → ASCII decimal digits, left-aligned in a 3-byte slot.
///
/// See the module docs for the layout rationale. The padding bytes at
/// `buf[digits..3]` are always overwritten by the caller's next write.
pub(crate) const U8_PADDED: [[u8; 3]; 256] = {
    let mut table = [[0u8; 3]; 256];
    let mut i = 0u16;
    while i < 256 {
        let n = i as u8;
        let d_hundreds = b'0' + n / 100;
        let d_tens = b'0' + (n / 10) % 10;
        let d_ones = b'0' + n % 10;
        if n >= 100 {
            table[i as usize] = [d_hundreds, d_tens, d_ones];
        } else if n >= 10 {
            table[i as usize] = [d_tens, d_ones, 0];
        } else {
            table[i as usize] = [d_ones, 0, 0];
        }
        i += 1;
    }
    table
};

/// Digit count (1, 2, or 3) for each u8 value. Companion to `U8_PADDED`.
pub(crate) const U8_LEN: [u8; 256] = {
    let mut table = [0u8; 256];
    let mut i = 0u16;
    while i < 256 {
        let n = i as u8;
        table[i as usize] = 1 + (n >= 10) as u8 + (n >= 100) as u8;
        i += 1;
    }
    table
};

/// Precomputed bold escape sequences for branchless selection.
///
/// `BOLD_ESCAPES[0]` = bold OFF (`\x1b[22m`, 5 bytes).
/// `BOLD_ESCAPES[1]` = bold ON  (`\x1b[1m`,  4 bytes).
///
/// Index via `cell.bold as usize` (or any `bool as usize`) — compiles
/// to a `setne` on x86, no branch. Pair with [`BOLD_ESCAPE_LENS`] for
/// the `copy_from_slice` length.
pub(crate) const BOLD_ESCAPES: [&[u8]; 2] = [b"\x1b[22m", b"\x1b[1m"];

/// Byte lengths paired with [`BOLD_ESCAPES`] — avoids `slice.len()` per call.
pub(crate) const BOLD_ESCAPE_LENS: [usize; 2] = [5, 4];

/// Push a u8 as ASCII decimal digits into a fixed-size slice starting at
/// `buf[0]`. Returns the number of bytes written (1, 2, or 3).
///
/// Branchless: 2 L1-cached table lookups + 1 3-byte memcpy + return.
/// Eliminates the 3-branch cascade (n<10, n<100, else) of the original
/// `sgr_format::push_u8` implementation.
///
/// The 3-byte memcpy always writes `U8_PADDED[n]` to `buf[..3]`, including
/// padding bytes when the digit count is < 3. The padding is harmless —
/// the caller advances `pos` by the returned digit count, and the next
/// sequential write (a `;` separator or the next channel's digits)
/// overwrites the padding. This holds because the SGR fast path builds
/// the sequence strictly left-to-right with no gaps.
#[inline]
pub(crate) fn write_u8_to_slice(buf: &mut [u8], n: u8) -> usize {
    let digits = U8_LEN[n as usize] as usize;
    buf[..3].copy_from_slice(&U8_PADDED[n as usize]);
    digits
}

/// Push a u8 as ASCII decimal digits into a growable `Vec<u8>`.
///
/// Branchless counterpart to `sgr_format::push_u8` (which was branchy:
/// `if n < 10 { push 1 } else if n < 100 { push 2 } else { push 3 }`).
/// Uses the same `U8_PADDED` + `U8_LEN` tables as [`write_u8_to_slice`]
/// but writes only `digits` bytes (no padding) into the Vec, since Vec
/// growth is `push`-based and padding bytes would accumulate.
#[inline]
pub(crate) fn push_u8(buf: &mut Vec<u8>, n: u8) {
    let digits = U8_LEN[n as usize] as usize;
    buf.extend_from_slice(&U8_PADDED[n as usize][..digits]);
}

/// Push a u16 as ASCII decimal digits into a growable `Vec<u8>`.
///
/// Routes 0..=255 through [`push_u8`] (branchless table lookup). For
/// 256..=65535, falls back to a digit-extraction loop (rare path —
/// only triggered by cursor row/col values > 255, i.e. terminals
/// wider/taller than 255 cells).
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify U8_PADDED + U8_LEN produce identical output to the original
    /// branchy cascade for all 256 u8 values.
    #[test]
    fn u8_padded_matches_branchy_for_all_values() {
        for n in 0u8..=255 {
            let expected_len = 1 + (n >= 10) as usize + (n >= 100) as usize;
            assert_eq!(
                U8_LEN[n as usize] as usize, expected_len,
                "U8_LEN[{n}] mismatch"
            );

            let mut expected = [0u8; 3];
            if n >= 100 {
                expected[0] = b'0' + n / 100;
                expected[1] = b'0' + (n / 10) % 10;
                expected[2] = b'0' + n % 10;
            } else if n >= 10 {
                expected[0] = b'0' + n / 10;
                expected[1] = b'0' + n % 10;
            } else {
                expected[0] = b'0' + n;
            }
            assert_eq!(U8_PADDED[n as usize], expected, "U8_PADDED[{n}] mismatch");
        }
    }

    /// `write_u8_to_slice` returns the correct digit count and writes the
    /// correct left-aligned digits for all 256 u8 values.
    #[test]
    fn write_u8_to_slice_roundtrip() {
        for n in 0u8..=255 {
            let mut buf = [0u8; 3];
            let digits = write_u8_to_slice(&mut buf, n);
            assert_eq!(digits, U8_LEN[n as usize] as usize);
            let s = std::str::from_utf8(&buf[..digits]).unwrap();
            assert_eq!(s.parse::<u32>().unwrap(), n as u32);
        }
    }

    /// `push_u8` produces the same digits as `format!("{n}")`.
    #[test]
    fn push_u8_matches_format() {
        for n in 0u8..=255 {
            let mut buf = Vec::new();
            push_u8(&mut buf, n);
            assert_eq!(buf, format!("{n}").into_bytes());
        }
    }

    /// `push_u16` covers the full u16 range without divergence from `format!("{n}")`.
    #[test]
    fn push_u16_matches_format() {
        for n in [0u16, 1, 9, 10, 99, 100, 255, 256, 1000, 9999, 10000, 65535] {
            let mut buf = Vec::new();
            push_u16(&mut buf, n);
            assert_eq!(buf, format!("{n}").into_bytes(), "push_u16({n}) mismatch");
        }
    }

    /// `BOLD_ESCAPES` indices and lengths are consistent.
    #[test]
    fn bold_escapes_lengths_match() {
        for (idx, esc) in BOLD_ESCAPES.iter().enumerate() {
            assert_eq!(esc.len(), BOLD_ESCAPE_LENS[idx]);
        }
        assert_eq!(BOLD_ESCAPES[0], b"\x1b[22m");
        assert_eq!(BOLD_ESCAPES[1], b"\x1b[1m");
    }
}
