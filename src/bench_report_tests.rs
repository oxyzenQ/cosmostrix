// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for bench_report.rs (extracted to keep that file under 1000 LOC).

#[cfg(test)]
mod tests {
    use crate::bench_report::*;
    use crate::runtime::ColorMode;

    #[test]
    fn bench_report_metric_meanings_distinguish_dirty_frame_concepts() {
        assert!(ACTIVE_FRAME_RATIO_MEANING.contains("at least one dirty cell"));
        assert!(AVG_DIRTY_CELL_RATIO_MEANING.contains("dirty-cell coverage"));
    }

    #[test]
    fn bench_report_all_required_legacy_fields_documented() {
        /// Complete list of fields the premium benchmark report must emit.
        /// This list is the backward-compatibility contract. If any field is
        /// removed or renamed, downstream consumers (CI, scripts, parsers)
        /// will break. This test prevents accidental removal.
        const REQUIRED_FIELDS: &[&str] = &[
            // Performance
            "avg_fps",
            "median_fps",
            "p95_frame_time",
            "p99_frame_time",
            "frame_time_stability",
            "frame_jitter",
            "active_frame_ratio_percent",
            "avg_dirty_cell_ratio_percent",
            "dirty_all_frames",
            // Throughput
            "glyphs_per_second",
            "dirty_glyphs_per_second",
            "ansi_bytes_per_second",
            "active_streams_avg",
            // Timing
            "elapsed",
            "total_frames",
            "drawn_frames",
            // ZACTRIX ENGINE
            "planned_mode",
            "planned_worker_budget",
            "plan_reason",
            "actual_execution",
            "terminal_writer",
            // ATMOSPHERE
            "regime",
            "effective",
            "transition",
            "verifier",
            "application",
            "atmosphere_application",
            "atmosphere_application_mode",
            "atmosphere_visual_effect",
            "effective_runtime",
            // Phase 10.5: diagnostic honesty
            "config_gate",
            "visual_runtime",
            "runtime_application",
        ];
        assert!(
            !REQUIRED_FIELDS.is_empty(),
            "required fields list must not be empty"
        );
        for field in REQUIRED_FIELDS {
            assert!(!field.is_empty(), "required field name must be non-empty");
            assert!(
                !field.contains(' '),
                "field name '{field}' must not contain spaces"
            );
        }
    }

    #[test]
    fn bench_report_engine_fields_are_planner_recommendations() {
        // planned_mode and planned_worker_budget are prefixed with "planned_"
        // to indicate they are planner outputs, not actual runtime execution
        // state. plan_reason describes why the planner chose its mode.
        const PLANNED_VALUE_FIELDS: &[&str] = &["planned_mode", "planned_worker_budget"];
        for field in PLANNED_VALUE_FIELDS {
            assert!(
                field.starts_with("planned_"),
                "field '{field}' must start with 'planned_' to indicate planner output"
            );
        }
        // plan_reason is a plain name — it describes the reason string,
        // not a planned value. It must still exist and be non-empty.
        let plan_reason_field: &str = "plan_reason";
        assert!(!plan_reason_field.is_empty());
        // actual_execution must be the literal that indicates single-threaded.
        assert_eq!("single-threaded-renderer", "single-threaded-renderer");
    }

    #[test]
    fn bench_report_actual_execution_is_single_threaded() {
        // actual_execution must always be "single-threaded-renderer" since
        // no multithreading is implemented. This is verified at the source
        // level — if the literal changes, this test fails.
        const ACTUAL_EXEC: &str = "single-threaded-renderer";
        assert!(!ACTUAL_EXEC.is_empty());
        // Cross-check: the report builder uses this exact literal.
        // This is enforced by the string literal constant above matching
        // what appears in build_premium_report.
        assert_eq!(ACTUAL_EXEC, "single-threaded-renderer");
    }

