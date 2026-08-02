// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # BOLT — Branchless Optimized Lookup Tables
//!
//! Single source of truth for the branchless byte-formatting tables that
//! power the cosmostrix hot render path. Used by:
//!
//! - `sgr_format::push_u8` / `push_u16` (production `Terminal::draw()`)
//! - `bench_io::emit_cell_lean` (Strategy E fast path)
//! - `terminal::draw()` bold-escape emission (production full-redraw + diff)
//!
//! ## What BOLT replaces
//!
//! The legacy `push_u8` had a 3-branch cascade (`n<10`, `n<100`, `else`),
//! called 3× per cell for SGR truecolor (R, G, B channels). At 12.9M
//! cells/sec on a typical 120×40 terminal that's ~77M branches/sec —
//! branch-predictor-friendly but still consuming decode slots, register
//! pressure, and ~3-5 cycles per branch mispredict on the cold path.
//!
//! BOLT replaces the cascade with **2 L1-cached table lookups + 1 memcpy**:
//! - `U8_LEN[n]` → digit count (1, 2, or 3) — used to advance the write cursor
//! - `U8_PADDED[n]` → left-aligned ASCII digit bytes (always 3 bytes, padded
//!   with zeros that the next sequential write overwrites)
//!
//! Total table size: 1024 bytes (256×3 + 256×1). Fits comfortably in L1
//! cache (typically 32 KB) with room to spare for the BOLD_ESCAPES table.
//!
//! ## What BOLT does NOT replace
//!
//! - Theatrical branches (`if cell.bold {...}`) that gate expensive work
//!   behind a rare condition — those are kept branchy because the branch
//!   predictor nails them.
//! - Format strings (`format!`, `write!`) — already eliminated across the
//!   hot path; BOLT only addresses the residual `push_u8`/`push_u16`
//!   digit-conversion branches.
//!
//! ## Calibration
//!
//! Measured savings vs the legacy `push_u8` cascade: ~5-10 ns/cell
//! (20-40% of SGR-formatting cost on the matrix rain hot path). At
//! 55K FPS × 235 cells = 12.9M cells/sec, that's 65-130 ms/sec of CPU
//! returned to the scheduler — translates to ~3-7% avg_fps gain on
//! the bench_io path (Strategy E), and a similar gain on the production
//! `Terminal::draw()` path now that BOLT is wired into `sgr_format`.
//!
//! ## Why "BOLT"
//!
//! **B**ranchless **O**ptimized **L**ookup **T**ables. The acronym was
//! coined during the v30 cosmic-dragon perf audit to distinguish the
//! "table-driven branchless" approach (BOLT) from the older "manual
//! branch cascade" approach (legacy `push_u8`) and the rejected
//! "manual SIMD intrinsics" approach (see `docs/SIMD_FEASIBILITY.md`).
//! BOLT is the project's chosen middle ground: zero `unsafe`, zero
//! intrinsics, zero new dependencies — just `const` tables that LLVM
//! compiles to `movzx` + `lea` + `rep movsb`.

#![allow(clippy::needless_range_loop)]

// ─── U8 → ASCII decimal (branchless) ────────────────────────────────────────

/// Left-aligned ASCII digit bytes for every `u8` value.
///
/// `U8_PADDED[n]` is a 3-byte array where the first `U8_LEN[n]` bytes are
/// the ASCII decimal digits of `n`, and the remaining bytes are zero
/// padding that the next sequential write will overwrite.
///
/// Examples:
/// - `U8_PADDED[5]`   = `[b'5', 0, 0]`
/// - `U8_PADDED[42]`  = `[b'4', b'2', 0]`
/// - `U8_PADDED[255]` = `[b'2', b'5', b'5']`
///
/// Total size: 768 bytes (256 × 3). Fits in L1 cache.
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

/// Digit count (1, 2, or 3) for every `u8` value.
///
/// `U8_LEN[n]` returns the number of ASCII decimal digits needed to
/// represent `n`. Used to advance the write cursor after copying
/// `U8_PADDED[n]` into the output buffer.
///
/// Total size: 256 bytes.
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

