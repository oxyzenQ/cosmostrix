// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Canonical early-return command ladder (NIGHT-lts-6, 2026-09-10).
//!
//! cosmostrix accepts several "print something and exit" commands. When
//! users combine them (`cosmostrix -v -s --dump-config <path> --version
//! --doctor`) exactly ONE must fire, and the winner must NOT depend on
//! argv order. This module is the single source of truth for that
//! precedence.
//!
//! # The ladder
//!
//! ```text
//! Boundary 1  clap parse errors (unknown flag, invalid value) ALWAYS win
//!             — parsing must complete before any command flag is read.
//! ─────────────────────────────────────────────────────────────────────
//! PRE-CONFIG  (run before config-apply; work even with a broken or
//!             missing config file):
//!   1. --help            curated reference manual
//!   2. --reset-terminal  emergency 5-layer terminal reset
//!   3. --dump-config     print or write the example config
//!   4. --config-path     print the resolved config path
//!   5. --testconf        validate the config file
//!   6. --list-scenes     list scene names
//!   7. --list-charsets   list charset names
//!   8. --list-colors     list color theme names
//!   9. --show-scene      show one scene's definition
//! ─────────────────────────────────────────────────────────────────────
//! Boundary 3  config_apply::apply_config_and_runtime_defaults
//! ─────────────────────────────────────────────────────────────────────
//! POST-CONFIG (run after config-apply; report the merged view):
//!  10. --doctor          diagnostics report
//!  11. --version / -V    version + build info
//!  12. --docs            full engine documentation
//!  13. --check-update    upstream release check
//! ─────────────────────────────────────────────────────────────────────
//! Boundary 5  benchmark modes, then the interactive rain loop
//! ```
//!
//! # Owner contract (NIGHT-lts-6)
//!
//! - Deterministic: the same set of flags always selects the same
//!   winner regardless of the order the user typed them (verified by
//!   `test/cli/early_return_precedence_tests.rs`).
//! - Single-owner: `--doctor` is handled ONLY by
//!   `handle_post_config_returns` — the duplicate check that lived in
//!   `main.rs` (dead code after the v50 LOC refactor: main.rs fired
//!   first, so the branch here was unreachable) was removed 2026-09-10.
//! - Inert runtime flags: `-v`, `-s`, and other non-command flags
//!   are silently ignored when an early return fires (standard
//!   early-exit semantics — same as `ls --version --all`).
//!
//! The order is encoded once in `classify_pre_config` /
//! `classify_post_config` (pure functions, no side effects) and the
//! dispatchers below consume them, so the ladder can never silently
//! drift between the two phases.

use crate::config::Args;
use crate::config::{print_list_charsets, print_list_colors, print_list_scenes, print_show_scene};
use crate::configfile;
use crate::doctor;
use crate::help_detail;
use crate::info;
use crate::output::println_safe;
use crate::platform::update;
use crate::safepath::validate_config_path;
use crate::terminal::reset_terminal_emergency;
use crate::testconf;
use crate::ux;

/// Pre-config-apply early-return command kind.
///
/// Variants are listed in canonical ladder order (see module docs).
/// Classification is pure: it performs no I/O, no validation, and no
/// process exits — all of that lives in the dispatcher's match arms.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PreConfigCmd {
    Help,
    ResetTerminal,
    DumpConfig,
    ConfigPath,
    Testconf,
    ListScenes,
    ListCharsets,
    ListColors,
    ShowScene,
}

/// Post-config-apply early-return command kind.
///
/// Variants are listed in canonical ladder order (see module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PostConfigCmd {
    Doctor,
    Version,
    Docs,
    CheckUpdate,
}

/// Pure classification: which pre-config early-return command wins?
///
/// Returns `None` when no pre-config command was passed and the caller
/// should continue to config-apply + interactive mode. The if-chain
/// order IS the documented ladder order — do not reorder without
/// updating the module docs and the precedence tests.
pub(crate) fn classify_pre_config(args: &Args) -> Option<PreConfigCmd> {
    if args.help {
        return Some(PreConfigCmd::Help);
    }
    if args.reset_terminal {
        return Some(PreConfigCmd::ResetTerminal);
    }
    if args.dump_config.is_some() {
        return Some(PreConfigCmd::DumpConfig);
    }
    if args.config_path {
        return Some(PreConfigCmd::ConfigPath);
    }
    if args.testconf {
        return Some(PreConfigCmd::Testconf);
    }
    if args.list_scenes {
        return Some(PreConfigCmd::ListScenes);
    }
    if args.list_charsets {
        return Some(PreConfigCmd::ListCharsets);
    }
    if args.list_colors {
        return Some(PreConfigCmd::ListColors);
    }
    if args.show_scene.is_some() {
        return Some(PreConfigCmd::ShowScene);
    }
    None
}

