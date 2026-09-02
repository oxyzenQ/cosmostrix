// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Application configuration: CloudConfig struct and density calculation helpers.

use crate::cloud::Cloud;
use crate::constants::*;
use crate::intro_style::IntroType;
use crate::msg_fill_style::MsgFillStyle;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, MonolithSize, ShadingMode};

// --- CloudConfig struct for deduplicating cloud initialization ---

/// Aggregated configuration for creating and running a `Cloud` instance.
/// Collected from CLI args and config file, then passed to the interactive
/// loop or benchmark runner.
#[derive(Clone)]
pub struct CloudConfig {
    pub color_mode: ColorMode,
    pub shading_mode: ShadingMode,
    pub bold_mode: BoldMode,
    pub async_mode: bool,
    pub default_bg: bool,
    pub color_scheme: ColorScheme,
    /// Custom palette override (v16). When Some, the cloud uses this palette
    /// instead of the built-in palette from color_scheme. The color_scheme
    /// enum is still tracked for verbose output + cycling, but the actual
    /// colors come from this palette.
    pub custom_palette: Option<crate::palette::Palette>,
    /// Name of the active custom palette (for live reload). When Some,
    /// rebuild_cloud_config reloads the palette definition from config
    /// so editing colors-custom entries takes effect immediately.
    pub custom_palette_name: Option<String>,
    pub rain_style: RainStyle,
    /// Glitch enable flag, derived from `glitch_level != GlitchLevel::None`.
    /// Replaces the old `noglitch: bool` field (v30 simplify: --noglitch CLI
    /// flag removed; positive polarity is clearer and matches `cloud.glitchy`).
    pub glitch_enabled: bool,
    /// v50.0.0-beta.7: glitch level preset (none/subtle/default/intense)
    /// stored on CloudConfig so the HUD can display it without reaching
    /// back into Args. Set during CloudConfig construction in main.rs.
    pub glitch_level: crate::config::GlitchLevel,
    pub glitch_pct: f32,
    pub glitch_low: u16,
    pub glitch_high: u16,
    pub linger_low: u16,
    pub linger_high: u16,
    pub short_pct: f32,
    pub die_early_pct: f32,
    pub max_dpc: u8,
    pub density: f32,
    pub speed: f32,
    pub monolith_size: MonolithSize,
    pub chars: Vec<char>,
    pub message: Option<String>,
    pub message_border: bool,
    /// v80.0.0-beta.1 msg-fill-style: message overlay reveal style (typewriter /
    /// fade / words / slide / instant / engrave / hologram / glitch /
    /// scorch / cascade). Default `Typewriter` = bit-identical
    /// to the pre-v80.0.0-beta.1 renderer (LTS guarantee). Applied in `create_cloud`
    /// via `cloud.set_msg_fill_style` (engrave arms the spark sidecar;
    /// hologram adds a stateless scanline pass; glitch extends
    /// `CellReveal` with `glyph_override`; scorch extends `CellReveal`
    /// with `tint` and adds a smoke sidecar; cascade reuses the signed
    /// `slide_rows` field for drop-from-above).
    pub msg_fill_style: MsgFillStyle,
    pub target_fps: f64,
    /// (FPS-F1): xterm.js host + 30 FPS cap, copied from `TerminalCaps`
    /// at startup so the event loop's live-reload path can re-apply the cap
    /// when the user edits `fps =` in config.toml. See `resolve_capped_fps`.
    pub(crate) xtermjs_host: bool,
    pub(crate) default_fps_cap: f64,
    pub duration: Option<f64>,
    pub duration_s: Option<f64>,
    pub bench_frames: Option<u64>,
    pub benchmark: bool,
    /// Optional benchmark duration override in seconds.
    /// When None, defaults to BENCHMARK_DURATION_SECS (5s).
    /// Resolved exclusively from --bench-duration (bare seconds or compound
    /// like 6s/30m/1h30m). The hidden --duration flag is interactive-mode
    /// only (auto-exit deadline) and is NOT consulted by the benchmark
    /// dispatcher.
    pub bench_duration: Option<u64>,
    /// Parsed --screen-size WxH value. None means dynamic (use terminal size).
    /// When set, benchmark uses this fixed size; interactive renders to fixed virtual size.
    pub screen_size: Option<(u16, u16)>,
    /// Parsed --color-tune value. None means no tune (identity).
    pub color_tune: crate::color_tune::ColorTune,
    /// Output benchmark report as JSON (--json flag).
    pub json: bool,
    /// --save-baseline PATH: save benchmark JSON to file
    pub save_baseline: Option<String>,
    /// --compare-baseline PATH: compare against saved baseline
    pub compare_baseline: Option<String>,
    /// --bench-io: wet terminal I/O benchmark (write to /dev/null)
    pub bench_io: bool,
    /// --bench-all: run scaling benchmark across multiple sizes
    pub bench_all: bool,
    /// --bench-scene <NAME>: bench I/O scene selector. None = default
    /// (emit_cell_lean); Some("production-draw") routes the writer through
    /// the production Terminal::draw hot path (MoveTo per row + ColorCache
    /// SGR + BOLT bold escape) so the BOLT-backed production path is
    /// measurable. Requires --bench-io.
    pub bench_scene: Option<String>,
    /// --verbose flag: print diagnostic info to stderr.
    pub verbose: bool,
    pub density_auto: bool,
    pub base_density: f32,
    pub perf_stats: bool,
    pub screensaver: bool,
    pub intro: IntroType,
    /// v50: Optional intro color override. When set, the intro animation
    /// uses this color theme instead of the rain color. Config-only
    /// (no CLI flag). Values: builtin theme name or custom palette name.
    pub intro_color: Option<String>,
    pub mouse: bool,
    pub charset_preset: String,
    pub user_ranges: Vec<(char, char)>,
    pub def_ascii: bool,
    /// Crystal Dragon Engine: ambient intelligence for palette drift.
    pub crystal_dragon: bool,
    /// v50: Power Dragon toggle. When false, disables aggressive_throttle
    /// and idle FPS reduction. Default: true (protection enabled).
    pub power_dragon: bool,
    /// v50-beta.3: msg-mode master switch for the message overlay subsystem.
    /// When false, disables BOTH the default message AND any message/
    /// message-border config key. CLI -m/-mb always wins (handled in
    /// main.rs). Default: true (message overlay active).
    pub msg_mode: bool,
    /// PERF-4: particle effects enabled flag. When false, ALL cosmetic
    /// subsystems are disabled — even the most valuable ones (owner
    /// v80.0.0-alpha.1 directive: "disable all cosmetic effects to peak
    /// optimize performance"): quantum ripple, border spark, mouse-click
    /// flash waves, anomaly zones, ghost events, emergent storytelling
    /// moments, msg-fill-style particle sidecars (engrave spark /
    /// hologram scanline / scorch smoke), the CRT vignette post-process,
    /// and the cursor hover glow. Set from CLI --no-effects. Default:
    /// true (effects on). Auto-forced false in benchmark mode.
    /// Rain-core visuals (droplet trails, phosphor fade, palette wave
    /// transitions, climate drift) are NOT cosmetics — they stay on.
    pub effects_enabled: bool,
    /// Path to the config file being watched for live reload.
    /// None = no watcher (CLI-only run, no config file).
    pub(crate) config_path_for_watcher: Option<std::path::PathBuf>,
    /// Resolved scene name for this session. Used to initialize the
    /// event loop's scene_name (for verbose output and interactive cycling).
    pub(crate) scene_name: String,
    /// Name of the active custom scene (set via `--scene-custom <name>`).
    /// When Some, `rebuild_cloud_config` looks up `[scene-custom.<name>]`
    /// in the new config and applies its fields on top of the base
    /// CloudConfig so live-edits to a custom scene take effect immediately.
    /// v20: custom scenes are first-class citizens — this field is the
    /// bridge that lets live reload track which custom scene is active.
    pub(crate) scene_custom_name: Option<String>,
    /// v80.0.0-beta.2 (S-master-HUNT): is the active scene-custom layer
    /// owned by RUNTIME CONFIG intent, or by the startup LOCK?
    ///
    /// - `true`  — the layer was selected by config-side intent at runtime:
    ///   the config `scene` key names the custom scene (the scene block in
    ///   `rebuild_cloud_config`), or the ambient scheduler applied a custom
    ///   scene (the runtime-scene sync writes the tracker). The tail block
    ///   re-applies the block's fields — a config-selected block is present
    ///   config content and wins at runtime (S-master-LOGIC-3).
    /// - `false` — the tracker reflects the LOCKED startup resolution
    ///   (startup construction or `restore_locked_scene_family`). The
    ///   startup snapshot already resolved the block layer correctly
    ///   (explicit CLI flags shadow block fields, everything else carries
    ///   the block values), so the tail block must NOT re-derive the
    ///   fields over the lock — that stomped CLI-shadowed values and kept
    ///   a REMOVED config scene's profile alive after the overlay lifted
    ///   (owner bug: `--scene tron_legacy -c test -C test` + comment out
    ///   `scene`/`ambient.*` -> charset/color stuck on the block values
    ///   instead of returning to the CLI setup).
    pub(crate) scene_custom_config_owned: bool,
    /// Bug 3 fix: tracks which CloudConfig fields were set explicitly via
    /// CLI flags (vs derived from config.toml or scene defaults).
    ///
    /// The priority contract is **CLI > config.toml > scene default**.
    /// At startup, `apply_config_and_runtime_defaults` records which fields
    /// the user set on the command line (via clap's `ValueSource::CommandLine`).
    /// On live reload, `rebuild_cloud_config` consults this tracker to skip
    /// applying config.toml values for fields the user explicitly pinned via
    /// CLI — preserving the CLI's authority across reloads.
    ///
    /// Without this tracker, a user running `cosmostrix -c green` would
    /// have their CLI `--color green` overridden the moment they edit
    /// `color = "snow"` in config.toml during live reload. That violates
    /// the priority contract.
    pub(crate) cli_explicit: CliExplicit,
    /// Ambient phase schedule — collected from `ambient.<HH-MM>` config keys
    /// by `crate::crystal_dragon_engine::ambient::collect_ambient_schedule`. Empty = no ambient
    /// entries (scheduler thread idles). The event loop spawns an
    /// `AmbientSchedulerHandle` from this and reloads it on every
    /// live-reload (see `event_loop.rs`).
    pub(crate) ambient_schedule: crate::crystal_dragon_engine::ambient::AmbientSchedule,
    /// v50.0.0-beta.7: Config-tunable ambient auto-snapback delay (seconds).
    /// After the user presses `x`/`c`/`s` (manual override) and is then
    /// idle for this many seconds, the event loop automatically re-applies
    /// the current ambient phase. None = use the default
    /// (AUTO_SNAPBACK_DELAY_SECS = 30.0). Set via `ambient-snapback-secs`
    /// in config.toml (range 0.0..=86400.0). Setting to 86400 (24h)
    /// effectively disables snapback; 0.0 means instant snapback.
    pub(crate) ambient_snapback_secs: Option<f64>,
    /// v80.0.0-alpha.1: Crystal Dragon polling interval (seconds) — the
    /// harmony twin of `ambient_snapback_secs`. None = default 60.0
    /// (CRYSTAL_DRAGON_POLLING_SECS). Set via `--crystal-dragon-secs` or
    /// the `crystal-dragon-secs` config key (range 0.0..=86400.0,
    /// validated identically to ambient-snapback-secs). Applied to
    /// `cloud.crystal_dragon_control.polling_secs` in create_cloud and
    /// re-applied on live-reload (config key present wins over the CLI
    /// lock; absent key keeps the locked startup value).
    pub(crate) crystal_dragon_secs: Option<f64>,
}

