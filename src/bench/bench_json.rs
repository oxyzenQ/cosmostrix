// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Manual JSON serializer for the benchmark report (zero dependencies).
//!
//! Outputs the same data as the text report (`bench_report.rs`) but as a
//! single-line JSON object. Designed for CI/scripts that want to parse
//! benchmark results programmatically without grepping text.
//!
//! ## Why manual (not serde)?
//!
//! Adding `serde` + `serde_json` would pull in ~15 transitive crates just
//! for this one feature. The BenchReportData struct is flat (no nested
//! enums, no recursive types), so a manual serializer is ~150 LOC and
//! keeps the zero-deps promise.
//!
//! ## Output shape
//!
//! ```json
//! {"status":{"was_interrupted":false},"system":{...},"renderer":{...},
//!  "config":{...},"environment":{...},"performance":{...},...}
//! ```
//!
//! All values are JSON primitives (string/number/bool/null). Option<T>
//! fields emit `null` when None. The structure mirrors the text report's
//! sections so a reader familiar with one can navigate the other.
//!
//! ## Schema stability contract (LTS)
//!
//! The JSON schema follows a long-term-stability policy so that
//! downstream consumers (CI scripts, regression dashboards, the
//! `--compare-baseline` workflow) do not break across releases.
//!
//! ### Additive-only changes
//!
//! New fields MAY be added to any section in any release. Consumers MUST
//! ignore unknown keys (per the JSON spec) — the flat-parser in
//! `bench_baseline::parse_json_flat` already does this.
//!
//! ### Renames and removals
//!
//! Existing field names MUST NOT be removed or renamed without a
//! deprecation cycle:
//!   1. The new name is added alongside the old name (both emit the
//!      same value) for at least one minor release.
//!   2. The old name is documented as a deprecated alias in this file
//!      and in `docs/BENCHMARK_ADVANCED.md`.
//!   3. The old name is removed only after CI has run green for one
//!      full release cycle with both names present.
//!
//! Example: `bandwidth_mbps` was the original name (misleading — the
//! unit is MiB/s, not MB/s). `bandwidth_mibps` was added as the
//! corrected name and both fields emitted the same value for the
//! v50 LTS audit cycle; the deprecated `bandwidth_mbps` alias was
//! then removed under explicit owner approval (staying within v50 —
//! the normal one-full-release-cycle rule was waived by the owner
//! for this alias because the only in-tree consumers were updated
//! in the same change and no external consumers were known).
//!
//! ### Type stability
//!
//! The JSON type of an existing field MUST NOT change. A field that
//! emits a number MUST continue to emit a number (or null when
//! unavailable); a field that emits a string MUST continue to emit a
//! string. If a different representation is needed, add a new field
//! (e.g. `bytes_written` is u64, `bytes_written_human` is the
//! human-readable string form) rather than retyping the existing one.
//!
//! ### Section presence
//!
//! All 19 top-level sections (`status`, `system`, `renderer`, `config`,
//! `environment`, `performance`, `memory`, `cpu`, `resource`,
//! `component_timing`, `cell_efficiency`, `drift`, `throughput`,
//! `timing`, `terminal_io`, `energy`, `microarchitecture`,
//! `allocator`, `visual_objective`) are stable. A section will not be
//! removed without a major version bump.
//!
//! ### Optional-section presence flag
//!
//! Sections that wrap platform-optional subsystems (`energy`,
//! `microarchitecture`, `allocator`, `visual_objective`) emit an
//! `available: true/false` field so consumers can detect presence
//! without inspecting individual sub-fields. `terminal_io` uses
//! `enabled: true/false` (semantic: the feature was enabled for this
//! run) — the different name reflects the different meaning (user
//! choice vs. platform support).
//!
//! ### NaN / Infinity
//!
//! JSON spec forbids `NaN` and `Infinity` literals. The serializer
//! emits `null` for any non-finite `f64` value (both the bare
//! `JsonValue for f64` impl and the `push_kv_opt(Option<f64>)` helper
//! finite-check before emitting). Consumers reading numeric fields
//! MUST handle `null` as "value unavailable or non-finite".

