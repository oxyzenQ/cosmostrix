// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Argv shorthand expansion — extracted from `main.rs` to keep that
//! file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! clap short flags are single-character, so the multi-character
//! shorthands `-mb` and `-mfs` cannot be registered on the `Args`
//! struct directly. Instead, raw argv is rewritten BEFORE clap parses
//! it:
//!
//! | User token      | Expanded to                        |
//! |------------------|------------------------------------|
//! | `-mb` + value    | `--message-border -m <value>`      |
//! | `-mb=<text>`     | `--message-border -m <text>`       |
//! | `-mfs` + value   | `--msg-fill-style <value>`         |
//! | `-mfs=<style>`   | `--msg-fill-style <style>`         |
//! | `-mfs<style>`    | `--msg-fill-style <style>` (attached form, value must be a valid style) |
//!
//! A `-mfs…` token whose trailing text is NOT a valid style value
//! (e.g. the doubled-s typo `-mfss`) is rejected with a clap-format
//! "tip: a similar argument exists: '--msg-fill-style'" error,
//! matching the long-flag typo UX that clap's `suggestions` feature
//! already provides (e.g. `--msg-fill-styl`).
//! Without this guard clap would silently parse `-mfss` as `-m` with
//! the attached message text "fss" — a silent footgun.
//!
//! Side effect (documented in `--help`): an attached `-m` message that
//! itself starts with "fs" (e.g. `-mfss is my message`) now resolves to
//! the style shorthand. Use the space-separated form `-m "fss …"` for
//! such messages.

use clap::ValueEnum;

use crate::msg_fill_style::MsgFillStyle;

