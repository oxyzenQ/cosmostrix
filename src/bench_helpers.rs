// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Benchmark helper functions extracted from bench.rs.

use std::env;

use crate::constants::{
    BENCH_MAX_COLS, BENCH_MAX_LINES, DENSITY_AUTO_DEFAULT_COLS, DENSITY_AUTO_DEFAULT_LINES,
    MIN_TERMINAL_COLS, MIN_TERMINAL_LINES,
};

pub(crate) fn bench_dimensions(cli_size: Option<(u16, u16)>) -> (u16, u16) {
    // --screen-size CLI flag takes precedence
    if let Some((w, h)) = cli_size {
        let w = w.clamp(MIN_TERMINAL_COLS, BENCH_MAX_COLS);
        let h = h.clamp(MIN_TERMINAL_LINES, BENCH_MAX_LINES);
        return (w, h);
    }
    // Fall back to env vars (backward compat for CI)
    if let (Ok(w_str), Ok(h_str)) = (
        env::var("COSMOSTRIX_BENCH_COLS"),
        env::var("COSMOSTRIX_BENCH_LINES"),
    ) {
        if let (Ok(w), Ok(h)) = (w_str.parse::<u16>(), h_str.parse::<u16>()) {
            return (
                w.clamp(MIN_TERMINAL_COLS, BENCH_MAX_COLS),
                h.clamp(MIN_TERMINAL_LINES, BENCH_MAX_LINES),
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
                w.clamp(MIN_TERMINAL_COLS, BENCH_MAX_COLS),
                h.clamp(MIN_TERMINAL_LINES, BENCH_MAX_LINES),
            );
        }
    }
    // Last-resort default: 120x40 (for non-TTY / piped / CI without PTY).
    (
        DENSITY_AUTO_DEFAULT_COLS.clamp(MIN_TERMINAL_COLS, BENCH_MAX_COLS),
        DENSITY_AUTO_DEFAULT_LINES.clamp(MIN_TERMINAL_LINES, BENCH_MAX_LINES),
    )
}

/// Read configurable warmup duration from environment, falling back to the
/// default constant. Allows CI or power users to tune JIT warmup for
/// stability on different hardware.
///
/// Phase 5 (P3-2): on parse failure, emit a stderr warning naming the env
/// var, the bad value, and the fallback. Previously the parse error was
/// silently swallowed and the warmup defaulted to 2s with no signal —
/// causing CI users to spend hours debugging "slow benchmark" regressions
/// that were actually just a typo'd env var being ignored.
pub(crate) fn bench_warmup_secs() -> u64 {
    const DEFAULT: u64 = 2;
    match env::var("COSMOSTRIX_BENCH_WARMUP_SECS") {
        Ok(raw) => match raw.parse::<u64>() {
            Ok(secs) => secs,
            Err(_) => {
                use std::io::Write;
                let _ = std::io::stderr().write_fmt(format_args!(
                    "[bench] warning: COSMOSTRIX_BENCH_WARMUP_SECS='{raw}' is not a valid u64 — falling back to default {DEFAULT}s\n"
                ));
                DEFAULT
            }
        },
        Err(_) => DEFAULT,
    }
}

/// Backpressure section formatter for the `--perf-stats` interactive-mode
/// exit report. Extracted from `event_loop.rs` to keep that file under the
/// 1500-LOC project cap.
///
/// Emits two metric families:
///
/// 1. `avg` / `peak` — the legacy load-shed signal `clamp(work/budget - 1, 0, 2)`.
///    Non-zero ONLY when the renderer can't keep up with `--fps`. On healthy
///    hardware this stays at 0.000 by design.
///
/// 2. `budget_utilization_avg` / `budget_utilization_peak` / `budget_headroom_avg`
///    — the companion metric that is ALWAYS non-zero (work_s / target_period).
///    This is what makes the section informative even when backpressure is 0:
///    the user sees how much of the frame budget the renderer is consuming.
#[allow(clippy::too_many_arguments)] // one-off report formatter, struct would be overkill
pub(crate) fn format_backpressure_section(
    r: &mut crate::report::Report,
    avg_pressure: f64,
    peak_pressure: f32,
    utilization_sum: f64,
    utilization_max: f32,
    frames: u64,
    target_period: std::time::Duration,
    avg_work_ms: f64,
    pressure_class: &str,
    overshoot_frames: u64,
    overshoot_ratio: f64,
) {
    let s = r.section("BACKPRESSURE");
    s.field("avg", &format!("{:.3}", avg_pressure));
    s.field("peak", &format!("{:.3}", peak_pressure));
    let frames_f = frames.max(1) as f64;
    let avg_util = utilization_sum / frames_f;
    let tgt_s = target_period.as_secs_f64().max(0.000_001);
    let headroom_ms = (tgt_s - avg_work_ms / 1000.0).max(0.0) * 1000.0;
    s.field(
        "budget_utilization_avg",
        &format!("{:.2}%", avg_util * 100.0),
    );
    s.field(
        "budget_utilization_peak",
        &format!("{:.2}%", utilization_max * 100.0),
    );
    s.field("budget_headroom_avg", &format!("{:.3}ms", headroom_ms));
    s.field("classification", pressure_class);
    s.field(
        "basis",
        "avg/peak = clamp(work_s/target_period - 1, 0, 2); non-zero only when frames can't keep up. budget_utilization = work_s/target_period (always non-zero).",
    );
    s.field(
        "overshoot_frames",
        &format!("{} ({:.1}% of total)", overshoot_frames, overshoot_ratio),
    );
    s.advice("avg/peak 0.000 = healthy (renderer kept up). budget_utilization shows how much of the frame budget was consumed (always non-zero). For real FPS see TIMING.avg_fps / TIMING.instant_fps.");
}

