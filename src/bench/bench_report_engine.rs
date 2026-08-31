// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Benchmark report: engine + terminal + energy + microarch +
//! allocator + visual objective sections — extracted from
//! `bench_report.rs` to keep that file under the 800-LOC cap.

use crate::bench_report::BenchReportData;
use crate::bench_report::{STABILITY_DIRTY_RATIO_MAX, STABILITY_JITTER_STD_MAX};
use crate::constants::DIRTY_THRESHOLD_RATIO;
use crate::humanize::{humanize, humanize_throughput};
use crate::report::Report;
use crate::runtime::ColorMode;

/// Build the engine diagnostics + terminal I/O + energy +
/// microarchitecture + allocator + visual objective sections.
pub(crate) fn build_engine_sections(r: &mut Report, data: &BenchReportData) {
    // ── Engine diagnostics ─────────────────────────────────────────────
    // cosmostrix is single-thread by design — terminal writer is single-owner.
    //
    // v30 strengthen (audit): removed the legacy RUNTIME section
    // (runtime_mode/render_plan/idle_policy/architecture). Its four fields
    // were all hardcoded string constants with no runtime basis — they
    // described a fictional "runtime mode" that doesn't exist, and
    // duplicated concepts already covered by the ENGINE section below
    // (render_plan="single-owner" == ENGINE.terminal_writer="single-owner").
    // Keeping them would violate the honesty contract: the report must
    // reflect actual state, not feel-good constants.
    {
        let s = r.section("ENGINE");
        s.field("planned_mode", "single-core");
        s.field("planned_worker_budget", "0");
        s.field(
            "plan_reason",
            "single-thread renderer — cosmostrix optimized for single-core execution",
        );
        s.field("actual_execution", "single-threaded-renderer");
        s.field("terminal_writer", "single-owner");
    }

    if data.color_mode == ColorMode::Color16
        && data.avg_dirty_cell_ratio_percent >= (100.0 / DIRTY_THRESHOLD_RATIO as f64)
    {
        r.section("NOTES")
            .advice(
                "16-color mode with foreground palette retinting can dirty many colored cells.",
            )
            .advice(
                "Compare runs with --color-mode 0, --color-mode 256, or a truecolor-capable terminal.",
            );
    }

    if data.avg_dirty_cell_ratio_percent < STABILITY_DIRTY_RATIO_MAX
        && data.jitter_std < STABILITY_JITTER_STD_MAX
    {
        r.section("STABILITY NOTES")
            .advice("Frame time stability is good (std < 0.5ms).")
            .advice("avg FPS alone is not enough; always check p99/p95 frame times.")
            .advice("dirty-cell ratio < 5% indicates efficient differential rendering.")
            .advice("p95 frame time < 2x avg frame time confirms throughput stability.");
    }

    // ── Phase 2: Terminal I/O (wet) ──────────────────────────────────
    {
        let s = r.section("TERMINAL I/O");
        match &data.terminal_io {
            Some(io) if io.enabled => {
                s.field("status", "enabled (wet)");
                s.field("target", &io.target);
                s.field(
                    "write_bandwidth",
                    &humanize_throughput(io.bytes_written, io.elapsed_secs),
                );
                s.field(
                    "avg_write_latency",
                    &format!("{:.2} µs", io.avg_latency_us()),
                );
                s.field("backpressure_events", &io.backpressure_events.to_string());
                s.field(
                    "effective_write_fps",
                    &crate::humanize::humanize_f64(io.effective_write_fps()),
                );
                s.field(
                    "total_bytes_written",
                    &crate::humanize::humanize_bytes(io.bytes_written),
                );
            }
            _ => {
                s.field("status", "dry (use --bench-io for wet mode)");
            }
        }
    }

    // ── Phase 3: ENERGY (RAPL, Linux only) ───────────────────────────
    {
        let s = r.section("ENERGY");
        match &data.energy {
            Some(e) if e.available => {
                s.field("status", "available (RAPL)");
                s.field("packages", &e.package_count.to_string());
                s.field("total_energy", &format!("{:.2} J", e.total_energy_joules));
                s.field("avg_power", &format!("{:.2} W", e.avg_power_watts));
                s.field(
                    "energy_per_frame",
                    &format!("{:.2} µJ", e.energy_per_frame_uj),
                );
                s.field(
                    "energy_per_cell",
                    &format!("{:.2} nJ", e.energy_per_cell_nj),
                );
            }
            _ => {
                s.field("status", "not available (RAPL requires Linux + powercap)");
                s.field(
                    "hint",
                    "See docs/BENCHMARK_ADVANCED.md for setup instructions",
                );
            }
        }
    }

    // ── Phase 4: MICROARCHITECTURE (perf counters, Linux x86 only) ───
    {
        let s = r.section("MICROARCHITECTURE");
        match &data.perf {
            Some(p) if p.available => {
                s.field("status", "available (perf_event_open)");
                s.field("cycles", &crate::humanize::humanize(p.cycles));
                s.field("instructions", &crate::humanize::humanize(p.instructions));
                s.field("ipc", &format!("{:.2}", p.instructions_per_cycle));
                s.field(
                    "branch_instructions",
                    &crate::humanize::humanize(p.branch_instructions),
                );
                s.field("branch_misses", &crate::humanize::humanize(p.branch_misses));
                s.field(
                    "branch_mispredict_rate",
                    &format!("{:.2}%", p.branch_mispredict_rate),
                );
                s.field("note", "Linux x86 perf counters; varies by CPU model");
            }
            _ => {
                s.field("status", "not available (perf counters require Linux x86)");
                s.field(
                    "hint",
                    "See docs/BENCHMARK_ADVANCED.md for setup instructions",
                );
            }
        }
    }

    // ── Phase 5: ALLOCATOR ────────────────────────────────────────────
    {
        let s = r.section("ALLOCATOR");
        match &data.allocator {
            Some(a) => {
                s.field("alloc_calls", &crate::humanize::humanize(a.alloc_calls));
                s.field("dealloc_calls", &crate::humanize::humanize(a.dealloc_calls));
                s.field("realloc_calls", &crate::humanize::humanize(a.realloc_calls));
                s.field(
                    "alloc_calls_per_frame",
                    &format!("{:.1}", a.alloc_calls_per_frame),
                );
                s.field(
                    "dealloc_calls_per_frame",
                    &format!("{:.1}", a.dealloc_calls_per_frame),
                );
                s.field(
                    "bytes_allocated",
                    &crate::humanize::humanize_bytes(a.bytes_allocated_total),
                );
                s.field(
                    "bytes_deallocated",
                    &crate::humanize::humanize_bytes(a.bytes_deallocated_total),
                );
                s.field(
                    "heap_retained",
                    &crate::humanize::humanize_bytes(a.heap_retained_bytes),
                );
                if a.heap_virtual_kib > 0 {
                    s.field(
                        "heap_virtual",
                        &crate::humanize::humanize_bytes(a.heap_virtual_kib.saturating_mul(1024)),
                    );
                }
            }
            None => {
                s.field("status", "not measured");
            }
        }
    }

    // ── Phase 6: VISUAL OBJECTIVE METRICS ────────────────────────────
    {
        let s = r.section("VISUAL OBJECTIVE");
        match &data.visual {
            Some(v) => {
                s.field(
                    "frame_entropy_bits",
                    &format!("{:.2}", v.frame_entropy_bits),
                );
                s.field("density_gini", &format!("{:.4}", v.density_gini));
                s.field(
                    "color_transition_delta",
                    &format!("{:.2}", v.color_transition_delta_avg),
                );
                s.field(
                    "samples",
                    &format!("{} ({})", humanize(v.samples.into()), v.samples),
                );
                s.field(
                    "entropy_meaning",
                    "Shannon entropy of dirty-cell column distribution; higher = more uniform",
                );
                s.field(
                    "gini_meaning",
                    "0 = perfectly uniform density, 1 = maximally concentrated",
                );
            }
            None => {
                s.field("status", "not measured");
            }
        }
    }

    // Final report goes to stdout — clean, pipeable.
    r.print();
}
