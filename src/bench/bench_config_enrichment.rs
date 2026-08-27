// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CONFIG-enrichment fields derived from CloudConfig for benchmark reports.

use crate::app::CloudConfig;

/// CONFIG-enrichment fields derived from CloudConfig for benchmark reports.
///
/// All fields are derived from `cfg` so both `run_premium_benchmark` and
/// `run_premium_benchmark_silent` emit identical values without duplicating
/// the derivation logic.
///
/// (chroma dragon audit): `color_pipeline` and `chroma_in_benchmark` disclose
/// (a) which color pipeline the run is using (`chroma_dragon` or `legacy_rgb`)
/// and (b) the chroma engine status during benchmarking. Owner question:
/// "when benchmarking mode 'cosmostrix --benchmark' is the chroma dragon
/// enable/disable?" Answer: chroma is ENABLED in benchmark mode -- crystal_dragon
/// palette drift is forced OFF (see `cloud.crystal_dragon = false` in
/// bench.rs entry points), the chroma engine itself still runs every cell. The `chroma_in_benchmark`
/// field makes this explicit in the report so the user does not have to read
/// the source to find out.
pub(crate) struct ConfigEnrichment {
    /// Resolved color mode label ("truecolor", "256", etc.).
    pub color_mode_label: &'static str,
    /// Custom palette name, if active.
    pub custom_palette_name: Option<String>,
    /// Custom palette background as hex string, if palette defines one.
    pub custom_palette_bg_hex: Option<String>,
    /// Resolved color background label.
    pub color_bg_label: &'static str,
    /// Compact color-tune summary string.
    pub color_tune_summary: String,
    /// Whether async mode is enabled.
    pub async_mode: bool,
    /// Whether glitch is enabled.
    pub glitch_enabled: bool,
    /// Glitch level label ("none", "subtle", "default", "intense").
    pub glitch_level: &'static str,
    /// Glitch percentage.
    pub glitch_pct: f32,
    /// Active color pipeline label.
    pub color_pipeline: &'static str,
    /// Chroma engine status during benchmark.
    pub chroma_in_benchmark: &'static str,
    /// PERF-2: power_dragon state (transparency — not a bench throttle).
    pub power_dragon: bool,
    /// PERF-2: crystal_dragon state (false in bench for determinism).
    pub crystal_dragon: bool,
    /// PERF-2: msg_mode state (skipped in bench per Z-6, shown for parity).
    pub msg_mode: bool,
    /// PERF-2: intro type label (not rendered in bench, config parity).
    pub intro: &'static str,
    /// PERF-2-Supreme: particle effects disabled state (--no-effects).
    /// `true` = ALL particle subsystems are no-ops (effects_enabled = false):
    /// quantum ripple, border spark, mouse-click flash waves, anomaly zones.
    /// Auto-enabled in bench mode (--benchmark/--bench-all/--bench-frames):
    /// particles are input-driven and never spawn during a benchmark
    /// anyway, so effects_enabled is forced false at CloudConfig
    /// construction time. This field is pure transparency — it never
    /// changes bench numbers; it just reports the CLI/config value.
    pub no_effects: bool,
}