/// Expand the `-mb` / `-mfs` shorthands in the raw argv.
///
/// The returned vector always starts with `argv[0]` (the program name)
/// and preserves the order of all other tokens. Exits with code 2 on a
/// `-mfs…` typo.
pub(crate) fn expand_argv_shorthands(argv: &[std::ffi::OsString]) -> Vec<std::ffi::OsString> {
    let mut expanded: Vec<std::ffi::OsString> = Vec::with_capacity(argv.len() + 1);
    expanded.push(argv[0].clone());
    let mut i = 1;
    while i < argv.len() {
        let arg = &argv[i];

        // v80.0.0-beta.1 msg-fill-style: -mfs shorthand. Checked BEFORE -mb (no
        // prefix collision — "-mb" vs "-mfs" differ at the 3rd char).
        if arg == "-mfs" {
            // Space-separated form: the next token is the style value.
            // When no value follows, the bare flag is emitted and clap
            // reports the missing-value error with the possible-values
            // list.
            expanded.push("--msg-fill-style".into());
            if i + 1 < argv.len() {
                expanded.push(argv[i + 1].clone());
                i += 2;
            } else {
                i += 1;
            }
            continue;
        } else if let Some(s) = arg.to_str() {
            if let Some(rest) = s.strip_prefix("-mfs=") {
                // -mfs=<style> form.
                expanded.push("--msg-fill-style".into());
                expanded.push(rest.into());
                i += 1;
                continue;
            } else if let Some(rest) = s.strip_prefix("-mfs") {
                // Attached form: -mfs<style> (mirrors clap's short-flag
                // attached-value semantics). The trailing text must be a
                // valid style value — anything else is a typo of -mfs.
                if is_valid_style_value(rest) {
                    expanded.push("--msg-fill-style".into());
                    expanded.push(rest.into());
                    i += 1;
                    continue;
                }
                die_mfs_typo(s);
            }
        }

        // -mb shorthand: expand to the hidden --message-border boolean
        // plus -m with the message text.
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
    expanded
}

/// Case-sensitive style-value check matching clap's ValueEnum parsing
/// (the CLI surface is case-sensitive; the config.toml key is not —
/// see `config_apply.rs`).
fn is_valid_style_value(rest: &str) -> bool {
    MsgFillStyle::from_str(rest, false).is_ok()
}

/// Reject a `-mfs…` typo with a clap-format error so the UX matches the
/// long-flag typo path (`--msg-fill-styl` → clap's own "tip:" +
/// main.rs's "tip: a similar argument exists" line).
fn die_mfs_typo(token: &str) -> ! {
    eprintln!("error: unexpected argument '{token}' found");
    eprintln!();
    eprintln!("  tip: a similar argument exists: '--msg-fill-style' (short form: -mfs)");
    eprintln!(
        "  [possible values: typewriter, fade, words, slide, instant, engrave, hologram, glitch, scorch, cascade]"
    );
    eprintln!();
    eprintln!(
        "{}  tip: a similar argument exists: '--msg-fill-style'{}",
        crate::output::warn_open(),
        crate::output::reset()
    );
    std::process::exit(2);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expand(args: &[&str]) -> Vec<String> {
        let argv: Vec<std::ffi::OsString> = args.iter().map(std::ffi::OsString::from).collect();
        expand_argv_shorthands(&argv)
            .into_iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn mfs_space_form_expands_to_long_flag() {
        let out = expand(&["cosmostrix", "-mfs", "fade"]);
        assert_eq!(out, vec!["cosmostrix", "--msg-fill-style", "fade"]);
    }

    #[test]
    fn mfs_equals_form_expands() {
        let out = expand(&["cosmostrix", "-mfs=pulse"]);
        assert_eq!(out, vec!["cosmostrix", "--msg-fill-style", "pulse"]);
    }

    #[test]
    fn mfs_attached_form_expands_for_valid_values() {
        for value in [
            "typewriter",
            "fade",
            "words",
            "slide",
            "instant",
            "engrave",
            "hologram",
            "glitch",
            "scorch",
            "cascade",
        ] {
            let token = format!("-mfs{value}");
            let out = expand(&["cosmostrix", token.as_str()]);
            assert_eq!(
                out,
                vec!["cosmostrix", "--msg-fill-style", value],
                "attached form {token} must expand"
            );
        }
    }

    #[test]
    fn mfs_without_value_still_emits_flag_for_clap_error() {
        // No trailing token: the flag is emitted alone so clap reports
        // "a value is required" with the possible-values list.
        let out = expand(&["cosmostrix", "-mfs"]);
        assert_eq!(out, vec!["cosmostrix", "--msg-fill-style"]);
    }

    #[test]
    fn mb_expansion_still_works() {
        let out = expand(&["cosmostrix", "-mb", "hello"]);
        assert_eq!(out, vec!["cosmostrix", "--message-border", "-m", "hello"]);
        let out = expand(&["cosmostrix", "-mb=hello"]);
        assert_eq!(out, vec!["cosmostrix", "--message-border", "-m", "hello"]);
    }

    #[test]
    fn unrelated_tokens_pass_through_untouched() {
        let out = expand(&["cosmostrix", "-m", "hello", "--fps", "30", "-mfs", "slide"]);
        assert_eq!(
            out,
            vec![
                "cosmostrix",
                "-m",
                "hello",
                "--fps",
                "30",
                "--msg-fill-style",
                "slide"
            ]
        );
    }

    #[test]
    fn m_attached_values_not_starting_with_mfs_pass_through() {
        // `-mhello` (attached message) must NOT be touched.
        let out = expand(&["cosmostrix", "-mhello"]);
        assert_eq!(out, vec!["cosmostrix", "-mhello"]);
        // `-m fz` (space-separated) is also untouched.
        let out = expand(&["cosmostrix", "-m", "fss message"]);
        assert_eq!(out, vec!["cosmostrix", "-m", "fss message"]);
    }

    #[test]
    fn mfs_typo_exits_with_suggestion() {
        // `-mfss` (doubled s) must exit(2) rather than silently becoming
        // `-m fss`. Verified via the process exit, so this test only runs
        // when spawned; here we assert the guard predicate instead.
        assert!(!is_valid_style_value("s"));
        assert!(!is_valid_style_value("ss"));
        assert!(!is_valid_style_value("FADE"));
        assert!(is_valid_style_value("fade"));
    }
}