/// Pure classification: which post-config early-return command wins?
///
/// Returns `None` when no post-config command was passed and the caller
/// should continue to argument validation.
pub(crate) fn classify_post_config(args: &Args) -> Option<PostConfigCmd> {
    if args.doctor {
        return Some(PostConfigCmd::Doctor);
    }
    if args.version {
        return Some(PostConfigCmd::Version);
    }
    if args.docs {
        return Some(PostConfigCmd::Docs);
    }
    if args.check_update {
        return Some(PostConfigCmd::CheckUpdate);
    }
    None
}

/// Dispatch pre-config-apply early-return commands.
///
/// Returns `Some(Ok(()))` when an early return fires (caller should return
/// the result immediately). Returns `None` when no early-return command
/// matched and the caller should continue to config-apply + interactive.
pub(crate) fn handle_pre_config_returns(args: &mut Args) -> Option<std::io::Result<()>> {
    match classify_pre_config(args) {
        Some(PreConfigCmd::Help) => {
            help_detail::print_help();
            Some(Ok(()))
        }

        Some(PreConfigCmd::ResetTerminal) => {
            reset_terminal_emergency();
            Some(Ok(()))
        }

        Some(PreConfigCmd::DumpConfig) => {
            // --dump-config: print example config to stdout (TTY only), OR
            // write to a file if a path argument was given.
            //
            // Security (v15 strict policy):
            //   1. Path must be inside the strict whitelist
            //      (~/.config/cosmostrix/ or /etc/cosmostrix/) — same as
            //      --config.
            //   2. Path must have a .toml extension — same as --config.
            //   3. Shell redirection (>, >|) is BLOCKED: if --dump-config
            //      is used without a path argument AND stdout is
            //      redirected to a regular file, cosmostrix refuses to
            //      write. This prevents bypassing the whitelist via
            //      `cosmostrix --dump-config > /tmp/a.txt`.
            //      The user MUST use the explicit path form:
            //        cosmostrix --dump-config ~/.config/cosmostrix/config.toml
            //      Piping to another command (cosmostrix --dump-config |
            //      less) is still allowed — only file redirection is
            //      blocked.
            //
            // The flag uses clap's num_args=0..=1 pattern:
            //   --dump-config            → Some("") → print to stdout (TTY
            //                              or pipe only)
            //   --dump-config <path>     → Some("<path>") → write to file
            //                              (validated)
            //   (not passed)             → None → skip
            let dump_path = args.dump_config.as_ref()?;
            if dump_path.is_empty() {
                // No path argument: print to stdout. But BLOCK if stdout
                // is redirected to a file (shell > or >| operator). This
                // forces the user to use --dump-config <path> for file
                // output, which enforces the whitelist.
                #[cfg(unix)]
                {
                    if crate::config_io::stdout_is_redirected_to_file() {
                        // Route through ux::die_input so the exit code (2)
                        // and error formatting match every other CLI input
                        // error. Previously this used process::exit(2)
                        // directly, bypassing the ux module's centralized
                        // error handling.
                        ux::die_input(
                            "refusing to write --dump-config to a redirected file\n  \
                             Shell redirection (>, >|) bypasses the strict whitelist.\n  \
                             Use the explicit path form instead:\n    \
                             cosmostrix --dump-config ~/.config/cosmostrix/config.toml\n  \
                             The path must be inside ~/.config/cosmostrix/ or \
                             /etc/cosmostrix/ and have a .toml extension.\n  \
                             Piping to another command (cosmostrix --dump-config | less) \
                             is allowed.",
                        );
                    }
                }
                print!("{}", configfile::dump_config_with_header());
                return Some(Ok(()));
            }
            // Path argument given: validate whitelist + .toml extension.
            // Reuse validate_config_path() so --dump-config and --config
            // stay perfectly in sync. Map the --config label to
            // --dump-config in error messages. Use the RESOLVED path for
            // all I/O (expands %APPDATA% on Windows — the raw path would
            // create a literal %APPDATA% directory instead of resolving
            // it).
            let path_str = dump_path;
            let resolved_path = match validate_config_path(path_str, args.verbose) {
                Ok(r) => r,
                Err(e) => ux::die_input(e.replace("--config", "--dump-config")),
            };
            // Write the example config to the validated path.
            // Phase 5 (P3-7): refuse to overwrite an existing file.
            // Previously --dump-config silently overwrote any existing
            // config at the path, causing data loss if the user pointed
            // it at their carefully-tuned ~/.config/cosmostrix/config.toml.
            // Now: if the file exists, exit with a clear error + suggest
            // a sibling path that passes every validation rule (see
            // `dump_config_overwrite_refusal`).
            //
            // v30 (2026-08-05): --force flag bypasses this guard. Use
            // case: a user who has read the existing config, decided they
            // want to start fresh, and explicitly opts in to overwrite.
            // Still scoped to --dump-config only (does not affect
            // --save-baseline or other write paths). The error message
            // tells the user about --force so they don't have to read the
            // docs to discover it.
            if std::path::Path::new(&resolved_path).exists() && !args.force {
                ux::die_input(dump_config_overwrite_refusal(path_str));
            }
            // v30: atomic write via temp-file + fsync + rename.
            // Previously a direct `std::fs::write` — if the process was
            // killed mid-write (Ctrl-C, OOM, power loss), the target file
            // could be left as a zero-byte or truncated stub. With
            // `--force` overwriting an existing config, that meant
            // destroying the user's previous config AND leaving an
            // incomplete one — the worst data-loss scenario the guard was
            // supposed to make explicit. Atomic rename guarantees readers
            // see either the old file or the complete new file, never a
            // half-written one.
            let text = configfile::dump_config_with_header();
            match crate::config_io::write_config_atomic(&resolved_path, &text) {
                Ok(()) => {
                    if args.verbose {
                        crate::output::eprintln_verbose_raw(&format!(
                            "dump-config: wrote example config to {resolved_path}"
                        ));
                    }
                    Some(Ok(()))
                }
                Err(e) => {
                    // Family routing: a filesystem rejection of a
                    // CLI-supplied path is an invocation-adjacent failure,
                    // same family as the overwrite guard above (die_input
                    // + help footer). The old die_config route rendered it
                    // footer-less and tip-less — the same flag failing two
                    // lines earlier rendered a guided 5-line message, so
                    // the bare shape read as a bug.
                    ux::die_input(format!(
                        "error: cannot write --dump-config to '{path_str}': {e}\n  \
                         Verify the directory exists and is writable by this user, then \
                         retry:\n    \
                         cosmostrix --dump-config {path_str}\n  \
                         Or print the example config to stdout instead (no file write):\n  \
                         cosmostrix --dump-config"
                    ));
                }
            }
        }

        Some(PreConfigCmd::ConfigPath) => {
            // Show the actually-resolved path (falls back to system
            // config if user config doesn't exist), not just the default
            // user path.
            let default_path = configfile::default_config_file_path();
            if default_path.exists() {
                println_safe!("{}", default_path.display());
            } else {
                let candidates = configfile::config_candidate_paths();
                let resolved = candidates
                    .into_iter()
                    .find(|p| p.exists())
                    .unwrap_or(default_path);
                println_safe!("{}", resolved.display());
            }
            Some(Ok(()))
        }

        Some(PreConfigCmd::Testconf) => Some(testconf::run(args)),

        Some(PreConfigCmd::ListScenes) => {
            // depth-test fix: --list-* and --show-scene bypass strict
            // config validation. Depth-test user with
            // `charset-custom.long2.set` exceeding the 256-char limit
            // could not run `--list-charsets` because the strict
            // validation in apply_config_and_runtime_defaults killed the
            // process before list-commands ran. List/show commands only
            // need to READ the config (non-strict — bad keys are silently
            // dropped by load_config_file), not validate it. They use
            // load_config_file(None) internally so the user-supplied
            // --config path is irrelevant for them. Path-security
            // validation for --show-scene is preserved (its existing
            // inline check).
            print_list_scenes();
            Some(Ok(()))
        }

        Some(PreConfigCmd::ListCharsets) => {
            print_list_charsets();
            Some(Ok(()))
        }

        Some(PreConfigCmd::ListColors) => {
            print_list_colors();
            Some(Ok(()))
        }

        Some(PreConfigCmd::ShowScene) => {
            let name = args.show_scene.as_ref()?;
            // Security (v16 audit): validate --config path BEFORE reading.
            // Previously --show-scene called load_config_file directly
            // without is_safe_path, allowing
            // `cosmostrix --show-scene X --config /etc/passwd` to parse
            // arbitrary files as TOML and leak their content via error
            // messages. Now applies the same check as the main startup
            // path.
            if let Some(ref config_path) = args.config {
                let path_str = config_path.to_string_lossy();
                if let Err(e) = validate_config_path(&path_str, args.verbose) {
                    ux::die_input(e);
                }
                // validate_config_path resolved the path (expands
                // %APPDATA% etc.), but load_config_file takes an
                // Option<&Path> from the original args.config. On Windows,
                // if the user passed %APPDATA%\..., the OS file APIs won't
                // resolve it. Override args.config with the resolved path
                // so load_config_file reads the correct file.
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
                Ok(()) => Some(Ok(())),
                // die_input family: --show-scene <unknown> is a typed CLI
                // value error — it gains the help footer, same shape as
                // --scene <unknown> (owner report 2026-09-04 consistency
                // sweep; previously misrouted through die_config).
                Err(e) => ux::die_input(e),
            }
        }

        None => None,
    }
}

