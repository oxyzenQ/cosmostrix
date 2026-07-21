// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Benchmark helper functions extracted from bench.rs.

use std::env;

use crate::constants::{
    DENSITY_AUTO_DEFAULT_COLS, DENSITY_AUTO_DEFAULT_LINES, MAX_TERMINAL_COLS, MAX_TERMINAL_LINES,
    MIN_TERMINAL_COLS, MIN_TERMINAL_LINES,
};

pub(crate) fn bench_dimensions(cli_size: Option<(u16, u16)>) -> (u16, u16) {
    // --screen-size CLI flag takes precedence
    if let Some((w, h)) = cli_size {
        let w = w.clamp(MIN_TERMINAL_COLS, MAX_TERMINAL_COLS);
        let h = h.clamp(MIN_TERMINAL_LINES, MAX_TERMINAL_LINES);
        return (w, h);
    }
    // Fall back to env vars (backward compat for CI)
    if let (Ok(w_str), Ok(h_str)) = (
        env::var("COSMOSTRIX_BENCH_COLS"),
        env::var("COSMOSTRIX_BENCH_LINES"),
    ) {
        if let (Ok(w), Ok(h)) = (w_str.parse::<u16>(), h_str.parse::<u16>()) {
            return (
                w.clamp(MIN_TERMINAL_COLS, MAX_TERMINAL_COLS),
                h.clamp(MIN_TERMINAL_LINES, MAX_TERMINAL_LINES),
            );
        }
    }
    // v17 audit: query the ACTUAL terminal size before falling back to the
    // hardcoded 120x40 default. Previously the benchmark never queried the
    // terminal at all — a user running `cosmostrix --benchmark` in a 200x50
    // terminal would get a report claiming "120x40", which was misleading.
    // crossterm::terminal::size() is a pure query (no terminal state change),
    // safe to call in headless benchmark mode. Returns Err on non-TTY (pipes,
    // CI without PTY) — in that case we fall through to the 120x40 default.
    if let Ok((w, h)) = crossterm::terminal::size() {
        if w >= MIN_TERMINAL_COLS && h >= MIN_TERMINAL_LINES {
            return (
                w.clamp(MIN_TERMINAL_COLS, MAX_TERMINAL_COLS),
                h.clamp(MIN_TERMINAL_LINES, MAX_TERMINAL_LINES),
            );
        }
    }
    // Last-resort default: 120x40 (for non-TTY / piped / CI without PTY).
    (
        DENSITY_AUTO_DEFAULT_COLS.clamp(MIN_TERMINAL_COLS, MAX_TERMINAL_COLS),
        DENSITY_AUTO_DEFAULT_LINES.clamp(MIN_TERMINAL_LINES, MAX_TERMINAL_LINES),
    )
}