use crate::bench_meta::{cpu_model_label, format_rss_kb};
use crate::bench_report::BenchReportData;

/// Build the JSON report and print it to stdout as a single line.
pub(crate) fn build_json_string(data: &BenchReportData) -> String {
    let mut out = String::with_capacity(4096);
    out.push('{');

    // ── status ──
    json_object(&mut out, "status", |o| {
        o.push_kv("was_interrupted", data.was_interrupted);
    });

    // ── system ──
    let cpu = crate::diagnostics::detect_cpu_info();
    json_object(&mut out, "system", |o| {
        o.push_kv_str("variant", cpu.variant);
        o.push_kv_str("optimization", env!("COSMOSTRIX_OPTIMIZATION"));
        o.push_kv_str("build", cpu.build_variant);
        o.push_kv_str("rustc_version", env!("COSMOSTRIX_RUSTC_VERSION"));
        o.push_kv_str("git_sha", env!("COSMOSTRIX_GIT_SHA"));
        o.push_kv_str("cpu_baseline", env!("COSMOSTRIX_CPU_BASELINE"));
        o.push_kv_str("target_features", env!("COSMOSTRIX_TARGET_FEATURES"));
        o.push_kv_str("lto", env!("COSMOSTRIX_LTO"));
        o.push_kv_str("panic", env!("COSMOSTRIX_PANIC"));
        o.push_kv_str("strip", env!("COSMOSTRIX_STRIP"));
        o.push_kv_str("pgo", env!("COSMOSTRIX_PGO"));
        o.push_kv_str("cpu_model", &cpu_model_label());
    });

    // ── renderer ──
    let ri = crate::renderer_info::renderer_info(data.color_mode);
    json_object(&mut out, "renderer", |o| {
        o.push_kv_str("backend", ri.backend);
        o.push_kv_str("pacing", ri.pacing);
        o.push_kv_str("frame_strategy", ri.frame_strategy);
        o.push_kv_str("color_depth", ri.color_depth);
        o.push_kv_str("gpu_usage", "not_applicable");
        o.push_kv_str(
            "gpu_basis",
            "cosmostrix is a CPU + stdout renderer; no GPU context is ever created",
        );
    });

    // ── config ──
    json_object(&mut out, "config", |o| {
        o.push_kv_str("scene", &data.scene);
        o.push_kv_str("color_scheme", &data.color_scheme_name);
        o.push_kv_str("charset", &data.charset_preset);
        o.push_kv("glyph_count", data.glyph_count as u64);
        o.push_kv_str("rain_style", data.rain_style);
        o.push_kv_str("monolith_size", data.monolith_size);
        o.push_kv_str("bold", &data.bold_mode);
        o.push_kv_str("shading", &data.shading_mode);
        o.push_kv("cols", data.w);
        o.push_kv("lines", data.h);
        o.push_kv("target_fps", data.target_fps);
        o.push_kv("speed", data.speed);
        o.push_kv("density", data.density);
        // v30 strengthen (Bug #3): expose glitch_enabled
        // + color_tune_summary in JSON output so CI/scripts can verify the
        // exact glitch / tune state of a benchmark run. Previously
        // these were only in the text report — JSON consumers had to grep
        // the text output or guess.
        o.push_kv("glitch_enabled", data.glitch_enabled);
        o.push_kv_str("glitch_level", data.glitch_level);
        o.push_kv("glitch_pct", data.glitch_pct);
        o.push_kv_str("color_tune", &data.color_tune_summary);
        o.push_kv("async_mode", data.async_mode);
    });

    // ── environment ──
    json_object(&mut out, "environment", |o| {
        o.push_kv_opt_str("kernel_version", data.env.kernel_version.as_deref());
        o.push_kv_str("libc_variant", data.env.libc_variant);
        o.push_kv_opt_str("term", data.env.term.as_deref());
        o.push_kv_opt_str("term_program", data.env.term_program.as_deref());
        o.push_kv_opt_str("term_version", data.env.term_version.as_deref());
        o.push_kv_opt_str("cpu_governor", data.env.cpu_governor.as_deref());
        o.push_kv_opt_str("smt_active", data.env.smt_active.as_deref());
    });

    // ── performance ──
    json_object(&mut out, "performance", |o| {
        o.push_kv("avg_fps", data.avg_fps);
        o.push_kv("peak_fps", data.peak_fps);
        o.push_kv_str(
            "avg_fps_human",
            &crate::humanize::humanize_f64(data.avg_fps),
        );
        o.push_kv_str(
            "peak_fps_human",
            &crate::humanize::humanize_f64(data.peak_fps),
        );
        o.push_kv("avg_frame_time_ms", data.avg_frame_time);
        o.push_kv("p95_frame_time_ms", data.p95_frame_time);
        o.push_kv("p99_frame_time_ms", data.p99_frame_time);
        o.push_kv("p99_9_frame_time_ms", data.p99_9_frame_time);
        o.push_kv("max_frame_time_ms", data.max_frame_time);
        o.push_kv_str("frame_jitter", data.jitter_classification);
        o.push_kv("median_fps", data.median_fps);
        o.push_kv_str("frame_time_stability", data.frame_time_stability);
        o.push_kv("jitter_std_ms", data.jitter_std);
        o.push_kv("active_frame_ratio_percent", data.active_frame_ratio);
    });

    // ── memory ──
    json_object(&mut out, "memory", |o| {
        o.push_kv_opt_str("peak_rss", data.peak_rss_kb.map(format_rss_kb).as_deref());
        o.push_kv_opt_str("avg_rss", data.avg_rss_kb.map(format_rss_kb).as_deref());
        o.push_kv("rss_samples", data.rss_samples);
        o.push_kv("rss_supported", data.rss_supported);
    });

    // ── cpu ──
    json_object(&mut out, "cpu", |o| {
        o.push_kv_opt("avg_cpu_percent", data.avg_cpu_percent);
        o.push_kv_opt("peak_cpu_percent", data.peak_cpu_percent);
        o.push_kv("cpu_samples", data.cpu_samples);
        o.push_kv("cpu_supported", data.cpu_supported);
    });

    // ── resource ──
    json_object(&mut out, "resource", |o| {
        if let Some(d) = &data.rusage_delta {
            o.push_kv("minor_faults", d.minor_faults);
            o.push_kv("major_faults", d.major_faults);
            o.push_kv("voluntary_ctxt", d.voluntary_ctxt);
            o.push_kv("involuntary_ctxt", d.involuntary_ctxt);
        } else {
            o.push_kv_null("minor_faults");
            o.push_kv_null("major_faults");
            o.push_kv_null("voluntary_ctxt");
            o.push_kv_null("involuntary_ctxt");
        }
    });

    // ── component_timing ──
    json_object(&mut out, "component_timing", |o| {
        o.push_kv("avg_sim_ms", data.avg_sim_ms);
        o.push_kv("avg_render_ms", data.avg_render_ms);
        o.push_kv("avg_io_ms", data.avg_io_ms);
        o.push_kv("max_sim_ms", data.max_sim_ms);
        o.push_kv("max_render_ms", data.max_render_ms);
        o.push_kv("max_io_ms", data.max_io_ms);
    });

    // ── cell_efficiency (P3: DeepSeek metrics) ──
    json_object(&mut out, "cell_efficiency", |o| {
        o.push_kv("logical_cells_per_frame", data.logical_cells_per_frame);
        o.push_kv("dirty_cells_per_frame", data.avg_dirty_cells_per_frame);
        o.push_kv("render_ns_per_cell", data.render_ns_per_cell);
        o.push_kv("io_ns_per_cell", data.io_ns_per_cell);
        o.push_kv("total_ns_per_cell", data.total_ns_per_cell);
    });

    // ── drift ──
    json_object(&mut out, "drift", |o| {
        o.push_kv("bench_duration_secs", data.bench_duration_secs);
        o.push_kv_opt("first_half_fps", data.first_half_fps);
        o.push_kv_opt("second_half_fps", data.second_half_fps);
        o.push_kv_opt("fps_drift_percent", data.fps_drift_percent);
    });

    // ── throughput ──
    // v50 LTS audit (Issue 2): renamed `glyphs_per_second` →
    // `glyphs_per_second_theoretical` in the JSON output to match the
    // struct field. The old name implied actual throughput; the value
    // is the theoretical upper bound. JSON consumers reading the old
    // key must migrate to the new key. `dirty_glyphs_per_second`
    // (already present) is the actual rendered throughput.
    json_object(&mut out, "throughput", |o| {
        o.push_kv(
            "glyphs_per_second_theoretical",
            data.glyphs_per_second_theoretical,
        );
        o.push_kv("dirty_glyphs_per_second", data.dirty_glyphs_per_second);
        o.push_kv("ansi_bytes_per_second", data.ansi_bytes_per_second);
        o.push_kv("active_streams_avg", data.active_streams_avg);
        o.push_kv("total_drawn_cells", data.total_drawn_cells);
        o.push_kv_str(
            "glyphs_per_second_theoretical_human",
            &crate::humanize::humanize(data.glyphs_per_second_theoretical),
        );
        o.push_kv_str(
            "cells_drawn_total_human",
            &crate::humanize::humanize(data.total_drawn_cells),
        );
    });

    // ── timing ──
    json_object(&mut out, "timing", |o| {
        o.push_kv("elapsed_s", data.elapsed_s);
        o.push_kv("total_frames", data.total_frames);
        o.push_kv("drawn_frames", data.drawn_frames);
        o.push_kv_str(
            "total_frames_human",
            &crate::humanize::humanize(data.total_frames),
        );
    });

    // ── terminal_io (Phase 2: wet I/O metrics) ──
    //
    // v50 LTS audit (Task 3): added `bytes_written_human` and
    // `bandwidth_mibps_human` for symmetry with the text report
    // (which already used `humanize_bytes` / `humanize_throughput`
    // after the centralization refactor in commit f8f6a5e). JSON
    // consumers can read the raw u64/f64 fields for precise math
    // and the `_human` fields for display, matching the pattern
    // already established in the `throughput` and `timing`
    // sections.
    json_object(&mut out, "terminal_io", |o| match &data.terminal_io {
        Some(io) if io.enabled => {
            o.push_kv("enabled", true);
            o.push_kv_str("target", &io.target);
            o.push_kv("bytes_written", io.bytes_written);
            o.push_kv_str(
                "bytes_written_human",
                &crate::humanize::humanize_bytes(io.bytes_written),
            );
            o.push_kv("write_calls", io.write_calls);
            o.push_kv("backpressure_events", io.backpressure_events);
            // `bandwidth_mibps` is the sole bandwidth field. The
            // deprecated `bandwidth_mbps` alias (same value, misleading
            // MB/s label for a MiB/s divisor) was removed under owner
            // approval — see the module-level "Renames and removals"
            // contract above for the full lifecycle.
            let bw = io.bandwidth_mibps();
            o.push_kv("bandwidth_mibps", bw);
            o.push_kv_str(
                "bandwidth_human",
                &crate::humanize::humanize_throughput(io.bytes_written, io.elapsed_secs),
            );
            o.push_kv("avg_latency_us", io.avg_latency_us());
            o.push_kv("effective_write_fps", io.effective_write_fps());
        }
        _ => {
            o.push_kv("enabled", false);
        }
    });

    // ── energy (Phase 3: RAPL, Linux only) ──
    json_object(&mut out, "energy", |o| match &data.energy {
        Some(e) if e.available => {
            o.push_kv("available", true);
            o.push_kv("total_energy_joules", e.total_energy_joules);
            o.push_kv("avg_power_watts", e.avg_power_watts);
            o.push_kv("energy_per_frame_uj", e.energy_per_frame_uj);
            o.push_kv("energy_per_cell_nj", e.energy_per_cell_nj);
            o.push_kv("package_count", e.package_count);
        }
        _ => {
            o.push_kv("available", false);
        }
    });

    // ── microarchitecture (Phase 4: perf counters, Linux x86 only) ──
    json_object(&mut out, "microarchitecture", |o| match &data.perf {
        Some(p) if p.available => {
            o.push_kv("available", true);
            o.push_kv("cycles", p.cycles);
            o.push_kv("instructions", p.instructions);
            o.push_kv("ipc", p.instructions_per_cycle);
            o.push_kv("branch_instructions", p.branch_instructions);
            o.push_kv("branch_misses", p.branch_misses);
            o.push_kv("branch_mispredict_rate", p.branch_mispredict_rate);
        }
        _ => {
            o.push_kv("available", false);
        }
    });

    // ── allocator (Phase 5: tracing) ──
    //
    // v50 LTS audit (Task 3): emit `available: true` in the Some(a)
    // branch for schema consistency with `energy` and `microarchitecture`
    // (which always emit `available: true/false`). Previously the
    // `allocator` section emitted `available: false` when None but
    // omitted the flag entirely when Some — leaving consumers with no
    // reliable presence flag. Now `available` is always present.
    //
    // Also added `_human` forms for the four byte-typed fields so JSON
    // consumers can display them without re-implementing the binary
    // 1024-based scaling logic. The raw u64 fields remain for precise
    // math; the `_human` fields are for display only.
    json_object(&mut out, "allocator", |o| match &data.allocator {
        Some(a) => {
            o.push_kv("available", true);
            o.push_kv("alloc_calls", a.alloc_calls);
            o.push_kv("dealloc_calls", a.dealloc_calls);
            o.push_kv("realloc_calls", a.realloc_calls);
            o.push_kv("bytes_allocated_total", a.bytes_allocated_total);
            o.push_kv_str(
                "bytes_allocated_total_human",
                &crate::humanize::humanize_bytes(a.bytes_allocated_total),
            );
            o.push_kv("bytes_deallocated_total", a.bytes_deallocated_total);
            o.push_kv_str(
                "bytes_deallocated_total_human",
                &crate::humanize::humanize_bytes(a.bytes_deallocated_total),
            );
            o.push_kv("heap_retained_bytes", a.heap_retained_bytes);
            o.push_kv_str(
                "heap_retained_bytes_human",
                &crate::humanize::humanize_bytes(a.heap_retained_bytes),
            );
            o.push_kv("alloc_calls_per_frame", a.alloc_calls_per_frame);
            o.push_kv("dealloc_calls_per_frame", a.dealloc_calls_per_frame);
            o.push_kv("heap_virtual_kib", a.heap_virtual_kib);
            o.push_kv_str(
                "heap_virtual_kib_human",
                &crate::humanize::humanize_bytes(a.heap_virtual_kib.saturating_mul(1024)),
            );
        }
        None => {
            o.push_kv("available", false);
        }
    });

    // ── visual_objective (Phase 6) ──
    //
    // v50 LTS audit (Task 3): emit `available: true` in the Some(v)
    // branch for schema consistency with `energy` and `microarchitecture`.
    // Previously the section emitted `available: false` when None but
    // omitted the flag entirely when Some — consumers had no reliable
    // presence flag. Now `available` is always present.
    json_object(&mut out, "visual_objective", |o| match &data.visual {
        Some(v) => {
            o.push_kv("available", true);
            o.push_kv("frame_entropy_bits", v.frame_entropy_bits);
            o.push_kv("density_gini", v.density_gini);
            o.push_kv("color_transition_delta_avg", v.color_transition_delta_avg);
            o.push_kv("samples", v.samples);
        }
        None => {
            o.push_kv("available", false);
        }
    });

    // Remove trailing comma from the last section.
    if out.ends_with(',') {
        out.pop();
    }
    out.push('}');

    out
}

