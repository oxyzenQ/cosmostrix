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
// Ambient phase scheduler — config-driven time-of-day scene/parameter
// switching. Replaces the archived `adaptive-custom` subsystem with a
// simpler contract: config-only (no CLI flag), instant switch (no blend
// window), dynamic idle/wake scheduler thread (zero CPU between phase
// boundaries). Moved into crystal_dragon_engine module.
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
// All bench_*.rs submodules now live under src/bench/ and are declared
// inside bench/mod.rs. The re-export below keeps the historical
// `crate::bench_X::Foo` paths working unchanged for all 49 existing
// call sites across main.rs, interactive/, cloud/, and bench/ itself.
pub(crate) use bench::*;
mod bolt;
mod brightness_factors;
mod cell;
mod charset;
mod charset_custom;
// Chroma Dragon coloring engine — Phase 1: relocated palette + catalog here.
// Re-exports below keep the old `crate::palette::…` paths working unchanged.
mod chroma;
pub use chroma::catalog;
pub use chroma::palette;
mod central_control_dragon_power;
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
mod config_io;
mod configfile;
#[cfg(test)]
mod configfile_tests;
mod constants;
mod cosmic_dragon;
mod cpustat;
// Crystal Dragon Engine — ambient intelligence for palette drift.
// Point-based temperature group system (Cold/Medium/Hot) + calc-v1
// probabilistic weighted selection. See src/crystal_dragon_engine/.
mod crystal_dragon_engine;
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
mod live_config_state;
#[cfg(test)]
mod loc_tests;
mod memstat;
mod message;
mod output;
mod panic_hook;
// `palette` now lives at `src/chroma/palette.rs`; re-exported above.
mod platform;
mod posix_time;
mod profile;
#[cfg(test)]
mod property_tests;
mod rain_style;
mod renderer_info;
mod report;
mod runtime;
mod safepath;
mod scene;
mod scene_custom;
mod sgr_format;
mod termdetect;
mod terminal;
#[cfg(test)]
mod terminal_tests;
mod terminal_tty;
mod testconf;
mod theme;
mod tier2;
mod update;
mod usagestat;
mod ux;
mod validation;
mod verbose;
#[cfg(test)]
mod width_guard_tests;

use clap::{CommandFactory, FromArgMatches};

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
use crate::terminal::restore_terminal_best_effort;
use crate::validation::{
    prevalidate_cli_args, suggest_cli_flag, validate_f32_range, validate_f64_range, validate_speed,
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

/// Fork guard: protects the terminal from being left in raw mode when
/// cosmostrix is killed unexpectedly (SIGKILL, segfault, OOM).
///
/// When cosmostrix starts, it switches the terminal to raw mode. Normally
/// `Terminal::drop()` restores the original settings on graceful exit.
/// But SIGKILL bypasses all Rust cleanup — the terminal stays broken:
/// no echo, no line buffering, keys produce garbage. The user must blindly
/// type `reset` or `stty sane` to recover.
///
/// Three strategies by platform:
///
/// - **Linux**: `fork()` + `prctl(PR_SET_PDEATHSIG)`. A child process holds
///   the original termios and waits for SIGTERM (delivered instantly by the
///   kernel when the parent dies). Zero latency, zero CPU overhead. This is
///   the gold standard — `prctl` is Linux-only.
///
/// - **All other Unix** (macOS, FreeBSD, OpenBSD, NetBSD, Android/Termux):
///   A background thread polls `getppid()` every 500ms. When the parent dies,
///   the child is reparented to PID 1 (launchd/init) — ppid becomes 1. The
///   thread detects this and restores the terminal. 500ms worst-case latency
///   (typically ~250ms average), negligible CPU (one syscall per 500ms).
///   This covers macOS (no prctl), BSD (no prctl), and Android (fork may be
///   restricted by seccomp, but threads always work).
///
/// - **Windows**: No-op. ConPTY (Windows Terminal, PowerShell 7+) automatically
///   restores console state when the attached process exits, even on
///   Task Manager kill. Legacy cmd.exe has `SetConsoleMode` but it also
///   reverts on process exit. The panic hook and watchdog still cover the
///   graceful-shutdown path. Set `COSMOSTRIX_NO_FORK_GUARD=1` to skip.
//
// ── Linux: fork + prctl(PR_SET_PDEATHSIG) ─────────────────────────────
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
        // child receive SIGTERM — the parent's Terminal::drop() handles
        // all terminal cleanup. After PR_SET_PDEATHSIG, check ppid:
        // - ppid == 1: parent already dead (SIGKILL or crash) → restore
        // - ppid != 1: parent still alive or exiting normally → do nothing
        if sig == libc::SIGTERM && libc::getppid() == 1 {
            let _ = libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &orig);
            restore_terminal_best_effort();
        }

        libc::_exit(0);
    }
}

// ── All other Unix (macOS, BSD, Android/Termux): getppid polling ───────