impl CloudConfig {
    /// v50.0.0-beta.7: resolve the effective ambient snapback delay.
    /// None = default AUTO_SNAPBACK_DELAY_SECS (30.0); Some(n) = user-set.
    #[must_use]
    pub(crate) fn effective_snapback_delay(&self, default: f64) -> f64 {
        self.ambient_snapback_secs.unwrap_or(default)
    }

    /// v80.0.0-alpha.1: resolve the effective Crystal Dragon poll interval
    /// (f32 for the engine control field). None = default 60.0; Some(n) =
    /// user-set via CLI/config/live-reload.
    #[must_use]
    pub(crate) fn effective_crystal_dragon_secs(&self, default: f32) -> f32 {
        self.crystal_dragon_secs.map_or(default, |s| s as f32)
    }
}

/// Per-field record of which CloudConfig fields were set via CLI.
///
/// v80.0.0-beta.1 (owner contract, 2026-09-01) — the flags are the CLI LOCK, not a
/// blocker:
///
/// ```text
/// Startup:  CLI > config.toml > scene defaults > built-in defaults
/// Runtime:  config key > CLI lock > scene defaults > built-in defaults
/// ```
///
/// At startup the CLI flags win over config.toml. After startup an
/// explicit config key overrides the flag (the file edit is the most
/// recent user intent) — but the CLI value stays LOCKED underneath:
/// when the key is commented out, the engine falls back to the locked
/// startup value without an exit + rerun. `rebuild_cloud_config` reads
/// these flags for the fallback arms (color.tune / message / msg-mode)
/// and the scene-default gates (CLI lock > scene-managed defaults);
/// `any()` drives the ambient startup deferral (ANY CLI flag present
/// → ambient waits for the snapback delay).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct CliExplicit {
    pub color: bool,
    pub charset: bool,
    pub speed: bool,
    pub density: bool,
    pub fps: bool,
    pub scene: bool,
    pub glitch_level: bool,
    /// Track whether `--crystal-dragon` was set on CLI (intent
    /// preservation: CLI flag wins over config.toml on live reload).
    pub crystal_dragon: bool,
    /// v80.0.0-alpha.1: track `--crystal-dragon-secs` CLI explicit — a
    /// tuning flag still counts as user intent for the v80 ambient
    /// startup deferral contract ("ANY CLI flag present → ambient waits
    /// for the snapback delay"), so any() covers it by construction.
    pub crystal_dragon_secs: bool,
    /// v50.0.0-alpha.7: track `--power-dragon` CLI explicit (was missing;
    /// live-reload path overrode CLI intent on config edit).
    pub power_dragon: bool,
    /// v50.0.0-alpha.7: track `--async-mode` CLI explicit (was missing;
    /// live-reload path overrode CLI intent on config edit).
    pub async_mode: bool,
    /// v50.0.0-alpha.7: track `--msg-mode` CLI explicit (was missing;
    /// needed for live-reload msg-mode gate).
    pub msg_mode: bool,
    /// v80.0.0-beta.1 msg-fill-style: track `-mfs`/`--msg-fill-style` CLI explicit
    /// (intent preservation: CLI flag wins over config.toml on live
    /// reload, same contract as every other flag).
    pub msg_fill_style: bool,
    /// v50.0.0-alpha.7: track `--intro-color` CLI explicit (was missing;
    /// needed for live-reload intro-color validation).
    pub intro_color: bool,
    /// v50.0.0-alpha.7: track `-m` / `-mb` CLI explicit (was missing;
    /// needed for live-reload message override).
    pub message: bool,
    /// v50.0.0-alpha.7: track `--monolith-size` CLI explicit (was missing;
    /// live-reload path overrode CLI intent on config edit — Issue #4).
    pub monolith_size: bool,
    /// v50.0.0-alpha.7: track `--color-tune` CLI explicit (was missing;
    /// needed for live-reload color.tune reset-on-comment fix).
    pub color_tune: bool,
    /// Z-master-2-v2: track `--bold` CLI explicit (was missing — same bug
    /// class as the monolith-size Issue #4: the config `bold` key silently
    /// overrode the CLI flag on every live-reload).
    pub bold: bool,
    /// Z-master-2-v2: track `--shading-mode` CLI explicit (was missing —
    /// same bug class; the config `shading-mode` key overrode the flag on
    /// reload).
    pub shading_mode: bool,
    /// Z-master-2-v2: track `--color-bg` CLI explicit (was missing —
    /// the config `color-bg` key overrode the flag on reload).
    pub color_bg: bool,
    /// Z-master-2-v2: track `--colors-custom` CLI explicit (was missing —
    /// switching the config `color` key to a builtin cleared the CLI-owned
    /// custom palette on reload; startup never does this).
    pub colors_custom: bool,
    /// Z-master-2-v2: track `--scene-custom` CLI explicit (was missing —
    /// a config `scene` key silently replaced the CLI-selected custom
    /// scene on reload; startup applies the CLI custom scene last).
    pub scene_custom: bool,
}

