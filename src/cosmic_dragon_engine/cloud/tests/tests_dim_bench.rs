// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Dimension consistency + bench cosmetics gate lock tests — extracted
//! from `cloud/tests/mod.rs` to keep that file under the 800-LOC cap.
//!
//! Dimension tests pin the invariant that every size-dependent structure
//! agrees with clamped dimensions. Bench cosmetics tests lock that
//! CRT vignette + storytelling are gated on !bench_mode.

use super::*;

// ── Dimension consistency tests (triple-engine LTS audit LOW-2) ───────────
//
// Cloud::reset previously built self.cols/self.lines and the droplet pool
// from the CLAMPED dimensions while the RNG ranges, column tables, and
// per-cell LUTs used the RAW parameters. These tests pin the invariant:
// every size-dependent structure must agree with the clamped dimensions.

#[test]
fn reset_clamps_oversized_dimensions_consistently() {
    let mut cloud = make_cloud();
    cloud.reset(2000, 600);

    assert_eq!(cloud.cols, crate::constants::MAX_TERMINAL_COLS);
    assert_eq!(cloud.lines, crate::constants::MAX_TERMINAL_LINES);
    // Column-indexed structures must match the clamped width.
    assert_eq!(
        cloud.col_stat.len(),
        crate::constants::MAX_TERMINAL_COLS as usize
    );
    assert_eq!(
        cloud.column_palette_slot.len(),
        crate::constants::MAX_TERMINAL_COLS as usize
    );
    // Line-indexed structures must match the clamped height.
    assert_eq!(
        cloud.edge_fade_lut.len(),
        crate::constants::MAX_TERMINAL_LINES as usize
    );
    // Cell-indexed structures must match the clamped cell count.
    let clamped_cells = (crate::constants::MAX_TERMINAL_COLS as usize)
        * (crate::constants::MAX_TERMINAL_LINES as usize);
    assert_eq!(cloud.vignette_lut.len(), clamped_cells);
    assert_eq!(
        cloud.vignette_lut_dims.0,
        crate::constants::MAX_TERMINAL_COLS
    );
    assert_eq!(cloud.phosphor.len(), clamped_cells);
}

#[test]
fn reset_clamps_degenerate_dimensions_consistently() {
    let mut cloud = make_cloud();
    cloud.reset(0, 0);

    assert_eq!(cloud.cols, crate::constants::MIN_TERMINAL_COLS);
    assert_eq!(cloud.lines, crate::constants::MIN_TERMINAL_LINES);
    assert_eq!(
        cloud.col_stat.len(),
        crate::constants::MIN_TERMINAL_COLS as usize
    );
    assert_eq!(
        cloud.edge_fade_lut.len(),
        crate::constants::MIN_TERMINAL_LINES as usize
    );
}

#[test]
fn reset_bench_allows_benchmark_dimensions() {
    let mut cloud = make_cloud();
    // 1920x540 exceeds the interactive cap (1024x500) but is within the
    // benchmark bounds — reset_bench must keep it intact so the stress
    // benchmarks exercise full bench-bounded dimensions (mirroring
    // Frame::new_bench). The interactive reset() clamps the same input.
    cloud.reset_bench(1920, 540);
    assert_eq!(cloud.cols, 1920);
    assert_eq!(cloud.lines, 540);
    assert_eq!(cloud.col_stat.len(), 1920);
    assert_eq!(cloud.edge_fade_lut.len(), 540);
    assert_eq!(cloud.vignette_lut_dims, (1920, 540));

    cloud.reset(1920, 540);
    assert_eq!(cloud.cols, crate::constants::MAX_TERMINAL_COLS);
    assert_eq!(cloud.lines, crate::constants::MAX_TERMINAL_LINES);
}

// ── PERF-1-Supreme bench cosmetics gate lock tests ────────────────────────────────────
//
// Owner directive: benchmark mode measures the critical path only
// (rain + 3 dragon engines). Cinematic cosmetics — CRT vignette and
// emergent storytelling moments — must never run during measurement
// frames. These tests lock the two rain.rs gates so a future refactor
// cannot silently reintroduce the cosmetic workload into bench runs.

#[test]
fn bench_mode_storytelling_moments_stay_empty() {
    // Behavioral lock: step a bench-mode cloud through many sim seconds
    // and verify the emergent storytelling engine never spawns a moment.
    // The storytelling tick is gated on !bench_mode, so the moments vec
    // must stay empty for the entire simulated run — even past the
    // STORYTELLING_TICK_SECS cadence and with RNG draws available.
    let mut cloud = make_cloud();
    cloud.reset_bench(80, 24);
    let mut frame = Frame::new_bench(80, 24, cloud.palette.bg);
    let now = Instant::now();

    // 120 steps at 60 FPS = 2 sim-minutes — far beyond the storytelling
    // tick cadence (per-second) and longer than any default bench run.
    for i in 0..120u32 {
        let t = now + Duration::from_millis((i as u64) * 16);
        cloud.rain_at(&mut frame, t);
        frame.clear_dirty();
    }
    assert!(
        cloud.storytelling.moments.is_empty(),
        "PERF-1-Supreme: bench mode must not spawn emergent storytelling moments"
    );
    assert!(
        cloud.storytelling.cooldown_until.is_none(),
        "PERF-1-Supreme: bench mode must not set storytelling cooldown"
    );
}

#[test]
fn bench_cosmetics_gates_exist_in_rain_source() {
    // Structural lock (source-scan, same pattern as
    // benchmark_output_fields_complete): the CRT vignette call and the
    // storytelling tick must be wrapped in `!self.bench_mode` guards.
    // If a refactor removes either guard, this test fails before the
    // cosmetic workload can silently return to the bench hot path.
    // v50.0.0-beta.7 LOC refactor: post_rain_processing extracted to
    // post_rain.rs, so we check both rain.rs + post_rain.rs.
    let source = include_str!("../rain.rs");
    let post_source = include_str!("../post_rain.rs");
    let combined = format!("{source}\n{post_source}");

    assert!(
        combined.contains("if !self.bench_mode {\n            self.apply_crt_vignette(frame);"),
        "PERF-1-Supreme: CRT vignette must be gated on !bench_mode in rain_at"
    );
    assert!(
        combined.contains(
            "if !self.bench_mode {\n            if let Some(kind) = self.storytelling.tick("
        ),
        "PERF-1-Supreme: storytelling tick must be gated on !bench_mode in rain_at"
    );
}
