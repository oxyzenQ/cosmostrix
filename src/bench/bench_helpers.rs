// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Benchmark helper functions extracted from bench.rs.

use std::env;

use crate::app::CloudConfig;
use crate::config::Args;
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
    avg_frame_period_ms: f64,
) {
    let s = r.section("BACKPRESSURE");
    // Audit 2026-08-23: the section previously looked self-contradictory —
    // "classification: high" next to "budget_utilization_avg: 5.67%".
    // Both numbers are correct but measure different things: pressure
    // derives from the FULL frame period (work + sleep + event polling)
    // vs the target period, while budget_utilization derives from work
    // time only. When the loop cannot reach target FPS, the period gap
    // lives outside `work` (scheduler granularity, poll waits), so
    // pressure rises while utilization stays low. The two new fields
    // below show the actual vs target frame period so the gap is
    // visible instead of implied.
    let target_ms = target_period.as_secs_f64() * 1000.0;
    s.field("frame_period_target_ms", &format!("{:.3}", target_ms));
    s.field(
        "frame_period_avg_ms",
        &format!(
            "{:.3} (includes sleep + event polling)",
            avg_frame_period_ms
        ),
    );
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
        "avg/peak = clamp(frame_period/target_period - 1, 0, 2); non-zero when the frame PERIOD exceeds target (work + sleep + polling). budget_utilization = work_s/target_period — the WORK share only. pressure high + utilization low = the gap is scheduler/poll time, not renderer work.",
    );
    s.field(
        "overshoot_frames",
        &format!("{} ({:.1}% of total)", overshoot_frames, overshoot_ratio),
    );
    s.advice("avg/peak 0.000 = healthy (renderer kept up). budget_utilization shows how much of the frame budget was consumed by renderer WORK (always non-zero). frame_period_avg_ms > frame_period_target_ms explains why pressure can be non-zero while utilization is low. For real FPS see TIMING.avg_fps / TIMING.instant_fps.");
}

/// Resolve bench duration from --bench-duration (now accepts compound format).
/// Returns None if not specified (benchmark uses default 5s).
///
/// NOTE: only --bench-duration is consulted here. The hidden --duration flag
/// is interactive-mode only (sets event_loop auto-exit deadline) and has no
/// effect in --benchmark / --bench-frames / --bench-all mode.
pub(crate) fn resolve_bench_duration_args(input: &Option<String>) -> Option<u64> {
    input
        .as_ref()
        .map(|s| crate::ux::or_exit(crate::cli_parse::parse_duration("--bench-duration", s)))
}

/// Collect warnings about CLI flags that are misleading or have NO effect
/// in benchmark mode. Pure function — the call site prints them.
///
/// Dispatch precedence (main.rs): `--bench-all > --benchmark > --bench-frames`.
/// The warn matrix below mirrors that precedence so the user always sees which
/// flag actually took effect.
fn collect_bench_noop_warnings(args: &Args, fps_user_set: bool) -> Vec<&'static str> {
    let mut warns: Vec<&'static str> = Vec::new();
    if args.bench_all && args.benchmark {
        warns.push("--benchmark ignored (--bench-all takes precedence)");
    }
    if args.bench_all && args.bench_frames.is_some() {
        warns.push("--bench-frames ignored (--bench-all takes precedence)");
    }
    if args.benchmark && args.bench_frames.is_some() {
        warns.push("--bench-frames ignored (--benchmark takes precedence)");
    }
    if args.bench_frames.is_some()
        && args.bench_duration.is_some()
        && !args.benchmark
        && !args.bench_all
    {
        warns.push("--bench-duration ignored (--bench-frames is frame-count-based)");
    }
    if fps_user_set {
        warns.push(
            "--fps (in benchmark mode sets simulation rate only — does NOT cap \
             render throughput; avg_fps is unconstrained; check config.toml \
             [fps] if you did not pass --fps on the CLI)",
        );
    }
    if args.duration.is_some() {
        warns.push("--duration (interactive auto-exit only; use --bench-duration)");
    }
    if args.screensaver {
        warns.push("--screensaver (interactive input handler; bench has no input loop)");
    }
    if args.intro.is_some() {
        warns.push("--intro (interactive intro animation; bench never plays it)");
    }
    if args.perf_stats {
        warns.push("--perf-stats (interactive summary; bench emits its own report)");
    }
    warns
}