impl CliExplicit {
    /// True when ANY CLI flag was explicitly set.
    ///
    /// Drives the v50.0.0-beta.7 ambient startup deferral ("CLI wins
    /// first, then ambient takes over after ambient-snapback-secs").
    /// v80.0.0-beta.1: previously an inline `||` chain in event_loop.rs listing
    /// only 15 of the 21 flags — `--bold`, `--shading-mode`,
    /// `--color-bg`, `--colors-custom`, `--scene-custom`, and
    /// `-mfs`/`--msg-fill-style` did NOT defer ambient even though the
    /// documented rule says "ANY CLI flag". The method covers all
    /// fields, so future flags are included by construction.
    #[must_use]
    pub(crate) const fn any(self) -> bool {
        self.color
            || self.charset
            || self.speed
            || self.density
            || self.fps
            || self.scene
            || self.glitch_level
            || self.crystal_dragon
            || self.crystal_dragon_secs
            || self.power_dragon
            || self.async_mode
            || self.msg_mode
            || self.msg_fill_style
            || self.intro_color
            || self.message
            || self.monolith_size
            || self.color_tune
            || self.bold
            || self.shading_mode
            || self.color_bg
            || self.colors_custom
            || self.scene_custom
    }
}

impl CloudConfig {
    /// (FPS-F1): resolve `target_fps` for live-reload, re-applying
    /// the xterm.js 30 FPS cap. Without this, a user in VSCode could edit
    /// `fps = 240` in config.toml and resurrect the multi-hour OOM crash
    /// Tier 2 was designed to prevent. Native terminals have
    /// `default_fps_cap = 240.0` (effectively uncapped — startup clamps
    /// to [1, 240]). Falls back to `fallback_fps` if `self.target_fps`
    /// is ≤ 0.
    pub(crate) fn resolve_capped_fps(&self, fallback_fps: f64) -> f64 {
        let raw = if self.target_fps > 0.0 {
            self.target_fps
        } else {
            fallback_fps.max(1.0)
        };
        if self.xtermjs_host && raw > self.default_fps_cap {
            crate::lr_trace!(
                "live-reload: xterm.js host — capping fps {:.1}→{:.0}",
                raw,
                self.default_fps_cap,
            );
            self.default_fps_cap
        } else {
            raw
        }
    }