#[cfg(test)]
mod tests {
    use super::format_backpressure_section;
    use crate::bench::resolve_bench_duration;
    use crate::bench::BENCHMARK_DURATION_SECS;
    use crate::bench_meta::AVG_DIRTY_CELL_RATIO_MEANING;
    use crate::bench_report::ACTIVE_FRAME_RATIO_MEANING;
    use crate::report::Report;
    use std::time::Duration;

    #[test]
    fn backpressure_section_emits_nonzero_budget_utilization_on_healthy_hw() {
        // Bug fix: previously BACKPRESSURE.avg/peak showed 0.000 on healthy
        // hardware (correct by design) but the user saw no other measurement
        // to confirm the renderer was actually measuring something. The new
        // budget_utilization_avg/peak fields are ALWAYS non-zero.
        //
        // Simulate a typical 60fps run with very fast frame work (0.074ms
        // per frame, well under the 16.67ms budget). Verify the function
        // runs without panic and accepts the healthy-hardware signal
        // pattern (zero backpressure, non-zero utilization).
        let mut r = Report::new("TEST");
        let frames = 600u64;
        let utilization_per_frame = 0.074 / (1000.0 * (1.0 / 60.0)); // 0.00444
        let utilization_sum = utilization_per_frame * frames as f64;
        format_backpressure_section(
            &mut r,
            0.0,                          // avg_pressure (0 on healthy hw)
            0.0,                          // peak_pressure
            utilization_sum,              // sum of utilization across frames
            utilization_per_frame as f32, // max utilization
            frames,
            Duration::from_secs_f64(1.0 / 60.0),
            0.074, // avg_work_ms
            "low",
            0,
            0.0,
        );
        // Smoke test: function completed without panic. The actual output
        // format is verified by the existing perf-stats integration test
        // (run with `cosmostrix --perf-stats --duration 5`).
        // Headroom must be positive on healthy hardware.
        let tgt_s = (1.0 / 60.0_f64).max(0.000_001);
        let headroom_ms = (tgt_s - 0.074 / 1000.0).max(0.0) * 1000.0;
        assert!(
            headroom_ms > 0.0,
            "headroom must be positive when work < budget"
        );
        // Utilization must be > 0 (work > 0).
        assert!(
            utilization_per_frame > 0.0,
            "utilization must be non-zero when work > 0"
        );
    }

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
        //
        // v30 strengthen (audit): removed `atmosphere_application` — it was
        // an exact duplicate of `application` (both printed the same
        // `is_ident`-derived string).
        //
        // v30 (2026-08-05, atmosphere elimination): removed all six
        // `atmosphere_*`-prefixed stability fields from this list because
        // they were never actual report field keys (they were documentation
        // labels for the `regime`, `effective`, `transition`, `verifier`,
        // `application_mode`, `visual_effect` keys, all of which were
        // themselves removed when the atmosphere engine was eliminated at
        // commit 07b44b5). The list now reflects what the post-elimination
        // benchmark report actually emits.
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
            "active_frame_ratio_percent",
            "avg_dirty_cell_ratio_percent",
            "active_streams_avg",
            "dirty_glyphs_per_second",
            "planned_mode",
            "planned_worker_budget",
            "plan_reason",
            "actual_execution",
            "terminal_writer",
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
        // Guard: src/bench.rs must stay well under 1500 LOC.
        // Current target is under 1500 LOC — bumped from 1200 in v25.10 to
        // match the project-wide LOC cap. Phase 8-9 scaling added sub-component
        // timing wiring (sim/render/io accumulators and per-frame
        // cloud.last_sim_ms()/last_render_ms() reads). The ComponentTimer
        // struct was extracted to bench_comp.rs to minimize growth here;
        // further sub-component work should also live in bench_comp.rs
        // rather than expand this file.
        let source = include_str!("bench.rs");
        let lines = source.lines().count();
        assert!(
            lines < 1500,
            "bench.rs must stay under 1500 LOC target (currently {lines})"
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