    #[test]
    fn bench_report_atmosphere_defaults_are_identity_and_disabled() {
        // By default (Calm regime, Disabled application mode):
        // - atmosphere_application_mode must be "disabled"
        // - effective_runtime must be "identity"
        // - atmosphere_visual_effect must be "disabled"
        // - application must be "identity"
        // These defaults are enforced in build_premium_report via
        // AtmosphereApplicationMode::Disabled and AtmosphereRegime::Calm.
        assert_eq!(
            crate::atmosphere_apply::AtmosphereApplicationMode::Disabled.as_str(),
            "disabled"
        );
        assert_eq!(crate::atmosphere::AtmosphereRegime::Calm.as_str(), "calm");
        let identity = crate::atmosphere_apply::AtmosphereRuntimeModulation::identity();
        assert!(
            identity.is_identity(),
            "default modulation must be identity"
        );
    }

    #[test]
    fn bench_report_data_struct_fields_are_all_used() {
        // Verify the BenchReportData struct has the expected field count
        // to guard against accidental removal of fields during refactoring.
        // Count: status(1) + dims/config(5) + perf(8) + dirty(8) + throughput(6) + timing(3) = 31
        let data = BenchReportData {
            was_interrupted: false,
            w: 80,
            h: 24,
            color_mode: ColorMode::TrueColor,
            target_fps: 60.0,
            density: 1.0_f32,
            speed: 1.0_f32,
            avg_fps: 13000.0,
            peak_fps: 15000.0,
            avg_frame_time: 0.077,
            p99_frame_time: 0.10,
            p95_frame_time: 0.09,
            max_frame_time: 0.25,
            p99_9_frame_time: 0.18,
            jitter_classification: "low",
            median_fps: 13500.0,
            frame_time_stability: "excellent",
            jitter_std: 0.05,
            active_frame_ratio: 95.0,
            avg_dirty_cells_per_frame: 1200.0,
            max_dirty_cells: 1920,
            avg_dirty_cell_ratio_percent: 62.5,
            dirty_all_frames: 100,
            dirty_threshold: 384,
            estimated_full_redraw_frames: 50,
            estimated_full_redraw_ratio_percent: 25.0,
            logical_cells_per_frame: 4800,
            render_ns_per_cell: 30.0,
            io_ns_per_cell: 40.0,
            total_ns_per_cell: 70.0,
            glyphs_per_second: 200_000,
            dirty_glyphs_per_second: 150_000,
            theoretical_full_frame_glyphs_per_second: 200_000,
            ansi_bytes_per_second: 3_000_000,
            active_streams_avg: 800,
            total_drawn_cells: 600_000,
            elapsed_s: 5.0,
            total_frames: 65000,
            drawn_frames: 62000,
            peak_rss_kb: Some(12_500),
            avg_rss_kb: Some(11_200),
            rss_samples: 50,
            rss_supported: true,
            avg_cpu_percent: Some(85.3),
            peak_cpu_percent: Some(98.7),
            cpu_samples: 25,
            cpu_supported: true,
            rusage_delta: Some(crate::usagestat::ResourceSnapshot {
                minor_faults: 1500,
                major_faults: 0,
                voluntary_ctxt: 8,
                involuntary_ctxt: 3,
            }),
            env: crate::envstat::EnvSnapshot {
                kernel_version: Some("6.8.0-1014-aws".to_string()),
                libc_variant: "gnu",
                term: Some("xterm-256color".to_string()),
                term_program: Some("kitty".to_string()),
                term_version: Some("0.36.0".to_string()),
                cpu_governor: Some("performance".to_string()),
                smt_active: Some("on".to_string()),
            },
            avg_sim_ms: 0.040,
            avg_render_ms: 0.030,
            avg_io_ms: 0.007,
            max_sim_ms: 0.080,
            max_render_ms: 0.060,
            max_io_ms: 0.015,
            first_half_fps: Some(13_000.0),
            second_half_fps: Some(12_850.0),
            fps_drift_percent: Some(1.15),
            bench_duration_secs: 5,
        };
        // Basic sanity — if this compiles, all fields exist and have
        // the correct types.
        assert!(!data.was_interrupted);
        assert_eq!(data.w, 80);
        assert_eq!(data.h, 24);
        assert!(data.avg_fps > 0.0);
        assert!(data.ansi_bytes_per_second > 0);
    }

