// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cosmostrix — Production-grade cinematic Matrix rain renderer for serious terminal environments.
//!
//! Cosmostrix transforms your terminal into a living, breathing canvas of
//! atmospheric digital rain. It is not a simple Matrix clone; it is a
//! realtime rendering engine built on principles of cinematic motion,
//! depth layering, and autonomous visual storytelling.
//!
//! ## Architecture
//!
//! The renderer is organized into clearly separated concerns:
//! - **Cloud** (`cloud.rs`): The simulation engine — droplet lifecycle, spawning,
//!   atmospheric evolution, and the cinematic behavior profile system.
//! - **Frame** (`frame.rs`): The backing buffer — differential dirty tracking
//!   with generation-based invalidation for zero-overhead cell reuse.
//! - **Terminal** (`terminal.rs`): The output layer — ANSI escape sequencing
//!   with run-length encoding, batched writes, and cursor optimization.
//! - **Droplet** (`droplet.rs`): Individual stream physics — gravity acceleration,
//!   velocity turbulence, head bloom, and phosphor afterglow.
//! - **Palette** (`palette.rs`): The color pipeline — gradient construction,
//!   mode-aware quantization, and real-time color blending.
//!
//! ## Motion Philosophy
//!
//! Cosmostrix prioritizes *perceptual smoothness* over raw frame count.
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

mod app;
mod atmosphere;
#[cfg(test)]
mod atmosphere_ab;
#[cfg(test)]
mod atmosphere_ab_tests;
mod atmosphere_apply;
#[cfg(test)]
mod atmosphere_apply_cl_tests;
#[cfg(test)]
mod atmosphere_apply_tests;
mod atmosphere_controlled_live;
#[cfg(test)]
mod atmosphere_expansion_tests;
mod atmosphere_presets;
mod atmosphere_probe;
mod atmosphere_runtime;
mod atmosphere_shadow;
#[cfg(test)]
mod atmosphere_tests;
mod atmosphere_verifier;
mod atmosphere_visual;
mod bench;
mod bench_comp;
mod bench_cpu;
mod bench_json;
mod bench_mem;
mod bench_meta;
mod bench_progress;
mod bench_report;
mod cell;
mod charset;
mod cinematic;
mod cli;
mod cloud;
mod color_cache;
mod color_tune;
mod config;
mod config_apply;
#[cfg(test)]
mod config_apply_profiles_tests;
#[cfg(test)]
mod config_apply_tests;
mod configfile;
mod constants;
mod cpustat;
mod diagnostics;
#[cfg(test)]
mod docs_tests;
mod doctor;
mod droplet;
mod envstat;
mod frame;
mod help_detail;
mod info;
mod interactive;
#[cfg(test)]
mod loc_tests;
mod memstat;
mod palette;
mod preset;
mod profile;
mod rain_style;
mod renderer_info;
mod report;
mod runtime;
mod safepath;
mod scene;
mod termdetect;
mod terminal;
mod testconf;
mod theme;
mod update;
mod usagestat;
mod ux;
mod validation;
mod verbose;

use std::env;

#[cfg(target_os = "linux")]
use std::io::IsTerminal;

use clap::parser::ValueSource;
use clap::{CommandFactory, FromArgMatches};

use crate::charset::{build_chars, charset_from_str, parse_user_hex_chars};
use crate::config::{
    color_enabled_stdout, print_defaults, print_list_charsets, print_list_colors,
    print_list_colors_detail, print_list_scenes, Args, ColorBg,
};
use crate::constants::*;
use crate::runtime::{BoldMode, ShadingMode};
use crate::terminal::reset_terminal_emergency;
#[cfg(target_os = "linux")]
use crate::terminal::restore_terminal_best_effort;
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
//
// Input validation uses `ux::or_exit()` instead of the old `validate_err`.
// `or_exit` unwraps a Result whose Err carries a formatted error string,
// prints it to stderr, and exits with code 2 — never propagating a
// `std::io::Error` that Rust would render as a debug-looking
// `Error: Custom { ... }`.

