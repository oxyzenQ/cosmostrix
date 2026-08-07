// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cosmostrix — Professional-grade cinematic Matrix rain renderer for serious terminal environments.
//!
//! Cosmostrix transforms your terminal into a living, breathing canvas of
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

// Phase 5: Global allocator tracing wrapper.
#[global_allocator]
static GLOBAL_ALLOC: crate::alloc_trace::TraceAlloc = crate::alloc_trace::TraceAlloc;
mod alloc_trace;
mod ambient;
// Ambient phase scheduler — config-driven time-of-day scene/parameter
// switching. Replaces the archived `adaptive-custom` subsystem with a
// simpler contract: config-only (no CLI flag), instant switch (no blend
// window), dynamic idle/wake scheduler thread (zero CPU between phase
// boundaries). See `src/ambient.rs` and `src/ambient_scheduler.rs`.
mod ambient_scheduler;
mod app;
// Atmosphere engine subsystem fully eliminated (Dragon Hunt v2 Phase 6
// Tier E item 31 — full elimination). Owner decided atmosphere engine
// is not used in the future. All 14 atmosphere source files (~6.5k LOC)
// and their CLI flags / config keys / profile fields / docs have been
// removed. Design knowledge preserved in
// docs/archive/audits/ATMOSPHERE_SUBSYSTEM_ARCHIVAL.md.
//
// NOTE: src/chroma/post/climate.rs (ClimateCtx — luminance/
// saturation/instability shader) is a SEPARATE Chroma Dragon post-FX
// subsystem and is KEPT. The `EntropyDrift` struct in
// cloud/ecosystem.rs (drift/gust events) is also SEPARATE and KEPT.
mod bench;
mod bench_baseline;
mod bench_comp;
mod bench_cpu;
mod bench_energy;
mod bench_helpers;
mod bench_io;
mod bench_json;
mod bench_mem;
mod bench_meta;
mod bench_perf;
mod bench_progress;
mod bench_report;
#[cfg(test)]
mod bench_report_tests;
mod bench_scale;
mod bench_visual;
mod bolt;
mod cell;
mod charset;
mod charset_custom;
// Chroma Dragon coloring engine — Phase 1: relocated palette + catalog here.
// Re-exports below keep the old `crate::palette::…` paths working unchanged.
mod chroma;
pub use chroma::catalog;
pub use chroma::palette;
mod central_control_rains;
mod cinematic;
mod cli;
mod cli_parse;
mod clock;
mod cloud;
mod color_cache;
mod color_tune;
mod colors_custom;
mod config;
mod config_apply;
#[cfg(test)]
mod config_apply_tests;
mod config_hints;
mod configfile;
#[cfg(test)]
mod configfile_tests;
mod constants;
mod cosmic_dragon;
mod cpustat;
// Owner-editable control file for --auto-color-drift system feeling.
// This is the single taste file: FeelingState enum, CPU/time thresholds,
// and the state→ColorFamily mapping. Edit this to retune drift behavior.
mod control_color_drift;
mod diagnostics;
#[cfg(test)]
mod docs_tests;
mod doctor;
mod droplet;
mod envstat;
mod frame;
mod help_detail;
mod humanize;
mod info;
mod interactive;
// live_config_trace MUST be declared before live_config so the
// `lr_trace!` macro it exports is in scope for live_config.rs.
// `#[macro_use]` re-exports the macro crate-wide as a defense-in-depth
// (it is also `#[macro_export]`-ed from inside the module).
#[macro_use]
mod live_config_trace;
mod live_config;
mod live_config_poll;
#[cfg(test)]
mod loc_tests;
mod memstat;
mod output;
// `palette` now lives at `src/chroma/palette.rs`; re-exported above.
mod profile;
mod rain_style;
mod renderer_info;
mod report;
mod runtime;
mod safepath;
mod scene;
mod scene_custom;
mod sgr_format;
// Signal-driven palette drift classifier. Reads CPU% (cpustat) + local
// wall-clock hour, classifies into a FeelingState (defined in
// control_color_drift.rs), and feeds the state to ColorEcosystem::tick()
// for family-targeted drift selection.
mod system_feeling;
#[cfg(test)]
mod system_feeling_tests;
mod termdetect;
mod terminal;
#[cfg(test)]
mod terminal_tests;
mod terminal_tty;
mod testconf;
mod theme;
mod update;
mod usagestat;
mod ux;
mod validation;
mod verbose;
#[cfg(test)]
mod width_guard_tests;

use clap::{CommandFactory, FromArgMatches};

#[cfg(target_os = "linux")]
use std::io::IsTerminal;

use std::env;