// ── JSON builder helpers ────────────────────────────────────────────────────

/// Helper trait for building JSON objects with proper comma handling.
/// Each `push_kv*` call appends `"key":value,` — the trailing comma is
/// stripped by `build_json_string` before closing the root object.
trait JsonBuf {
    fn push_kv_str(&mut self, key: &str, value: &str);
    fn push_kv_opt_str(&mut self, key: &str, value: Option<&str>);
    fn push_kv_opt(&mut self, key: &str, value: Option<f64>);
    fn push_kv_null(&mut self, key: &str);
    fn push_kv(&mut self, key: &str, value: impl JsonValue);
}

impl JsonBuf for String {
    fn push_kv_str(&mut self, key: &str, value: &str) {
        self.push_str(&format!("\"{key}\":"));
        push_json_string(self, value);
        self.push(',');
    }

    fn push_kv_opt_str(&mut self, key: &str, value: Option<&str>) {
        match value {
            Some(v) => self.push_kv_str(key, v),
            None => self.push_kv_null(key),
        }
    }

    fn push_kv_opt(&mut self, key: &str, value: Option<f64>) {
        // v50 LTS audit (Task 3): finite-check the inner f64 before
        // emitting. The non-Option `JsonValue for f64` impl already
        // emits `null` for NaN/Infinity, but `push_kv_opt` previously
        // wrote the bare `{v}` literal — producing invalid JSON like
        // `"key":NaN,` or `"key":inf,` which strict JSON parsers
        // reject. Now any non-finite value (None OR Some(NaN/inf))
        // emits `null`, matching the `JsonValue for f64` behavior and
        // guaranteeing the output is always valid JSON.
        match value {
            Some(v) if v.is_finite() => {
                self.push_str(&format!("\"{key}\":{v},"));
            }
            _ => self.push_kv_null(key),
        }
    }

