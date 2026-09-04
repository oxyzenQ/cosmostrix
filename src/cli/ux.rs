// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Central CLI user-experience contract (owner mandate 2026-09-04).
//!
//! THE single source of truth for every user-facing CLI error, tip,
//! usage line, and help footer in cosmostrix. All fatal CLI paths
//! render through this module so the output shape is identical no
//! matter which layer rejected the input:
//!
//! 1. clap parse errors (unknown argument, missing value, invalid
//!    value) flow through [`exit_clap_error`];
//! 2. pre-clap argv errors (removed-flag migration hints, the -mfs
//!    typo guard) flow through [`die_input_with_usage`];
//! 3. post-parse value validation errors flow through [`die_input`];
//! 4. config-file / runtime failures flow through [`die_config`].
//!
//! Canonical shapes (the whole point of this module — before it,
//! identical error kinds rendered with different shapes, duplicate
//! tip lines, and a misleading suggestion-narrowed usage line):
//!
//! ```text
//! error: a value is required for '--glitch-ms <GLITCH_MS>' but none was supplied
//!
//! Usage: cosmostrix [OPTIONS]
//!
//! For more information, try '--help'.
//! ```
//!
//! Layers 1-2 append the real usage line ("Usage: cosmostrix
//! [OPTIONS]" — never clap's suggestion-narrowed "Usage: cosmostrix
//! --testconf", which reads as if the suggested flag were required)
//! plus the help footer. Layers 3-4 append the footer only: their
//! messages already carry the valid range or migration guidance, so a
//! usage line would add noise, not information.
//!
//! Exit-code contract: every fatal CLI error exits 2 (clap's usage
//! code; also the pre-existing ux convention). Config errors share
//! the code 2 deliberately — see `die_config` below.
//!
//! Rendering: layered on `crate::output` (branded color primitives +
//! line-aware labeled blocks). Suggestion/tip lines render white per
//! the S-master-HUNT-5 owner color contract; clap-side tips render
//! white via the `valids` style in `cli::clap_styles()`.

use clap::builder::Command;
use clap::error::{ContextKind, ContextValue};

use crate::output::eprintln_safe;

// ── Canonical suffixes ─────────────────────────────────────────────────────

/// Help footer matching clap's canonical wording, appended to every
/// fatal CLI error so all layers end identically. The usage line is
/// built from `Command::render_usage()` for clap-rendered errors and
/// hardcoded in `print_usage_line` for ux-rendered errors — both
/// verified against each other by the tests below.
const HELP_FOOTER: &str = "For more information, try '--help'.";

// ── Fatal exit helpers (moved from output/ux.rs) ───────────────────────────
//
// Re-exported at the crate root (`crate::ux`) by main.rs so all
// pre-existing call sites resolve unchanged.

/// Print `msg` to stderr in branded red and exit 2 (invalid input / usage error).
#[cold]
pub(crate) fn die_input(msg: impl AsRef<str>) -> ! {
    print_branded_error(msg.as_ref());
    eprintln_safe!("\n{HELP_FOOTER}");
    std::process::exit(2);
}

/// Print `msg`, the real usage line, and the help footer; exit 2.
///
/// For pre-clap structural errors: unknown flags typed by the user
/// before clap ever parses (removed-flag migration hints, the -mfs
/// typo guard). Mirrors the clap UnknownArgument shape.
#[cold]
pub(crate) fn die_input_with_usage(msg: impl AsRef<str>) -> ! {
    print_branded_error(msg.as_ref());
    print_usage_line();
    eprintln_safe!("\n{HELP_FOOTER}");
    std::process::exit(2);
}

/// Print `msg` to stderr in red and exit 2 (config / runtime failure).
///
/// Config failures exit 2 (not 1): the historical contract matched
/// --testconf's invalid-config exit code, and the doc header of the
/// old output/ux.rs claiming exit 1 was stale — code 2 is the shipped
/// behavior every script and test relies on. No usage line: a config
/// problem is not an invocation problem.
#[cold]
pub(crate) fn die_config(msg: impl AsRef<str>) -> ! {
    print_branded_error(msg.as_ref());
    std::process::exit(2);
}

/// Print an error message in branded red. Strips a leading "error: " prefix
/// if present (since `eprintln_error_labeled` adds its own "error:" label).
fn print_branded_error(msg: &str) {
    let stripped = msg.strip_prefix("error: ").unwrap_or(msg);
    crate::output::eprintln_error_labeled(stripped);
}

/// Print the real usage line, styled to match clap's error-usage
/// rendering (the "Usage:" label in brand purple bold, which is the
/// same RGB as the clap `usage` style in `clap_styles()`), with a
/// separating blank line before it — clap's canonical error shape.
fn print_usage_line() {
    eprintln_safe!(
        "\n{}Usage:{} cosmostrix [OPTIONS]",
        crate::output::brand_bold_open(),
        crate::output::reset()
    );
}

// ── Combinators ────────────────────────────────────────────────────────────

/// Unwrap a `Result<T, E>` whose `Err` carries a pre-formatted error string.
///
/// On `Err` the message is printed to stderr and the process exits 2.
/// On `Ok` the inner value is returned directly — no `?`, no propagation.
///
/// # Example
///
/// ```ignore
/// let speed = ux::or_exit(validate_speed(args.speed));
/// ```
pub(crate) fn or_exit<T, E: AsRef<str>>(r: Result<T, E>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => die_input(e),
    }
}

// ── Clap error bridge ──────────────────────────────────────────────────────

