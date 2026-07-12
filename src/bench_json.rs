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
        o.push_kv_str("pgo", "no");
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
        o.push_kv("cols", data.w);
        o.push_kv("lines", data.h);
        o.push_kv("target_fps", data.target_fps);
        o.push_kv("density", data.density);
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
    json_object(&mut out, "throughput", |o| {
        o.push_kv("glyphs_per_second", data.glyphs_per_second);
        o.push_kv("dirty_glyphs_per_second", data.dirty_glyphs_per_second);
        o.push_kv("ansi_bytes_per_second", data.ansi_bytes_per_second);
        o.push_kv("active_streams_avg", data.active_streams_avg);
        o.push_kv("total_drawn_cells", data.total_drawn_cells);
        o.push_kv_str(
            "glyphs_per_second_human",
            &crate::humanize::humanize(data.glyphs_per_second),
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
/// stripped by `print_json_report` before closing the root object.
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
        match value {
            Some(v) => {
                self.push_str(&format!("\"{key}\":{v},"));
            }
            None => self.push_kv_null(key),
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
    let start_len = out.len();
    body(out);
    // Strip trailing comma from the last KV pair inside this object.
    if out.ends_with(',') {
        out.pop();
    }
    let _ = start_len; // suppress unused warning
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
}
