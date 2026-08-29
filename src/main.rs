// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! cosmostrix — the cinematic Matrix rain renderer (see `--version` for full description).
//!
//! cosmostrix transforms your terminal into a living, breathing canvas of
//! atmospheric digital rain. It is not a simple Matrix clone; it is a
//! realtime rendering engine built on principles of cinematic motion,
//! depth layering, and autonomous visual storytelling.
//!
//! ## Architecture
//!
//! The renderer is organized into clearly separated concerns:
//! - **Cloud** (`cloud/`): The simulation engine — droplet lifecycle, spawning,
//!   atmospheric evolution, and the cinematic behavior profile system.
//! - **Frame** (`frame.rs`): The backing buffer — differential dirty tracking
//!   with generation-based invalidation for zero-overhead cell reuse.
//! - **Terminal** (`terminal.rs`): The output layer — ANSI escape sequencing
//!   with run-length encoding, batched writes, and cursor optimization.
//! - **Droplet** (`droplet.rs`): Individual stream physics — gravity acceleration,
//!   velocity turbulence, head bloom, and phosphor afterglow.
//! - **Chroma Dragon** (`chroma/`): The coloring engine — palette construction,
//!   OKLab gradients, palette-relative brightness floor, and the shader pipeline
//!   that decides what color each cell becomes. (Phase 1 relocated the
//!   pre-existing `palette.rs` and `central_colors.rs` into `chroma/`.)
//!
//! ## Motion Philosophy
//!
//! cosmostrix prioritizes *perceptual smoothness* over raw frame count.
//! The adaptive pacing system modulates simulation time under performance
//! pressure, preferring slight visual slowdown over stutter. Frame timing
//! uses single-reschedule logic to prevent cascading overshoot jitter.
//!
//! ## Optimization Philosophy
//!
//! Performance work follows a "measure, don't guess" discipline. The benchmark
//! subsystem (`bench.rs`) provides reproducible metrics with warmup phases
//! and outlier trimming. Optimizations target real bottlenecks identified
//! through profiling, not hypothetical micro-optimizations.

// Phase 5: Global allocator tracing wrapper.
#[global_allocator]
static GLOBAL_ALLOC: crate::alloc_trace::TraceAlloc = crate::alloc_trace::TraceAlloc;
// ── Module declarations (src/ root contains only main.rs) ──────────────────
//
// Owner mandate 2026-08-19: src/ root must contain ONLY main.rs. All other
// modules live in subdirectories. See src/RULES.md for the policy.
//
// Re-export pattern: each group declares `mod <group>;` + `pub(crate) use
// <group>::{<submodules>};` so all existing `crate::<submodule>::Foo` call
// sites continue to resolve unchanged.

// Group: Bench subsystem (17 bench_*.rs files)
mod bench;
pub(crate) use bench::*;

// Group: CLI subsystem (cli.rs → mod.rs, cli_parse.rs, app.rs, help_detail.rs)
mod cli;
pub(crate) use cli::{app, cli_parse, help_detail};
// v50.0.0-beta.7 LOC refactor: extract_clap_suggestion + canonicalize_runtime_args
// moved to cli/suggestion.rs + cli/canonicalize.rs to keep main.rs under 800 LOC.
// Re-exported at crate root so 'use crate::extract_clap_suggestion' in
// tests/clap_suggestion.rs + 'crate::canonicalize_runtime_args' in
// chroma_dragon_engine/tests/color_detection.rs continue to resolve.
pub(crate) use cli::canonicalize::canonicalize_runtime_args;
pub(crate) use cli::suggestion::extract_clap_suggestion;

// Group: Chroma Dragon coloring engine
mod chroma_dragon_engine;
pub use chroma_dragon_engine::catalog;
pub use chroma_dragon_engine::palette;
pub(crate) use chroma_dragon_engine::{color_cache, color_tune, colors_custom};

// Group: Central Control — Dragon Power + Rains
mod central_control_dragon_power;
mod central_control_rains;

// Group: Clock subsystem (mod.rs + posix_time.rs)
mod clock;
pub(crate) use clock::posix_time;

// Group: Cosmic Dragon rendering engine (cloud/frame/runtime/terminal)
mod cosmic_dragon_engine;
pub(crate) use cloud::{brightness_factors, cinematic};
pub(crate) use cosmic_dragon_engine::{cloud, frame, runtime, terminal};

// Group: Cosmic Dragon incubator (experimental / concluded work)
mod cosmic_dragon_incubator;

// Group: Config subsystem (config*.rs, live_config*.rs, config_hints)
mod config;
pub(crate) use config::*;

// Group: Crystal Dragon ambient intelligence engine
mod crystal_dragon_engine;

// Group: Diagnostics subsystem (diagnostics.rs → mod.rs, alloc_trace.rs, info.rs, humanize.rs)
mod diagnostics;
pub(crate) use diagnostics::{alloc_trace, humanize, info};