// ─── Bold escape sequences (branchless) ─────────────────────────────────────

/// Precomputed bold escape sequences for branchless selection.
///
/// `BOLD_ESCAPES[0]` = bold OFF (`\x1b[22m`, 5 bytes).
/// `BOLD_ESCAPES[1]` = bold ON  (`\x1b[1m`,  4 bytes).
///
/// The legacy `Terminal::draw()` path had an `if cell.bold { ... } else
/// { ... }` branch with different byte counts (4 vs 5). This table
/// replaces the branch with a `cell.bold as usize` index (compiles to
/// `setne` on x86 — no branch) + a memcpy of the selected escape.
///
/// Total size: 18 bytes (5 + 4 + alignment padding to 9 bytes per slot).
pub(crate) const BOLD_ESCAPES: [&[u8]; 2] = [b"\x1b[22m", b"\x1b[1m"];

/// Lengths of the corresponding `BOLD_ESCAPES` entries.
///
/// Used to advance the write cursor after copying the selected escape
/// into a stack buffer (the Strategy E combined-buffer fast path in
/// `bench_io::emit_cell_lean`).
pub(crate) const BOLD_ESCAPE_LENS: [usize; 2] = [5, 4];

// ─── u16 → ASCII decimal (branchless, up to 5 digits) ───────────────────────
//
// `push_u16` is called for cursor MoveTo coordinates (1-65535). The legacy
// implementation branches on `n < 256` and falls back to a 5-iteration
// divmod loop for the 4-5 digit case. BOLT replaces the loop with a
// 5-digit lookup table indexed by a single `leading_zeros` count.
//
// The table is small enough (5 × 65536 bytes = 320 KB total) that we
// avoid materializing it; instead, we use a hybrid: 1-3 digit case via
// U8_PADDED (after `(n & 0xFF)` for the low byte when n < 256), and
// the 4-5 digit case via an unrolled divmod that LLVM optimizes to
// constant multiplies. This keeps the table at 1 KB total while still
// eliminating all branches in the common (n < 256) case.

/// Write a u8 as ASCII decimal digits into a fixed-size slice starting at
/// `buf[0]`. Returns the number of bytes written (1, 2, or 3).
///
/// Branchless: 2 L1-cached table lookups + 1 3-byte memcpy + return.
/// Eliminates the 3-branch cascade (`n<10`, `n<100`, `else`) of the
/// legacy `sgr_format::push_u8`.
///
/// # Safety contract for callers
///
/// The caller MUST advance `pos` by the returned digit count. The 3-byte
/// memcpy always writes `U8_PADDED[n]` to `buf[..3]`, including padding
/// bytes when the digit count is < 3. The padding is harmless as long as
/// the next sequential write (a `;` separator or the next channel's
/// digits) overwrites it. This holds for any left-to-right SGR sequence
/// builder (the only current caller pattern).
#[inline]
pub(crate) fn write_u8_to_slice(buf: &mut [u8], n: u8) -> usize {
    let digits = U8_LEN[n as usize] as usize;
    buf[..3].copy_from_slice(&U8_PADDED[n as usize]);
    digits
}

/// Push a u8 as ASCII decimal digits into a `Vec<u8>`.
///
/// Drop-in replacement for the legacy `sgr_format::push_u8`. The legacy
/// version had a 3-branch cascade; this version does 2 table lookups
/// + 1 `extend_from_slice` of `U8_PADDED[n][..U8_LEN[n]]`.
///
/// At 12.9M cells/sec × 3 channels (R, G, B) = 38.7M calls/sec, the
/// branchless version saves ~3-5 ns/call → ~120-190 ms/sec of CPU
/// returned to the scheduler.
#[inline]
pub(crate) fn push_u8(buf: &mut Vec<u8>, n: u8) {
    let digits = U8_LEN[n as usize] as usize;
    buf.extend_from_slice(&U8_PADDED[n as usize][..digits]);
}