/// Unix fallback: background thread polling `getppid()`.
///
/// Used on all Unix platforms except Linux (which has the superior fork+prctl).
/// Covers macOS, FreeBSD, OpenBSD, NetBSD, DragonFly BSD, and Android/Termux.
///
/// When the parent cosmostrix process dies (SIGKILL, crash, OOM), the OS
/// reparents this thread to PID 1. The thread detects ppid==1 and restores
/// the terminal. Worst-case latency: 500ms. CPU overhead: one `getppid()`
/// syscall per 500ms — negligible.
#[cfg(all(unix, not(target_os = "linux")))]
pub fn spawn_kill9_terminal_guard() {
    if env_var_truthy("COSMOSTRIX_NO_FORK_GUARD") {
        return;
    }

    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return;
    }

    // SAFETY: tcgetattr is the standard POSIX call to read terminal
    // attributes. stdin is confirmed to be a TTY above.
    let orig = unsafe {
        let mut termios: std::mem::MaybeUninit<libc::termios> = std::mem::MaybeUninit::uninit();
        if libc::tcgetattr(libc::STDIN_FILENO, termios.as_mut_ptr()) != 0 {
            return;
        }
        termios.assume_init()
    };

    std::thread::Builder::new()
        .name("cx-term-guard".to_string())
        .spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                // SAFETY: getppid() is a simple POSIX call, always safe.
                // On parent death, OS reparents to PID 1 (launchd/init).
                if unsafe { libc::getppid() } == 1 {
                    // Parent died — restore terminal and exit this thread.
                    let _ = unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &orig) };
                    restore_terminal_best_effort();
                    return;
                }
            }
        })
        .expect("failed to spawn terminal guard thread");
}

// ── Windows: no-op (ConPTY auto-restores) ──────────────────────────────

/// Windows: no fork guard needed.
///
/// ConPTY (Windows Terminal, PowerShell 7+, VSCode) automatically restores
/// console mode when the attached process exits — even on Task Manager kill
/// or crash. Legacy cmd.exe with `SetConsoleMode` also reverts on exit.
/// The panic hook and watchdog still cover graceful shutdown.
#[cfg(not(unix))]
pub fn spawn_kill9_terminal_guard() {}

/// Extract the unknown flag name from a clap error message.
///
/// Clap's "unexpected argument" error has the format:
///   `error: unexpected argument '--foo' found`
///
/// This function extracts `foo` (without the `--` prefix) so it can be
/// passed to [`suggest_cli_flag`] for edit-distance matching.
fn extract_unknown_flag(err_str: &str) -> Option<&str> {
    // Look for the pattern: unexpected argument '--FLAG'
    // or: unexpected argument 'FLAG' (short-flag form, less common)
    if !err_str.contains("unexpected argument") {
        return None;
    }
    // Find the single-quoted token after "unexpected argument"
    let marker = "unexpected argument '";
    let start = err_str.find(marker)? + marker.len();
    let rest = &err_str[start..];
    let end = rest.find('\'')?;
    let token = &rest[..end];
    // Strip the leading -- if present (long flag form)
    Some(token.strip_prefix("--").unwrap_or(token))
}

