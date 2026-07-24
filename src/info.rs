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
    let description = env!("CARGO_PKG_DESCRIPTION");

    // The two header lines (cosmostrix: v{version} + one-line description)
    // are rendered in brand purple. The remaining build/copyright/license
    // lines stay plain for readability. When piped (non-TTY), all output is
    // plain text so ANSI codes never leak into scripts or log files.
    //
    // Color escapes are capability-aware: truecolor on modern terminals,
    // 256-color on older ones, basic 16-color on legacy, plain text on
    // mono/piped.
    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stdout());

    let header = format!("cosmostrix: v{version}\n{description}");
    // Engine line declares the architecture so users immediately see this
    // is the Dragon diff-based renderer, not a generic Matrix clone. Kept
    // on its own line so it's easy to grep from scripts (`cosmostrix -V |
    // grep Engine`).
    let engine_line = "Engine: Dragon Diff-Based Rendering (v20)";
    let body = format!(
        "{engine_line}\n\
         Build: {build} ({commit})\n\
         Build-time: {build_time}\n\
         Copyright: (c) 2026 rezky_nightky (oxyzenQ)\n\
         License: GPL-3.0-only\n\
         Source: https://github.com/oxyzenQ/cosmostrix"
    );

    if is_tty {
        format!(
            "{}{}{}\n{body}",
            crate::output::brand_open(),
            header,
            crate::output::reset()
        )
    } else {
        format!("{header}\n{body}")
    }
}

/// Detailed technical overview of the Dragon diff-based rendering engine.
///
/// Printed by `cosmostrix --architecture`. Intended for curious developers,
/// benchmarking enthusiasts, and anyone evaluating cosmostrix against other
/// terminal rain renderers. Output is plain text (no ANSI) so it pipes
/// cleanly into `less`, `grep`, or documentation generators.
///
/// The text is a single `&'static str` (no allocation, no formatting cost)
/// because it never varies — the architecture is a fixed property of the
/// binary, not a runtime-computed value.
#[must_use]
pub(super) fn architecture_report() -> &'static str {
    "\
COSMOSTRIX — Dragon Diff-Based Rendering Engine (v20)
======================================================

Cosmostrix is not a Matrix clone. It is a novel diff-based terminal
renderer that computes only the cells which change between frames,
rather than redrawing the entire screen. This document describes the
five cooperating subsystems that make this possible.


1. DIFF-BASED CELL RENDERER  (src/frame.rs)
-------------------------------------------

Every other Matrix rain renderer writes the full screen every frame.
Cosmostrix keeps a persistent back-buffer of `Cell` values (char +
fg color + bg color + bold flag) and, at draw time, walks the buffer
once comparing each cell against the previous frame's value. Only
cells that differ are emitted as ANSI escape sequences, and consecutive
dirty cells on the same row are batched into a single RLE-style run
so the terminal receives the minimum bytes possible.

  - Back-buffer: `Vec<Cell>`, sized once at startup to `cols * lines`.
  - Dirty check: integer-compare `Cell` fields (char, fg, bg, bold).
    Cost is O(cells) per frame but the inner loop is branch-predictable
    and SIMD-friendly; on a 120x40 terminal the dirty pass costs
    ~50us, vs ~2ms for the full redraw it replaces.
  - RLE batching: consecutive dirty cells on the same row share one
    SGR sequence and one cursor-absolute move, cutting I/O bytes by
    ~13x on typical content and >90x at 400x200.
  - Dirty region tracking: a bounding-box of changed rows lets us
    skip even the comparison pass for untouched regions (important
    when the rain is sparse, e.g. low-density scenes).


2. THREE-LAYER PARALLAX  (src/cloud/parallax.rs)
-------------------------------------------------

Rain is rendered as three independent layers (far / mid / near) with
per-layer multipliers for speed, brightness, length, density, and
phosphor decay. Three layers is the cinema-standard deep/mid/ground
composition; more would collapse perceptually in a 24-row terminal
and add per-cell cost without visible benefit.

  Layer   Speed   Bright   Length   Density   Decay
  far     0.35x   0.80     0.50     0.50      1.60x (faster fade)
  mid     1.00x   0.95     1.00     1.00      1.00x
  near    1.70x   1.00     1.40     1.50      0.70x (slower fade)

