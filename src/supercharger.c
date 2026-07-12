// =============================================================================
// supercharger.c — SIMD-optimized cell comparison for cosmostrix
// =============================================================================
// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only
// =============================================================================
// EXPERIMENTAL — dragon-experimental branch only.
//
// This file provides SIMD-optimized cell equality comparison using SSE2
// instructions. The Cell struct is 16 bytes (ch:u32 + fg:u32 + bg:u32 +
// flags:u32), which fits exactly in one __m128i register.
//
// _mm_cmpeq_epi8 compares 16 bytes in 1 cycle. The scalar derived == in
// Rust emits a 4-cycle byte-wise comparison. Theoretical speedup: 4x.
//
// REAL-WORLD CAVEAT: Rust's derived == on Cell short-circuits on the
// first differing byte (usually `ch`, the first 4 bytes). So most
// comparisons return in 1-2 cycles anyway. SIMD only wins when cells
// are ACTUALLY equal (full 16-byte compare needed). The net gain is
// typically <10% on the frame.set() hot path, not 4x.
//
// This is still worth measuring. If the benchmark shows >5% improvement
// in headless FPS, we keep it. Otherwise, we document why and revert.
//
// Build: compiled as a static library and linked via FFI from Rust.
// =============================================================================

#include <emmintrin.h>  // SSE2
#include <immintrin.h>  // AVX2 (optional, runtime-detected)
#include <stdbool.h>
#include <stdint.h>
#include <string.h>

// -----------------------------------------------------------------------------
// Cell layout — must match Rust's Cell (16 bytes, niche-optimized)
// -----------------------------------------------------------------------------
// Rust: Cell { ch: char (4B), fg: Option<Color> (4B niche), bg: Option<Color>
// (4B niche), bold: bool (1B) + 3B padding }
//
// The niche optimization makes Option<Color> 4 bytes (Color::Reset is the
// niche value). Total: 4+4+4+4(padding) = 16 bytes.
//
// This struct MUST be kept in sync with src/cell.rs. If Rust Cell changes
// layout, this C struct must change too. A static_assert in the FFI
// wrapper (supercharger.rs) verifies size == 16 at compile time.
// -----------------------------------------------------------------------------

typedef struct __attribute__((packed, aligned(16))) {
    uint32_t ch;      // char (4 bytes, Unicode scalar value)
    uint32_t fg;      // Option<Color> niche-encoded (4 bytes)
    uint32_t bg;      // Option<Color> niche-encoded (4 bytes)
    uint8_t  bold;    // bool (1 byte)
    uint8_t  pad[3];  // padding to 16 bytes
} cell16_t;

_Static_assert(sizeof(cell16_t) == 16, "cell16_t must be 16 bytes");

// -----------------------------------------------------------------------------
// SIMD cell equality — SSE2 (baseline x86-64, available everywhere)
// -----------------------------------------------------------------------------
// Compares 16 bytes in 1 cycle using _mm_cmpeq_epi8.
// Returns true if all 16 bytes match.
//
// vs scalar: Rust's derived == emits 4 separate 4-byte compares with
// short-circuit. Most comparisons differ in `ch` (first 4 bytes) and
// return after 1 compare. SIMD does the full 16 bytes unconditionally.
//
// Net: SIMD wins when cells are EQUAL (scalar does 4 compares, SIMD does 1).
//      SIMD loses when cells differ in byte 0 (scalar exits in 1 compare,
//      SIMD still does 1 compare but with higher latency).
//
// The frame.set() hot path calls this on cells that USUALLY differ (only
// ~10-20% of cells are unchanged per frame). So SIMD may actually be SLOWER
// for the common case. Measure before keeping.

bool cell_eq_sse2(const cell16_t* a, const cell16_t* b) {
    __m128i va = _mm_loadu_si128((const __m128i*)a);
    __m128i vb = _mm_loadu_si128((const __m128i*)b);
    __m128i cmp = _mm_cmpeq_epi8(va, vb);
    return _mm_movemask_epi8(cmp) == 0xFFFF;
}

// -----------------------------------------------------------------------------
// SIMD cell equality — AVX2 (2 cells at once, x86-64-v3)
// -----------------------------------------------------------------------------
// Compares 2 cells (32 bytes) in 1 cycle using _mm256_cmpeq_epi8.
// Returns a 32-bit mask: bits 0-15 = cell[0] result, bits 16-31 = cell[1].
// 0xFFFFFFFF = both cells equal. 0x0000FFFF = only cell[0] equal. Etc.
//
// This is useful for batch comparison: iterate 2 cells at a time in the
// diff path's dirty scan. Theoretical 2x throughput vs SSE2.
//
// Requires AVX2 (x86-64-v3). Must runtime-detect with cpuid before calling.
// cosmostrix's pro-linux-v3 build targets this, but release build is v1.