/// Compute the CONFIG-enrichment fields from a CloudConfig.
///
/// Kept as a free function (not a method on CloudConfig) so it can be unit-tested
/// in isolation and stays out of the hot measurement path.
pub(crate) fn compute_config_enrichment(cfg: &CloudConfig) -> ConfigEnrichment {
    use crate::cli::color_mode_label;
    use crate::palette;
    use crate::runtime::ColorPipeline;

    let color_mode_label = color_mode_label(cfg.color_mode);

    // Custom palette name + bg hex (None when no custom palette is active).
    let (custom_palette_name, custom_palette_bg_hex) = match &cfg.custom_palette {
        Some(p) => {
            let name = cfg.custom_palette_name.clone();
            // Only surface the bg hex when the palette actually defines one.
            // When the palette has no bg field, we emit nothing so the
            // downstream `color_bg` field is the sole authority on background.
            let hex = p.bg.map(|c| palette::format_color_hex(Some(c)));
            (name, hex)
        }
        None => (None, None),
    };

    // --color-bg label. Mirrors verbose.rs::describe_color_bg priority:
    // custom palette bg (if present) overrides --color-bg; otherwise the
    // --color-bg setting stands. We emit a compact label here because the
    // full descriptive string is too verbose for the benchmark CONFIG block.
    //
    // Note: CloudConfig stores `default_bg: bool` (true = DefaultBackground,
    // false = Black) rather than the full ColorBg enum, so we reverse-map here.
    // The custom-palette-bg override check happens first to match the actual
    // runtime behavior in app.rs::create_cloud (set_palette overwrites bg).
    let color_bg_label: &'static str = if cfg.custom_palette.is_some()
        && cfg.custom_palette.as_ref().and_then(|p| p.bg).is_some()
    {
        // Custom palette's bg field wins — the --color-bg flag is moot.
        "custom-palette-bg"
    } else if cfg.default_bg {
        "default-background"
    } else {
        "black"
    };

    // Compact color-tune summary (mirrors verbose format).
    let color_tune_summary = format!(
        "sat={:.2} bright={:.2} head={:.2} body={:.2} tail={:.2}",
        cfg.color_tune.saturation,
        cfg.color_tune.brightness,
        cfg.color_tune.head,
        cfg.color_tune.body,
        cfg.color_tune.tail
    );

    let async_mode = cfg.async_mode;
    let glitch_enabled = cfg.glitch_enabled;
    // Derive glitch_level label from glitch_pct (CloudConfig doesn't carry
    // the GlitchLevel enum, only the resolved pct). Thresholds match
    // cloud/scene_runtime.rs:
    //   0.0 → none, <5.0 → subtle, <15.0 → default, >=15.0 → intense
    let glitch_level: &'static str = if !glitch_enabled || cfg.glitch_pct < 0.01 {
        "none"
    } else if cfg.glitch_pct < 5.0 {
        "subtle"
    } else if cfg.glitch_pct < 15.0 {
        "default"
    } else {
        "intense"
    };
    let glitch_pct = cfg.glitch_pct;
    // benchmark mode always disables palette drift
    // to keep p99/max metrics clean.
    // The report must reflect the actual cloud state, not the user's --crystal-dragon
    // flag — otherwise the disclosure violates the honesty contract.

    // (chroma dragon audit): detect the active color pipeline from the
    // color mode. The chroma engine itself is NOT disabled in benchmark mode
    // — only palette drift is disabled (palette rebuilds inject timing
    // spikes that corrupt p99/max). Climate drift still runs because it is
    // deterministic (fixed RNG seed) and has no rebuild cost. The benchmark
    // report must disclose both facts so the user can answer "is the chroma
    // dragon running during benchmark?" without reading the source.
    let pipeline = ColorPipeline::detect(cfg.color_mode);
    let color_pipeline_label = pipeline.label();
    let chroma_in_benchmark: &'static str = if !pipeline.is_chroma() {
        "legacy fallback (color mode lacks truecolor; no chroma engine in benchmark either)"
    } else if cfg.crystal_dragon {
        "chroma enabled (crystal_dragon OFF for determinism, climate_drift active)"
    } else {
        "chroma enabled (crystal_dragon was already off, climate_drift active)"
    };

    // PERF-2: dragon system + message/intro state for CONFIG transparency.
    // These are NOT throttles in bench mode (PERF-1 audit confirmed),
    // but owner wants them shown so users can verify their config.
    let power_dragon = cfg.power_dragon;
    let crystal_dragon = cfg.crystal_dragon;
    let msg_mode = cfg.msg_mode;
    let intro: &'static str = match cfg.intro {
        crate::config::IntroType::Logo => "logo",
        crate::config::IntroType::Cosmic => "cosmic",
        crate::config::IntroType::None => "none",
    };

    // PERF-2-Supreme: --no-effects state (inverted into the
    // owner-requested `no_effects` naming: true = effects OFF).
    // `effects_enabled` gates all particle subsystems — quantum-ripple
    // mouse-click burst, border-touch splash crown spark, mouse-click
    // flash waves, and anomaly zones — all input-driven and never fire
    // during a benchmark run, so the value never changes bench numbers.
    // Auto-enabled in bench mode: effects_enabled is forced false at
    // CloudConfig construction when bench_mode is true. Reported for
    // CONFIG transparency only.
    let no_effects = !cfg.effects_enabled;

    ConfigEnrichment {
        color_mode_label,
        custom_palette_name,
        custom_palette_bg_hex,
        color_bg_label,
        color_tune_summary,
        async_mode,
        glitch_enabled,
        glitch_level,
        glitch_pct,
        color_pipeline: color_pipeline_label,
        chroma_in_benchmark,
        power_dragon,
        crystal_dragon,
        msg_mode,
        intro,
        no_effects,
    }
}
