// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Cosmostrix — High-performance cinematic Matrix rain renderer for the terminal.
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
mod bench;
mod cell;
mod charset;
mod cli;
mod cloud;
mod config;
mod configfile;
mod constants;
mod diagnostics;
mod doctor;
mod droplet;
mod frame;
mod info;
mod interactive;
mod palette;
mod renderer_info;
mod report;
mod runtime;
mod terminal;
mod validation;

use std::env;

#[cfg(target_os = "linux")]
use std::io::IsTerminal;

use clap::parser::ValueSource;
use clap::{CommandFactory, FromArgMatches};

use crate::charset::{build_chars, charset_from_str, parse_user_hex_chars};
use crate::config::{
    color_enabled_stdout, print_defaults, print_help_detail, print_list_charsets,
    print_list_colors, Args, ColorBg, GlitchLevel, U16Range,
};
use crate::constants::*;
use crate::runtime::{BoldMode, ShadingMode};
use crate::terminal::{reset_terminal_emergency, restore_terminal_best_effort};
use crate::validation::{
    validate_f32_range, validate_f64_range, validate_u16_range, validate_u8_range,
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

/// Convert a `Result<T, String>` validation error to `io::Error`.
/// Side effect: restores the terminal and prints the error message to stderr
/// before returning the error, so the user doesn't see a broken terminal.
fn validate_err<T>(name: &str, r: Result<T, String>) -> std::io::Result<T> {
    r.map_err(|e| {
        restore_terminal_best_effort();
        eprintln!("{}", e);
        std::io::Error::new(std::io::ErrorKind::InvalidInput, name)
    })
}

#[cfg(target_os = "linux")]
pub fn spawn_kill9_terminal_guard() {
    if env_var_truthy("COSMOSTRIX_NO_FORK_GUARD") {
        return;
    }

    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return;
    }

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
        if sig == libc::SIGTERM {
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

    std::panic::set_hook(Box::new(|info| {
        restore_terminal_best_effort();
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

    let mut argv: Vec<std::ffi::OsString> = env::args_os().collect();
    for arg in argv.iter_mut().skip(1) {
        if arg == "-mB" || arg == "-mb" {
            *arg = "--message-no-border".into();
        }
    }

    let matches = cmd.get_matches_from(argv);
    let mut args = Args::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());

    if args.reset_terminal {
        reset_terminal_emergency();
        return Ok(());
    }

    // Apply config file defaults for args not explicitly set by user
    app::apply_config_defaults(&matches, &mut args);

    // Apply --low-power overrides for args still at their default values.
    // Explicit CLI flags always take precedence; config file values are also
    // overridden since --low-power is an intentional one-shot mode request.
    if args.low_power {
        if matches.value_source("fps") == Some(ValueSource::DefaultValue) {
            args.fps = 30.0;
        }
        if matches.value_source("speed") == Some(ValueSource::DefaultValue) {
            args.speed = 5.0;
        }
        if matches.value_source("density") == Some(ValueSource::DefaultValue) {
            args.density = 0.5;
        }
    }

    // Apply --glitch-level preset. Each level sets glitch-related parameters
    // to curated values. Individual hidden flags (--glitchpct, --glitchms, etc.)
    // override the preset if explicitly provided by the user.
    let is_explicit = |key: &str| matches.value_source(key) != Some(ValueSource::DefaultValue);
    match args.glitch_level {
        GlitchLevel::None => {
            if !is_explicit("noglitch") {
                args.noglitch = true;
            }
        }
        GlitchLevel::Subtle => {
            if !is_explicit("noglitch") {
                args.noglitch = false;
            }
            if !is_explicit("glitchpct") {
                args.glitch_pct = 3.0;
            }
            if !is_explicit("glitchms") {
                args.glitch_ms = U16Range {
                    low: 200,
                    high: 300,
                };
            }
            if !is_explicit("shortpct") {
                args.shortpct = 60.0;
            }
            if !is_explicit("rippct") {
                args.rippct = 45.0;
            }
        }
        GlitchLevel::Default => {
            if !is_explicit("noglitch") {
                args.noglitch = false;
            }
            if !is_explicit("glitchpct") {
                args.glitch_pct = 10.0;
            }
            if !is_explicit("glitchms") {
                args.glitch_ms = U16Range {
                    low: 300,
                    high: 400,
                };
            }
            if !is_explicit("shortpct") {
                args.shortpct = 50.0;
            }
            if !is_explicit("rippct") {
                args.rippct = 33.33333;
            }
        }
        GlitchLevel::Intense => {
            if !is_explicit("noglitch") {
                args.noglitch = false;
            }
            if !is_explicit("glitchpct") {
                args.glitch_pct = 25.0;
            }
            if !is_explicit("glitchms") {
                args.glitch_ms = U16Range {
                    low: 500,
                    high: 800,
                };
            }
            if !is_explicit("shortpct") {
                args.shortpct = 30.0;
            }
            if !is_explicit("rippct") {
                args.rippct = 20.0;
            }
        }
    }

    if args.list_charsets {
        print_list_charsets();
        return Ok(());
    }

    if args.list_colors {
        print_list_colors();
        return Ok(());
    }

    if args.defaults {
        print_defaults();
        return Ok(());
    }

    if args.help_detail {
        print_help_detail();
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
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
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
            s.field("color", &args.color);
            s.field("charset", &args.charset);
            s.field(
                "glitch_level",
                &format!("{:?}", args.glitch_level).to_lowercase(),
            );
            s.field_if("low_power", "on", args.low_power);
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

    let target_fps = validate_err("--fps", validate_f64_range("--fps", args.fps, 1.0, 240.0))?;

    let duration_s = args.duration.map(|s| {
        if !s.is_finite() {
            eprintln!("failed to apply --duration {} (must be a finite number)", s);
            std::process::exit(1);
        }
        if s > 0.0 {
            return validate_err(
                "--duration",
                validate_f64_range("--duration", s, 0.1, 86400.0),
            )
            .unwrap();
        }
        s
    });

    let color_scheme = validate_err("--color", parse_color_scheme(&args.color))?;

    let glitch_pct = validate_err(
        "--glitchpct",
        validate_f32_range("--glitchpct", args.glitch_pct, 0.0, 100.0),
    )?;
    let glitch_low = validate_err(
        "--glitchms low",
        validate_u16_range("--glitchms low", args.glitch_ms.low, 1, 5000),
    )?;
    let glitch_high = validate_err(
        "--glitchms high",
        validate_u16_range("--glitchms high", args.glitch_ms.high, 1, 5000),
    )?;
    let linger_low = validate_err(
        "--lingerms low",
        validate_u16_range("--lingerms low", args.linger_ms.low, 1, 60000),
    )?;
    let linger_high = validate_err(
        "--lingerms high",
        validate_u16_range("--lingerms high", args.linger_ms.high, 1, 60000),
    )?;
    let short_pct = validate_err(
        "--shortpct",
        validate_f32_range("--shortpct", args.shortpct, 0.0, 100.0),
    )?;
    let die_early_pct = validate_err(
        "--rippct",
        validate_f32_range("--rippct", args.rippct, 0.0, 100.0),
    )?;
    let max_dpc = validate_err(
        "--maxdpc",
        validate_u8_range("--maxdpc", args.max_droplets_per_column, 1, 3),
    )?;
    let speed = validate_err(
        "--speed",
        validate_f32_range("--speed", args.speed, 0.001, 1000.0),
    )?;

    let mut user_ranges: Vec<(char, char)> = Vec::new();
    if let Some(spec) = &args.chars {
        match parse_user_hex_chars(spec) {
            Ok(list) => {
                if list.len() % 2 != 0 {
                    eprintln!("--chars: odd number of unicode chars given (must be even)");
                    std::process::exit(1);
                }
                for pair in list.chunks(2) {
                    user_ranges.push((pair[0], pair[1]));
                }
            }
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    }

    let charset = match charset_from_str(&args.charset, def_ascii) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    let charset_preset = normalize_charset_preset_name(&args.charset);

    let chars = build_chars(charset, &user_ranges, def_ascii);

    let density_auto = matches.value_source("density") == Some(ValueSource::DefaultValue);
    let base_density = validate_err(
        "--density",
        validate_f32_range(
            "--density",
            args.density,
            DENSITY_CLAMP_MIN,
            DENSITY_CLAMP_MAX,
        ),
    )?;

    let default_bg = matches!(
        args.color_bg,
        ColorBg::DefaultBackground | ColorBg::Transparent
    );

    let cloud_cfg = CloudConfig {
        color_mode,
        fullwidth: args.fullwidth,
        shading_mode,
        bold_mode,
        async_mode: args.async_mode,
        default_bg,
        color_scheme,
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
        chars,
        message: args.message.clone(),
        message_no_border: args.message_no_border,
        target_fps,
        duration: args.duration,
        duration_s,
        bench_frames: args.bench_frames,
        benchmark: args.benchmark,
        density_auto,
        base_density,
        perf_stats: args.perf_stats,
        screensaver: args.screensaver,
        mouse: args.mouse,
        charset_preset,
        user_ranges,
        def_ascii,
    };

    if args.benchmark {
        return bench::run_premium_benchmark(&cloud_cfg);
    }

    if let Some(_bench_frames) = args.bench_frames {
        return bench::run_benchmark(&cloud_cfg);
    }

    interactive::run_interactive(&cloud_cfg)
}

#[cfg(test)]
mod color_detection_tests {
    use crate::cli::detect_color_mode_from_terms;
    use crate::runtime::ColorMode;

    #[test]
    fn term_xterm_direct_detects_truecolor_without_colorterm() {
        assert_eq!(
            detect_color_mode_from_terms("", "xterm-direct"),
            ColorMode::TrueColor
        );
    }

    #[test]
    fn term_tmux_direct_detects_truecolor_without_colorterm() {
        assert_eq!(
            detect_color_mode_from_terms("", "tmux-direct"),
            ColorMode::TrueColor
        );
    }

    #[test]
    fn term_xterm_256color_preserves_256color_detection() {
        assert_eq!(
            detect_color_mode_from_terms("", "xterm-256color"),
            ColorMode::Color256
        );
    }

    #[test]
    fn colorterm_truecolor_still_overrides_term() {
        assert_eq!(
            detect_color_mode_from_terms("truecolor", "xterm"),
            ColorMode::TrueColor
        );
    }
}