// Group: Doctor subsystem
mod doctor;

// Group: Docs tests (integration)
#[cfg(test)]
mod docs_tests;

// Group: Droplet subsystem
mod droplet;

// Group: Interactive subsystem (event loop, HUD, intro, etc.)
mod interactive;

// Group: Output subsystem (output.rs → mod.rs, report.rs, verbose.rs, ux.rs, message.rs)
mod output;
pub(crate) use output::{message, report, ux};

// v50.0.0-beta.7 LOC refactor: verbose startup block extracted to
// main_verbose.rs to keep main.rs under the 800-LOC hard cap.
mod main_early_returns;
mod main_verbose;
mod main_bench_dispatch;

// Group: Platform subsystem (platform.rs → mod.rs, panic_hook.rs, update.rs)
mod platform;
pub(crate) use platform::{panic_hook, update};
// v50.0.0-beta.7 LOC refactor: spawn_kill9_terminal_guard moved to
// platform/fork_guard.rs. Re-exported at crate root so the existing
// 'crate::spawn_kill9_terminal_guard()' call site resolves unchanged.
pub(crate) use platform::fork_guard::spawn_kill9_terminal_guard;

// Group: Safepath subsystem
mod safepath;

// Group: Scene/Charset subsystem (scene.rs → mod.rs, charset.rs, charset_custom.rs)
mod scene;
pub(crate) use scene::{charset, charset_custom};

// Group: Scene custom subsystem
mod scene_custom;

// Group: Sysstat subsystem (cpustat, memstat, usagestat, envstat)
mod sysstat;
pub(crate) use sysstat::*;

// Group: Termdetect subsystem
mod termdetect;

// Group: Terminal subsystem (re-exported from cosmic_dragon_engine)
pub(crate) use terminal::{sgr_format, terminal_tty, tier2};

// Group: Testconf subsystem
mod testconf;

// Group: Tests (crate-level integration/regression tests)
#[cfg(test)]
mod tests;

// Group: Theme subsystem
mod theme;

// Group: Types subsystem (constants.rs, cell.rs, rain_style.rs, renderer_info.rs)
mod types;
pub(crate) use types::{cell, constants, rain_style, renderer_info};

// Standalone modules (file → dir, transparent resolution)
mod bolt;
mod validation;

use clap::{CommandFactory, FromArgMatches};

use std::env;

use crate::charset::{build_chars, charset_from_str};
use crate::config::{color_enabled_stdout, Args, ColorBg};
use crate::constants::*;
use crate::runtime::{BoldMode, ColorScheme, ShadingMode};
use crate::validation::{
    prevalidate_cli_args, validate_f32_range, validate_f64_range, validate_speed,
    validate_u16_range, validate_u8_range,
};

// Re-exports: items moved to submodules but still accessed by sibling
// modules via `super::`.
pub use app::{auto_density_factor, effective_density, CloudConfig};
pub use cli::{
    color_mode_label, cycle_charset_preset, cycle_color_scheme, default_to_ascii,
    detect_color_mode, detect_color_mode_auto, normalize_charset_preset_name, parse_color_scheme,
};
pub use info::env_var_truthy;

// --- Helpers kept in the crate root ---
// Input validation uses `ux::or_exit()` instead of the old `validate_err`.
// `or_exit` unwraps a Result whose Err carries a formatted error string,
// prints it to stderr, and exits with code 2 — never propagating a
// `std::io::Error` that Rust would render as a debug-looking
// `Error: Custom { ... }`.

// Path security validation lives in src/safepath/mod.rs.
pub(crate) use crate::safepath::{is_safe_path, validate_config_path};

