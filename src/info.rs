// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Build information, memory estimation, CPU feature detection, and
//! environment variable helpers.

use std::env;

// --- Build info helpers ---

#[must_use]
pub(super) fn build_commit_short() -> Option<&'static str> {
    match option_env!("COSMOSTRIX_GIT_SHA") {
        Some(s) if !s.is_empty() => Some(s),
        _ => None,
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
            eprintln!("  cargo pro-linux-v2    # x86-64-v2 (SSE4.2, POPCNT) — most CPUs");
            eprintln!("  cargo pro-linux-v3    # x86-64-v3 (AVX2) — modern CPUs");
            std::process::exit(1);
        }
    } else if build.contains("-v3") && !std::arch::is_x86_feature_detected!("avx2") {
        eprintln!("\x1b[1;31mFATAL:\x1b[0m This binary requires \x1b[1mAVX2\x1b[0m (x86-64-v3)");
        eprintln!("       but your CPU does not support it.");
        eprintln!();
        eprintln!("Rebuild with:");
        eprintln!("  cargo pro-linux-v1    # x86-64-v1 (baseline)");
        eprintln!("  cargo pro-linux-v2    # x86-64-v2 (SSE4.2, POPCNT)");
        std::process::exit(1);
    }
}