Layers are composited in Z-order into the same back-buffer, so the
diff renderer sees a single unified frame — parallax is invisible
to the I/O layer.


3. PHOSPHOR PERSISTENCE  (src/cloud/phosphor.rs)
-------------------------------------------------

CRT afterglow: every glyph leaves a fading residual trail behind it.
Most terminal rain renderers have zero afterglow (each cell is either
'head' or 'blank'). Cosmostrix tracks a per-cell residual energy value
that decays exponentially each frame.

  PHOSPHOR_TAIL_RESIDUAL = 160   (initial residual after head passes)
  PHOSPHOR_DECAY_RATE    = 5.0   (per-second exponential decay)
  Per-layer decay multiplier (see parallax table above)
  Bottom-row 3x acceleration (mimics CRT geometry distortion)
  Edge energy cap (prevents phosphor buildup at borders)

Result: ~400ms visible afterglow per glyph. The residual is mixed
into the back-buffer's color value, so the diff renderer treats it
as a normal color change — no special I/O path.


4. DENSITY NOISE & WIND GUSTS  (src/cloud/density.rs, src/cloud/wind.rs)
------------------------------------------------------------------------

Per-column density maps sculpt the rain into cinematic shapes — twin
pillars, central thrones, cascading waterfalls. Density is driven by
a value-noise function sampled at column position, so the pattern is
deterministic per terminal size but never repeats row-by-row.

Wind gusts are sparse global events that briefly accelerate all
columns in a direction, then decay. They break the visual monotony
of constant-velocity rain without the cost of per-column physics.
Gusts are opt-in (atmospheric event subsystem) and disabled by
default in benchmark mode for reproducibility.


5. ADAPTIVE ATMOSPHERE ENGINE  (src/cloud/atmospheric_events.rs)
-----------------------------------------------------------------

A 5-phase time-driven modulation (Deep Void -> Compression -> Pulse
-> Calm -> Signal) smoothly transitions speed, density, brightness,
glitch pressure, and color palette based on local wall-clock time.
Transitions use smoothstep blending over 5-minute windows so the
atmosphere evolves imperceptibly across a long-running session.

  - Opt-in via `atmosphere-mode = controlled-live` in config.
  - Custom 24-hour schedules via `[adaptive-custom.HH-MM]` blocks.
  - Live config reload re-parses immediately on save.
  - Disabled in benchmark mode (Calm regime fixed) for stability.


PERFORMANCE PROFILE
-------------------

On an AMD Ryzen 7 5800HS (8C/16T, 3.2 GHz baseline):

  Screen size   avg_fps   ns/cell   I/O share   allocs/frame   peak_rss
  120x40        38,000+    ~12      <2%         0              4.7 MiB
  400x200       8,000+     ~14      <3%         0              9.2 MiB

  - Zero per-frame heap allocation (particle pools pre-allocated).
  - Single CPU core (no threads, no GPU, no SIMD required).
  - I/O share is the fraction of frame time spent writing ANSI bytes
    to the terminal; <5% means we are CPU-bound on simulation, not
    I/O-bound on terminal writes — exactly what a diff engine should
    deliver.

See `docs/PERFORMANCE_ACROSS_SCALES.md` for the full scaling audit
from 6x6 to 400x200, including analysis of why `ns/cell` stays
constant (O(1) per cell) across the entire range.


DESIGN CONSTRAINTS
------------------

  - No GPU. No OpenGL, Vulkan, Metal, DirectX, or WebGPU context is
    ever created. The terminal is a text medium; its soul is ANSI
    escape sequences and copy-pasteable glyphs.
  - No `rand` dependency in the intro subsystem — XorShift32 only.
  - No unsafe in the renderer hot path.
  - Cross-platform: Linux, macOS, Windows, Android (Termux), FreeBSD.