fn main() -> std::io::Result<()> {
    // v50.0.0-rc.1: capture program-start Instant for the verbose exit
    // summary. MUST be the first statement so `duration:` reflects the
    // true wall-clock lifetime of the cosmostrix process (including arg
    // parsing, config load, intro animation, rain loop, teardown). Uses
    // Instant (monotonic) so NTP jumps cannot make duration negative.
    let start_time = std::time::Instant::now();

    // MUST be first — checks CPU features before any v3/v4 instructions execute
    #[cfg(target_arch = "x86_64")]
    info::check_cpu_features();

    // Panic hook: restore the terminal BEFORE printing the panic message.
    // See `install_panic_hook()` for the full rationale.
    crate::panic_hook::install_panic_hook();

    let mut cmd = Args::command();
    #[cfg(unix)]
    {
        cmd = cmd.styles(cli::clap_styles());
    }
    cmd = cmd.help_template(cli::help_template(color_enabled_stdout()));
    cmd.build();

    // v30 simplify: --help-detail merged into --help.
    // disable_help_flag = true on the #[command] macro prevents clap from
    // auto-generating its own --help, so there is no clap "help" arg to
    // re-style here. The --help field is defined manually on Args and
    // intercepted in main() below.

    let argv: Vec<std::ffi::OsString> = env::args_os().collect();
    // Prevalidate raw argv BEFORE -mb expansion so that --message-border
    // typed directly by the user is caught by REMOVED_FLAGS (not silently
    // accepted as a hidden clap boolean).  The -mb shorthand itself is
    // NOT in REMOVED_FLAGS, so it passes prevalidation cleanly.
    if let Err(e) = prevalidate_cli_args(&argv) {
        ux::die_input(e);
    }

    // Expand -mb "text" into --message-border -m "text"
    // -m "text" = message without border (default)
    // -mb "text" = message with border
    // Also handle -mb=text form.
    // This runs AFTER prevalidation so the internal --message-border token
    // injected here is not caught by the REMOVED_FLAGS check.
    let mut expanded: Vec<std::ffi::OsString> = Vec::with_capacity(argv.len() + 1);
    expanded.push(argv[0].clone());
    let mut i = 1;
    while i < argv.len() {
        let arg = &argv[i];
        if arg == "-mb" {
            expanded.push("--message-border".into());
            if i + 1 < argv.len() {
                expanded.push("-m".into());
                expanded.push(argv[i + 1].clone());
                i += 2;
                continue;
            }
        } else if let Some(s) = arg.to_str() {
            if let Some(rest) = s.strip_prefix("-mb=") {
                expanded.push("--message-border".into());
                expanded.push("-m".into());
                expanded.push(rest.into());
                i += 1;
                continue;
            }
        }
        expanded.push(arg.clone());
        i += 1;
    }
    let argv = expanded;

    let matches = cmd.try_get_matches_from(&argv).unwrap_or_else(|e| {
        // Intercept clap's "unexpected argument" errors and append a
        // "Did you mean --<flag>?" suggestion. The suggestion is extracted
        // from clap's OWN "tip:" line (not a separate edit-distance engine),
        // which guarantees the two lines always agree on which flag to
        // suggest and eliminates the hand-maintained flag list that caused
        // the v50.0.0-beta.7 drift bug (--no-effects was missing from
        // KNOWN_LONG_FLAGS after the rename from --disable-effects).
        let err_str = e.to_string();
        // Only intercept "unexpected argument" errors (not missing-value,
        // not invalid-value, etc.). For those, fall through to clap's
        // default error display.
        if err_str.contains("unexpected argument") {
            if let Some(suggestion) = extract_clap_suggestion(&err_str) {
                // Print clap's original error (includes the "tip:" line +
                // usage), then append our "Did you mean?" line using the
                // SAME flag clap already chose.
                e.print().ok();
                eprintln!(
                    "{}  Did you mean --{}?{}",
                    crate::output::warn_open(),
                    suggestion,
                    crate::output::reset()
                );
                std::process::exit(2);
            }
        }
        // No suggestion found (clap didn't find a close match) — fall
        // through to clap's default error display.
        e.exit();
    });
    let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

    // --help: print the full curated reference manual and exit.
    //
    // Checked early (before --dump-config, --doctor, --version, etc.) so
    // `cosmostrix --help` always works even if other flags are malformed
    // or the config file is broken. This mirrors how clap's auto-help
    // behaves: help wins over everything else.
    // v50.0.0-beta.7 LOC refactor: pre-config-apply early-return commands
    // extracted to main_early_returns.rs.
    if let Some(result) = main_early_returns::handle_pre_config_returns(&mut args) {
        return result;
    }

    // Benchmark default scene override:
    //
    // When running in benchmark mode (--benchmark or --bench-all) without an
    // explicit --scene, default to "monolith" — the signature scene that
    // produces peak FPS. Interactive mode keeps DEFAULT_SCENE (cinematic)
    // as its signature, since cinematic is the richer visual showcase.
    //
    // This prevents user confusion: the headline "38k FPS" claims come from
    // monolith, but cinematic (the interactive default) is significantly
    // heavier and runs much slower. Users who run `cosmostrix --benchmark`
    // expect the peak number, not the cinematic one.
    //
    // Users can still override with `--scene <name>` to benchmark any scene
    // (e.g. `cosmostrix --benchmark --scene cinematic`). The benchmark
    // report discloses the active scene + a disclaimer for non-monolith scenes.
    let bench_mode = args.benchmark || args.bench_all || args.bench_frames.is_some();
    if bench_mode && args.scene.is_none() {
        args.scene = Some("monolith".to_string());
    }

    // v50-beta.3: Power Dragon defaults to true (protection enabled) when
    // neither CLI nor config provides a value. config_apply sets
    // args.power_dragon = Some(...) when either source is explicit.
    if args.power_dragon.is_none() {
        args.power_dragon = Some(true);
    }
    // v50-beta.3: msg-mode defaults to true (message overlay active).
    if args.msg_mode.is_none() {
        args.msg_mode = Some(true);
    }

    if let Err(e) = config_apply::apply_config_and_runtime_defaults(&matches, &mut args) {
        ux::die_config(e);
    }
    canonicalize_runtime_args(&mut args);

    if args.doctor {
        doctor::print_doctor_report(&args);
        return Ok(());
    }

    if args.version {
        println!("{}", info::version_report());
        return Ok(());
    }

    if args.docs {
        // Print the full engine documentation and architecture overview,
        // then exit. Plain text only (no ANSI) so it pipes cleanly into
        // `less`, `grep`, or documentation generators.
        println!("{}", info::docs_report());
        return Ok(());
    }

    if args.check_update {
        if let Err(e) = update::check_update(env!("CARGO_PKG_VERSION")) {
            ux::die_config(format!("error: update check failed: {e}"));
        }
        return Ok(());
    }

    // v17: --info/-i REMOVED. Merged into --doctor. Use --doctor for all diagnostics.

    // --- Validate all arguments using Result-based validators ---
    let def_ascii = default_to_ascii();
    let color_mode = detect_color_mode(&args);

    let shading_mode =
        match ux::or_exit(validate_u8_range("--shadingmode", args.shading_mode, 0, 1)) {
            1 => ShadingMode::DistanceFromHead,
            _ => ShadingMode::Random,
        };

    let bold_mode = match ux::or_exit(validate_u8_range("--bold", args.bold, 0, 2)) {
        0 => BoldMode::Off,
        2 => BoldMode::All,
        _ => BoldMode::Random,
    };

    // dynamic default FPS (terminal-aware: 144 high-perf / 60 std
    // / 30 xterm.js) when user didn't set --fps. track which
    // resolution layer won so verbose can show `fps_precedence:`. See
    // the FPS Precedence Chain doc in termdetect.rs.
    let term_caps = crate::termdetect::detect();
    let cli_fps_explicit = matches!(
        matches.value_source("fps"),
        Some(clap::parser::ValueSource::CommandLine)
    );
    let fps_user_set = cli_fps_explicit || args.fps != 60.0;
    // Resolution layer: cli > scene > config > dynamic_default. Computed
    // BEFORE the dynamic-default override mutates args.fps.
    let fps_precedence: &'static str = if cli_fps_explicit {
        "cli"
    } else if fps_user_set {
        // args.fps != 60.0 but not CLI → set by scene or config. Distinguish
        // by checking if the active scene has a matching fps override.
        if args
            .scene
            .as_deref()
            .and_then(crate::scene::get_scene)
            .and_then(|s| s.config.fps)
            .map(|f| (f - args.fps).abs() < 0.01)
            .unwrap_or(false)
        {
            "scene"
        } else {
            "config"
        }
    } else {
        "dynamic_default"
    };
    if !fps_user_set && !args.benchmark && term_caps.dynamic_default_fps != args.fps {
        crate::lr_trace!(
            "fps: no user override — applying dynamic default {:.0}",
            term_caps.dynamic_default_fps
        );
        args.fps = term_caps.dynamic_default_fps;
    }

    let target_fps = ux::or_exit(validate_f64_range("--fps", args.fps, 1.0, 240.0));

    // Tier 2: xterm.js hosts get 30 FPS cap to prevent OOM. OVERRIDES resolution
    // layer. also re-applied on live-reload. (FPS-F5): skip in ALL bench modes.
    let in_bench_mode = args.benchmark || args.bench_all || args.bench_frames.is_some();
    let xtermjs_cap_fired =
        !in_bench_mode && term_caps.xtermjs_host && target_fps > term_caps.default_fps_cap;
    let target_fps = if xtermjs_cap_fired {
        let capped = term_caps.default_fps_cap;
        crate::output::eprintln_warn_labeled(&format!(
            "xterm.js-based terminal detected (TERM_PROGRAM={}); \
             capping --fps from {target_fps:.1} to {capped:.0} to prevent \
             xterm.js OOM crash over long runs (see docs/TERMINAL_COMPATIBILITY.md)",
            std::env::var("TERM_PROGRAM").unwrap_or_default()
        ));
        capped
    } else {
        target_fps
    };
    let fps_precedence: &'static str = if xtermjs_cap_fired {
        "xtermjs_cap"
    } else {
        fps_precedence
    };

    let duration_s = args.duration.map(|s| {
        if !s.is_finite() {
            ux::die_config(format!("--duration {s}: must be a finite number"));
        }
        if s > 0.0 {
            return ux::or_exit(validate_f64_range("--duration", s, 0.1, 86400.0));
        }
        s
    });

    // Unified color resolution (v50.0.0-beta.6 Option D policy: custom wins):
    //   1. --colors-custom <name> → custom palette (explicit intent)
    //   2. -c/--color <name> matches [colors-custom.<name>] → custom wins
    //   3. -c/--color <name> matches builtin theme → builtin
    //   4. Neither → error with "did you mean" suggestions
    //
    // When custom shadows a builtin (branch 2 and builtin also exists), a
    // collision warning is emitted so the user knows the custom block won.
    // This aligns colors with charset (which already had custom-wins) and
    // scene (also updated to custom-wins in this commit).
    let cfg_for_color = configfile::load_config_file(args.config.as_deref());
    let (color_scheme, custom_palette, custom_palette_name) =
        if let Some(ref name) = args.colors_custom {
            // Explicit --colors-custom: only loads from config, never built-in.
            match colors_custom::load_custom_palette(&cfg_for_color, name) {
                Ok(p) => (ColorScheme::Green, Some(p), Some(name.clone())),
                Err(e) => ux::die_input(format!("error: --colors-custom '{name}': {e}")),
            }
        } else if colors_custom::is_colors_custom_name(&cfg_for_color, &args.color) {
            // v50.0.0-beta.6 Option D: custom palette wins over builtin
            // when the name matches both. Previously builtin was checked
            // first and silently blocked custom — now custom is checked
            // first so user-defined palettes always take precedence.
            let is_builtin = parse_color_scheme(&args.color).is_ok();
            if is_builtin {
                crate::output::warn_name_collision(
                    "color",
                    &args.color,
                    "builtin theme (see --list-colors)",
                    "custom palette from [colors-custom.*]",
                );
            }
            match colors_custom::load_custom_palette(&cfg_for_color, &args.color) {
                Ok(p) => (ColorScheme::Green, Some(p), Some(args.color.clone())),
                Err(e) => ux::die_input(e),
            }
        } else if let Ok(c) = parse_color_scheme(&args.color) {
            // -c/--color resolved to a built-in theme (no custom collision).
            (c, None, None)
        } else {
            // Not a built-in theme and not a custom palette — use the original
            // error from parse_color_scheme (includes "did you mean" suggestions).
            ux::die_input(parse_color_scheme(&args.color).unwrap_err())
        };
    let color_tune = match args.color_tune.as_deref() {
        Some(s) => ux::or_exit(color_tune::parse_color_tune(s)),
        None => {
            // v17: read [color.tune] from config.toml.
            let cfg_map = configfile::load_config_file(args.config.as_deref());
            color_tune::color_tune_from_config(&cfg_map)
        }
    };

    // rain_style resolution — built-in → its rain_style; custom →
    // base-scene's rain_style; otherwise → Glyph.
    let cfg = configfile::load_config_file(args.config.as_deref());
    let rain_style = scene_custom::resolve_rain_style(args.scene.as_deref(), &cfg);

    // v17 ghost labels: #[arg(skip)] fields, not live CLI flags (--glitchpct etc. removed in v17).
    let glitch_pct = ux::or_exit(validate_f32_range(
        "glitch_pct (internal, set via --glitch-level)",
        args.glitch_pct,
        0.0,
        100.0,
    ));
    let glitch_low = ux::or_exit(validate_u16_range(
        "--glitchms low",
        args.glitch_ms.low,
        1,
        5000,
    ));
    let glitch_high = ux::or_exit(validate_u16_range(
        "--glitchms high",
        args.glitch_ms.high,
        1,
        5000,
    ));
    let linger_low = ux::or_exit(validate_u16_range(
        "--lingerms low",
        args.linger_ms.low,
        1,
        60000,
    ));
    let linger_high = ux::or_exit(validate_u16_range(
        "--lingerms high",
        args.linger_ms.high,
        1,
        60000,
    ));
    let short_pct = ux::or_exit(validate_f32_range(
        "short_pct (internal, set via --glitch-level)",
        args.shortpct,
        0.0,
        100.0,
    ));
    let die_early_pct = ux::or_exit(validate_f32_range(
        "rippct (internal, set via --glitch-level)",
        args.rippct,
        0.0,
        100.0,
    ));
    let max_dpc = ux::or_exit(validate_u8_range(
        "max_droplets_per_column (internal, set via --glitch-level)",
        args.max_droplets_per_column,
        1,
        3,
    ));
    let speed = ux::or_exit(validate_speed(args.speed));

    // --chars CLI flag was removed (audit FLAGS_AUDIT_bench-frames_chars_bold.md §2).
    // Custom charsets now exclusively come from [charset-custom.<name>] in config.toml
    // loaded via --charset <name>. The user_ranges Vec stays (always empty here) because
    // removing it would touch ~15 call sites with zero functional benefit.
    let user_ranges: Vec<(char, char)> = Vec::new();

    let charset_preset = normalize_charset_preset_name(&args.charset);

    // v25: Custom charset loading from [charset-custom.<name>] in config.toml.
    // Replaces the legacy --charset-file CLI flag. If `args.charset` (after
    // normalization) matches a custom block name, load its char pool;
    // otherwise fall through to the built-in charset_from_str path.
    //
    // This runs BEFORE the verbose print so the verbose output can show
    // whether a custom charset was used. The custom_palette block below
    // follows the same pattern for --colors-custom.
    let cfg_for_charset = configfile::load_config_file(args.config.as_deref());
    let chars = if let Some(custom_chars) =
        charset_custom::load_custom_charset_if_matches(&cfg_for_charset, &charset_preset)
    {
        if args.verbose {
            crate::output::eprintln_verbose_raw(&format!(
                "charset: {} (custom, {} chars)",
                charset_preset,
                custom_chars.len()
            ));
        }
        custom_chars
    } else {
        let charset = match charset_from_str(&args.charset, def_ascii) {
            Ok(c) => c,
            Err(e) => ux::die_input(e),
        };
        build_chars(charset, &user_ranges, def_ascii)
    };

    // (custom_palette and custom_palette_name are now resolved above
    // in the unified color resolution block.)

    let density_auto =
        matches.value_source("density") == Some(clap::parser::ValueSource::DefaultValue);
    let base_density = ux::or_exit(validate_f32_range(
        "--density",
        args.density,
        DENSITY_CLAMP_MIN,
        DENSITY_CLAMP_MAX,
    ));

    let default_bg = matches!(args.color_bg, ColorBg::DefaultBackground);

    // v50-beta.3: --async-mode CLI flag replaces --uniform.
    // Default: true (async variable pacing on). --async-mode false = uniform.
    let effective_async = args.async_mode.unwrap_or(true);

    // Parse --screen-size once here so verbose block and CloudConfig both
    // see the same validated value (previously verbose used .ok().flatten()
    // which silently swallowed parse errors).
    let screen_size = crate::ux::or_exit(crate::cli_parse::parse_screen_size_optional(
        &args.screen_size,
    ));

    // ── Verbose output (before CloudConfig moves values) ──
    let cli_explicit_color = matches!(
        matches.value_source("color"),
        Some(clap::parser::ValueSource::CommandLine)
    );
    // Bug 3 fix: capture which CLI flags were explicitly set so live reload
    // can enforce CLI > config.toml > scene priority (otherwise a CLI flag
    // like `-c green` would be silently overridden when config is edited).
    let cli_explicit = crate::app::CliExplicit {
        color: cli_explicit_color,
        charset: matches!(
            matches.value_source("charset"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        speed: matches!(
            matches.value_source("speed"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        density: matches!(
            matches.value_source("density"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        fps: matches!(
            matches.value_source("fps"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        scene: matches!(
            matches.value_source("scene"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        glitch_level: matches!(
            matches.value_source("glitch_level"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        // Same intent tracking for --crystal-dragon.
        crystal_dragon: matches!(
            matches.value_source("crystal_dragon"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        // v50.0.0-alpha.7: track --power-dragon, --async-mode, --msg-mode,
        // --intro-color, and -m/-mb CLI explicit (was missing — live-reload
        // path overrode CLI intent on config edit).
        power_dragon: matches!(
            matches.value_source("power_dragon"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        async_mode: matches!(
            matches.value_source("async_mode"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        msg_mode: matches!(
            matches.value_source("msg_mode"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        intro_color: matches!(
            matches.value_source("intro_color"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        message: matches!(
            matches.value_source("message"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        // v50.0.0-alpha.7: track --monolith-size CLI explicit (Issue #4).
        monolith_size: matches!(
            matches.value_source("monolith_size"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
        // v50.0.0-alpha.7: track --color-tune CLI explicit (color.tune
        // reset-on-comment fix — when CLI --color-tune is set, config
        // [color.tune] block absence must NOT reset to identity).
        color_tune: matches!(
            matches.value_source("color_tune"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
    };
    if args.verbose {
        main_verbose::run_verbose_startup(
            &args,
            rain_style,
            color_scheme,
            color_mode,
            color_tune,
            &custom_palette,
            &custom_palette_name,
            custom_palette.as_ref().and_then(|p| p.bg),
            &charset_preset,
            &chars,
            target_fps,
            fps_precedence,
            speed,
            base_density,
            density_auto,
            effective_async,
            bold_mode,
            shading_mode,
            glitch_pct,
            glitch_low,
            glitch_high,
            screen_size,
            bench_mode,
            cli_explicit_color,
            &default_message_text(),
        );
    }
    // v14 Peak Monolith: resolve per-column density map from the active
    // scene-custom block (if any). The map sculpts monolith pillar formation.
    let monolith_density_map = args.scene_custom.as_deref().and_then(|name| {
        let cfg = configfile::load_config_file(args.config.as_deref());
        let scenes = scene_custom::collect_custom_scenes(&cfg);
        scenes
            .get(name)
            .and_then(|s| s.density_map.as_deref())
            .and_then(scene_custom::parse_density_map)
    });

    // CliExplicit is Copy — field copy after CloudConfig move (avoids E0382).
    let cloud_cfg = CloudConfig {
        color_mode,
        shading_mode,
        bold_mode,
        async_mode: effective_async,
        default_bg,
        color_scheme,
        custom_palette,
        custom_palette_name,
        rain_style,
        glitch_enabled: args.glitch_level != crate::config::GlitchLevel::None,
        glitch_pct,
        glitch_low,
        glitch_high,
        linger_low,
        linger_high,
        short_pct,
        die_early_pct,
        max_dpc,
        density: base_density,
        speed,
        monolith_size: args.monolith_size,
        chars,
        // v50-beta.3: msg-mode gate + default message fallback.
        // Precedence (highest wins):
        //   1. CLI -m / -mb (always active — CLI wins over msg-mode=false)
        //   2. msg-mode=false → disable BOTH default AND config message
        //      (user must set msg-mode=true to use message/message-border config)
        //   3. config `message` / `message-border` (when msg-mode=true)
        //   4. default fallback "cosmostrix v<CARGO_PKG_VERSION>" with border
        //      (only when !bench_mode AND msg-mode=true)
        // Benchmark mode never shows a message overlay (keeps reports clean).
        // Version is dynamic (env! CARGO_PKG_VERSION), never hardcoded.
        message: {
            // msg_mode_effective: CLI flag wins (already applied via config_value
            // is_explicit); default true when neither CLI nor config sets it.
            let msg_mode_on = args.msg_mode.unwrap_or(true);
            // CLI explicit? Check is via clap's value_source — but for -m / -mb
            // we already have args.message set with the text. So:
            //   - If args.message is Some AND was set via CLI → always show
            //   - If args.message is Some AND was set via config → only if msg_mode_on
            //   - If args.message is None AND !bench_mode AND msg_mode_on → default
            // We can't easily distinguish CLI vs config origin here without
            // tracking is_explicit for the message flag. Instead: trust the
            // config_apply layer — when msg-mode=false AND no CLI -m/-mb,
            // args.message should already be None. main.rs doesn't need to
            // re-check. The msg_mode_on flag here only affects the DEFAULT
            // fallback (when args.message is None).
            let msg: Option<String> = if !bench_mode && args.message.is_none() && msg_mode_on {
                Some(default_message_text())
            } else {
                args.message.clone()
            };
            msg.as_deref().map(|m| {
                if m.len() > MESSAGE_MAX_LEN {
                    ux::die_input(format!(
                        "error: -m text exceeds {MESSAGE_MAX_LEN} character limit (got {})",
                        m.len()
                    ));
                }
                crate::message::sanitize_message_text(m)
            })
        },
        // v50: When the default message fallback fired (args.message was
        // None and !bench_mode), force border=true so the overlay looks
        // intentional. When the user explicitly set -m (no border), keep
        // their choice.
        message_border: args.message_border || (!bench_mode && args.message.is_none()),
        target_fps,
        xtermjs_host: term_caps.xtermjs_host, // (FPS-F1): live-reload cap
        default_fps_cap: term_caps.default_fps_cap,
        duration: args.duration,
        duration_s,
        bench_frames: args.bench_frames,
        benchmark: args.benchmark,
        bench_duration: crate::bench_helpers::resolve_bench_duration_args(&args.bench_duration),
        screen_size,
        color_tune,
        json: args.json,
        save_baseline: args.save_baseline.clone(),
        compare_baseline: args.compare_baseline.clone(),
        bench_io: args.bench_io,
        bench_all: args.bench_all,
        bench_scene: args.bench_scene.clone(),
        verbose: args.verbose,
        density_auto,
        base_density,
        perf_stats: args.perf_stats,
        screensaver: args.screensaver,
        intro: args.intro.unwrap_or(crate::config::IntroType::Logo),
        intro_color: args.intro_color.take(),
        mouse: true, // v17: always-on (--mouse flag deleted)
        charset_preset,
        user_ranges,
        def_ascii,
        crystal_dragon: args.crystal_dragon.unwrap_or(false),
        power_dragon: args.power_dragon.unwrap_or(true),
        msg_mode: args.msg_mode.unwrap_or(true),
        // Auto-disable particle effects in bench mode — particles are
        // input-driven (mouse clicks, border touches) and never spawn
        // during a benchmark run. This means `cosmostrix --benchmark`
        // is equivalent to `cosmostrix --benchmark --no-effects` — the
        // user no longer needs to pass --no-effects explicitly to get
        // the cleanest bench numbers. The bench CONFIG report's
        // `no_effects` field will automatically show `true` for any
        // bench mode (--benchmark, --bench-all, --bench-frames).
        effects_enabled: !args.no_effects && !bench_mode,
        monolith_density_map,
        config_path_for_watcher: {
            // Termux fix: multi-candidate path resolution so the
            // watcher watches the file the user is ACTUALLY editing. On
            // Termux with XDG_CONFIG_HOME=$PREFIX/etc, the old single-
            // candidate resolver picked a system path the user wasn't
            // editing. The new resolver prioritizes $HOME/.config.
            let (resolved, existed) =
                configfile::resolve_watcher_config_path(args.config.as_deref());
            if crate::live_config_trace::live_reload_debug_enabled() {
                crate::live_config_trace::debug_trace(format_args!(
                    "watcher path resolved: {} (existed candidates: {})\n",
                    resolved.display(),
                    existed
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            Some(resolved)
        },
        scene_name: args
            .scene
            .as_deref()
            .unwrap_or(crate::scene::DEFAULT_SCENE)
            .to_string(),
        // v20: track active custom scene name so live reload can re-apply
        // its fields when the user edits [scene-custom.<name>] in config.
        scene_custom_name: args.scene_custom.clone(),
        // Bug 3: tracker for CLI-explicit flags, used by rebuild_cloud_config
        // to enforce CLI > config.toml > scene priority during live reload.
        cli_explicit,
        // Ambient phase schedule (config-only). Collected from
        // `ambient.<HH-MM>` keys; empty = scheduler idles.
        ambient_schedule: crate::crystal_dragon_engine::ambient::collect_ambient_schedule(
            &configfile::load_config_file(args.config.as_deref()),
        ),
        // v50.0.0-beta.7: ambient-snapback-secs config key (config-only,
        // no CLI flag). None = use default AUTO_SNAPBACK_DELAY_SECS (30.0).
        // Range 0.0..=86400.0 validated by parse_f64_config. Invalid values
        // emit a startup error and fall back to None (default).
        ambient_snapback_secs: configfile::load_config_file(args.config.as_deref())
            .get("ambient-snapback-secs")
            .and_then(|v| {
                crate::config_apply::parse_f64_config("ambient-snapback-secs", v, 0.0, 86400.0)
            }),
    };

    // fps_user_set was computed earlier (before dynamic default) — USER intent.

    // v50.0.0-beta.7 LOC refactor: bench dispatch extracted to
    // main_bench_dispatch.rs.
    if let Some(result) = main_bench_dispatch::dispatch_bench(&args, &cloud_cfg, fps_user_set) {
        return result;
    }

    let result = interactive::run_interactive(&cloud_cfg);

    // v16 audit: Explicitly print run_interactive errors to stderr (after
    // Terminal::drop restored the terminal). See install_panic_hook().
    //
    // v25 (terminal-close coredump fix): use `write_fmt` with error
    // discarded instead of `eprintln!`. When the terminal is closed
    // (SIGHUP), stderr is a broken pipe — `eprintln!` panics on write
    // failure → panic hook fires → hook's `eprintln!` panics again →
    // double-panic → `abort()` → systemd-coredump. Bulletproof write
    // breaks the chain.
    if let Err(ref e) = result {
        use std::io::Write;
        crate::terminal::restore_terminal_best_effort();
        let _ = std::io::stderr().write_fmt(format_args!("error: {e}\n"));
        let _ = std::io::stderr().flush();
    }

    if args.verbose && result.is_ok() {
        // Post-exit verbose dump extracted to output/post_exit.rs to keep
        // main.rs under the 1500-LOC cap + comply with src/RULES.md (only
        // main.rs at src/ root). Owns:
        // - startup ambient info (captured during the loop, printed here
        //   because the alternate screen discards stderr)
        // - "final runtime state" section via
        //   interactive::print_final_runtime_state (exit_time + duration
        //   + live-reload field changes + always-printed ambient lines)
        output::post_exit::print_post_exit_verbose(&args, &cloud_cfg, color_scheme, start_time);
    }

    // Live-reload fatal exit ( bug #15): watcher panics + validation
    // errors set LIVE_RELOAD_EXIT_CODE=2, break the rain loop, print here
    // after Terminal::drop (no alt-screen leak).
    if live_config::LIVE_RELOAD_EXIT_CODE.load(std::sync::atomic::Ordering::Acquire) != 0 {
        if let Ok(guard) = live_config::LIVE_RELOAD_ERROR.lock() {
            if let Some(ref msg) = *guard {
                crate::output::eprintln_safe!(
                    "{} [live-reload] ERROR: {}{}",
                    crate::output::error_bold_open(),
                    msg,
                    crate::output::reset()
                );
                crate::output::eprintln_safe!(
                    "{}  Config NOT applied. Fix the error and restart cosmostrix.{}",
                    crate::output::error_open(),
                    crate::output::reset()
                );
            }
        }
        use std::io::Write;
        let _ = std::io::stderr().flush();
        std::process::exit(2);
    }

    // AB-10: drain buffered runtime warnings + debug traces post-exit.
    for w in live_config::drain_runtime_warnings() {
        crate::output::eprintln_warn_labeled(&w);
    }
    for t in crate::live_config_trace::drain_debug_traces() {
        crate::output::eprintln_safe!("{t}");
    }
    result
}