fn main() -> std::io::Result<()> {
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

    let matches = cmd.try_get_matches_from(&argv).unwrap_or_else(|e| {
        // Intercept clap's "unexpected argument" errors and append a
        // "Did you mean --<flag>?" suggestion based on edit distance.
        // This turns a bare `error: unexpected argument '--crystal-dragons'`
        // into a helpful `Did you mean --crystal-dragon?` — matching the
        // same UX already provided for config key typos in config_hints.rs.
        let err_str = e.to_string();
        // Clap's unknown-arg error contains "unexpected argument" and the
        // flag name in quotes. Extract the flag name (without --) so we
        // can compute a suggestion.
        if let Some(flag_name) = extract_unknown_flag(&err_str) {
            if let Some(suggestion) = suggest_cli_flag(flag_name) {
                // Print clap's original error, then our suggestion line.
                // We use e.print() for the original formatted error, then
                // append the hint on a new line to stderr.
                e.print().ok();
                eprintln!("\n  Did you mean --{suggestion}?");
                std::process::exit(2);
            }
        }
        // No suggestion found — fall through to clap's default error display.
        e.exit();
    });
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
                if crate::config_io::stdout_is_redirected_to_file() {
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
        match crate::config_io::write_config_atomic(&resolved_path, &text) {
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
        // Show the actually-resolved path (falls back to system config
        // if user config doesn't exist), not just the default user path.
        let default_path = configfile::default_config_file_path();
        if default_path.exists() {
            println!("{}", default_path.display());
        } else {
            let candidates = configfile::config_candidate_paths();
            let resolved = candidates
                .into_iter()
                .find(|p| p.exists())
                .unwrap_or(default_path);
            println!("{}", resolved.display());
        }
        return Ok(());
    }

    if args.testconf {
        return testconf::run(&args);
    }

    // depth-test fix: --list-* and --show-scene bypass strict config
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

    // v16: Load custom palette if --colors-custom is set, from config.toml's
    // [colors-custom] section. custom_palette_name is stored for live reload.
    // Runs BEFORE verbose print so verbose shows the correct palette name.
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
        let commit_sha = option_env!("COSMOSTRIX_GIT_SHA").unwrap_or("unknown");
        let verbose_ambient_schedule =
            crate::crystal_dragon_engine::ambient::collect_ambient_schedule(
                &configfile::load_config_file(args.config.as_deref()),
            );
        verbose::print_verbose(&verbose::VerboseCtx {
            version: env!("CARGO_PKG_VERSION"),
            scene_name: args.scene.as_deref(),
            rain_style,
            color_scheme,
            color_mode,
            color_tune,
            color_bg: args.color_bg,
            custom_palette_bg: custom_palette.as_ref().and_then(|p| p.bg),
            charset_preset: &charset_preset,
            chars: &chars,
            target_fps,
            fps_precedence,
            speed,
            base_density,
            density_auto,
            monolith_size: args.monolith_size,
            async_mode: effective_async,
            bold_mode,
            shading_mode,
            glitch_enabled: args.glitch_level != crate::config::GlitchLevel::None,
            glitch_pct,
            glitch_low,
            glitch_high,
            glitch_level: &format!("{:?}", args.glitch_level),
            screensaver: args.screensaver,
            crystal_dragon: args.crystal_dragon,
            message: args.message.as_deref(),
            message_border: args.message_border,
            duration: args.duration,
            screen_size,
            custom_palette_name: custom_palette_name.as_deref(),
            scene_arg: &args.scene,
            config_path: args.config.as_deref(),
            cli_explicit_color,
            intro_type_label: intro_label,
            commit_sha,
            bench_mode,
            scene_custom: args.scene_custom.as_deref(),
            ambient_schedule: &verbose_ambient_schedule,
        });
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
        message: args.message.as_deref().map(|m| {
            if m.len() > MESSAGE_MAX_LEN {
                ux::die_input(format!(
                    "error: --message text exceeds {MESSAGE_MAX_LEN} character limit (got {})",
                    m.len()
                ));
            }
            crate::message::sanitize_message_text(m)
        }),
        message_border: args.message_border,
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
        mouse: true, // v17: always-on (--mouse flag deleted)
        charset_preset,
        user_ranges,
        def_ascii,
        crystal_dragon: args.crystal_dragon,
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
    };

    // fps_user_set was computed earlier (before dynamic default) — USER intent.

    if args.bench_all {
        crate::bench_helpers::warn_bench_noop_flags(&args, fps_user_set);
        let duration =
            crate::bench_helpers::resolve_bench_duration_args(&args.bench_duration).unwrap_or(2);
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
        crate::bench_helpers::warn_bench_noop_flags(&args, fps_user_set);
        return bench::run_premium_benchmark(&cloud_cfg);
    }

    if let Some(_bench_frames) = args.bench_frames {
        crate::bench_helpers::warn_bench_noop_flags(&args, fps_user_set);
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
        // print startup ambient info post-exit (event_loop prints
        // are invisible — alternate screen discards stderr on exit).
        if let Some(info) = interactive::startup_ambient_info() {
            crate::output::eprintln_verbose_purple(&info);
        }

        // live-reload errors cause immediate exit (see below).
        let changed = final_color != startup_color
            || final_scene != startup_scene
            || final_charset != startup_charset
            || (final_speed - startup_speed).abs() >= 0.01
            || (final_density - startup_density).abs() >= 0.01;

        if changed {
            let ts = crate::output::now_hhmm();
            let purple = crate::output::brand_open();
            let reset = crate::output::reset();
            crate::output::eprintln_verbose_purple("final runtime state");
            if final_color != startup_color {
                crate::output::eprintln_safe!(
                    "{purple}[verbose]{reset} {ts} {purple}  color_scheme:{reset}  {} (was {})",
                    final_color,
                    startup_color
                );
            }
            if final_scene != startup_scene {
                crate::output::eprintln_safe!(
                    "{purple}[verbose]{reset} {ts} {purple}  scene:{reset}         {} (was {})",
                    final_scene,
                    startup_scene
                );
            }
            if final_charset != startup_charset {
                crate::output::eprintln_safe!(
                    "{purple}[verbose]{reset} {ts} {purple}  charset:{reset}       {} (was {})",
                    final_charset,
                    startup_charset
                );
            }
            if (final_speed - startup_speed).abs() >= 0.01 {
                crate::output::eprintln_safe!(
                    "{purple}[verbose]{reset} {ts} {purple}  speed:{reset}         {:.1} (was {:.1})",
                    final_speed, startup_speed
                );
            }
            if (final_density - startup_density).abs() >= 0.01 {
                crate::output::eprintln_safe!(
                    "{purple}[verbose]{reset} {ts} {purple}  density:{reset}       {:.2} (was {:.2})",
                    final_density, startup_density
                );
            }
            let diag = interactive::ambient_diag_summary();
            crate::output::eprintln_safe!("{purple}[verbose]{reset} {ts} {purple}  {diag}{reset}");
        }
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

fn canonicalize_runtime_args(args: &mut Args) {
    if let Some(canonical) = theme::canonical_name_for_input(&args.color) {
        args.color = canonical.to_string();
    }
}

#[cfg(test)]
mod color_detection_tests;