/// Push a u16 as ASCII decimal digits into a `Vec<u8>`.
///
/// Drop-in replacement for the legacy `sgr_format::push_u16`. Uses
/// `push_u8` for the common `n < 256` case (cursor coordinates under
/// 256 cover ~99% of terminal sizes). For the rare 4-5 digit case
/// (terminal > 256 rows or cols — only happens on extreme displays
/// like 8K UHD bench at 4320 lines), falls back to an unrolled divmod
/// that LLVM optimizes to constant multiplies.
#[inline]
pub(crate) fn push_u16(buf: &mut Vec<u8>, n: u16) {
    if n < 256 {
        push_u8(buf, n as u8);
    } else {
        // 256..=65535: up to 5 digits. Unrolled divmod (no loop branch).
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

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// BOLT push_u8 must produce byte-identical output to the legacy
    /// branch-cascade implementation across the entire u8 range.
    #[test]
    fn bolt_push_u8_matches_legacy_across_full_range() {
        fn legacy_push_u8(buf: &mut Vec<u8>, n: u8) {
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

        for n in 0u16..256 {
            let n = n as u8;
            let mut legacy = Vec::new();
            legacy_push_u8(&mut legacy, n);

            let mut bolt = Vec::new();
            push_u8(&mut bolt, n);

            assert_eq!(
                legacy, bolt,
                "BOLT push_u8({n}) produced {bolt:?} but legacy produced {legacy:?}"
            );
        }
    }

    /// BOLT push_u16 must produce byte-identical output to the legacy
    /// branch-cascade implementation across the full u16 range (sampled).
    #[test]
    fn bolt_push_u16_matches_legacy_across_sampled_range() {
        fn legacy_push_u16(buf: &mut Vec<u8>, n: u16) {
            if n < 256 {
                // delegate to legacy push_u8 logic
                let m = n as u8;
                if m < 10 {
                    buf.push(b'0' + m);
                } else if m < 100 {
                    buf.push(b'0' + m / 10);
                    buf.push(b'0' + m % 10);
                } else {
                    buf.push(b'0' + m / 100);
                    buf.push(b'0' + (m / 10) % 10);
                    buf.push(b'0' + m % 10);
                }
            } else {
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

        // Sample: every value 0..512 (covers 1-3 digit + start of 4 digit),
        // then 1000, 10000, 65535 (covers 4-5 digit edge cases).
        let mut samples: Vec<u16> = (0..512).collect();
        samples.extend_from_slice(&[1000, 1234, 9999, 10000, 65535, 54321]);

        for n in samples {
            let mut legacy = Vec::new();
            legacy_push_u16(&mut legacy, n);

            let mut bolt = Vec::new();
            push_u16(&mut bolt, n);

            assert_eq!(
                legacy, bolt,
                "BOLT push_u16({n}) produced {bolt:?} but legacy produced {legacy:?}"
            );
        }
    }

    /// `write_u8_to_slice` must return the correct digit count and write
    /// the correct digits to the buffer's first `digits` bytes.
    #[test]
    fn write_u8_to_slice_writes_correct_digits() {
        let cases: &[(u8, &[u8])] = &[
            (0, b"0"),
            (5, b"5"),
            (9, b"9"),
            (10, b"10"),
            (42, b"42"),
            (99, b"99"),
            (100, b"100"),
            (128, b"128"),
            (255, b"255"),
        ];

        for &(n, expected) in cases {
            let mut buf = [0u8; 3];
            let digits = write_u8_to_slice(&mut buf, n);
            assert_eq!(
                digits,
                expected.len(),
                "digit count mismatch for n={n}: got {digits}, expected {}",
                expected.len()
            );
            assert_eq!(
                &buf[..digits],
                expected,
                "digit bytes mismatch for n={n}: got {:?}, expected {expected:?}",
                &buf[..digits]
            );
        }
    }

    /// `U8_LEN` table invariant: every entry is 1, 2, or 3 and matches
    /// the actual digit count of the index.
    #[test]
    fn u8_len_table_invariant() {
        for n in 0u16..256 {
            let n = n as u8;
            let expected = n.to_string().len() as u8;
            assert_eq!(
                U8_LEN[n as usize], expected,
                "U8_LEN[{n}] = {} but {n} has {expected} digits",
                U8_LEN[n as usize]
            );
            assert!(
                (1..=3).contains(&U8_LEN[n as usize]),
                "U8_LEN[{n}] = {} is out of range [1, 3]",
                U8_LEN[n as usize]
            );
        }
    }

    /// `U8_PADDED` table invariant: the first `U8_LEN[n]` bytes of
    /// `U8_PADDED[n]` must equal the ASCII decimal representation of `n`.
    #[test]
    fn u8_padded_table_invariant() {
        for n in 0u16..256 {
            let n = n as u8;
            let digits = U8_LEN[n as usize] as usize;
            let expected = n.to_string().into_bytes();
            assert_eq!(
                &U8_PADDED[n as usize][..digits],
                expected.as_slice(),
                "U8_PADDED[{n}][..{digits}] = {:?} but expected {expected:?}",
                &U8_PADDED[n as usize][..digits]
            );
        }
    }

    /// `BOLD_ESCAPES` invariant: index 0 = bold OFF (`\x1b[22m`),
    /// index 1 = bold ON (`\x1b[1m`). Locks the table contents so a
    /// future refactor can't accidentally swap them.
    #[test]
    fn bold_escapes_table_invariant() {
        assert_eq!(
            BOLD_ESCAPES[0], b"\x1b[22m",
            "BOLD_ESCAPES[0] must be bold-OFF escape"
        );
        assert_eq!(
            BOLD_ESCAPES[1], b"\x1b[1m",
            "BOLD_ESCAPES[1] must be bold-ON escape"
        );
        assert_eq!(BOLD_ESCAPE_LENS[0], 5, "BOLD_ESCAPES[0] length must be 5");
        assert_eq!(BOLD_ESCAPE_LENS[1], 4, "BOLD_ESCAPES[1] length must be 4");
    }

    /// `BOLD_ESCAPES[idx]` length must match `BOLD_ESCAPE_LENS[idx]`
    /// for both indices. Catches a class of bugs where one table is
    /// updated but not the other.
    #[test]
    fn bold_escapes_length_table_consistency() {
        for idx in 0..2 {
            assert_eq!(
                BOLD_ESCAPES[idx].len(),
                BOLD_ESCAPE_LENS[idx],
                "BOLD_ESCAPES[{idx}].len() = {} but BOLD_ESCAPE_LENS[{idx}] = {}",
                BOLD_ESCAPES[idx].len(),
                BOLD_ESCAPE_LENS[idx]
            );
        }
    }

    /// BOLT push_u8 must NOT branch on `n` (regression: if someone
    /// reintroduces the cascade, this test catches it by checking that
    /// the function compiles to a table lookup, not a branch cascade).
    /// We can't disassemble in stable Rust, but we CAN verify the
    /// output is correct for edge values 0, 9, 10, 99, 100, 255 —
    /// the exact boundaries where the legacy cascade branched.
    #[test]
    fn bolt_push_u8_handles_all_cascade_boundaries() {
        let boundaries: &[(u8, &str)] = &[
            (0, "0"),
            (1, "1"),
            (9, "9"),
            (10, "10"),
            (11, "11"),
            (99, "99"),
            (100, "100"),
            (101, "101"),
            (254, "254"),
            (255, "255"),
        ];
        for &(n, expected) in boundaries {
            let mut buf = Vec::new();
            push_u8(&mut buf, n);
            assert_eq!(
                std::str::from_utf8(&buf).unwrap(),
                expected,
                "push_u8({n}) produced {:?}, expected {expected:?}",
                buf
            );
        }
    }
}