/// Dispatch post-config-apply early-return commands.
///
/// Runs AFTER `config_apply::apply_config_and_runtime_defaults` +
/// `canonicalize_runtime_args`. Handles (in ladder order):
/// - `--doctor` (diagnostics report)
/// - `--version` (version string)
/// - `--docs` (full engine documentation)
/// - `--check-update` (latest upstream release check)
///
/// Returns `Some(Ok(()))` when an early return fires (caller should return
/// the result immediately). Returns `None` when no early-return command
/// matched and the caller should continue to argument validation.
pub(crate) fn handle_post_config_returns(args: &Args) -> Option<std::io::Result<()>> {
    match classify_post_config(args) {
        Some(PostConfigCmd::Doctor) => {
            doctor::print_doctor_report(args);
            Some(Ok(()))
        }

        Some(PostConfigCmd::Version) => {
            println_safe!("{}", info::version_report());
            Some(Ok(()))
        }

        Some(PostConfigCmd::Docs) => {
            // Print the full engine documentation and architecture
            // overview, then exit. Plain text only (no ANSI) so it pipes
            // cleanly into `less`, `grep`, or documentation generators.
            println_safe!("{}", info::docs_report());
            Some(Ok(()))
        }

        Some(PostConfigCmd::CheckUpdate) => {
            if let Err(e) = update::check_update(env!("CARGO_PKG_VERSION")) {
                ux::die_config(format!("error: update check failed: {e}"));
            }
            Some(Ok(()))
        }

        None => None,
    }
}