/// Render a clap parse error in the canonical cosmostrix shape and
/// exit 2. The single exit path for `try_get_matches_from` and
/// `from_arg_matches` failures.
///
/// Fixes three historical defects of the raw `e.exit()` path:
///
/// 1. Duplicate tip lines — the old main.rs interceptor printed
///    clap's error (which already contains the tip) and then appended
///    its own "tip: a similar argument exists" line extracted by
///    string-parsing the rendered error.
/// 2. Misleading usage — clap injects the suggested flag into the
///    usage generation, so `--test` rendered "Usage: cosmostrix
///    --testconf" (reads as if --testconf were required). The usage
///    context is replaced with the real full usage.
/// 3. Shape drift — missing-value errors rendered with no usage line
///    at all while unknown-argument errors carried one. Every error
///    kind now carries the same usage + footer suffix.
///
/// Note: no suggestion extraction is needed anymore. clap's own
/// render already prints the "tip: a similar argument exists" line
/// from the `SuggestedArg` context (styled white via the `valids`
/// entry in `clap_styles()`), so this function never appends a
/// second one.
pub(crate) fn exit_clap_error(mut e: clap::Error, cmd: &mut Command) -> ! {
    // Replace (or add) the usage context with the real full usage.
    // RichFormatter renders the Usage context verbatim; without this,
    // suggestion-carrying errors show the narrowed usage and
    // missing-value errors show none.
    let real_usage = cmd.render_usage();
    e.insert(ContextKind::Usage, ContextValue::StyledStr(real_usage));

    // Re-render with the command's styles applied, then print to the
    // error's own stream (stderr for error kinds). Broken pipe is
    // swallowed, matching clap's own `exit()` behavior.
    let e = e.format(cmd);
    let _ = e.print();

    // clap cannot render the "For more information" footer here: the
    // footer key comes from an ArgAction::Help argument, and cosmostrix
    // intercepts --help manually to print its curated reference manual
    // (see early_returns). Append the footer ourselves so clap-side
    // errors end exactly like ux-side errors.
    eprintln_safe!("\n{HELP_FOOTER}");
    std::process::exit(2);
}

// ── Value suggestion formatting (moved from cli/suggestion.rs) ─────────────

/// Format a VALUE suggestion as a consistent "tip:" line.
///
/// Returns `\n  tip: a similar value exists: '<value>'`. This is the
/// canonical format for all enum/value typo suggestions (colors,
/// scenes, charsets, glitch-level, msg-fill-style, etc.) — the same
/// wording clap uses for its built-in ValueEnum suggestions, so the
/// custom engine and clap's engine render identically.
///
/// Presentation lives here (the UX contract module); the suggestion
/// ENGINE (edit distance, closest-match) stays in `cli/suggestion.rs`.
pub(crate) fn format_value_suggestion(suggestion: &str) -> String {
    format!("\n  tip: a similar value exists: '{suggestion}'")
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ContextKind;

    #[test]
    fn help_footer_matches_clap_wording() {
        assert_eq!(HELP_FOOTER, "For more information, try '--help'.");
    }

    #[test]
    fn value_suggestion_shape_is_two_space_indented_tip() {
        assert_eq!(
            format_value_suggestion("subtle"),
            "\n  tip: a similar value exists: 'subtle'"
        );
    }

    /// The real-usage contract: `Command::render_usage()` (what
    /// `exit_clap_error` injects into every clap error) must be the
    /// plain full-usage form, never a suggestion-narrowed form.
    #[test]
    fn render_usage_is_full_usage_not_narrowed() {
        use clap::CommandFactory;
        let mut cmd = crate::config::Args::command();
        let usage = cmd.render_usage().to_string();
        assert!(
            usage.contains("Usage: cosmostrix [OPTIONS]"),
            "render_usage must produce the full usage line, got: {usage:?}"
        );
    }

    /// Regression (owner report 2026-09-04, the `--test` case):
    /// a suggestion-carrying clap error must be re-writable with the
    /// real usage via the Usage context — the mechanism
    /// `exit_clap_error` relies on. This locks the clap API contract
    /// (message is context-driven, so `insert` steers the render).
    #[test]
    fn usage_context_insert_steers_clap_render() {
        use clap::CommandFactory;
        use clap::Parser;
        let err = crate::config::Args::try_parse_from(["cosmostrix", "--test"])
            .expect_err("--test must be unknown");
        // The parser stored the suggestion-narrowed usage.
        let narrowed = match err.get(ContextKind::Usage) {
            Some(ContextValue::StyledStr(s)) => s.to_string(),
            other => panic!("expected a Usage context, got {other:?}"),
        };
        assert!(
            narrowed.contains("--testconf"),
            "precondition: parser usage is narrowed to the suggestion, got {narrowed:?}"
        );
        // Override and re-render: the rendered output must now carry
        // the real usage instead.
        let mut err = err;
        let mut cmd = crate::config::Args::command();
        let real_usage = cmd.render_usage();
        err.insert(ContextKind::Usage, ContextValue::StyledStr(real_usage));
        let err = err.format(&mut cmd);
        let rendered = err.render().to_string();
        assert!(
            rendered.contains("Usage: cosmostrix [OPTIONS]"),
            "rendered error must show the real usage, got: {rendered}"
        );
        assert!(
            !rendered.contains("Usage: cosmostrix --testconf"),
            "rendered error must not show the narrowed usage, got: {rendered}"
        );
        // The tip line renders exactly once (clap's own; the old
        // interceptor's duplicate append is gone).
        assert_eq!(
            rendered.matches("tip: a similar argument exists").count(),
            1,
            "exactly one tip line expected, got: {rendered}"
        );
    }
}