    fn push_kv_null(&mut self, key: &str) {
        self.push_str(&format!("\"{key}\":null,"));
    }

    fn push_kv(&mut self, key: &str, value: impl JsonValue) {
        self.push_str(&format!("\"{key}\":"));
        value.write_json(self);
        self.push(',');
    }
}

/// Trait for types that can write themselves as a JSON value.
trait JsonValue {
    fn write_json(&self, out: &mut String);
}

impl JsonValue for bool {
    fn write_json(&self, out: &mut String) {
        out.push_str(if *self { "true" } else { "false" });
    }
}

impl JsonValue for u16 {
    fn write_json(&self, out: &mut String) {
        out.push_str(&self.to_string());
    }
}

impl JsonValue for u32 {
    fn write_json(&self, out: &mut String) {
        out.push_str(&self.to_string());
    }
}

impl JsonValue for u64 {
    fn write_json(&self, out: &mut String) {
        out.push_str(&self.to_string());
    }
}

impl JsonValue for f32 {
    fn write_json(&self, out: &mut String) {
        if self.is_finite() {
            out.push_str(&self.to_string());
        } else {
            out.push_str("null");
        }
    }
}

impl JsonValue for f64 {
    fn write_json(&self, out: &mut String) {
        if self.is_finite() {
            out.push_str(&self.to_string());
        } else {
            out.push_str("null");
        }
    }
}