/// Build the `--dump-config` overwrite-refusal message for an existing
/// config at `path_str`.
///
/// NIGHT-depthtest-2 (owner report 2026-09-11): the message used to
/// suggest `cosmostrix --dump-config <path>.new` — a path the same
/// command then REJECTED with "must have a .toml extension"
/// (`validate_config_path` requires the final extension to be `.toml`,
/// and `config.toml.new` ends in `.new`). The owner hit exactly that
/// loop: follow the suggestion, get a second error. The suggested path
/// must itself satisfy every validation rule the flag enforces, so the
/// suggestion is now `<stem>.new.toml` (the final extension is `.toml`,
/// the name still reads as "the new one next to the old one", and the
/// review-then-rename workflow is unchanged: rename `config.new.toml`
/// over `config.toml` after moving the old file aside).
///
/// Pure function (no I/O, no process exit) so the regression suite can
/// assert the suggestion/validator contract directly: the suggested
/// path ends in `.toml`, differs from the guarded path, and the
/// `--force` escape hatch is still advertised.
pub(crate) fn dump_config_overwrite_refusal(path_str: &str) -> String {
    // path_str passed validate_config_path immediately before this
    // call, so it ends with .toml (case-insensitive). The fallback
    // (append instead of replace) keeps the function total if a
    // future caller loosens that ordering — the suggestion stays
    // valid either way (a .toml-suffixed sibling). strip_suffix runs
    // on the ORIGINAL string: lowering first would corrupt the
    // suggested path when the stem contains uppercase letters.
    let stem = path_str.strip_suffix(".toml").unwrap_or(path_str);
    let suggested = format!("{stem}.new.toml");
    format!(
        "error: --dump-config refuses to overwrite existing file '{path_str}'\n  \
         Move the existing file aside first, or write to a new path:\n    \
         cosmostrix --dump-config {suggested}\n  \
         Then review the new file and rename if appropriate.\n  \
         To overwrite deliberately (destructive), pass --force:\n    \
         cosmostrix --dump-config {path_str} --force"
    )
}

#[cfg(test)]
#[path = "../../test/cli/early_return_precedence_tests.rs"]
mod tests;