uint32_t cell_eq2_avx2(const cell16_t* a, const cell16_t* b) {
    __m256i va = _mm256_loadu_si256((const __m256i*)a);
    __m256i vb = _mm256_loadu_si256((const __m256i*)b);
    __m256i cmp = _mm256_cmpeq_epi8(va, vb);
    return (uint32_t)_mm256_movemask_epi8(cmp);
}

// -----------------------------------------------------------------------------
// Batch dirty scan — find changed cells in a row
// -----------------------------------------------------------------------------
// Given a row of N cells from the current frame and N cells from the last
// frame, returns a bitmask indicating which cells changed.
//
// This replaces the per-cell `last.cells[idx] == current` loop in
// terminal.rs draw() diff path. SSE2 processes 1 cell/cycle, AVX2
// processes 2 cells/cycle.
//
// The Rust side calls this via FFI for each row, then iterates only the
// dirty cells (bit set in the mask).
//
// limit = max cells to compare (row width). Returns count of dirty cells.
// dirty_mask must point to at least ceil(limit/8) bytes.

uint32_t row_dirty_scan_sse2(
    const cell16_t* current,
    const cell16_t* last,
    uint32_t limit,
    uint8_t* dirty_mask  // output: 1 bit per cell, 1=dirty
) {
    uint32_t dirty_count = 0;
    memset(dirty_mask, 0, (limit + 7) / 8);

    // Process 1 cell at a time (SSE2)
    for (uint32_t i = 0; i < limit; i++) {
        __m128i vc = _mm_loadu_si128((const __m128i*)&current[i]);
        __m128i vl = _mm_loadu_si128((const __m128i*)&last[i]);
        __m128i cmp = _mm_cmpeq_epi8(vc, vl);
        if (_mm_movemask_epi8(cmp) != 0xFFFF) {
            dirty_mask[i / 8] |= (1 << (i % 8));
            dirty_count++;
        }
    }
    return dirty_count;
}

// AVX2 version: 2 cells at a time
uint32_t row_dirty_scan_avx2(
    const cell16_t* current,
    const cell16_t* last,
    uint32_t limit,
    uint8_t* dirty_mask
) {
    uint32_t dirty_count = 0;
    memset(dirty_mask, 0, (limit + 7) / 8);

    uint32_t i = 0;
    // Process 2 cells at a time
    while (i + 2 <= limit) {
        __m256i vc = _mm256_loadu_si256((const __m256i*)&current[i]);
        __m256i vl = _mm256_loadu_si256((const __m256i*)&last[i]);
        __m256i cmp = _mm256_cmpeq_epi8(vc, vl);
        uint32_t mask = (uint32_t)_mm256_movemask_epi8(cmp);
        // Cell 0 = bits 0-15, cell 1 = bits 16-31
        if ((mask & 0x0000FFFF) != 0x0000FFFF) {
            dirty_mask[i / 8] |= (1 << (i % 8));
            dirty_count++;
        }
        if ((mask & 0xFFFF0000) != 0xFFFF0000) {
            dirty_mask[(i+1) / 8] |= (1 << ((i+1) % 8));
            dirty_count++;
        }
        i += 2;
    }
    // Handle remaining cell (odd limit)
    if (i < limit) {
        __m128i vc = _mm_loadu_si128((const __m128i*)&current[i]);
        __m128i vl = _mm_loadu_si128((const __m128i*)&last[i]);
        __m128i cmp = _mm_cmpeq_epi8(vc, vl);
        if (_mm_movemask_epi8(cmp) != 0xFFFF) {
            dirty_mask[i / 8] |= (1 << (i % 8));
            dirty_count++;
        }
    }
    return dirty_count;
}

// -----------------------------------------------------------------------------
// CPU feature detection
// -----------------------------------------------------------------------------
// Returns: 0 = SSE2 only (baseline x86-64), 1 = AVX2 available (v3)
// Used by the Rust FFI wrapper to pick the right implementation.

#if defined(__x86_64__)
#include <cpuid.h>

uint32_t cpu_has_avx2(void) {
    unsigned int eax, ebx, ecx, edx;
    // CPUID leaf 7, subleaf 0
    if (__get_cpuid_count(7, 0, &eax, &ebx, &ecx, &edx)) {
        return (ebx & (1 << 5)) ? 1 : 0;  // bit 5 = AVX2
    }
    return 0;
}
#else
uint32_t cpu_has_avx2(void) { return 0; }
#endif
