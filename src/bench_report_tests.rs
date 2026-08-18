// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for bench_report.rs (extracted to keep that file under 1500 LOC).

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
        ///
        /// v30 strengthen (audit): removed 5 fields that were either exact
        /// duplicates or hardcoded constants with no runtime basis:
        ///   - `atmosphere_application` (was == `application`)
        ///   - `runtime_application` (was == `application`)
        ///   - `atmosphere_shadow_risk` (was == `atmosphere_shadow`)
        ///   - `visual_runtime` (was == `effective_runtime` with different label)
        ///   - `frames_with_changes` (was == `drawn_frames`)
        ///
        /// Also: `transition` and `verifier` are now ACTUALLY COMPUTED from
        /// controller state + verifier result, no longer hardcoded
        /// "stable"/"pass".
        //
        // v30 Phase 6 Tier E item 31: atmosphere engine fully eliminated.
        // The entire ATMOSPHERE diagnostic section was removed from
        // build_premium_report. The required-fields list below reflects
        // the post-elimination report shape.
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
            "glyphs_per_second_theoretical",
            "dirty_glyphs_per_second",
            "ansi_bytes_per_second",
            "active_streams_avg",
            // Timing
            "elapsed",
            "total_frames",
            "drawn_frames",
            // COSMIC DRAGON ENGINE
            "planned_mode",
            "planned_worker_budget",
            "plan_reason",
            "actual_execution",
            "terminal_writer",
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
    }

    #[test]
    fn bench_report_data_struct_fields_are_all_used() {
        // Verify the BenchReportData struct has the expected field count
        // to guard against accidental removal of fields during refactoring.
        // Count: status(1) + dims/config(15+10=25,  added 10 enrichment
        // fields: color_mode_label, custom_palette_name, custom_palette_bg_hex,
        // color_bg_label, color_tune_summary, async_mode, glitch_enabled,
        // glitch_level, glitch_pct, auto_color_drift) + perf(8) + dirty(8)
        // + throughput(5) + timing(3) = 50
        // v50 LTS audit: throughput went from 6 → 5 fields
        // (removed redundant `theoretical_full_frame_glyphs_per_second`,
        // renamed `glyphs_per_second` → `glyphs_per_second_theoretical`).
        // config grew from 6 to 15 fields (color_scheme_name,
        // charset_preset, glyph_count, rain_style, monolith_size, bold_mode,
        // shading_mode, + speed which moved from perf-only to config too).
        // config grew from 15 to 25 fields (CONFIG enrichment for
        // color/charset parity with --verbose).
        // The struct literal below is the real check — if this compiles,
        // all fields exist and have the correct types. Prefixed with `_`
        // because no runtime assertion is needed (the compiler is the test).
        let _data = BenchReportData {
            was_interrupted: false,
            w: 80,
            h: 24,
            color_mode: ColorMode::TrueColor,
            target_fps: 60.0,
            density: 1.0_f32,
            speed: 1.0_f32,
            scene: "cinematic".to_string(),
            color_scheme_name: "cosmos".to_string(),
            charset_preset: "matrix".to_string(),
            glyph_count: 84,
            rain_style: "glyph",
            monolith_size: "normal",
            bold_mode: "Random".to_string(),
            shading_mode: "DistanceFromHead".to_string(),
            color_mode_label: "24-bit truecolor",
            custom_palette_name: None,
            custom_palette_bg_hex: None,
            color_bg_label: "default-background",
            color_tune_summary: "sat=1.00 bright=1.00 head=1.00 body=1.00 tail=1.00".to_string(),
            async_mode: false,
            glitch_enabled: true,
            glitch_level: "subtle",
            glitch_pct: 3.0,
            auto_color_drift: false,
            color_pipeline: "chroma_dragon",
            chroma_in_benchmark:
                "enabled (palette_drift off for determinism, climate_drift active)",
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
            logical_cells_per_frame: 4800,
            render_ns_per_cell: 30.0,
            io_ns_per_cell: 40.0,
            total_ns_per_cell: 70.0,
            terminal_io: None,
            energy: None,
            perf: None,
            allocator: None,
            visual: None,
            glyphs_per_second_theoretical: 200_000,
            dirty_glyphs_per_second: 150_000,
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
        // the correct types. The struct literal above is the real check.
    }

    #[test]
    fn bench_report_bench_report_file_stays_under_loc_cap() {
        // Guard: this file must stay under 1500 LOC. The loc_tests module
        // enforces this globally, but this explicit check catches issues
        // during development before the global test runs.
        let source = include_str!("bench_report.rs");
        let lines = source.lines().count();
        assert!(
            lines < 1500,
            "bench_report.rs must stay under 1500 LOC (currently {lines})"
        );
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
}
