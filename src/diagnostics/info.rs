// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Build information, memory estimation, CPU feature detection, and
//! environment variable helpers.

// Only used inside check_cpu_features(), which is x86_64-only (AVX2/AVX-512
// detection). On every other target arch (aarch64 linux/macos/android) the
// import is unused and fails `-D unused-imports` on the CI cross-builds.
#[cfg(target_arch = "x86_64")]
use crate::output::eprintln_safe;
use std::env;

use crate::constants::{CHAR_POOL_SIZE, DROPLET_COUNT_FACTOR, GLITCH_POOL_SIZE};

// --- Branding signature ---

/// Embedded build signature.
///
/// Marked `pub(crate)` so external tooling (e.g. FFI probes, binary diff
/// scripts, supply-chain scanners) can grep for it both in source and
/// in the produced artifact. Referenced by `version_report()` so the
/// linker keeps it in the final binary even under aggressive dead-code
/// elimination.
///
/// Verification:
///
/// ```text
/// strings ./cosmostrix | grep "Cosmic Dragon"
/// ```
pub(crate) const COSMIC_DRAGON_SIGNATURE: &str =
    "Cosmic Dragon — Official Build by rezky_nightky (oxyzenQ)";

// --- Build info helpers ---

/// Canonical build label (e.g. "linux-amd64-v3", "darwin-aarch64-native").
///
/// Source of truth: `COSMOSTRIX_BUILD` env var set at compile time by
/// `build.rs` (which reads it from `.cargo/config.toml` aliases or the
/// `COSMOSTRIX_BUILD` environment variable passed by CI/release scripts).
/// All diagnostics (`--doctor`, `--benchmark`) and
/// `--version`/`-V` share this single source.
#[must_use]
pub(crate) fn canonical_build_label() -> &'static str {
    option_env!("COSMOSTRIX_BUILD").unwrap_or("unknown")
}

#[must_use]
pub(crate) fn build_commit_short() -> Option<&'static str> {
    match option_env!("COSMOSTRIX_GIT_SHA") {
        Some(s) if !s.is_empty() => Some(s),
        _ => None,
    }
}

