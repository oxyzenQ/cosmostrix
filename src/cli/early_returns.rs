// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Pre-config-apply early-return command dispatchers.
//!
//! Extracted from `main.rs` to keep that file under the 800-LOC cap.
//! Pure code motion — no behavior change.
//!
//! Handles the early-return CLI commands that run BEFORE
//! `config_apply::apply_config_and_runtime_defaults`:
//! - `--help` (curated reference manual)
//! - `--reset-terminal` (5-layer nuclear terminal reset)
//! - `--dump-config` (print or write example config)
//! - `--config-path` (print resolved config path)
//! - `--testconf` (validate config file)
//! - `--list-scenes` / `--list-charsets` / `--list-colors`
//! - `--show-scene <name>`
//!
//! Returns `Some(result)` when an early return fires, `None` when the
//! caller should continue to config-apply + interactive mode.

use crate::config::Args;
use crate::config::{print_list_charsets, print_list_colors, print_list_scenes, print_show_scene};
use crate::configfile;
use crate::doctor;
use crate::help_detail;
use crate::info;
use crate::platform::update;
use crate::safepath::validate_config_path;
use crate::terminal::reset_terminal_emergency;
use crate::testconf;
use crate::ux;

/// Check pre-config-apply early-return commands.
///
/// Returns `Some(Ok(()))` when an early return fires (caller should return
/// the result immediately). Returns `None` when no early-return command
/// matched and the caller should continue to config-apply + interactive.
pub(crate) fn handle_pre_config_returns(args: &mut Args) -> Option<std::io::Result<()>> {
    if args.help {
        help_detail::print_help();
        return Some(Ok(()));
    }

    if args.reset_terminal {
        reset_terminal_emergency();
        return Some(Ok(()));
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
            return Some(Ok(()));
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
                return Some(Ok(()));
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
        return Some(Ok(()));
    }

    if args.testconf {
        return Some(testconf::run(args));
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
        return Some(Ok(()));
    }

    if args.list_charsets {
        print_list_charsets();
        return Some(Ok(()));
    }

    if args.list_colors {
        print_list_colors();
        return Some(Ok(()));
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
            Ok(()) => return Some(Ok(())),
            // die_input family: --show-scene <unknown> is a typed CLI
            // value error — it gains the help footer, same shape as
            // --scene <unknown> (owner report 2026-09-04 consistency
            // sweep; previously misrouted through die_config).
            Err(e) => ux::die_input(e),
        }
    }

    None
}

/// Check post-config-apply early-return commands.
///
/// Runs AFTER `config_apply::apply_config_and_runtime_defaults` +
/// `canonicalize_runtime_args`. Handles:
/// - `--doctor` (diagnostics report)
/// - `--version` (version string)
/// - `--docs` (full engine documentation)
/// - `--check-update` (latest upstream release check)
///
/// Returns `Some(Ok(()))` when an early return fires (caller should return
/// the result immediately). Returns `None` when no early-return command
/// matched and the caller should continue to argument validation.
pub(crate) fn handle_post_config_returns(args: &Args) -> Option<std::io::Result<()>> {
    if args.doctor {
        doctor::print_doctor_report(args);
        return Some(Ok(()));
    }

    if args.version {
        println!("{}", info::version_report());
        return Some(Ok(()));
    }

    if args.docs {
        // Print the full engine documentation and architecture overview,
        // then exit. Plain text only (no ANSI) so it pipes cleanly into
        // `less`, `grep`, or documentation generators.
        println!("{}", info::docs_report());
        return Some(Ok(()));
    }

    if args.check_update {
        if let Err(e) = update::check_update(env!("CARGO_PKG_VERSION")) {
            ux::die_config(format!("error: update check failed: {e}"));
        }
        return Some(Ok(()));
    }

    None
}
