// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! FFI wrapper for supercharger.c — SIMD-optimized cell comparison.
//!
//! EXPERIMENTAL — dragon-experimental branch only.
//!
//! This module provides Rust bindings to the C SIMD functions in
//! `supercharger.c`. The C code must be compiled and linked via
//! `build.rs` (see below).
//!
//! ## Usage
//!
//! ```ignore
//! use supercharger::cell_eq_sse2;
//!
//! let a = Cell { ch: 'x', fg: None, bg: None, bold: false };
//! let b = Cell { ch: 'x', fg: None, bg: None, bold: false };
//!
//! // SAFETY: Cell is 16 bytes, matches cell16_t layout
//! let eq = unsafe { cell_eq_sse2(&a as *const _ as *const _, &b as *const _ as *const _) };
//! assert!(eq);
//! ```
//!
//! ## Build
//!
//! Add to `build.rs`:
//! ```ignore
//! fn main() {
//!     cc::Build::new()
//!         .file("src/supercharger.c")
//!         .flag("-march=x86-64-v3")  // or "x86-64" for SSE2 only
//!         .flag("-O3")
//!         .compile("supercharger");
//!     println!("cargo:rerun-if-changed=src/supercharger.c");
//! }
//! ```
//!
//! And add to Cargo.toml:
//! ```toml
//! [build-dependencies]
//! cc = "1.0"
//! ```

#![cfg(feature = "supercharger")]
#![allow(dead_code)]

use crate::cell::Cell;

// Verify Cell is 16 bytes at compile time — must match cell16_t in C.
const _: () = assert!(
    std::mem::size_of::<Cell>() == 16,
    "Cell must be 16 bytes for SIMD supercharger"
);

extern "C" {
    fn cell_eq_sse2(a: *const Cell, b: *const Cell) -> bool;
    fn cell_eq2_avx2(a: *const Cell, b: *const Cell) -> u32;
    fn row_dirty_scan_sse2(
        current: *const Cell,
        last: *const Cell,
        limit: u32,
        dirty_mask: *mut u8,
    ) -> u32;
    fn row_dirty_scan_avx2(
        current: *const Cell,
        last: *const Cell,
        limit: u32,
        dirty_mask: *mut u8,
    ) -> u32;
    fn cpu_has_avx2() -> u32;
}

/// Compare two cells using SSE2 SIMD (1 cycle, 16-byte compare).
///
/// # Safety
/// Both pointers must be valid, aligned to 1 byte (unaligned load is OK
/// on x86-64), and point to 16 bytes of readable memory.
#[inline]
pub unsafe fn cell_eq_simd(a: &Cell, b: &Cell) -> bool {
    cell_eq_sse2(a as *const Cell, b as *const Cell)
}

/// Compare two pairs of cells using AVX2 SIMD (1 cycle, 32-byte compare).
///
/// Returns a 32-bit mask:
/// - bits 0-15: cell pair 0 (0xFFFF = equal)
/// - bits 16-31: cell pair 1 (0xFFFF = equal)
///
/// # Safety
/// Both pointers must point to 32 bytes (2 cells) of readable memory.
/// Caller must verify AVX2 is available via `has_avx2()`.
#[inline]
pub unsafe fn cell_eq2_simd(a: &[Cell; 2], b: &[Cell; 2]) -> u32 {
    cell_eq2_avx2(a.as_ptr(), b.as_ptr())
}

/// Scan a row of cells for changes vs last frame, SSE2 version.
///
/// Returns the count of dirty cells. `dirty_mask` receives 1 bit per cell
/// (1 = dirty, 0 = unchanged). `dirty_mask` must be at least
/// `ceil(limit / 8)` bytes.
///
/// # Safety
/// - `current` and `last` must point to `limit` valid Cells
/// - `dirty_mask` must point to at least `(limit + 7) / 8` bytes
#[inline]
pub unsafe fn row_dirty_scan_simd(
    current: &[Cell],
    last: &[Cell],
    dirty_mask: &mut [u8],
) -> u32 {
    debug_assert_eq!(current.len(), last.len());
    debug_assert!(dirty_mask.len() >= (current.len() + 7) / 8);
    row_dirty_scan_sse2(
        current.as_ptr(),
        last.as_ptr(),
        current.len() as u32,
        dirty_mask.as_mut_ptr(),
    )
}

/// AVX2 version of `row_dirty_scan_simd` — 2 cells per cycle.
///
/// # Safety
/// Same as `row_dirty_scan_simd`, plus caller must verify `has_avx2()`.
#[inline]
pub unsafe fn row_dirty_scan_avx2(
    current: &[Cell],
    last: &[Cell],
    dirty_mask: &mut [u8],
) -> u32 {
    debug_assert_eq!(current.len(), last.len());
    debug_assert!(dirty_mask.len() >= (current.len() + 7) / 8);
    row_dirty_scan_avx2(
        current.as_ptr(),
        last.as_ptr(),
        current.len() as u32,
        dirty_mask.as_mut_ptr(),
    )
}

/// Check if the CPU supports AVX2 (x86-64-v3).
///
/// Returns true on AVX2-capable CPUs, false otherwise.
/// On non-x86 platforms, always returns false.
#[must_use]
pub fn has_avx2() -> bool {
    unsafe { cpu_has_avx2() != 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_size_is_16_bytes() {
        // This test exists to catch layout changes early.
        // If Cell size changes, supercharger.c must be updated.
        assert_eq!(std::mem::size_of::<Cell>(), 16);
    }

    #[test]
    fn avx2_detection_does_not_crash() {
        // Just verify the function is callable. Result depends on CPU.
        let _ = has_avx2();
    }
}