    pub fn create_cloud(&self, density: f32) -> Cloud {
        let mut cloud = Cloud::new(
            self.color_mode,
            self.shading_mode,
            self.bold_mode,
            self.async_mode,
            self.default_bg,
            self.color_scheme,
            self.rain_style,
        );

        cloud.glitchy = self.glitch_enabled;
        cloud.set_glitch_pct(self.glitch_pct / 100.0);
        cloud.set_glitch_times(self.glitch_low, self.glitch_high);
        cloud.set_linger_times(self.linger_low, self.linger_high);
        cloud.short_pct = self.short_pct / 100.0;
        cloud.die_early_pct = self.die_early_pct / 100.0;
        cloud.set_max_droplets_per_column(self.max_dpc);

        // Effective runtime values = base values (atmosphere engine eliminated
        // 2026-08-05 at commit 07b44b5; default modulation was always identity
        // even when it existed, so this assignment is unchanged in behavior).
        cloud.set_droplet_density(density);
        cloud.set_chars_per_sec(self.speed);
        cloud.set_monolith_size(self.monolith_size);

        cloud.init_chars(self.chars.clone());
        cloud.reset(DENSITY_AUTO_DEFAULT_COLS, DENSITY_AUTO_DEFAULT_LINES);

        // v16: Apply custom palette AFTER cloud.reset() to guarantee no
        // initialization code overwrites it. set_palette handles color_map
        // regen + transition wave + monolith reset internally.
        // v80.0.0-beta.2 HUD honesty: pass the palette NAME so the Cloud
        // tracks it for the `clr:` HUD line on every activation path.
        if let Some(ref custom) = self.custom_palette {
            cloud.set_palette(self.custom_palette_name.as_deref(), custom.clone());
        }

        // Apply --color-tune (if non-identity) to the palette AFTER custom
        // palette injection. This lets users tune custom palettes the same
        // way as built-in ones.
        crate::color_tune::apply_tune_to_palette(
            &mut cloud.palette,
            self.color_mode,
            &self.color_tune,
        );

        // v17 mastery: hover/click visual effects are ALWAYS ON (--mouse flag
        // deleted). Mouse reporting is also always on (terminal-level, blocks
        // text selection). cloud.mouse_enabled now always true.
        cloud.mouse_enabled = true;

        // Crystal Dragon Engine: when enabled, activates the point-based
        // temperature group system for palette drift.
        cloud.crystal_dragon = self.crystal_dragon;
        // v80.0.0-alpha.1: apply the tunable polling interval (the
        // "future CLI flags can override crystal_dragon_control here"
        // promise from the original comment is now real for this field).
        // min_dwell_secs stays at its 60s anti-flicker constant — a
        // deliberate floor, NOT config-exposed (over-engineering guard:
        // sub-minute palette flipping would read as flicker).
        cloud.crystal_dragon_control.polling_secs = self.effective_crystal_dragon_secs(
            crate::crystal_dragon_engine::crystal_dragon_control::CRYSTAL_DRAGON_POLLING_SECS,
        );
        // Z-master-1X bug fix: track whether the ambient scheduler has any
        // entries. When the schedule is empty, the drift gate in
        // `cloud/post_rain.rs` MUST NOT consult `user_override_since_ambient`
        // (which is forced to `true` at startup by `event_loop_setup.rs` to
        // protect the first live reload, and is only cleared by an ambient
        // fire — see commit 2b0e28b). Without this gate, ambient-off +
        // crystal-dragon-on would never drift, even though the HUD reports
        // `crdr: on`. The schedule presence is the authoritative signal
        // because ambient fire is the only mechanism that clears the flag.
        cloud.ambient_schedule_active = !self.ambient_schedule.entries.is_empty();
        // (crystal_dragon_control polling override applied above — the
        // sensor/control were already initialized in Cloud::new().)

        // v30 strengthen (Bug #4): if a custom palette is active, drift's
        // set_color_scheme would overwrite the user's custom palette with a
        // built-in one (silent data loss). Track this so the rain loop can
        // suppress palette drift while still allowing climate drift (which
        // only modulates rendering params, not the palette itself).
        cloud.custom_palette_active = self.custom_palette.is_some();

        // v30 strengthen (Bug #5): store color_tune on Cloud so that
        // set_color_scheme can re-apply it after rebuilding the palette.
        // Without this, the first palette drift would silently drop the
        // user's --color-tune settings.
        cloud.color_tune = self.color_tune;

        if let Some(msg) = &self.message {
            cloud.set_message_border(self.message_border);
            // v80.0.0-beta.1 msg-fill-style: applied before set_message so the very
            // first reveal (post-intro) already uses the user's style.
            cloud.set_msg_fill_style(self.msg_fill_style);
            cloud.set_message(msg);
        } else {
            // No message: still track the style so a later live-reload
            // that adds a message uses the configured style.
            cloud.set_msg_fill_style(self.msg_fill_style);
        }

        cloud
    }