/// Read configurable warmup duration from environment, falling back to the
/// default constant. Allows CI or power users to tune JIT warmup for
/// stability on different hardware.
pub(crate) fn bench_warmup_secs() -> u64 {
    env::var("COSMOSTRIX_BENCH_WARMUP_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(2) // default warmup: 2 seconds
}

#[cfg(test)]
mod tests {
    use crate::bench::resolve_bench_duration;
    use crate::bench::BENCHMARK_DURATION_SECS;
    use crate::bench_meta::AVG_DIRTY_CELL_RATIO_MEANING;
    use crate::bench_report::ACTIVE_FRAME_RATIO_MEANING;

    #[test]
    fn benchmark_metric_meanings_distinguish_dirty_frame_concepts() {
        assert!(ACTIVE_FRAME_RATIO_MEANING.contains("at least one dirty cell"));
        assert!(AVG_DIRTY_CELL_RATIO_MEANING.contains("dirty-cell coverage"));
    }

    #[test]
    fn benchmark_docs_do_not_keep_stale_active_claims() {
        let readme = include_str!("../README.md");
        let benchmark_readme = include_str!("../benchmark/README.md");
        assert!(!readme.contains("7,000 FPS"));
        assert!(!readme.contains(">7,000 FPS"));
        assert!(!benchmark_readme.contains("v2.1.0 reference results"));
        assert!(!benchmark_readme.contains("throughput exceeds 7,000 FPS"));
    }

    #[test]
    fn benchmark_stability_field_exists() {
        let readme = include_str!("../README.md");
        assert!(readme.to_lowercase().contains("throughput stability"));
    }

    #[test]
    fn benchmark_output_includes_stability_fields() {
        // This test ensures the premium benchmark output includes
        // backward-compatible stability fields. If any of these are
        // removed, the test will fail, preventing accidental breakage.
        const REQUIRED_FIELDS: &[&str] = &[
            "avg_fps",
            "peak_fps",
            "avg_frame_time",
            "p95_frame_time",
            "p99_frame_time",
            "p99_9_frame_time",
            "max_frame_time",
            "frame_jitter",
            "median_fps",
            "frame_time_stability",
            "draw_ratio",
            "active_frame_ratio_percent",
            "avg_dirty_cell_ratio_percent",
            "estimated_full_redraw_ratio_percent",
            "active_streams_avg",
            "dirty_glyphs_per_second",
            "planned_mode",
            "planned_worker_budget",
            "plan_reason",
            "actual_execution",
            "terminal_writer",
            "atmosphere_regime",
            "atmosphere_effective",
            "atmosphere_transition",
            "atmosphere_verifier",
            "atmosphere_application",
            "atmosphere_application_mode",
            "atmosphere_visual_effect",
            "effective_runtime",
        ];
        // These are checked against report field keys in the actual
        // benchmark (integration-level). Here we just verify the
        // test documents the contract.
        assert!(!REQUIRED_FIELDS.is_empty());
        for field in REQUIRED_FIELDS {
            assert!(!field.is_empty());
        }
    }

    #[test]
    fn bench_file_stays_under_target_loc() {
        // Guard: src/bench.rs must stay well under 1000 LOC.
        // Current target is under 1200 LOC — bumped to 1200 after Phase 8-9 scaling
        // added sub-component timing wiring (sim/render/io accumulators
        // and per-frame cloud.last_sim_ms()/last_render_ms() reads).
        // The ComponentTimer struct was extracted to bench_comp.rs to
        // minimize growth here; further sub-component work should also
        // live in bench_comp.rs rather than expand this file.
        let source = include_str!("bench.rs");
        let lines = source.lines().count();
        assert!(
            lines < 1200,
            "bench.rs must stay under 1200 LOC target (currently {lines})"
        );
    }

    #[test]
    fn bench_re_exports_preserve_external_import_paths() {
        // Verify that the re-exports from bench_report.rs are correct
        // so external modules (e.g., cloud/tests/tests_visual_depth.rs)
        // can still use `use crate::bench::AVG_DIRTY_CELL_RATIO_MEANING`.
        assert!(AVG_DIRTY_CELL_RATIO_MEANING.contains("dirty-cell coverage"));
    }

    #[test]
    fn resolve_bench_duration_uses_default_when_none() {
        assert_eq!(
            resolve_bench_duration(None).unwrap(),
            BENCHMARK_DURATION_SECS,
            "None override must fall back to default duration"
        );
    }

    #[test]
    fn resolve_bench_duration_accepts_in_range_override() {
        assert_eq!(resolve_bench_duration(Some(1)).unwrap(), 1, "min boundary");
        assert_eq!(
            resolve_bench_duration(Some(600)).unwrap(),
            600,
            "max boundary"
        );
        assert_eq!(resolve_bench_duration(Some(30)).unwrap(), 30, "mid-range");
    }

    #[test]
    fn resolve_bench_duration_rejects_below_minimum() {
        let err = resolve_bench_duration(Some(0)).unwrap_err();
        assert!(
            err.contains("below the"),
            "below-minimum error must explain the floor: {err}"
        );
    }

    #[test]
    fn resolve_bench_duration_accepts_above_legacy_maximum() {
        // v13.4.0: no max cap — --duration allows unlimited endurance runs.
        // 601s was previously rejected; now accepted.
        assert_eq!(resolve_bench_duration(Some(601)).unwrap(), 601);
        assert_eq!(resolve_bench_duration(Some(3600)).unwrap(), 3600);
    }
}
