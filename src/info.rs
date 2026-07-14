// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Build information, memory estimation, CPU feature detection, and
//! environment variable helpers.

use std::env;

// --- Build info helpers ---

/// Canonical build label (e.g. "linux-amd64-v3", "darwin-aarch64-native").
///
/// Source of truth: `COSMOSTRIX_BUILD` env var set at compile time by
/// `build.rs` (which reads it from `.cargo/config.toml` aliases or the
/// `COSMOSTRIX_BUILD` environment variable passed by CI/release scripts).
/// All diagnostics (`--doctor`, `--benchmark`, `--info`) and
/// `--version`/`-V` share this single source.
#[must_use]
pub(super) fn canonical_build_label() -> &'static str {
    option_env!("COSMOSTRIX_BUILD").unwrap_or("unknown")
}

#[must_use]
pub(super) fn build_commit_short() -> Option<&'static str> {
    match option_env!("COSMOSTRIX_GIT_SHA") {
        Some(s) if !s.is_empty() => Some(s),
        _ => None,
    }
}

#[must_use]
pub(super) fn version_report() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let build = canonical_build_label();
    let commit = build_commit_short().unwrap_or("unknown");
    let build_time = option_env!("COSMOSTRIX_BUILD_TIME").unwrap_or("unknown");

    // Only the header line "cosmostrix: v{version}" is purple (brand color).
    // Remaining lines are plain for readability. When piped (non-TTY),
    // output is fully plain text for scripts.
    let purple = "\x1b[38;2;168;85;247m";
    let reset = "\x1b[0m";
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stdout());

    let header = format!("cosmostrix: v{version}");
    let body = format!(
        "Build: {build} ({commit})\n\
         Build-time: {build_time}\n\
         Copyright: (c) 2026 rezky_nightky (oxyzenQ)\n\
         License: GPL-3.0-only\n\
         Source: https://github.com/oxyzenQ/cosmostrix"
    );

    if is_tty {
        format!("{purple}{header}{reset}\n{body}")
    } else {
        format!("{header}\n{body}")
    }
}

// --- Environment variable helpers ---

#[must_use]
pub fn env_var_truthy(name: &str) -> bool {
    match env::var(name) {
        Ok(v) => {
            let v = v.trim();
            if v.is_empty() {
                return false;
            }
            let v = v.to_ascii_lowercase();
            !(v == "0" || v == "false" || v == "no" || v == "off")
        }
        Err(env::VarError::NotPresent) => false,
        Err(env::VarError::NotUnicode(_)) => true,
    }
}

// --- Memory budget estimation ---

#[must_use]
pub(super) fn estimate_memory_budget(w: u16, h: u16) -> usize {
    // Use actual Cell size rather than a magic number for accuracy
    let cell_size = std::mem::size_of::<crate::cell::Cell>();
    let frame_cells = (w as usize) * (h as usize) * cell_size;

    // Cloud internal buffers: char_pool (2048), glitch_pool (1024), color_map, glitch_map
    let cloud_pools = 2048 * 4 + 1024 * 4;
    let cloud_maps = (w as usize) * (h as usize) * 2; // color_map + glitch_map

    // Droplets: ~1.5 * cols droplets, each ~100 bytes
    let droplet_count = (1.5 * w as f32) as usize;
    let droplets_size = droplet_count * std::mem::size_of::<crate::droplet::Droplet>().max(100);

    // Terminal: LastFrame + row_dirty + touched_rows
    let terminal_last = (w as usize) * (h as usize) * cell_size;

    frame_cells * 2 + cloud_pools + cloud_maps + droplets_size + terminal_last
}

#[must_use]
pub(super) fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
    }
}

// --- CPU feature check ---

/// Runtime CPU feature check for x86-64 builds.
///
/// Detects if the CPU supports the required instruction set for the
/// compiled target level (v3 = AVX2, v4 = AVX-512). Prints a clear
/// error message and exits instead of crashing with SIGILL.
#[cfg(target_arch = "x86_64")]
pub(super) fn check_cpu_features() {
    let build = option_env!("COSMOSTRIX_BUILD").unwrap_or("");
    if build.contains("-v4") {
        if !std::arch::is_x86_feature_detected!("avx512f") {
            eprintln!(
                "\x1b[1;31mFATAL:\x1b[0m This binary requires \x1b[1mAVX-512\x1b[0m (x86-64-v4)"
            );
            eprintln!("       but your CPU does not support it.");
            eprintln!();
            eprintln!("Rebuild with a compatible target:");
            eprintln!("  cargo pro-linux-v3    # x86-64-v3 (AVX2) — modern CPUs");
            eprintln!("  cargo pro-linux-musl  # x86-64-v3 + musl static");
            std::process::exit(1);
        }
    } else if build.contains("-v3") && !std::arch::is_x86_feature_detected!("avx2") {
        eprintln!("\x1b[1;31mFATAL:\x1b[0m This binary requires \x1b[1mAVX2\x1b[0m (x86-64-v3)");
        eprintln!("       but your CPU does not support it.");
        eprintln!();
        eprintln!("Rebuild with:");
        eprintln!("  cargo pro-linux-musl  # x86-64-v3 + musl static (same baseline)");
        eprintln!(
            "  Note: v1/v2 profiles were dropped in v10.0.0. Use musl for max compatibility."
        );
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_build_label_reads_cosmostrix_build_env() {
        // canonical_build_label must return the value of COSMOSTRIX_BUILD
        // at compile time. When built with `cargo pro-linux-v3`, this is
        // "linux-amd64-v3". This test verifies the function is wired
        // correctly; the actual value depends on how the test binary was
        // compiled (plain `cargo test` sets COSMOSTRIX_BUILD via build.rs
        // inference to e.g. "linux-amd64-vN" or "unknown").
        let label = canonical_build_label();
        assert!(!label.is_empty(), "canonical_build_label must not be empty");
    }

    #[test]
    fn version_report_uses_canonical_build_label() {
        // version_report must use canonical_build_label, not a separate
        // os-arch string. Verify that the Build: line contains the same
        // value as canonical_build_label().
        let label = canonical_build_label();
        let report = version_report();
        assert!(
            report.contains(&format!("Build: {label}")),
            "version_report Build: line must contain the canonical build label '{label}'. \
             Full report:\n{report}"
        );
    }

    #[test]
    fn version_report_build_label_matches_doctor_build_label() {
        // Ensure version_report build label matches diagnostics::detect_cpu_info
        // build_variant — they must both read from COSMOSTRIX_BUILD.
        let version_label = canonical_build_label();
        let cpu = crate::diagnostics::detect_cpu_info();
        assert_eq!(
            version_label, cpu.build_variant,
            "version_report build label and doctor/benchmark build label must match"
        );
    }

    #[test]
    fn version_report_contains_version_and_commit() {
        let report = version_report();
        assert!(
            report.contains("cosmostrix: v"),
            "report must contain 'cosmostrix: v' header"
        );
        assert!(report.contains("Build:"), "report must contain Build: line");
        assert!(
            report.contains("Build-time:"),
            "report must contain Build-time: line"
        );
        assert!(
            report.contains("Copyright:"),
            "report must contain Copyright:"
        );
        assert!(report.contains("License:"), "report must contain License:");
        assert!(report.contains("Source:"), "report must contain Source:");
    }

    #[test]
    fn info_file_stays_under_loc_cap() {
        let source = include_str!("info.rs");
        let lines = source.lines().count();
        assert!(
            lines < 1000,
            "info.rs must stay under 1000 LOC (currently {lines})"
        );
    }
}