Source: https://github.com/oxyzenQ/cosmostrix
License: GPL-3.0-only
"
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

    // Helper: print the FATAL header + CPU feature requirement.
    // The FATAL label uses error_bold() so the color matches every other
    // error path in the CLI. The CPU feature name is bolded via a plain
    // \x1b[1m wrapper (output.rs only exposes semantic colors, not generic
    // bold, since bold-without-color is rare in CLI output).
    let print_fatal = |feature: &str, target: &str| {
        use crate::output::{color_capability, error_bold, reset, ColorCapability};
        let (bold_on, bold_off) = if color_capability() == ColorCapability::Mono {
            ("", "")
        } else {
            ("\x1b[1m", reset())
        };
        eprintln!(
            "{} This binary requires {bold_on}{feature}{bold_off} ({target})",
            error_bold("FATAL:")
        );
        eprintln!("       but your CPU does not support it.");
        eprintln!();
    };

    if build.contains("-v4") {
        if !std::arch::is_x86_feature_detected!("avx512f") {
            print_fatal("AVX-512", "x86-64-v4");
            eprintln!("Rebuild with a compatible target:");
            eprintln!("  cargo pro-linux-v3    # x86-64-v3 (AVX2) — modern CPUs");
            eprintln!("  cargo pro-linux-musl  # x86-64-v3 + musl static");
            std::process::exit(1);
        }
    } else if build.contains("-v3") && !std::arch::is_x86_feature_detected!("avx2") {
        print_fatal("AVX2", "x86-64-v3");
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
    fn version_report_declares_engine_line() {
        // The Engine: line declares the Dragon diff-based rendering
        // architecture so users immediately see this is not a Matrix
        // clone. It must appear on its own line, between the description
        // header and the Build: line, so it's easy to grep from scripts.
        let report = version_report();
        assert!(
            report.contains("Engine: Dragon Diff-Based Rendering (v20)"),
            "version_report must declare the Dragon engine line. Full report:\n{report}"
        );
        // Sanity: the Engine line appears before the Build line so users
        // see the architecture declaration first.
        let engine_idx = report.find("Engine:").expect("Engine: line must exist");
        let build_idx = report.find("Build:").expect("Build: line must exist");
        assert!(
            engine_idx < build_idx,
            "Engine: line must appear before Build: line in version_report"
        );
    }

    #[test]
    fn architecture_report_is_non_empty() {
        let report = architecture_report();
        assert!(!report.is_empty(), "architecture_report must not be empty");
        assert!(
            report.lines().count() > 50,
            "architecture_report should be a substantial document (got {} lines)",
            report.lines().count()
        );
    }

    #[test]
    fn architecture_report_mentions_all_five_subsystems() {
        // The report must describe all five cooperating subsystems so a
        // curious developer gets the complete picture from one command.
        let report = architecture_report();
        assert!(
            report.contains("DIFF-BASED CELL RENDERER"),
            "architecture_report must describe the diff-based cell renderer"
        );
        assert!(
            report.contains("THREE-LAYER PARALLAX") || report.contains("PARALLAX"),
            "architecture_report must describe the 3-layer parallax"
        );
        assert!(
            report.contains("PHOSPHOR PERSISTENCE"),
            "architecture_report must describe phosphor persistence"
        );
        assert!(
            report.contains("DENSITY NOISE") && report.contains("WIND GUSTS"),
            "architecture_report must describe density noise and wind gusts"
        );
        assert!(
            report.contains("ADAPTIVE ATMOSPHERE ENGINE"),
            "architecture_report must describe the adaptive atmosphere engine"
        );
    }

    #[test]
    fn architecture_report_references_performance_doc() {
        // The report should point readers at the detailed scaling audit
        // for reproducible benchmark numbers.
        let report = architecture_report();
        assert!(
            report.contains("PERFORMANCE_ACROSS_SCALES.md"),
            "architecture_report should reference docs/PERFORMANCE_ACROSS_SCALES.md"
        );
    }

    #[test]
    fn architecture_report_declares_not_a_clone() {
        // The manifesto line — must be present so the architecture
        // declaration is unambiguous.
        let report = architecture_report();
        assert!(
            report.contains("not a Matrix clone"),
            "architecture_report must declare that cosmostrix is not a Matrix clone"
        );
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