use crate::charset::{build_chars, charset_from_str};
use crate::config::{
    color_enabled_stdout, print_list_charsets, print_list_colors, print_list_scenes,
    print_show_scene, Args, ColorBg,
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
// Input validation uses `ux::or_exit()` instead of the old `validate_err`.
// `or_exit` unwraps a Result whose Err carries a formatted error string,
// prints it to stderr, and exits with code 2 — never propagating a
// `std::io::Error` that Rust would render as a debug-looking
// `Error: Custom { ... }`.

// Path security validation lives in src/safepath.rs.
pub(crate) use crate::safepath::{is_safe_path, validate_config_path};

/// Check if stdout is redirected to a regular file (shell `>` or `>|` operator).
/// Returns `true` if stdout is a regular file (shell redirect bypassing whitelist).
/// Returns `false` for TTY, pipe (allowed), char device, socket.
/// Used by `--dump-config` to block shell redirection that bypasses the path whitelist.
#[cfg(unix)]
fn stdout_is_redirected_to_file() -> bool {
    use std::os::unix::io::AsRawFd;
    let fd = std::io::stdout().as_raw_fd();
    // SAFETY: fstat on a valid fd (stdout=1, always open). The stat struct
    // is zeroed and overwritten by the syscall.
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: fstat reads metadata for an already-open fd. Writes only into
    // our zeroed stat struct; returns 0 on success.
    if unsafe { libc::fstat(fd, &mut st) } == 0 {
        return (st.st_mode & libc::S_IFMT) == libc::S_IFREG;
    }
    // If fstat fails (shouldn't happen on stdout), don't block — let the
    // write proceed. Better to be permissive than to break a valid use case.
    false
}

/// Write `text` to `target_path` atomically: temp-file + fsync + rename.
/// POSIX `rename(2)` is atomic — readers see either old or new file, never
/// a half-written one. Temp lives in same dir (same-filesystem move) as
/// `<target>.tmp.<pid>`. Best-effort cleanup on error.
fn write_config_atomic(target_path: &str, text: &str) -> std::io::Result<()> {
    let target = std::path::Path::new(target_path);
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    // Create parent dir if missing — skip /etc/ and /var/ (system paths
    // should require explicit user creation to avoid wrong-permission auto).
    if !parent.exists() {
        if let Some(parent_str) = parent.to_str() {
            if !parent_str.starts_with("/etc/") && !parent_str.starts_with("/var/") {
                std::fs::create_dir_all(parent)?;
            }
        }
    }
    let pid = std::process::id();
    let tmp_name = format!(
        "{}.tmp.{pid}",
        target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("config.toml")
    );
    let tmp_path = parent.join(&tmp_name);
    std::fs::write(&tmp_path, text)?;
    // fsync for crash-durability. If it fails, rename still proceeds (data
    // is in page cache). Surface error only if rename itself fails.
    if let Ok(file) = std::fs::File::open(&tmp_path) {
        let _ = file.sync_all();
    }
    std::fs::rename(&tmp_path, target)?;
    Ok(())
}

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

    // Panic hook: restore the terminal BEFORE printing the panic message.
    // See `install_panic_hook()` for the full rationale.
    install_panic_hook();

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

    // --help: print the full curated reference manual and exit.
    //
    // Checked early (before --dump-config, --doctor, --version, etc.) so
    // `cosmostrix --help` always works even if other flags are malformed
    // or the config file is broken. This mirrors how clap's auto-help
    // behaves: help wins over everything else.
    if args.help {
        help_detail::print_help();
        return Ok(());
    }

    if args.reset_terminal {
        reset_terminal_emergency();
        return Ok(());
    }

    // --dump-config: print example config to stdout (TTY only), OR write to
    // a file if a path argument was given.
    //
    // Security (v15 strict policy):
    //   1. Path must be inside the strict whitelist (~/.config/cosmostrix/
    //      or /etc/cosmostrix/) — same as --config.
    //   2. Path must have a .toml extension — same as --config.
    //   3. Shell redirection (>, >|) is BLOCKED: if --dump-config is used
    //      without a path argument AND stdout is redirected to a regular
    //      file, cosmostrix refuses to write. This prevents bypassing the
    //      whitelist via `cosmostrix --dump-config > /tmp/a.txt`.
    //      The user MUST use the explicit path form:
    //        cosmostrix --dump-config ~/.config/cosmostrix/config.toml
    //      Piping to another command (cosmostrix --dump-config | less) is
    //      still allowed — only file redirection is blocked.
    //
    // The flag uses clap's num_args=0..=1 pattern:
    //   --dump-config            → Some("") → print to stdout (TTY or pipe only)
    //   --dump-config <path>     → Some("<path>") → write to file (validated)
    //   (not passed)             → None → skip
    if let Some(ref dump_path) = args.dump_config {
        if dump_path.is_empty() {
            // No path argument: print to stdout. But BLOCK if stdout is
            // redirected to a file (shell > or >| operator). This forces
            // the user to use --dump-config <path> for file output, which
            // enforces the whitelist.
            #[cfg(unix)]
            {
                if stdout_is_redirected_to_file() {
                    // Route through ux::die_input so the exit code (2) and
                    // error formatting match every other CLI input error.
                    // Previously this used process::exit(2) directly, bypassing
                    // the ux module's centralized error handling.
                    ux::die_input(
                        "refusing to write --dump-config to a redirected file\n  \
                         Shell redirection (>, >|) bypasses the strict whitelist.\n  \
                         Use the explicit path form instead:\n    \
                         cosmostrix --dump-config ~/.config/cosmostrix/config.toml\n  \
                         The path must be inside ~/.config/cosmostrix/ or /etc/cosmostrix/ \
                         and have a .toml extension.\n  \
                         Piping to another command (cosmostrix --dump-config | less) is allowed.",
                    );
                }
            }
            print!("{}", configfile::dump_config_with_header());
            return Ok(());
        }
        // Path argument given: validate whitelist + .toml extension.
        // Reuse validate_config_path() so --dump-config and --config stay
        // perfectly in sync. Map the --config label to --dump-config in
        // error messages. Use the RESOLVED path for all I/O (expands
        // %APPDATA% on Windows — the raw path would create a literal
        // %APPDATA% directory instead of resolving it).
        let path_str = dump_path;
        let resolved_path = match validate_config_path(path_str, args.verbose) {
            Ok(r) => r,
            Err(e) => ux::die_input(e.replace("--config", "--dump-config")),
        };
        // Write the example config to the validated path.
        // Phase 5 (P3-7): refuse to overwrite an existing file. Previously
        // --dump-config silently overwrote any existing config at the path,
        // causing data loss if the user pointed it at their carefully-tuned
        // ~/.config/cosmostrix/config.toml. Now: if the file exists, exit
        // with a clear error + suggest writing to a .new suffix instead.
        //
        // v30 (2026-08-05): --force flag bypasses this guard. Use case: a
        // user who has read the existing config, decided they want to start
        // fresh, and explicitly opts in to overwrite. Still scoped to
        // --dump-config only (does not affect --save-baseline or other
        // write paths). The error message tells the user about --force so
        // they don't have to read the docs to discover it.
        if std::path::Path::new(&resolved_path).exists() && !args.force {
            ux::die_input(format!(
                "error: --dump-config refuses to overwrite existing file '{path_str}'\n  \
                 Move the existing file aside first, or write to a new path:\n    \
                 cosmostrix --dump-config {path_str}.new\n  \
                 Then review the new file and rename if appropriate.\n  \
                 To overwrite deliberately (destructive), pass --force:\n    \
                 cosmostrix --dump-config {path_str} --force"
            ));
        }
        // v30: atomic write via temp-file + fsync + rename. Previously a
        // direct `std::fs::write` — if the process was killed mid-write
        // (Ctrl-C, OOM, power loss), the target file could be left as a
        // zero-byte or truncated stub. With `--force` overwriting an
        // existing config, that meant destroying the user's previous config
        // AND leaving an incomplete one — the worst data-loss scenario the
        // guard was supposed to make explicit. Atomic rename guarantees
        // readers see either the old file or the complete new file, never
        // a half-written one.
        let text = configfile::dump_config_with_header();
        match write_config_atomic(&resolved_path, &text) {
            Ok(()) => {
                if args.verbose {
                    crate::output::eprintln_verbose_raw(&format!(
                        "dump-config: wrote example config to {resolved_path}"
                    ));
                }
                return Ok(());
            }
            Err(e) => {
                ux::die_config(format!(
                    "error: cannot write --dump-config to '{path_str}': {e}"
                ));
            }
        }
    }

    if args.config_path {
        println!("{}", configfile::default_config_file_path().display());
        return Ok(());
    }

    if args.testconf {
        return testconf::run(&args);
    }

    // v25.6 depth-test fix: --list-* and --show-scene bypass strict config
    // validation. Depth-test user with `charset-custom.long2.set` exceeding
    // the 256-char limit could not run `--list-charsets` because the strict
    // validation in apply_config_and_runtime_defaults killed the process
    // before list-commands ran. List/show commands only need to READ the
    // config (non-strict — bad keys are silently dropped by load_config_file),
    // not validate it. They use load_config_file(None) internally so the
    // user-supplied --config path is irrelevant for them. Path-security
    // validation for --show-scene is preserved (its existing inline check).
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

    if let Some(ref name) = args.show_scene {
        // Security (v16 audit): validate --config path BEFORE reading.
        // Previously --show-scene called load_config_file directly without
        // is_safe_path, allowing `cosmostrix --show-scene X --config /etc/passwd`
        // to parse arbitrary files as TOML and leak their content via
        // error messages. Now applies the same check as the main startup path.
        if let Some(ref config_path) = args.config {
            let path_str = config_path.to_string_lossy();
            if let Err(e) = validate_config_path(&path_str, args.verbose) {
                ux::die_input(e);
            }
            // validate_config_path resolved the path (expands %APPDATA% etc.),
            // but load_config_file takes an Option<&Path> from the original
            // args.config. On Windows, if the user passed %APPDATA%\..., the
            // OS file APIs won't resolve it. Override args.config with the
            // resolved path so load_config_file reads the correct file.
            // (Non-%VAR% paths: resolved == original, no-op.)
            #[cfg(windows)]
            {
                if let Ok(resolved) = validate_config_path(&path_str, false) {
                    args.config = Some(std::path::PathBuf::from(&resolved));
                }
            }
        }
        let cfg = configfile::load_config_file(args.config.as_deref());
        match print_show_scene(name, &cfg) {
            Ok(()) => return Ok(()),
            Err(e) => ux::die_config(e),
        }
    }

    // Benchmark default scene override (v25.16):
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
    let bench_mode = args.benchmark || args.bench_all;
    if bench_mode && args.scene.is_none() {
        args.scene = Some("monolith".to_string());
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

    let target_fps = ux::or_exit(validate_f64_range("--fps", args.fps, 1.0, 300.0));

    // v30 (VSCode crash fix): apply terminal-specific FPS cap after user
    // validation. VSCode's integrated terminal (TERM_PROGRAM=vscode) cannot
    // sustain 60 FPS indefinitely — xterm.js's in-memory buffer grows
    // without bound over multi-hour runs until V8 hits an OOM assertion
    // (SIGTRAP in the code-oss process). The cap is 30 FPS for VSCode,
    // 300 (effectively uncapped) for everything else. The user's --fps
    // value is clamped, not silently overridden — a warning is printed
    // so there's no confusion. Benchmark mode skips the cap (benchmarks
    // measure raw throughput, not terminal stability).
    let term_caps = crate::termdetect::detect();
    let target_fps =
        if !args.benchmark && term_caps.vscode_integrated && target_fps > term_caps.default_fps_cap
        {
            let capped = term_caps.default_fps_cap;
            crate::output::eprintln_warn_labeled(&format!(
                "VSCode integrated terminal detected (TERM_PROGRAM=vscode); \
             capping --fps from {target_fps:.1} to {capped:.0} to prevent \
             xterm.js OOM crash over long runs (see docs/TERMINAL_COMPATIBILITY.md)"
            ));
            capped
        } else {
            target_fps
        };

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
        None => {
            // v17: read [color.tune] from config.toml.
            let cfg_map = configfile::load_config_file(args.config.as_deref());
            color_tune::color_tune_from_config(&cfg_map)
        }
    };

    let rain_style = args
        .scene
        .as_deref()
        .and_then(scene::rain_style_for_scene)
        .unwrap_or(rain_style::RainStyle::Glyph);

    // v17 ghost labels: these struct fields are #[arg(skip)] and cannot be
    // set via CLI. The labels below are validator-name strings only — they
    // appear in error messages if the field defaults ever drift out of range.
    // Do NOT mistake them for live CLI flags; --glitchpct/--shortpct/--rippct/
    // --maxdpc were removed in v17 and replaced by --glitch-level.
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
    let startup_charset = charset_preset.clone();

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

    // v16: Load custom palette if --colors-custom is set.
    // The palette is loaded from config.toml's [colors-custom] section.
    // If loading fails, exit with a clear error (no silent fallback).
    // custom_palette_name is stored for live reload — when config changes,
    // rebuild_cloud_config reloads the palette definition by name.
    // This runs BEFORE verbose print so the verbose output can show the
    // correct palette name.
    let (custom_palette, custom_palette_name) = if let Some(ref name) = args.colors_custom {
        let cfg_map = configfile::load_config_file(args.config.as_deref());
        match colors_custom::load_custom_palette(&cfg_map, name) {
            Ok(p) => (Some(p), Some(name.clone())),
            Err(e) => ux::die_input(format!("error: --colors-custom '{name}': {e}")),
        }
    } else {
        (None, None)
    };

    let density_auto =
        matches.value_source("density") == Some(clap::parser::ValueSource::DefaultValue);
    let base_density = ux::or_exit(validate_f32_range(
        "--density",
        args.density,
        DENSITY_CLAMP_MIN,
        DENSITY_CLAMP_MAX,
    ));

    let default_bg = matches!(args.color_bg, ColorBg::DefaultBackground);

    // v17: --async flag removed. Async is always on (default true).
    // --uniform disables it (uniform wins = async off).
    let effective_async = args.async_mode && !args.uniform;

    // Parse --screen-size once here so verbose block and CloudConfig both
    // see the same validated value. Previously verbose used `.ok().flatten()`
    // which silently swallowed parse errors. Now we error out once, upfront.
    let screen_size = crate::ux::or_exit(crate::cli_parse::parse_screen_size_optional(
        &args.screen_size,
    ));

    // ── Verbose output (before CloudConfig moves values) ──
    let cli_explicit_color = matches!(
        matches.value_source("color"),
        Some(clap::parser::ValueSource::CommandLine)
    );
    // Bug 3 fix: capture which CLI flags were explicitly set so
    // rebuild_cloud_config can enforce the CLI > config.toml > scene
    // priority contract during live reload. Without this, a CLI flag
    // like `-c green` would be silently overridden the moment the
    // user edits `color = "snow"` in config.toml.
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
        // Phase D Bug #10 fix: track --auto-color-drift CLI intent so
        // live-reload doesn't silently override it with config.
        auto_color_drift: matches!(
            matches.value_source("auto_color_drift"),
            Some(clap::parser::ValueSource::CommandLine)
        ),
    };
    if args.verbose {
        // Resolve the intro type label for verbose output. Mirrors the
        // resolution in CloudConfig below: CLI --intro wins, else default
        // Logo. We emit the lowercase value-enum name to match the
        // --intro flag's accepted values (cosmic|logo|none).
        let resolved_intro = args.intro.unwrap_or(crate::config::IntroType::Logo);
        let intro_label = match resolved_intro {
            crate::config::IntroType::Cosmic => "cosmic",
            crate::config::IntroType::Logo => "logo",
            crate::config::IntroType::None => "none",
        };
        // Commit SHA: same source as -V output (COSMOSTRIX_GIT_SHA env var
        // injected at compile time by build.rs). Falls back to "unknown"
        // for local builds without git metadata.
        let commit_sha = option_env!("COSMOSTRIX_GIT_SHA").unwrap_or("unknown");
        verbose::print_verbose(
            env!("CARGO_PKG_VERSION"),
            args.scene.as_deref(),
            rain_style,
            color_scheme,
            color_mode,
            color_tune,
            args.color_bg,
            custom_palette.as_ref().and_then(|p| p.bg),
            &charset_preset,
            &chars,
            target_fps,
            speed,
            base_density,
            density_auto,
            args.monolith_size,
            effective_async,
            bold_mode,
            shading_mode,
            // v30 simplify: --noglitch CLI flag removed; derive glitch_enabled
            // from glitch_level. None => disabled, anything else => enabled.
            args.glitch_level != crate::config::GlitchLevel::None,
            glitch_pct,
            glitch_low,
            glitch_high,
            &format!("{:?}", args.glitch_level),
            args.screensaver,
            args.auto_color_drift,
            args.message.as_deref(),
            args.message_border,
            args.duration,
            screen_size,
            custom_palette_name.as_deref(),
            &args.scene,
            args.config.as_deref(),
            cli_explicit_color,
            intro_label,
            commit_sha,
            // v30 (Bug #1 doc clarification): pass bench_mode so verbose
            // output can disclose the benchmark palette-drift override
            // BEFORE the benchmark report prints `auto_color_drift: false`.
            // Without this, the user sees `auto_drift: true` (config) and
            // later `auto_color_drift: false` (report) and thinks it's a bug.
            bench_mode,
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

    // v25.16: CliExplicit now derives Copy (7 bools, 7 bytes), so reading
    // cli_explicit.fps after the CloudConfig move is a cheap field copy
    // rather than a move. This avoids the E0382 (use of moved value) that
    // broke Windows + MSRV CI builds on commit 5ec253b.
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
        message: args.message.as_deref().map(|m| {
            if m.len() > MESSAGE_MAX_LEN {
                ux::die_input(format!(
                    "error: --message text exceeds {MESSAGE_MAX_LEN} character limit (got {})",
                    m.len()
                ));
            }
            sanitize_message_text(m)
        }),
        message_border: args.message_border,
        target_fps,
        duration: args.duration,
        duration_s,
        bench_frames: args.bench_frames,
        benchmark: args.benchmark,
        bench_duration: resolve_bench_duration_args(&args.bench_duration),
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
        mouse: true, // v17: always-on (--mouse flag deleted)
        charset_preset,
        user_ranges,
        def_ascii,
        auto_color_drift: args.auto_color_drift,
        monolith_density_map,
        config_path_for_watcher: {
            // v25.2 Termux fix: use multi-candidate path resolution so
            // the watcher always watches the file the user is ACTUALLY
            // editing. On Termux, when XDG_CONFIG_HOME is set to
            // $PREFIX/etc, the old `args.config.unwrap_or_else(default_config_file_path)`
            // resolved to a system path the user wasn't editing — causing
            // "live reload doesn't work on Termux" reports. The new
            // resolver picks the first existing candidate from a list
            // that prioritizes $HOME/.config/cosmostrix/config.toml.
            let (resolved, existed) =
                configfile::resolve_watcher_config_path(args.config.as_deref());
            // v25.2: bulletproof diagnostic logging so users can verify
            // the watcher is watching the right file. Uses the same
            // env-gated path as live_config's lr_trace! (zero cost when
            // COSMOSTRIX_LIVE_RELOAD_DEBUG is unset).
            if crate::live_config_trace::live_reload_debug_enabled() {
                crate::live_config_trace::debug_trace(format_args!(
                    "[live-reload-trace] watcher path resolved: {} (existed candidates: {})\n",
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
        ambient_schedule: crate::ambient::collect_ambient_schedule(&configfile::load_config_file(
            args.config.as_deref(),
        )),
    };

    // v25.16: detect fps set by ANY source (CLI --fps OR config.toml fps=).
    // cli_explicit.fps tracks CLI only; args.fps != 60.0 catches config.toml
    // fps set to a non-default value (config_apply.rs applies config -> args).
    // Together they cover: --fps 20, --fps 60 (explicit default), config fps=10.
    // Edge case not covered: config fps=60 (set to default value — a no-op,
    // so not warning is correct).
    let fps_user_set = cli_explicit.fps || args.fps != 60.0;

    if args.bench_all {
        warn_bench_noop_flags(&args, fps_user_set);
        let duration = resolve_bench_duration_args(&args.bench_duration).unwrap_or(2);
        let results = crate::bench_scale::run_scaling_benchmark(&cloud_cfg, duration)?;
        if args.json {
            println!(
                "{}",
                crate::bench_scale::build_scaling_json(&results, &cloud_cfg.scene_name)
            );
        }
        return Ok(());
    }

    if args.benchmark {
        warn_bench_noop_flags(&args, fps_user_set);
        return bench::run_premium_benchmark(&cloud_cfg);
    }

    if let Some(_bench_frames) = args.bench_frames {
        warn_bench_noop_flags(&args, fps_user_set);
        return bench::run_benchmark(&cloud_cfg);
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
        let final_color = interactive::last_color_scheme();
        let final_scene = interactive::last_scene_name();
        let final_charset = interactive::last_charset_preset();
        let final_speed = interactive::last_speed();
        let final_density = interactive::last_density();
        let startup_color = format!("{:?}", color_scheme);
        let startup_scene = args
            .scene
            .as_deref()
            .unwrap_or(crate::scene::DEFAULT_SCENE)
            .to_string();
        let startup_speed = cloud_cfg.speed;
        let startup_density = cloud_cfg.density;

        // v25.13 (bug #15): the bug #14 post-exit verbose rejection summary
        // was removed. Config validation errors during live reload now cause
        // IMMEDIATE exit (see event_loop.rs Err handler). The error is printed
        // by the LIVE_RELOAD_EXIT_CODE path below — after terminal restoration,
        // never during rain. No session-log drain needed since we exit on the
        // first error rather than accumulating.

        let changed = final_color != startup_color
            || final_scene != startup_scene
            || final_charset != startup_charset
            || (final_speed - startup_speed).abs() >= 0.01
            || (final_density - startup_density).abs() >= 0.01;

        if changed {
            let ts = crate::output::now_hhmm();
            let purple = crate::output::brand_open();
            let reset = crate::output::reset();
            eprintln!("{purple}[verbose]{reset} {ts} final runtime state");
            if final_color != startup_color {
                eprintln!(
                    "{purple}[verbose]{reset} {ts}   color_scheme:  {} (was {})",
                    final_color, startup_color
                );
            }
            if final_scene != startup_scene {
                eprintln!(
                    "{purple}[verbose]{reset} {ts}   scene:         {} (was {})",
                    final_scene, startup_scene
                );
            }
            if final_charset != startup_charset {
                eprintln!(
                    "{purple}[verbose]{reset} {ts}   charset:       {} (was {})",
                    final_charset, startup_charset
                );
            }
            if (final_speed - startup_speed).abs() >= 0.01 {
                eprintln!(
                    "{purple}[verbose]{reset} {ts}   speed:         {:.1} (was {:.1})",
                    final_speed, startup_speed
                );
            }
            if (final_density - startup_density).abs() >= 0.01 {
                eprintln!(
                    "{purple}[verbose]{reset} {ts}   density:       {:.2} (was {:.2})",
                    final_density, startup_density
                );
            }
        }
    }

    // Live-reload fatal exit. v25.13 (bug #15): this path now fires for BOTH
    // watcher-thread panics AND config validation errors during live reload.
    // The previous v25.6 design kept rain running on the last valid config
    // when the user introduced a typo mid-edit — but that caused the watcher
    // thread's stderr writes to leak into the alternate-screen buffer,
    // polluting the rain matrix with "weird text". Now: any validation error
    // (malformed line, unknown key, OOR value) sets LIVE_RELOAD_EXIT_CODE=2
    // in the render thread's Err handler, breaks the rain loop, and the
    // error is printed HERE — after Terminal::drop restored the terminal
    // from alternate-screen mode. Printing during the rain loop would be
    // swallowed by the alternate screen or pollute the render.
    if live_config::LIVE_RELOAD_EXIT_CODE.load(std::sync::atomic::Ordering::Acquire) != 0 {
        if let Ok(guard) = live_config::LIVE_RELOAD_ERROR.lock() {
            if let Some(ref msg) = *guard {
                // Route through the centralized output helpers so the
                // error color matches every other error path in the CLI
                // (truecolor red on modern terminals, graceful fallback
                // to 256/16-color on older ones, plain text when piped).
                eprintln!(
                    "{} [live-reload] ERROR: {}{}",
                    crate::output::error_bold_open(),
                    msg,
                    crate::output::reset()
                );
                eprintln!(
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

    result
}

/// Resolve bench duration from --bench-duration (now accepts compound format).
/// Returns None if not specified (benchmark uses default 5s).
///
/// NOTE: only --bench-duration is consulted here. The hidden --duration flag
/// is interactive-mode only (sets event_loop auto-exit deadline) and has no
/// effect in --benchmark / --bench-frames / --bench-all mode.
fn resolve_bench_duration_args(input: &Option<String>) -> Option<u64> {
    input
        .as_ref()
        .map(|s| crate::ux::or_exit(crate::cli_parse::parse_duration("--bench-duration", s)))
}

/// Collect warnings about CLI flags that are misleading or have NO effect
/// in benchmark mode. Pure function — the call site prints them.
///
/// Audit findings (commit 5301572 + a34fcdb follow-up):
///   - `--fps`: in benchmark mode it sets the simulation rate (virtual time
///     delta fed to `cloud.rain_at`), NOT a render cap. `avg_fps` is
///     unconstrained — the bench loop spins full tilt with zero sleeps.
///     User reports showed the absence of this warning caused real
///     confusion: `cosmostrix --benchmark --fps 60` silently ran the same
///     as without `--fps 60`. Now warned whenever `--fps` is explicit
///     (detected via `cli_explicit.fps`) OR set in config.toml to a
///     non-default value (detected via `args.fps != 60.0`). The config
///     path catches `fps = 10` in `~/.config/cosmostrix/config.toml`.
///   - `--duration` (hidden): interactive auto-exit only; bench uses --bench-duration
///   - `--screensaver`: interactive input handler only; bench has no input loop
///   - `--intro`: interactive intro animation; bench never plays it
///   - `--perf-stats` (hidden): interactive summary only; bench emits BenchReportData
///
/// Dispatch precedence (main.rs): `--bench-all > --benchmark > --bench-frames`.
/// The warn matrix below mirrors that precedence so the user always sees which
/// flag actually took effect. The `--bench-duration` warning only fires when
/// `--bench-frames` is the *winning* dispatch (i.e. neither --bench-all nor
/// --benchmark is set); otherwise --bench-duration IS consumed by --bench-all
/// (per-run duration in the scaling sweep) or by --benchmark (override of the
/// 5s default), so warning about it would be wrong.
fn collect_bench_noop_warnings(args: &Args, fps_user_set: bool) -> Vec<&'static str> {
    let mut warns: Vec<&'static str> = Vec::new();
    // Phase D Task C fix: warn about silent-ignore combinations. Previously
    // these 4 cases silently dropped a flag with no warning, causing user
    // confusion ("I set --bench-frames but the bench ran for 5s, not N frames").
    if args.bench_all && args.benchmark {
        warns.push("--benchmark ignored (--bench-all takes precedence)");
    }
    if args.bench_all && args.bench_frames.is_some() {
        warns.push("--bench-frames ignored (--bench-all takes precedence)");
    }
    if args.benchmark && args.bench_frames.is_some() {
        warns.push("--bench-frames ignored (--benchmark takes precedence)");
    }
    // Only warn about --bench-duration being ignored when --bench-frames is the
    // winning dispatch (no --bench-all, no --benchmark). If --bench-all is set,
    // --bench-duration IS used as the per-run duration in the scaling sweep.
    if args.bench_frames.is_some()
        && args.bench_duration.is_some()
        && !args.benchmark
        && !args.bench_all
    {
        warns.push("--bench-duration ignored (--bench-frames is frame-count-based)");
    }
    if fps_user_set {
        warns.push(
            "--fps (in benchmark mode sets simulation rate only — does NOT cap \
             render throughput; avg_fps is unconstrained; check config.toml \
             [fps] if you did not pass --fps on the CLI)",
        );
    }
    if args.duration.is_some() {
        warns.push("--duration (interactive auto-exit only; use --bench-duration)");
    }
    if args.screensaver {
        warns.push("--screensaver (interactive input handler; bench has no input loop)");
    }
    if args.intro.is_some() {
        warns.push("--intro (interactive intro animation; bench never plays it)");
    }
    if args.perf_stats {
        warns.push("--perf-stats (interactive summary; bench emits its own report)");
    }
    warns
}

/// Warn the user about CLI flags that are misleading or have NO effect in
/// benchmark mode. See `collect_bench_noop_warnings` for the audit details.
fn warn_bench_noop_flags(args: &Args, fps_user_set: bool) {
    let warns = collect_bench_noop_warnings(args, fps_user_set);
    if warns.is_empty() {
        return;
    }
    eprintln!(
        "[warn] the following flags have no effect (or a different effect than the name \
         implies) in benchmark mode:"
    );
    for w in &warns {
        eprintln!("       {w}");
    }
}

fn canonicalize_runtime_args(args: &mut Args) {
    if let Some(canonical) = theme::canonical_name_for_input(&args.color) {
        args.color = canonical.to_string();
    }
}

/// v25.11 (bug #11): sanitize `--message` text to cell-safe characters.
///
/// The message box layout in `cloud::reset_message` assumes 1 char = 1
/// terminal cell (it uses `Vec<char>::len()` for content width). Wide
/// chars (CJK fullwidth like 世界, emoji) take 2 cells visually,
/// zero-width chars (combining marks, ZWJ) take 0 cells, and control
/// chars corrupt terminal state. When any of these appear in the message,
/// the box's column math desyncs from the terminal's actual cursor
/// position — each subsequent char overwrites the right half of the
/// previous wide char, and the rain to the right of the box glitches
/// (cells appear to "shift right" then normalize after a few seconds
/// when the periodic full-redraw kicks in).
///
/// Cosmic Dragon principle: stripping wide chars is a PERMANENT design
/// choice — not a temporary limitation. Cosmostrix will never support
/// emoji or full-width CJK glyphs; its soul is single-cell diff-based
/// rendering. The filter mirrors `charset_custom`'s wide-char rejection:
/// wide and zero-width chars are replaced with `?` (so the user sees
/// that a char was dropped, rather than silently losing text). Control
/// chars (except `\n` which is needed for multi-line messages) are
/// stripped entirely. ASCII printable chars (0x20-0x7E) and \n pass
/// through unchanged.
fn sanitize_message_text(input: &str) -> String {
    use unicode_width::UnicodeWidthChar;
    let mut out = String::with_capacity(input.len());
    let mut skipped_wide = 0u32;
    let mut skipped_ctrl = 0u32;
    for ch in input.chars() {
        if ch == '\n' {
            out.push(ch);
            continue;
        }
        // Reject C0/C1 control chars (except \n handled above).
        if ch.is_control() {
            skipped_ctrl += 1;
            continue;
        }
        match ch.width() {
            Some(1) => out.push(ch),
            Some(0) | Some(2) => {
                // Zero-width (combining marks, ZWJ) or wide (CJK, emoji) —
                // both break the 1-char-1-cell invariant. Cosmic Dragon
                // principle: these are PERMANENTLY rejected, never supported.
                // Replace with `?` so the user sees that a char was dropped.
                skipped_wide += 1;
                out.push('?');
            }
            // Some chars return None (e.g., unassigned) — skip entirely.
            None => {
                skipped_ctrl += 1;
            }
            // Chars with width >= 3 are extremely rare (some terminal
            // implementations reserve them for special glyphs). Treat
            // them as wide — replace with '?' to preserve alignment.
            // Same Cosmic Dragon principle: never render multi-cell chars.
            _ => {
                skipped_wide += 1;
                out.push('?');
            }
        }
    }
    if skipped_wide > 0 || skipped_ctrl > 0 {
        eprintln!(
            "[cosmostrix] warning: --message contained {} wide/zero-width char(s) (replaced with '?') and {} control char(s) (removed). Wide chars (CJK, emoji) break cell alignment — see Bug #11.",
            skipped_wide, skipped_ctrl
        );
    }
    out
}

/// Install the global panic hook (v16: Windows silent-exit fix; v25:
/// terminal-close double-panic guard).
///
/// The alt screen captures stdout AND stderr. Old hook printed to stderr
/// without restoring terminal first, so panic message was trapped in alt
/// screen and discarded on LeaveAlternateScreen — "silent exit". Fix:
/// restore terminal BEFORE printing, set global flag so Terminal::drop
/// skips cleanup (prevents rain data leaking to main screen).
///
/// v25 (terminal-close coredump fix): the previous hook used `eprintln!`
/// to print the panic message. When the terminal is closed (SIGHUP /
/// PTY destroyed), stderr becomes a broken pipe. `eprintln!` calls
/// `stderr().write_fmt(...)` which panics on write failure (Rust std
/// intentionally panics to surface I/O errors). A panic *inside* the
/// panic hook is treated as a double-panic by the Rust runtime, which
/// calls `abort()` → systemd-coredump fires.
///
/// This is the root cause of the journal entry:
///   `Process N (cosmostrix) of user 1000 dumped core.`
///   Stack trace: pthread_kill → raise → abort → cosmostrix internal.
///
/// Fix: use `write_fmt` directly with the error explicitly discarded
/// (`let _ = ...`). This makes the hook bulletproof — it cannot panic,
/// so any panic in worker threads (notify watcher, polling heartbeat,
/// crossterm event read) is cleanly caught by `catch_unwind` instead
/// of escalating to abort.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        use std::io::Write;
        crate::terminal::TERMINAL_RESTORED_BY_PANIC
            .store(true, std::sync::atomic::Ordering::Release);
        crate::terminal::restore_terminal_best_effort();
        // SAFETY: write_fmt returns Err if stderr is broken (terminal
        // closed). We discard the error — never panic from the panic
        // hook, or Rust will abort (double-panic → coredump).
        let _ = std::io::stderr().write_fmt(format_args!("{info}\n"));
        let _ = std::io::stderr().flush();
    }));
}

#[cfg(test)]
mod color_detection_tests;

#[cfg(test)]
mod main_tests {
    use super::sanitize_message_text;

    /// v25.11 (bug #11): ASCII-only messages pass through unchanged.
    #[test]
    fn sanitize_preserves_ascii_message() {
        let input = "Hello World! 0123 #hash $var";
        assert_eq!(sanitize_message_text(input), input);
    }

    /// v25.11 (bug #11): newlines are preserved (needed for multi-line `-m`).
    #[test]
    fn sanitize_preserves_newlines() {
        let input = "Line1\nLine2\nLine3";
        assert_eq!(sanitize_message_text(input), input);
    }

    /// v25.11 (bug #11): wide CJK chars replaced with '?'.
    /// Without this, "世界" (2 chars, 4 cells) breaks the 1-char-1-cell
    /// invariant in the message box layout, causing rain to the right
    /// of the box to glitch.
    #[test]
    fn sanitize_replaces_wide_cjk_chars() {
        let result = sanitize_message_text("Hello 世界");
        assert_eq!(result, "Hello ??");
    }

    /// v25.11 (bug #11): emoji replaced with '?'.
    #[test]
    fn sanitize_replaces_emoji() {
        let result = sanitize_message_text("Galaxy 🌌 emoji");
        assert_eq!(result, "Galaxy ? emoji");
    }

    /// v25.11 (bug #11): control chars (except \n) stripped.
    #[test]
    fn sanitize_strips_control_chars() {
        let result = sanitize_message_text("Tab\there\x07bell");
        assert_eq!(result, "Tabherebell");
    }

    /// v25.11 (bug #11): mixed content — ASCII passes, wide/control filtered.
    #[test]
    fn sanitize_handles_mixed_content() {
        let result = sanitize_message_text("Hello 世界 🌌 αβγ #hash $var");
        // "Hello " (6) + "??" (世界) + " " + "?" (🌌) + " " + "αβγ" (3) + " #hash $var"
        assert_eq!(result, "Hello ?? ? αβγ #hash $var");
    }

    /// v25.11 (bug #11): empty message stays empty.
    #[test]
    fn sanitize_handles_empty_message() {
        assert_eq!(sanitize_message_text(""), "");
    }
}