    /// Clone the config for scaling benchmark (bench-all).
    /// Only copies fields needed for benchmark, not interactive-only fields.
    pub fn clone_config(&self) -> Self {
        Self {
            color_mode: self.color_mode,
            shading_mode: self.shading_mode,
            bold_mode: self.bold_mode,
            async_mode: self.async_mode,
            default_bg: self.default_bg,
            color_scheme: self.color_scheme,
            custom_palette: self.custom_palette.clone(),
            custom_palette_name: self.custom_palette_name.clone(),
            rain_style: self.rain_style,
            glitch_enabled: self.glitch_enabled,
            glitch_level: self.glitch_level,
            glitch_pct: self.glitch_pct,
            glitch_low: self.glitch_low,
            glitch_high: self.glitch_high,
            linger_low: self.linger_low,
            linger_high: self.linger_high,
            short_pct: self.short_pct,
            die_early_pct: self.die_early_pct,
            max_dpc: self.max_dpc,
            density: self.density,
            speed: self.speed,
            monolith_size: self.monolith_size,
            chars: self.chars.clone(),
            message: self.message.clone(),
            message_border: self.message_border,
            msg_fill_style: self.msg_fill_style,
            target_fps: self.target_fps,
            xtermjs_host: self.xtermjs_host,
            default_fps_cap: self.default_fps_cap,
            duration: self.duration,
            duration_s: self.duration_s,
            bench_frames: self.bench_frames,
            benchmark: self.benchmark,
            bench_duration: self.bench_duration,
            screen_size: self.screen_size,
            color_tune: self.color_tune,
            json: false,
            save_baseline: None,
            compare_baseline: None,
            bench_io: false,
            bench_all: false,
            bench_scene: None,
            verbose: false,
            density_auto: self.density_auto,
            base_density: self.base_density,
            perf_stats: false,
            screensaver: false,
            intro: IntroType::None,
            intro_color: None,
            mouse: false,
            charset_preset: self.charset_preset.clone(),
            user_ranges: self.user_ranges.clone(),
            def_ascii: self.def_ascii,
            crystal_dragon: self.crystal_dragon,
            power_dragon: self.power_dragon,
            msg_mode: self.msg_mode,
            effects_enabled: self.effects_enabled,
            config_path_for_watcher: None, // watcher only for interactive, not benchmark
            scene_name: self.scene_name.clone(),
            scene_custom_name: self.scene_custom_name.clone(),
            // Benchmark conversion: the tracker reflects the startup
            // resolution (a lock, never runtime-config-owned).
            scene_custom_config_owned: false,
            cli_explicit: self.cli_explicit,
            ambient_schedule: self.ambient_schedule.clone(),
            ambient_snapback_secs: self.ambient_snapback_secs,
            crystal_dragon_secs: self.crystal_dragon_secs,
        }
    }
}