#[must_use]
pub(crate) fn version_report() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let build = canonical_build_label();
    let commit = build_commit_short().unwrap_or("unknown");
    let build_time = option_env!("COSMOSTRIX_BUILD_TIME").unwrap_or("unknown");
    let description = env!("CARGO_PKG_DESCRIPTION");
    // Pull the official-build signature into the version report so the
    // linker cannot dead-strip `COSMIC_DRAGON_SIGNATURE` from the final
    // binary. The string is also discoverable via `strings(1)`.
    let signature = COSMIC_DRAGON_SIGNATURE;

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
    // is The Cosmic Dragon Diff-Based Rendering Engine, not a generic
    // Matrix clone. Kept on its own line so it's easy to grep from scripts
    // (`cosmostrix -V | grep Engine`).
    let engine_line = "Engine: The Cosmic Dragon Diff-Based Rendering Engine";
    let body = format!(
        "{engine_line}\n\
         Build: {build} ({commit})\n\
         Build-time: {build_time}\n\
         Signature: {signature}\n\
         Copyright: (c) 2026 {authors}\n\
         License: {license}\n\
         Source: {repository}",
        authors = env!("CARGO_PKG_AUTHORS"),
        license = env!("CARGO_PKG_LICENSE"),
        repository = env!("CARGO_PKG_REPOSITORY"),
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

/// Detailed engine documentation and architecture overview.
///
/// Printed by `cosmostrix --docs`. Intended for curious developers,
/// benchmarking enthusiasts, and anyone evaluating cosmostrix against other
/// terminal rain renderers. Output is plain text (no ANSI) so it pipes
/// cleanly into `less`, `grep`, or documentation generators.
///
/// NIGHT-docs-audit round 2026-09-12 (owner task: "audit to avoid stale
/// data and simplify --docs"): every constant below was re-verified
/// against its source (`central_control_rains/parallax.rs`, living_rain
/// symbol names, engine folder paths), the stale chroma phase-history
/// block was cut (RULES.md owns that detail), and the missing Crystal
/// Dragon section was added so the three-engine architecture matches
/// `docs/THREE_DRAGON_ENGINES.md`. Numbers cite their source location
/// so future audits can re-verify mechanically.
///
/// Version info is NOT included here to avoid duplicate versioning —
/// the user gets the version from `--version` / `-V`, which is the
/// single source of truth.
#[must_use]
pub(crate) fn docs_report() -> String {
    format!(
        "\
COSMOSTRIX — The Cosmic Dragon Diff-Based Rendering Engine
==========================================================

cosmostrix is not a Matrix clone. It is a novel diff-based terminal
renderer that emits only the cells which change between frames,
paired with three cooperating dragon engines: the Cosmic Dragon
(simulation + diff render loop), the Chroma Dragon (every color
decision), and the Crystal Dragon (ambient intelligence — palette
drift + time-of-day scenes). Full detail lives in
`docs/THREE_DRAGON_ENGINES.md` and `docs/RENDER_ENGINE.md`.


1. DIFF-BASED CELL RENDERER  (src/engine/cosmic_dragon_engine/terminal/, frame.rs)
-------------------------------------------------------------

Every other Matrix rain renderer writes the full screen every frame.
cosmostrix keeps a `Frame` back-buffer of `Cell` values (char + fg +
bg + bold, 16 bytes) and a `LastFrame` shadow of what the terminal
physically holds; at draw time only cells that differ are emitted,
and consecutive dirty cells on the same row are batched into one
RLE-style run so the terminal receives the minimum bytes possible.

  - Dirty tracking: double-buffered generation counters — a single
    u32 bump clears the dirty map per frame (no memset); dirty
    indices land in a SmallVec (no heap at typical sizes).
  - Cell-skip: full redraws skip cells whose frame value matches the
    shadow (HUNT-27); a shadow reset arms `force_full_emit` so the
    unknown physical state is re-emitted once (NIGHT-hunter-34 —
    the color-bg residue family).
  - RLE batching: runs share one SGR sequence and one cursor move;
    ~13x fewer I/O bytes on typical content, >90x at 400x200.


2. THREE-LAYER PARALLAX  (src/central_control_rains/parallax.rs)
-----------------------------------------------------------------

Rain is rendered as three independent layers (far / mid / near).
Three layers is the cinema-standard deep/mid/ground composition;
more would collapse perceptually in a 24-row terminal. Verified
multipliers (PARALLAX_* in parallax.rs):

  Layer   Speed   Bright   Length   Density   Phosphor decay
  far     0.35x   0.56     0.50     0.45      1.90x (faster fade)
  mid     1.00x   0.82     1.00     0.62      1.15x
  near    1.70x   1.08     1.40     0.85      0.65x (slower fade)

Layers are composited in Z-order into the same back-buffer, so the
diff renderer sees a single unified frame — parallax is invisible
to the I/O layer. Multipliers are applied at droplet birth.


3. PHOSPHOR PERSISTENCE  (src/engine/cosmic_dragon_engine/cloud/phosphor.rs)
-------------------------------------------------

CRT afterglow: every glyph leaves a fading residual trail. The
per-cell residual energy decays exponentially each frame.

  PHOSPHOR_TAIL_RESIDUAL = 160   (initial residual after head passes)
  PHOSPHOR_DECAY_RATE    = 8.0   (per-second exponential decay)
  Per-layer decay multiplier (see parallax table above)
  Bottom rows decay 1.8x faster (CRT geometry illusion)
  Edge energy cap (prevents phosphor buildup at borders)

Result: a few hundred ms of visible afterglow per glyph. The
residual is mixed into the back-buffer's color value, so the diff
renderer treats it as a normal color change — no special I/O path.


4. DENSITY NOISE & WIND GUSTS  (src/engine/cosmic_dragon_engine/cloud/living_rain.rs)
--------------------------------------------------------------------------------

Density is driven by a value-noise function sampled at column
position (`column_density_modifier`) — deterministic per terminal
size, never repeating row-by-row. Wind gusts are sparse global
events that briefly accelerate all columns in a direction, then
decay (`GustState`). Gusts break the visual monotony of
constant-velocity rain without per-column physics and are disabled
in benchmark mode for reproducibility.


5. CHROMA DRAGON COLORING ENGINE  (src/engine/chroma_dragon_engine/)
-----------------------------------------------

The coloring counterpart to the Cosmic Dragon: it owns every
decision about *what color a cell becomes*.

  palette    Palette struct, build_palette(), gradient + blend helpers,
             palette-relative brightness floor.
  catalog    THEMES registry — single source of truth for all 44
             builtin themes.
  gradient   OKLab polar interpolation — the sole production color
             path since v30 (hue-preserving, perceptually uniform).
  shaders    Base cell shader (resolve_cell_color), head halo,
             transition L + chroma smoothing.
  post       Climate post-FX (luminance, saturation, persistence,
             instability), palette-aware ghost + anomaly halos.
  tuning     All Chroma tuning constants in one auditable place.

The engine is locked at Phase 9-D with 19 CI-enforced invariants
(theme sweep, floor bounds, hierarchy, hue preservation, round-trip
accuracy, ...). The phase history and the full invariant list live
in `src/engine/chroma_dragon_engine/RULES.md` — not repeated here
(NIGHT-docs-audit: single source of truth, no drifting copies).


6. CRYSTAL DRAGON AMBIENT ENGINE  (src/engine/crystal_dragon_engine/)
--------------------------------------------------

The ambient intelligence engine, two subsystems in harmony:

  - Palette drift: CPU load (or UTC clock fallback) maps to a
    1-99 point system -> Cold / Medium / Hot theme groups -> weighted
    selection (calc-v2: CDF + a 8-entry DriftHistory recency ring
    that prevents A->B->A oscillation). Cadence is tunable via
    --crystal-dragon-secs (60s default, dwell floor min(60, cadence)).
  - Ambient scheduler: time-of-day scene entries (config
    `ambient.HH-MM = scene`), owner-override detection with snapback,
    and the runtime priority contract Ambient > Config > CLI.

The engines never share mutable state — they communicate through
the immutable `Cloud` snapshot each frame.


PERFORMANCE PROFILE
-------------------

Representative reference hardware (AMD Ryzen 7 5800HS, 8C/16T,
3.2 GHz baseline); numbers drift across hardware, compilers, and
workloads — re-verify with `cosmostrix --benchmark`:

  Screen size   avg_fps   ns/cell   I/O share   allocs/frame   peak_rss
  120x40        38,000+    ~12      <2%         0              4.7 MiB
  400x200       8,000+     ~14      <3%         0              9.2 MiB

  - Zero per-frame heap allocation (pools pre-allocated).
  - Single CPU core (no threads, no GPU, no SIMD required).
  - I/O share <5% means the engine is CPU-bound on simulation, not
    I/O-bound on terminal writes — what a diff engine should deliver.

See `docs/PERFORMANCE_ACROSS_SCALES.md` for the full scaling audit
from 6x6 to 400x200, including why `ns/cell` stays O(1) per cell.


DESIGN CONSTRAINTS
------------------

  - No GPU. No OpenGL, Vulkan, Metal, DirectX, or WebGPU context is
    ever created. The terminal is a text medium; its soul is ANSI
    escape sequences and copy-pasteable glyphs.
  - No `rand` dependency in the intro subsystem — XorShift32 only.
  - No unsafe in the renderer hot path.
  - Cross-platform: Linux, macOS, Windows, Android (Termux), FreeBSD.

cosmostrix is powered by The Cosmic Dragon Diff-Based Rendering
Engine — a serious diff-based rendering masterpiece. It is designed
for cinematic art, not for toys. By principle, it will never support
emoji or wide characters (CJK fullwidth, zero-width combining marks),
as its focus is on pure, elegant, and exclusive visual quality built
on single-cell glyphs.

Source: {repository}
License: {license}
",
        repository = env!("CARGO_PKG_REPOSITORY"),
        license = env!("CARGO_PKG_LICENSE"),
    )
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
pub(crate) fn estimate_memory_budget(w: u16, h: u16) -> usize {
    // Use actual Cell size rather than a magic number for accuracy
    let cell_size = std::mem::size_of::<crate::cell::Cell>();
    let frame_cells = (w as usize) * (h as usize) * cell_size;

    // Cloud internal buffers: char_pool (CHAR_POOL_SIZE), glitch_pool (GLITCH_POOL_SIZE), color_map, glitch_map
    let cloud_pools = CHAR_POOL_SIZE * 4 + GLITCH_POOL_SIZE * 4;
    let cloud_maps = (w as usize) * (h as usize) * 2; // color_map + glitch_map

    // Droplets: ~DROPLET_COUNT_FACTOR * cols droplets, each ~100 bytes
    let droplet_count = (DROPLET_COUNT_FACTOR * w as f32) as usize;
    let droplets_size = droplet_count * std::mem::size_of::<crate::droplet::Droplet>().max(100);

    // Terminal: LastFrame + row_dirty + touched_rows
    let terminal_last = (w as usize) * (h as usize) * cell_size;

    frame_cells * 2 + cloud_pools + cloud_maps + droplets_size + terminal_last
}

#[must_use]
pub(crate) fn format_bytes(bytes: usize) -> String {
    // Delegate to the centralized binary-byte formatter. This keeps a single
    // source of truth for byte-unit formatting across perf-stats, bench
    // reports, and diagnostics. See `diagnostics/humanize.rs`.
    crate::humanize::humanize_bytes(bytes as u64)
}

// --- CPU feature check ---

/// Runtime CPU feature check for x86-64 builds.
///
/// Detects if the CPU supports the required instruction set for the
/// compiled target level (v3 = AVX2, v4 = AVX-512). Prints a clear
/// error message and exits instead of crashing with SIGILL.
#[cfg(target_arch = "x86_64")]
pub(crate) fn check_cpu_features() {
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
        eprintln_safe!(
            "{} This binary requires {bold_on}{feature}{bold_off} ({target})",
            error_bold("FATAL:")
        );
        eprintln_safe!("       but your CPU does not support it.");
        eprintln_safe!();
    };

    if build.contains("-v4") {
        if !std::arch::is_x86_feature_detected!("avx512f") {
            print_fatal("AVX-512", "x86-64-v4");
            eprintln_safe!("Rebuild with a compatible target:");
            eprintln_safe!("  cargo pro-linux-v3    # x86-64-v3 (AVX2) — modern CPUs");
            eprintln_safe!("  cargo pro-linux-musl  # x86-64-v3 + musl static");
            std::process::exit(1);
        }
    } else if build.contains("-v3") && !std::arch::is_x86_feature_detected!("avx2") {
        print_fatal("AVX2", "x86-64-v3");
        eprintln_safe!("Rebuild with:");
        eprintln_safe!("  cargo pro-linux-musl  # x86-64-v3 + musl static (same baseline)");
        eprintln_safe!(
            "  Note: v1/v2 profiles were dropped in v10.0.0. Use musl for max compatibility."
        );
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_is_non_empty_and_stable() {
        assert!(!COSMIC_DRAGON_SIGNATURE.is_empty());
        assert!(COSMIC_DRAGON_SIGNATURE.contains("Cosmic Dragon"));
        assert!(COSMIC_DRAGON_SIGNATURE.contains("rezky_nightky"));
        assert!(COSMIC_DRAGON_SIGNATURE.contains("oxyzenQ"));
    }

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
        // The Engine: line declares the Cosmic Dragon diff-based rendering
        // architecture so users immediately see this is not a Matrix
        // clone. It must appear on its own line, between the description
        // header and the Build: line, so it's easy to grep from scripts.
        // The Engine line must NOT hardcode a version number — the version
        // is already shown on the `cosmostrix: v{VERSION}` line above.
        let report = version_report();
        assert!(
            report.contains("Engine: The Cosmic Dragon Diff-Based Rendering Engine"),
            "version_report must declare the Cosmic Dragon engine line. Full report:\n{report}"
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
    fn docs_report_is_non_empty() {
        let report = docs_report();
        assert!(!report.is_empty(), "docs_report must not be empty");
        assert!(
            report.lines().count() > 50,
            "docs_report should be a substantial document (got {} lines)",
            report.lines().count()
        );
    }

    #[test]
    fn docs_report_has_no_duplicate_versioning() {
        // The docs_report header must NOT contain a version number — the
        // version is the single source of truth from `--version` / `-V`.
        // Including it here would be "duplicate versioning" (the user
        // sees it twice: once in --version, once in --docs). The header
        // is just the engine name, no version suffix.
        let report = docs_report();
        let first_line = report.lines().next().unwrap_or("");
        assert!(
            first_line.contains("The Cosmic Dragon Diff-Based Rendering Engine"),
            "docs_report first line must contain the engine name: {first_line}"
        );
        // The header must NOT contain a version number — no "(v25)",
        // no "(v20)", no version at all. Version info is exclusively
        // in --version output.
        assert!(
            !first_line.contains("(v"),
            "docs_report first line must NOT contain a version suffix (duplicate versioning). Got: {first_line}"
        );
    }

    #[test]
    fn docs_report_mentions_all_five_subsystems() {
        // The report must describe all five cooperating subsystems so a
        // curious developer gets the complete picture from one command.
        let report = docs_report();
        assert!(
            report.contains("DIFF-BASED CELL RENDERER"),
            "docs_report must describe the diff-based cell renderer"
        );
        assert!(
            report.contains("THREE-LAYER PARALLAX") || report.contains("PARALLAX"),
            "docs_report must describe the 3-layer parallax"
        );
        assert!(
            report.contains("PHOSPHOR PERSISTENCE"),
            "docs_report must describe phosphor persistence"
        );
        assert!(
            report.contains("DENSITY NOISE") && report.contains("WIND GUSTS"),
            "docs_report must describe density noise and wind gusts"
        );
        assert!(
            !report.contains("ADAPTIVE ATMOSPHERE ENGINE"),
            "docs_report must NOT mention the eliminated atmosphere engine"
        );
    }

    /// NIGHT-docs-audit (2026-09-12): the --docs output must describe
    /// all THREE dragon engines (the pre-audit text predates the Crystal
    /// Dragon and claimed a "five subsystems + Chroma" architecture).
    #[test]
    fn docs_report_mentions_all_three_dragon_engines() {
        let report = docs_report();
        assert!(
            report.contains("CHROMA DRAGON COLORING ENGINE"),
            "docs_report must describe the Chroma Dragon engine"
        );
        assert!(
            report.contains("CRYSTAL DRAGON AMBIENT ENGINE"),
            "docs_report must describe the Crystal Dragon engine (added NIGHT-docs-audit 2026-09-12)"
        );
        assert!(
            report.contains("crystal_dragon_engine"),
            "docs_report must cite the crystal engine source folder"
        );
    }

    /// NIGHT-docs-audit: the numbers in --docs must match the live
    /// constants (the pre-audit text carried Deep-Focus-era brightness
    /// 0.40/0.75/1.00, density 0.30/0.60/1.00, decay 2.20/1.20/0.50 and
    /// PHOSPHOR_DECAY_RATE = 5.0 — all stale). Source of truth:
    /// src/central_control_rains/parallax.rs.
    #[test]
    fn docs_report_parallax_and_phosphor_numbers_match_source() {
        let report = docs_report();
        // PARALLAX_BRIGHTNESS_MULT = [0.56, 0.82, 1.08]
        assert!(
            report.contains("0.56     0.50     0.45"),
            "far-layer row must match parallax.rs"
        );
        assert!(
            report.contains("1.00x   0.82     1.00     0.62"),
            "mid-layer row must match parallax.rs"
        );
        assert!(
            report.contains("1.70x   1.08     1.40     0.85"),
            "near-layer row must match parallax.rs"
        );
        // PHOSPHOR_LAYER_DECAY_MULT = [1.9, 1.15, 0.65]; PHOSPHOR_DECAY_RATE = 8.0
        assert!(
            report.contains("PHOSPHOR_DECAY_RATE    = 8.0"),
            "decay rate must match parallax.rs"
        );
        assert!(
            report.contains("1.90x (faster fade)"),
            "far decay mult must match parallax.rs"
        );
        // PHOSPHOR_BOTTOM_DECAY_MULT = 1.8
        assert!(
            report.contains("1.8x faster"),
            "bottom-row decay must match parallax.rs"
        );
    }

    /// NIGHT-docs-audit: stale symbol/path claims must stay gone —
    /// the pre-audit text referenced `density_noise_at` (renamed to
    /// `column_density_modifier`), `cloud::spawn::DropletSpawner`
    /// (symbol no longer exists), and placed the PARALLAX constants in
    /// `src/constants.rs` (they live in central_control_rains/parallax.rs).
    #[test]
    fn docs_report_has_no_stale_symbols_or_paths() {
        let report = docs_report();
        assert!(
            !report.contains("density_noise_at"),
            "stale symbol: density_noise_at was renamed column_density_modifier"
        );
        assert!(
            !report.contains("DropletSpawner"),
            "stale symbol: DropletSpawner no longer exists in cloud/spawn.rs"
        );
        assert!(
            !report.contains("src/constants.rs"),
            "stale path: PARALLAX constants live in central_control_rains/parallax.rs"
        );
        assert!(
            report.contains("column_density_modifier"),
            "the live density-noise symbol must be cited"
        );
        assert!(
            report.contains("central_control_rains/parallax.rs"),
            "the live parallax constant location must be cited"
        );
    }

    #[test]
    fn docs_report_references_performance_doc() {
        // The report should point readers at the detailed scaling audit
        // for reproducible benchmark numbers.
        let report = docs_report();
        assert!(
            report.contains("PERFORMANCE_ACROSS_SCALES.md"),
            "docs_report should reference docs/PERFORMANCE_ACROSS_SCALES.md"
        );
    }

    #[test]
    fn docs_report_declares_not_a_clone() {
        // The manifesto line — must be present so the architecture
        // declaration is unambiguous.
        let report = docs_report();
        assert!(
            report.contains("not a Matrix clone"),
            "docs_report must declare that cosmostrix is not a Matrix clone"
        );
    }

    /// (philosophy declaration): the --docs output must carry the
    /// condensed masterpiece philosophy so the engine's identity is embedded
    /// in the binary itself, not just in the README. This locks in the
    /// exact wording requested for the formal philosophy declaration.
    ///
    /// Note: the report wraps at ~70 cols for terminal display, so we
    /// normalize whitespace before substring-matching (a wrapped sentence
    /// like "serious diff-based rendering\nmasterpiece" must still match).
    #[test]
    fn docs_report_declares_masterpiece_philosophy() {
        let report = docs_report();
        let normalized: String = report.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            normalized.contains("cosmostrix is powered by The Cosmic Dragon Diff-Based Rendering"),
            "docs_report must open the philosophy with the engine name"
        );
        assert!(
            normalized.contains("serious diff-based rendering masterpiece"),
            "docs_report must declare masterpiece status (wrapped text normalized)"
        );
        assert!(
            normalized.contains("cinematic art, not for toys"),
            "docs_report must declare the cinematic-art-not-toys stance"
        );
        assert!(
            normalized.contains("will never support emoji"),
            "docs_report must declare the permanent no-emoji constraint"
        );
        assert!(
            normalized.contains("pure, elegant, and exclusive visual quality"),
            "docs_report must declare the visual-quality focus"
        );
    }

    #[test]
    fn info_file_stays_under_loc_cap() {
        // Hard cap per src/RULES_LOC.md (owner mandate 2026-08-28).
        let source = include_str!("info.rs");
        let lines = source.lines().count();
        assert!(
            lines < 800,
            "info.rs must stay under 800 LOC (currently {lines})"
        );
    }
}