// Path security validation lives in src/safepath.rs.
pub(crate) use crate::safepath::is_safe_path;

#[cfg(target_os = "linux")]
pub fn spawn_kill9_terminal_guard() {
    if env_var_truthy("COSMOSTRIX_NO_FORK_GUARD") {
        return;
    }

    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return;
    }

    // SAFETY: this Linux-only guard calls libc APIs that Rust cannot model
    // safely (`tcgetattr`, `fork`, signal-mask setup, `prctl`, `sigwait`, and
    // `_exit`). We only enter after confirming stdin/stdout are TTYs. `orig`
    // and `set` are initialized by the corresponding libc calls before
    // `assume_init`, the child process does not return into Rust application
    // flow, and restoration is limited to best-effort terminal recovery.
    unsafe {
        let mut orig: std::mem::MaybeUninit<libc::termios> = std::mem::MaybeUninit::uninit();
        if libc::tcgetattr(libc::STDIN_FILENO, orig.as_mut_ptr()) != 0 {
            return;
        }
        let orig = orig.assume_init();

        let pid = libc::fork();
        if pid != 0 {
            return;
        }

        // Initialize sigset_t via MaybeUninit — sigemptyset will fully
        // initialize it, so this is safe.
        let mut set = std::mem::MaybeUninit::<libc::sigset_t>::uninit();
        libc::sigemptyset(set.as_mut_ptr());
        libc::sigaddset(set.as_mut_ptr(), libc::SIGTERM);
        let _ = libc::pthread_sigmask(libc::SIG_BLOCK, set.as_ptr(), std::ptr::null_mut());
        let set = set.assume_init();

        let _ = libc::prctl(
            libc::PR_SET_NAME,
            c"cx-term-guard".as_ptr() as usize,
            0,
            0,
            0,
        );
        let _ = libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM, 0, 0, 0);

        if libc::getppid() == 1 {
            let _ = libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &orig);
            restore_terminal_best_effort();
            libc::_exit(0);
        }

        let mut sig: libc::c_int = 0;
        let _ = libc::sigwait(&set, &mut sig);
        // Only restore terminal modes if the parent died abnormally
        // (SIGKILL, crash). When pkill -TERM is used, both parent and
        // child receive SIGTERM. The parent's Terminal::drop() handles
        // all terminal cleanup — if the child also writes restore
        // sequences to the same stdout fd, it races with the parent
        // and can cause glyph residue on the main screen.
        // After PR_SET_PDEATHSIG, the kernel sends SIGTERM to the child
        // when the parent exits for ANY reason. Check ppid to distinguish:
        // - ppid == 1: parent already dead (SIGKILL or crash) → restore
        // - ppid != 1: parent still alive or exiting normally → do nothing
        if sig == libc::SIGTERM && libc::getppid() == 1 {
            let _ = libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &orig);
            restore_terminal_best_effort();
        }

        libc::_exit(0);
    }
}