/// Push a JSON object section: `"name":{...}`. The closure receives the
/// buffer to append key-value pairs.
fn json_object<F>(out: &mut String, name: &str, body: F)
where
    F: FnOnce(&mut String),
{
    out.push_str(&format!("\"{name}\":{{"));
    body(out);
    // Strip trailing comma from the last KV pair inside this object.
    if out.ends_with(',') {
        out.pop();
    }
    out.push_str("},");
}

/// Push a JSON-escaped string value into the buffer.
fn push_json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_json_string_escapes_correctly() {
        let mut out = String::new();
        push_json_string(&mut out, "hello \"world\"\n");
        assert_eq!(out, "\"hello \\\"world\\\"\\n\"");
    }

    #[test]
    fn push_json_string_handles_backslash() {
        let mut out = String::new();
        push_json_string(&mut out, "C:\\path\\to\\file");
        assert_eq!(out, "\"C:\\\\path\\\\to\\\\file\"");
    }

    #[test]
    fn push_json_string_handles_control_chars() {
        let mut out = String::new();
        push_json_string(&mut out, "a\x01b");
        assert_eq!(out, "\"a\\u0001b\"");
    }

    #[test]
    fn json_buf_push_kv_str_works() {
        let mut out = String::new();
        out.push_kv_str("key", "value");
        assert_eq!(out, "\"key\":\"value\",");
    }

    #[test]
    fn json_buf_push_kv_null_works() {
        let mut out = String::new();
        out.push_kv_null("key");
        assert_eq!(out, "\"key\":null,");
    }

    #[test]
    fn json_buf_push_kv_opt_str_some() {
        let mut out = String::new();
        out.push_kv_opt_str("key", Some("value"));
        assert_eq!(out, "\"key\":\"value\",");
    }

    #[test]
    fn json_buf_push_kv_opt_str_none() {
        let mut out = String::new();
        out.push_kv_opt_str("key", None);
        assert_eq!(out, "\"key\":null,");
    }

    #[test]
    fn json_object_strips_trailing_comma() {
        let mut out = String::from("{");
        json_object(&mut out, "section", |o| {
            o.push_kv_str("a", "1");
            o.push_kv_str("b", "2");
        });
        // Strip the trailing comma after the object.
        if out.ends_with(',') {
            out.pop();
        }
        out.push('}');
        assert_eq!(out, r#"{"section":{"a":"1","b":"2"}}"#);
    }

    // ── v50 LTS audit (Task 3) stress tests ───────────────────────────────
    //
    // Regression coverage for the JSON serializer hardening:
    //   1. push_kv_opt emits `null` for non-finite f64 (NaN, +/- infinity)
    //      instead of invalid JSON literals like `NaN` or `inf`.
    //   2. allocator and visual_objective sections always emit
    //      `available: true/false` so consumers have a reliable
    //      presence flag (previously the `Some` branch omitted it).
    //   3. terminal_io + allocator sections emit `_human` forms for
    //      byte-typed fields alongside the raw u64 values.
    //   4. The full JSON output is parseable as a strict JSON value
    //      (no trailing commas, balanced braces, no NaN/inf tokens).

    #[test]
    fn push_kv_opt_finite_value_emits_number() {
        let mut out = String::new();
        out.push_kv_opt("key", Some(42.5));
        assert_eq!(out, "\"key\":42.5,");
    }

    #[test]
    fn push_kv_opt_none_emits_null() {
        let mut out = String::new();
        out.push_kv_opt("key", None);
        assert_eq!(out, "\"key\":null,");
    }

    #[test]
    fn push_kv_opt_nan_emits_null_not_invalid_literal() {
        // NaN must NOT be emitted as the bare literal `NaN` — strict
        // JSON parsers reject it. The finite-check routes through
        // push_kv_null so the output is always valid JSON.
        let mut out = String::new();
        out.push_kv_opt("key", Some(f64::NAN));
        assert_eq!(out, "\"key\":null,");
    }

    #[test]
    fn push_kv_opt_positive_infinity_emits_null() {
        let mut out = String::new();
        out.push_kv_opt("key", Some(f64::INFINITY));
        assert_eq!(out, "\"key\":null,");
    }

    #[test]
    fn push_kv_opt_negative_infinity_emits_null() {
        let mut out = String::new();
        out.push_kv_opt("key", Some(f64::NEG_INFINITY));
        assert_eq!(out, "\"key\":null,");
    }

    #[test]
    fn push_kv_opt_zero_emits_zero() {
        // Boundary: 0.0 (positive) and -0.0 (negative) are both finite
        // and must emit as the number, not null.
        let mut out = String::new();
        out.push_kv_opt("pos_zero", Some(0.0));
        out.push_kv_opt("neg_zero", Some(-0.0));
        assert_eq!(out, "\"pos_zero\":0,\"neg_zero\":-0,");
    }

    #[test]
    fn json_value_f64_nan_emits_null() {
        // Direct trait impl check: bare f64 NaN must emit null.
        let mut out = String::new();
        let v: f64 = f64::NAN;
        v.write_json(&mut out);
        assert_eq!(out, "null");
    }

    #[test]
    fn json_value_f64_infinity_emits_null() {
        let mut out = String::new();
        let v: f64 = f64::INFINITY;
        v.write_json(&mut out);
        assert_eq!(out, "null");
    }

    #[test]
    fn json_value_f64_finite_emits_number() {
        let mut out = String::new();
        // Deliberately not a well-known constant approximation (3.14159,
        // 2.71828, ...) — clippy::approx_constant is deny-by-default and
        // CI runs `cargo clippy --all-targets`, which lints test code.
        let v: f64 = 1.23456;
        v.write_json(&mut out);
        assert_eq!(out, "1.23456");
    }
}