// --- Density calculation helpers ---

/// Auto-density factor for the current terminal size.
///
/// v17 audit: the old formula was `sqrt(area / (80*25))` clamped to [0.5, 2.0].
/// This was conceptually wrong for cosmostrix's per-column density model:
///
///   - cosmostrix's `density` means "fraction of columns active" (glyph) or
///     "active lane ratio scale" (monolith). Both are inherently scale-
///     invariant quantities — a 200x60 terminal should have the SAME column
///     density as an 80x24 terminal, just with more columns.
///   - The old `sqrt(area)` formula double-counted width scaling (cols ×
///     density already scales with width) and added bogus height scaling
///     (more rows = longer droplet lifetime = fewer spawns needed, already
///     handled by recalc_droplets_per_sec).
///   - At 200x60, the old formula gave factor=2.0, so base_density=0.85
///     became effective=1.7 — 62% above the monolith ceiling (1.04), maxing
///     out the 35% active-lane cap on every non-trivial terminal.
///
/// The new formula is a **width-only dampener** for small terminals:
///
///   factor = clamp(cols / 80, 0.6, 1.0)
///
/// - At 80+ cols: factor = 1.0 (identity — no amplification, no reduction)
/// - At 48 cols: factor = 0.6 (small terminals get slightly sparser rain
///   to avoid over-saturation when each column is more visible)
/// - Never amplifies above 1.0 — the per-column model is already scale-
///   invariant, so amplification was always a bug.
///
/// .0-alpha.3: the legacy `--fullwidth` parameter (which doubled the
/// column stride for monolith streams) was removed. The `fullwidth` flag
/// is gone, so this function no longer needs a `fullwidth` parameter —
/// columns are always single-width (the Cosmic Dragon principle forbids
/// wide chars permanently; the charset is always single-width).
#[must_use]
pub fn auto_density_factor(cols: u16) -> f32 {
    let eff_cols = cols.max(1) as f32;
    // Width-only dampener: terminals smaller than 80 cols get slightly
    // sparser rain; 80+ cols get identity (factor=1.0). Never amplifies.
    let factor = eff_cols / DENSITY_BASE_COLS;
    factor.clamp(DENSITY_AUTO_MIN, 1.0)
}

/// Compute the effective droplet density for the current terminal.
///
/// When `auto` is true (user did NOT pass `--density` explicitly), the
/// base density is multiplied by `auto_density_factor()` — a width-only
/// dampener that never amplifies. When `auto` is false (user passed
/// `--density N`), the base is returned as-is (clamped to safe bounds).
///
/// See `auto_density_factor()` for the rationale on why the old
/// `sqrt(area)` amplifier was removed.
#[must_use]
pub fn effective_density(base: f32, cols: u16, auto: bool) -> f32 {
    let base = base.clamp(DENSITY_CLAMP_MIN, DENSITY_CLAMP_MAX);
    if !auto {
        return base;
    }
    (base * auto_density_factor(cols)).clamp(DENSITY_CLAMP_MIN, DENSITY_CLAMP_MAX)
}