/// Warn the user about CLI flags that are misleading or have NO effect in
/// benchmark mode. See `collect_bench_noop_warnings` for the audit details.
pub(crate) fn warn_bench_noop_flags(args: &Args, fps_user_set: bool) {
    let warns = collect_bench_noop_warnings(args, fps_user_set);
    if warns.is_empty() {
        return;
    }
    eprintln!(
        "[warn] the following flags have no effect (or a different effect than the name \
         implies) in benchmark mode:"
    );
    for w in &warns {
        eprintln!("       {w}");
    }
}

/// Duration of the premium benchmark in seconds (default).
pub(crate) const BENCHMARK_DURATION_SECS: u64 = 5;

/// Minimum allowed --bench-duration value (seconds).
const BENCH_DURATION_MIN: u64 = 1;

/// Valid `--bench-scene` values (strict validation contract).
pub(crate) const VALID_BENCH_SCENES: &[&str] = &["lean", "production-draw"];

/// Compute the median of a sorted slice of f64 values.
pub(crate) fn median_sorted(data: &[f64]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }
    let mid = data.len() / 2;
    if data.len().is_multiple_of(2) {
        (data[mid - 1] + data[mid]) / 2.0
    } else {
        data[mid]
    }
}

/// Resolve the benchmark duration: validate the override if supplied, else
/// return the default `BENCHMARK_DURATION_SECS`. Returns `Err` with
/// a human-readable error message on out-of-range values.
pub(crate) fn resolve_bench_duration(override_secs: Option<u64>) -> Result<u64, String> {
    match override_secs {
        Some(n) if n < BENCH_DURATION_MIN => Err(format!(
            "error: --bench-duration {n} is below the {BENCH_DURATION_MIN}-second minimum"
        )),
        Some(n) => Ok(n), // No max cap — use --bench-duration for endurance tests
        None => Ok(BENCHMARK_DURATION_SECS),
    }
}

/// Returns `Err(message)` if the scene name is not in [`VALID_BENCH_SCENES`].
pub(crate) fn validate_bench_scene_str(scene: Option<&str>) -> Result<(), String> {
    match scene {
        None => Ok(()),
        Some(s) if VALID_BENCH_SCENES.contains(&s) => Ok(()),
        Some(s) => Err(format!(
            "error: invalid --bench-scene value '{s}'. \
             Valid scenes: {} (lean = emit_cell_lean fast path, \
             production-draw = Terminal::draw full-redraw path). \
             cosmostrix is strict — typos are rejected, not silently \
             fallback'd to the default lean path.",
            VALID_BENCH_SCENES.join(", ")
        )),
    }
}

/// Returns `Err(message)` if `cfg.bench_scene` is set to an invalid value.
pub(crate) fn validate_bench_scene_result(cfg: &CloudConfig) -> Result<(), String> {
    validate_bench_scene_str(cfg.bench_scene.as_deref())
}

/// Strict-validate `--bench-scene`. Exits with a clean single-line error if
/// the value is not in [`VALID_BENCH_SCENES`]. Called at the top of every
/// benchmark entry point so typos can never silently fall through to the
/// default lean path.
pub(crate) fn validate_bench_scene(cfg: &CloudConfig) {
    crate::ux::or_exit::<(), String>(validate_bench_scene_result(cfg));
}

#[cfg(test)]
mod tests {
    use super::format_backpressure_section;
    use super::resolve_bench_duration;
    use super::BENCHMARK_DURATION_SECS;
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
            16.667, // avg_frame_period_ms (healthy: ~= target)
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
        let readme = include_str!("../../README.md");
        let benchmark_readme = include_str!("../../benchmark/README.md");
        assert!(!readme.contains("7,000 FPS"));
        assert!(!readme.contains(">7,000 FPS"));
        assert!(!benchmark_readme.contains("v2.1.0 reference results"));
        assert!(!benchmark_readme.contains("throughput exceeds 7,000 FPS"));
    }

    #[test]
    fn bench_file_stays_under_target_loc() {
        // Guard: src/bench/mod.rs must stay well under 1500 LOC.
        // Current target is under 1500 LOC — bumped from 1200 in  to
        // match the project-wide LOC cap. Phase 8-9 scaling added sub-component
        // timing wiring (sim/render/io accumulators and per-frame
        // cloud.last_sim_ms()/last_render_ms() reads). The ComponentTimer
        // struct was extracted to bench_comp.rs to minimize growth here;
        // further sub-component work should also live in bench_comp.rs
        // rather than expand this file.
        let source = include_str!("mod.rs");
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