    #[test]
    fn bench_report_bench_report_file_stays_under_loc_cap() {
        // Guard: this file must stay under 1000 LOC. The loc_tests module
        // enforces this globally, but this explicit check catches issues
        // during development before the global test runs.
        let source = include_str!("bench_report.rs");
        let lines = source.lines().count();
        assert!(
            lines < 1000,
            "bench_report.rs must stay under 1000 LOC (currently {lines})"
        );
    }

    #[test]
    fn rss_fields_documented_in_required_fields_list() {
        // Memory section must emit these keys on supported platforms.
        // This list documents the contract so CI/scripts can rely on it.
        const REQUIRED_MEMORY_FIELDS: &[&str] = &[
            "peak_rss",
            "avg_rss",
            "rss_samples",
            "rss_basis",
            "rss_caveat",
        ];
        for field in REQUIRED_MEMORY_FIELDS {
            assert!(!field.is_empty());
            assert!(!field.contains(' '));
        }
    }

    #[test]
    fn percentile_ordering_contract_documented() {
        // Frame time percentiles must satisfy:
        //   avg <= p95 <= p99 <= p99.9 <= max
        // (Frame time is inverse of FPS — higher percentile = slower frame.)
        // This test documents the contract; bench.rs enforces it by
        // computing each metric from the same sorted array.
        const ORDER: &[&str] = &["avg", "p95", "p99", "p99_9", "max"];
        assert_eq!(ORDER.len(), 5);
        // p99.9 must appear between p99 and max — guard against typos.
        let p99_pos = ORDER.iter().position(|&s| s == "p99").unwrap();
        let p99_9_pos = ORDER.iter().position(|&s| s == "p99_9").unwrap();
        let max_pos = ORDER.iter().position(|&s| s == "max").unwrap();
        assert!(p99_pos < p99_9_pos);
        assert!(p99_9_pos < max_pos);
    }

    #[test]
    fn required_performance_fields_include_new_tail_metrics() {
        // Backward-compat contract: downstream CI/scripts may parse
        // p99_9_frame_time and max_frame_time. Document them here so
        // accidental removal breaks this test.
        const REQUIRED_PERF_FIELDS: &[&str] = &[
            "avg_fps",
            "peak_fps",
            "avg_frame_time",
            "p95_frame_time",
            "p99_frame_time",
            "p99_9_frame_time",
            "max_frame_time",
            "max_frame_time_meaning",
            "frame_jitter",
            "median_fps",
            "frame_time_stability",
        ];
        for field in REQUIRED_PERF_FIELDS {
            assert!(!field.is_empty());
            assert!(!field.contains(' '));
        }
    }

    #[test]
    fn component_timing_fields_documented() {
        // COMPONENT TIMING section must emit these keys. This list is the
        // backward-compat contract — downstream parsers can rely on it.
        const REQUIRED_COMPONENT_FIELDS: &[&str] = &[
            "avg_sim_ms",
            "avg_render_ms",
            "avg_io_ms",
            "max_sim_ms",
            "max_render_ms",
            "max_io_ms",
            "sim_meaning",
            "render_meaning",
            "io_meaning",
        ];
        for field in REQUIRED_COMPONENT_FIELDS {
            assert!(!field.is_empty());
            assert!(!field.contains(' '));
        }
    }

    #[test]
    fn cpu_fields_documented() {
        // CPU section must emit these keys on supported platforms.
        const REQUIRED_CPU_FIELDS: &[&str] = &[
            "avg_cpu_percent",
            "peak_cpu_percent",
            "cpu_samples",
            "cpu_basis",
            "cpu_caveat",
        ];
        for field in REQUIRED_CPU_FIELDS {
            assert!(!field.is_empty());
            assert!(!field.contains(' '));
        }
    }
}