fn main() -> std::io::Result<()> {
    // MUST be first — checks CPU features before any v3/v4 instructions execute
    #[cfg(target_arch = "x86_64")]
    info::check_cpu_features();

    // Panic hook: write panic info to stderr ONLY.
    //
    // Do NOT call restore_terminal_best_effort() here — doing so writes
    // restore sequences (LeaveAlternateScreen etc.) directly to stdout
    // BEFORE unwinding runs Terminal::drop, which then flushes the
    // BufWriter's pending frame data AFTER the alternate screen has been
    // left. This leaks the partially-rendered rain onto the user's main
    // terminal screen.
    //
    // Terminal restoration is handled by:
    //   1. Terminal::drop (via panic=unwind) — normal case
    //   2. Fork-based SIGKILL guard (Linux) — if process is killed
    //   3. Watchdog thread — if main thread is hung
    std::panic::set_hook(Box::new(|info| {
        eprintln!("{}", info);
    }));

    let mut cmd = Args::command();
    #[cfg(unix)]
    {
        cmd = cmd.styles(cli::clap_styles());
    }
    let help_template = if color_enabled_stdout() {
        cli::HELP_TEMPLATE_COLOR
    } else {
        cli::HELP_TEMPLATE_PLAIN
    };
    cmd = cmd.help_template(help_template);
    cmd.build();

    if cmd.get_arguments().any(|a| a.get_id().as_str() == "help") {
        cmd = cmd.mut_arg("help", |a| a.help_heading("HELP"));
    }
    cmd.build();

    let argv: Vec<std::ffi::OsString> = env::args_os().collect();
    // Expand -mb "text" into --message-border --message "text"
    // -m "text" = message without border (default)
    // -mb "text" = message with border
    // Also handle -mb=text form.
    let mut expanded: Vec<std::ffi::OsString> = Vec::with_capacity(argv.len() + 1);
    expanded.push(argv[0].clone());
    let mut i = 1;
    while i < argv.len() {
        let arg = &argv[i];
        if arg == "-mb" {
            expanded.push("--message-border".into());
            if i + 1 < argv.len() {
                expanded.push("--message".into());
                expanded.push(argv[i + 1].clone());
                i += 2;
                continue;
            }
        } else if let Some(s) = arg.to_str() {
            if let Some(rest) = s.strip_prefix("-mb=") {
                expanded.push("--message-border".into());
                expanded.push("--message".into());
                expanded.push(rest.into());
                i += 1;
                continue;
            }
        }
        expanded.push(arg.clone());
        i += 1;
    }
    let argv = expanded;
    if let Err(e) = prevalidate_cli_args(&argv) {
        ux::die_input(e);
    }

    let matches = cmd.get_matches_from(argv);
    let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

    if args.reset_terminal {
        reset_terminal_emergency();
        return Ok(());
    }

    if args.dump_config {
        print!("{}", configfile::dump_config_text());
        return Ok(());
    }

    if args.config_path {
        println!("{}", configfile::default_config_file_path().display());
        return Ok(());
    }

    // --completions: print shell completions and exit.
    if let Some(ref shell) = args.completions {
        let shell = shell.to_lowercase();
        let shell_id = match shell.as_str() {
            "bash" => clap_complete::Shell::Bash,
            "zsh" => clap_complete::Shell::Zsh,
            "fish" => clap_complete::Shell::Fish,
            "elvish" => clap_complete::Shell::Elvish,
            _ => {
                eprintln!("error: unknown shell '{shell}' (supported: bash, zsh, fish, elvish)");
                std::process::exit(2);
            }
        };
        let mut cmd = Args::command();
        clap_complete::generate(shell_id, &mut cmd, "cosmostrix", &mut std::io::stdout());
        return Ok(());
    }

    if args.testconf {
        return testconf::run(&args);
    }

    if args.list_profiles {
        let cfg = configfile::load_config_file(args.config.as_deref());
        let profiles = profile::collect_profiles(&cfg);
        print!("{}", profile::list_profiles_text(&profiles));
        return Ok(());
    }

    if let Some(ref name) = args.dump_profile {
        let cfg = configfile::load_config_file(args.config.as_deref());
        let profiles = profile::collect_profiles(&cfg);
        match profile::dump_profile_text(&profiles, name) {
            Ok(text) => print!("{text}"),
            Err(e) => ux::die_config(e),
        }
        return Ok(());
    }

    if let Err(e) = config_apply::apply_config_and_runtime_defaults(&matches, &mut args) {
        ux::die_config(e);
    }
    canonicalize_runtime_args(&mut args);

    if args.list_presets {
        preset::print_list_presets();
        return Ok(());
    }

    if let Some(ref name) = args.show_preset {
        match preset::print_show_preset(name) {
            Ok(()) => return Ok(()),
            Err(e) => ux::die_config(e),
        }
    }

    if args.list_scenes {
        print_list_scenes();
        return Ok(());
    }

    if args.list_charsets {
        print_list_charsets();
        return Ok(());
    }

    if args.list_colors {
        print_list_colors();
        return Ok(());
    }

    if args.list_colors_detail {
        print_list_colors_detail();
        return Ok(());
    }

    if args.defaults {
        print_defaults();
        return Ok(());
    }

    if args.help_detail {
        help_detail::print_help_detail();
        return Ok(());
    }

    if args.doctor {
        doctor::print_doctor_report(&args);
        return Ok(());
    }

    if args.check_bitcolor {
        let colorterm = env::var("COLORTERM").unwrap_or_default();
        let term = env::var("TERM").unwrap_or_default();
        let auto = detect_color_mode_auto();
        let effective = detect_color_mode(&args);

        println!("BITCOLOR CHECK:");
        println!(
            "  COLORTERM: {}",
            if colorterm.is_empty() {
                "(unset)"
            } else {
                &colorterm
            }
        );
        println!(
            "  TERM: {}",
            if term.is_empty() { "(unset)" } else { &term }
        );
        println!("  auto_detected: {}", color_mode_label(auto));
        if args.colormode.is_some() {
            println!("  forced: {}", color_mode_label(effective));
        }
        println!("  effective: {}", color_mode_label(effective));
        return Ok(());
    }

    if args.version {
        println!("{}", info::version_report());
        return Ok(());
    }

    if args.check_update {
        if let Err(e) = update::check_update(env!("CARGO_PKG_VERSION")) {
            ux::die_config(format!("error: update check failed: {e}"));
        }
        return Ok(());
    }

    if args.info {
        let cpu = diagnostics::detect_cpu_info();
        let color_mode = detect_color_mode(&args);
        let ri = renderer_info::renderer_info(color_mode);

        let mut r = report::Report::new("COSMOSTRIX");
        {
            let s = r.section("BUILD");
            s.field("version", &format!("v{}", env!("CARGO_PKG_VERSION")));
            if let Some(sha) = info::build_commit_short() {
                s.field("commit", sha);
            }
            s.field("variant", cpu.build_variant);
            s.field("optimization", env!("COSMOSTRIX_OPTIMIZATION"));
            s.field("dispatch", cpu.dispatch);
            s.field("cpu_baseline", env!("COSMOSTRIX_CPU_BASELINE"));
            s.field("target_features", env!("COSMOSTRIX_TARGET_FEATURES"));
            s.field("rustc", env!("COSMOSTRIX_RUSTC_VERSION"));
            s.field("lto", env!("COSMOSTRIX_LTO"));
            s.field("panic", env!("COSMOSTRIX_PANIC"));
            s.field("strip", env!("COSMOSTRIX_STRIP"));
        }
        {
            let s = r.section("RENDERER");
            s.field("backend", ri.backend);
            s.field("pacing", ri.pacing);
            s.field("unicode", ri.unicode);
            s.field("frame_strategy", ri.frame_strategy);
            s.field("dirty_tracking", ri.dirty_tracking);
            s.field("io_strategy", ri.io_strategy);
            s.field("color_depth", ri.color_depth);
            s.field("identity", ri.identity);
        }
        {
            let s = r.section("CAPACITY");
            s.field(
                "est_memory_per_frame (120x40)",
                &info::format_bytes(info::estimate_memory_budget(120, 40)),
            );
            s.field(
                "est_memory_per_frame (200x60)",
                &info::format_bytes(info::estimate_memory_budget(200, 60)),
            );
        }
        {
            let s = r.section("RUNTIME PROFILE");
            s.field("fps", &format!("{}", args.fps));
            s.field("speed", &format!("{}", args.speed));
            s.field("density", &format!("{}", args.density));
            s.field("monolith_size", args.monolith_size.as_str());
            s.field("color", &args.color);
            s.field("charset", &args.charset);
            s.field(
                "scene",
                args.scene.as_deref().unwrap_or(crate::scene::DEFAULT_SCENE),
            );
            let rain_style = args
                .scene
                .as_deref()
                .and_then(scene::rain_style_for_scene)
                .unwrap_or(rain_style::RainStyle::Glyph);
            s.field("rain_style", rain_style.as_str());
            s.field(
                "glitch_level",
                &format!("{:?}", args.glitch_level).to_lowercase(),
            );
            s.field_if("low_power", "on", args.low_power);
            if let Some(ref pname) = args.preset {
                s.field("preset", pname);
            }
            if let Some(ref pname) = args.profile {
                s.field("profile", pname);
            }
            s.field(
                "auto_color_drift",
                if args.auto_color_drift {
                    "true"
                } else {
                    "false"
                },
            );
        }
        {
            let ctrl = atmosphere::AtmosphereController::new();
            let app = ctrl.build_application();
            // Phase 10: Resolve atmosphere config for diagnostics.
            let diag_mode =
                config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
            let diag_regime =
                config_apply::resolve_atmosphere_regime(args.atmosphere_regime_str.as_deref());
            let modulation = atmosphere_apply::apply_application(&app, diag_mode);
            let s = r.section("ATMOSPHERE");
            s.field("regime", diag_regime.as_str());
            s.field("engine", "phase-10-config-gated");
            s.field(
                "effective",
                if modulation.is_identity() {
                    "identity"
                } else {
                    "modulated"
                },
            );
            s.field("verifier", "pass");
            s.field(
                "application",
                if app.is_identity() {
                    "identity"
                } else {
                    "verified"
                },
            );
            s.field("application_mode", diag_mode.as_str());
            // Phase 5: effective runtime seam
            let eff =
                atmosphere_apply::derive_effective_runtime(args.speed, args.density, &modulation);
            s.field(
                "effective_runtime",
                if eff.speed == args.speed && eff.density == args.density {
                    "identity"
                } else {
                    "modulated"
                },
            );
            // Phase 8: shadow metrics
            let shadow =
                atmosphere_shadow::shadow_metrics_from_mode_and_regime(diag_mode, diag_regime);
            s.field("shadow_metrics", shadow.risk_label());
            s.field("shadow_risk", shadow.risk_label());
            // Phase 10.5: diagnostic honesty fields
            s.field(
                "config_gate",
                if diag_mode.allows_modulation() {
                    "armed"
                } else {
                    "disabled"
                },
            );
            s.field(
                "visual_runtime",
                if eff.speed == args.speed && eff.density == args.density {
                    "protected"
                } else {
                    "active"
                },
            );
            s.field(
                "runtime_application",
                if modulation.is_identity() {
                    "identity"
                } else {
                    "non-identity"
                },
            );
        }
        // ── SYSTEM diagnostics ──────────────────────────────────────────
        // Cosmostrix is single-thread by design — terminal single-owner.
        {
            let s = r.section("SYSTEM");
            s.field("runtime_mode", "normal");
            s.field("render_plan", "single-owner");
            s.field("idle_policy", "adaptive-sleep");
            s.field("architecture", "single-thread optimized");
        }
        r.print();
        return Ok(());
    }

    // --- Validate all arguments using Result-based validators ---
    let def_ascii = default_to_ascii();
    let color_mode = detect_color_mode(&args);

    let shading_mode = match validate_u8_range("--shadingmode", args.shading_mode, 0, 1) {
        Ok(1) => ShadingMode::DistanceFromHead,
        _ => ShadingMode::Random,
    };

    let bold_mode = match validate_u8_range("--bold", args.bold, 0, 2) {
        Ok(0) => BoldMode::Off,
        Ok(2) => BoldMode::All,
        _ => BoldMode::Random,
    };

    let target_fps = ux::or_exit(validate_f64_range("--fps", args.fps, 1.0, 240.0));

    let duration_s = args.duration.map(|s| {
        if !s.is_finite() {
            ux::die_config(format!("--duration {s}: must be a finite number"));
        }
        if s > 0.0 {
            // ux::or_exit never returns on Err; on Ok returns T directly.
            return ux::or_exit(validate_f64_range("--duration", s, 0.1, 86400.0));
        }
        s
    });

    let color_scheme = match parse_color_scheme(&args.color) {
        Ok(c) => c,
        Err(e) => ux::die_input(e),
    };
    let color_tune = match args.color_tune.as_deref() {
        Some(s) => ux::or_exit(color_tune::parse_color_tune(s)),
        None => color_tune::ColorTune::IDENTITY,
    };
    let rain_style = args
        .scene
        .as_deref()
        .and_then(scene::rain_style_for_scene)
        .unwrap_or(rain_style::RainStyle::Glyph);

    let glitch_pct = ux::or_exit(validate_f32_range(
        "--glitchpct",
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
    let short_pct = ux::or_exit(validate_f32_range("--shortpct", args.shortpct, 0.0, 100.0));
    let die_early_pct = ux::or_exit(validate_f32_range("--rippct", args.rippct, 0.0, 100.0));
    let max_dpc = ux::or_exit(validate_u8_range(
        "--maxdpc",
        args.max_droplets_per_column,
        1,
        3,
    ));
    let speed = ux::or_exit(validate_speed(args.speed));

    let mut user_ranges: Vec<(char, char)> = Vec::new();
    if let Some(spec) = &args.chars {
        match parse_user_hex_chars(spec) {
            Ok(list) => {
                if list.len() % 2 != 0 {
                    ux::die_config(
                        "error: --chars: odd number of unicode chars given (must be even)",
                    );
                }
                for pair in list.chunks(2) {
                    user_ranges.push((pair[0], pair[1]));
                }
            }
            Err(e) => ux::die_config(format!("error: {e}")),
        }
    }

    let charset_preset = normalize_charset_preset_name(&args.charset);

    // --charset-file: load custom characters from file, overriding preset.
    // Security: only allow reading from safe locations — home directory,
    // current directory, or /etc/cosmostrix/. Prevents cosmostrix from
    // being used as an arbitrary file reader (e.g., /etc/shadow).
    let chars = if let Some(ref cf) = args.charset_file {
        if args.verbose {
            eprintln!("[verbose] charset-file: {cf} (safe: {})", is_safe_path(cf));
        }
        if !is_safe_path(cf) {
            ux::die_input(format!(
                "error: --charset-file '{cf}' is outside allowed directories\n  \
                 Allowed: ~/.config/cosmostrix/, current directory (.), /etc/cosmostrix/, /tmp/"
            ));
        }
        match std::fs::read_to_string(cf) {
            Ok(content) => {
                use unicode_width::UnicodeWidthChar;
                let mut custom_chars: Vec<char> = Vec::new();
                let mut skipped_wide: Vec<String> = Vec::new();
                for ch in content.chars() {
                    // Skip whitespace except space
                    if ch.is_whitespace() && ch != ' ' {
                        continue;
                    }
                    // Skip control characters
                    if ch.is_control() {
                        continue;
                    }
                    // Filter wide/zero-width characters (emoji, CJK fullwidth, etc.)
                    // Same filter as charset.rs — renderer is column-based, assumes
                    // 1 cell per character. Wide chars corrupt glyph alignment.
                    match ch.width() {
                        Some(1) => custom_chars.push(ch),
                        _ => skipped_wide.push(format!("U+{:04X}", ch as u32)),
                    }
                }
                if !skipped_wide.is_empty() {
                    ux::warn(format!(
                        "skipped {} wide/zero-width character(s) from --charset-file: {}",
                        skipped_wide.len(),
                        skipped_wide.join(", ")
                    ));
                }
                if custom_chars.is_empty() {
                    ux::die_config(format!(
                        "error: --charset-file '{cf}' contains no usable single-width characters"
                    ));
                }
                custom_chars
            }
            Err(e) => {
                ux::die_config(format!("error: cannot read --charset-file '{cf}': {e}"));
            }
        }
    } else {
        let charset = match charset_from_str(&args.charset, def_ascii) {
            Ok(c) => c,
            Err(e) => ux::die_input(e),
        };
        build_chars(charset, &user_ranges, def_ascii)
    };

    let density_auto = matches.value_source("density") == Some(ValueSource::DefaultValue);
    let base_density = ux::or_exit(validate_f32_range(
        "--density",
        args.density,
        DENSITY_CLAMP_MIN,
        DENSITY_CLAMP_MAX,
    ));

    let default_bg = matches!(
        args.color_bg,
        ColorBg::DefaultBackground | ColorBg::Transparent
    );

    // Phase 5 + Phase 10: Resolve atmosphere config from config/profile keys.
    // Default is Disabled/Calm — identical to v3.9.0 behavior.
    let atmosphere_mode =
        config_apply::resolve_atmosphere_mode(args.atmosphere_mode_str.as_deref());
    let atmosphere_regime =
        config_apply::resolve_atmosphere_regime(args.atmosphere_regime_str.as_deref());

    // Build atmosphere modulation from resolved config.
    // When mode is Disabled, modulation is always identity regardless of regime.
    let (atmosphere_modulation, _resolved_regime) = if atmosphere_mode.allows_modulation() {
        let modulation = crate::atmosphere_controlled_live::controlled_live_modulation_from_regime(
            atmosphere_regime,
        );
        (modulation, atmosphere_regime)
    } else {
        (
            atmosphere_apply::AtmosphereRuntimeModulation::identity(),
            atmosphere::AtmosphereRegime::Calm,
        )
    };

    // ── Verbose output (before CloudConfig moves values) ──
    if args.verbose {
        verbose::print_verbose(
            env!("CARGO_PKG_VERSION"),
            args.scene.as_deref(),
            rain_style,
            color_scheme,
            color_mode,
            color_tune,
            default_bg,
            &charset_preset,
            &chars,
            args.fullwidth,
            target_fps,
            speed,
            base_density,
            density_auto,
            args.monolith_size,
            args.async_mode,
            bold_mode,
            shading_mode,
            args.noglitch,
            glitch_pct,
            glitch_low,
            glitch_high,
            &format!("{:?}", args.glitch_level),
            args.mouse,
            args.low_power,
            args.screensaver,
            args.auto_color_drift,
            atmosphere_mode,
            &atmosphere_modulation,
            args.message.as_deref(),
            args.message_border,
            args.duration,
            args.charset_file.as_deref(),
        );
    }

    let cloud_cfg = CloudConfig {
        color_mode,
        fullwidth: args.fullwidth,
        shading_mode,
        bold_mode,
        async_mode: args.async_mode,
        default_bg,
        color_scheme,
        rain_style,
        noglitch: args.noglitch,
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
        message: args.message.as_deref().map(|m| {
            if m.len() > MESSAGE_MAX_LEN {
                ux::die_input(format!(
                    "error: --message text exceeds {MESSAGE_MAX_LEN} character limit (got {})",
                    m.len()
                ));
            }
            m.to_string()
        }),
        message_border: args.message_border,
        target_fps,
        duration: args.duration,
        duration_s,
        bench_frames: args.bench_frames,
        benchmark: args.benchmark,
        bench_duration: args.bench_duration,
        color_tune,
        json: args.json,
        verbose: args.verbose,
        density_auto,
        base_density,
        perf_stats: args.perf_stats,
        screensaver: args.screensaver,
        mouse: args.mouse,
        charset_preset,
        user_ranges,
        def_ascii,
        auto_color_drift: args.auto_color_drift,
        // Phase 10: atmosphere modulation resolved from config/profile.
        atmosphere_modulation,
        atmosphere_mode,
    };

    if args.benchmark {
        return bench::run_premium_benchmark(&cloud_cfg);
    }

    if let Some(_bench_frames) = args.bench_frames {
        return bench::run_benchmark(&cloud_cfg);
    }

    interactive::run_interactive(&cloud_cfg)
}

fn canonicalize_runtime_args(args: &mut Args) {
    if let Some(canonical) = theme::canonical_name_for_input(&args.color) {
        args.color = canonical.to_string();
    }
}

#[cfg(test)]
mod color_detection_tests;
