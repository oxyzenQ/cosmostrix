// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

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
//! | `-mb<text>`      | REFUSED — attached values are not accepted on the message shorthands |
//! | `-mfs` + value   | `--msg-fill-style <value>`         |
//! | `-mfs=<style>`   | `--msg-fill-style <style>`         |
//! | `-mfs<style>`    | REFUSED — attached values are not accepted on the message shorthands |
//! | `-m<text>`       | REFUSED — attached values are not accepted on the message shorthands |
//! | `sig` (argv[1])  | DISPATCH + exit: the six-depth signature report
//!                    (night-signature-advanced-4 — `sig` is a SUBCOMMAND,
//!                    owner directive "subcommand only, don't --sig"; the
//!                    `--sig` flag spelling is retired and clap reports it
//!                    like any unknown flag) |
//!
//! night-cafe-mclaren-4 (owner: "-mb/-m untuk message harusnya strict",
//! then "masih bisa" after mclaren-3's attached-form expansion): the
//! message shorthands refuse ATTACHED values outright — space and
//! equals are the only accepted value forms. The rationale is a
//! validation asymmetry, not a style choice: every OTHER value-taking
//! short flag (`-f`, `-S`, `-d`, `-c`, `-C`, `-b`, `-g`, `-l`, `-M`)
//! validates its value downstream, so an attached typo dies loudly
//! (`-fabc` → "invalid value for --fps: abc"; prevalidation pins the
//! canonical grammar for the attached spellings too — see the
//! `attached_short_values_prevalidate_like_long_forms` test). Message
//! text is the ONE free-form class where garbage silently SUCCEEDS: a
//! flag-shaped token must never become message data — neither mangled
//! (pre-mclaren-3: `-mbxxsaadadala` displayed "bxxsaadadala" with
//! border: false) nor reinterpreted (mclaren-3: the same token became
//! the bordered message "xxsaadadala"). Both were silent acceptance;
//! the strict contract is refusal.
//!
//! The refusal retires the documented side effects of the old attached
//! forms: an attached `-m` message starting with "fs" or "b"
//! (`-mfss is my message`, `-mblaze`) no longer resolves to a
//! shorthand — it is refused with a tip naming the accepted forms.
//! Use the space form (`-m "fss …"`, `-m "blaze …"`) or the equals
//! form (`-m=<text>`; clap-native, verified end-to-end).
//!
//! Bare flags keep their loud no-silent-failure contract: bare `-mb`
//! emits `--message-border -m` so clap reports the missing message
//! value (night-dragon-fly-2); bare `-mfs` emits `--msg-fill-style`
//! for the same missing-value error with the possible-values list.
//! The equals forms hand their value to clap's own validation
//! (`-mfs=<garbage>` dies in clap's ValueEnum check, loudly).

/// Expand the `-mb` / `-mfs` shorthands in the raw argv.
///
/// The returned vector starts with `argv[0]` (the program name) when
/// present and preserves the order of all other tokens. Exits with
/// code 2 on a message-shorthand attached value (see
/// `die_message_attached`).
///
/// depthtest-1 (NIGHT-hunter-19): argv CAN be empty — `execve()`
/// accepts an empty argvp (`[NULL]`), and `std::env::args_os()`
/// then yields zero items. The old `argv[0]` direct index panicked
/// (index out of bounds) before clap ever ran — a crash-on-startup
/// reachable from a wrapper script that re-execs cosmostrix with a
/// scrubbed argv. The first() guard degrades to an empty expansion;
/// clap handles the missing program name itself (falls back to the
/// command's name for usage lines).
pub(crate) fn expand_argv_shorthands(argv: &[std::ffi::OsString]) -> Vec<std::ffi::OsString> {
    let mut expanded: Vec<std::ffi::OsString> = Vec::with_capacity(argv.len() + 1);
    if let Some(program) = argv.first() {
        expanded.push(program.clone());
    }
    let mut i = 1;
    while i < argv.len() {
        let arg = &argv[i];

        // night-signature-advanced-4 (owner directive): `sig` is a
        // SUBCOMMAND, dispatched here — before clap, which has no
        // positionals, so a bare `sig` at argv[1] was always ours to
        // handle. First token ONLY: a "sig" anywhere else is message
        // text or a value, not a command (`cosmostrix -m sig` stays a
        // message). The retired `--sig` flag spelling exists nowhere —
        // clap reports it like any unknown flag, which is exactly the
        // contract: subcommand only. Extra tokens after `sig` are
        // refused with a clap-shaped error (the report takes no
        // arguments; the same refusal lane as the attached-value
        // guard). Exit codes: 0 authentic, 1 unsigned (missing
        // provenance link, night-hit-2), 2 tamper (print_sig_report
        // owns both non-zero exits) — the install gate greps the
        // AUTHENTIC verdict.
        if i == 1 && arg == "sig" {
            if let Some(extra) = argv.get(i + 1) {
                die_sig_extra_arg(&extra.to_string_lossy());
            }
            crate::diagnostics::sig_report::print_sig_report();
            // The authentic path reaches here (tamper exits 2 and
            // unsigned exits 1 inside print_sig_report). Flush before
            // exit: process::exit skips
            // the stdout destructor, and a piped report must not be
            // truncated by a still-buffered writer.
            use std::io::Write;
            let _ = std::io::stdout().flush();
            std::process::exit(0);
        }

        // v80.0.0-beta.1 msg-fill-style: -mfs shorthand. Bare and
        // equals forms expand; the attached form is refused by the
        // message-family guard below (night-cafe-mclaren-4). Checked
        // BEFORE -mb for the bare/equals arms (no prefix collision —
        // "-mb" vs "-mfs" differ at the 3rd char).
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
        } else if let Some(s) = arg.to_str()
            && let Some(rest) = s.strip_prefix("-mfs=")
        {
            // -mfs=<style> form: an invalid style dies in clap's own
            // ValueEnum check — loudly, with the possible-values list.
            expanded.push("--msg-fill-style".into());
            expanded.push(rest.into());
            i += 1;
            continue;
        }

        // -mb shorthand: expand to the hidden --message-border boolean
        // plus -m with the message text.
        //
        // night-dragon-fly-2: the bare form used to fall through and leak
        // the raw `-mb` token to clap, which parsed it as `-m` with the
        // ATTACHED value "b" — `cosmostrix -mb` displayed a bordered
        // overlay spelling just "b". Now the bare form emits
        // `--message-border -m` so clap reports the missing message
        // value ("a value is required for '-m <MESSAGE>'"), the same
        // no-silent-failure contract as bare `-m` and bare `-mfs`.
        if arg == "-mb" {
            expanded.push("--message-border".into());
            if i + 1 < argv.len() {
                expanded.push("-m".into());
                expanded.push(argv[i + 1].clone());
                i += 2;
            } else {
                expanded.push("-m".into());
                i += 1;
            }
            continue;
        } else if let Some(s) = arg.to_str()
            && let Some(rest) = s.strip_prefix("-mb=")
        {
            // -mb=<text> form. The equals arm is checked BEFORE the
            // attached-refusal guard — "-mb=x" is message "x" with
            // border, never "=x".
            expanded.push("--message-border".into());
            expanded.push("-m".into());
            expanded.push(rest.into());
            i += 1;
            continue;
        }

        // night-cafe-mclaren-4: the message family refuses attached
        // values. Reached only after the bare and equals arms above, so
        // every remaining `-m…` token that is not the bare flag or an
        // equals form is an attached blob — exactly what clap would
        // parse as message text. Bare `-m` and the equals form
        // `-m=<text>` (clap-native, verified end-to-end) pass through
        // untouched.
        if let Some(s) = arg.to_str()
            && let Some(lane) = message_attached_refusal_token(s)
        {
            match lane {
                "mfs" => {
                    die_message_attached(s, "`-mfs <style>` (space) or `-mfs=<style>` (equals)");
                }
                "mb" => {
                    die_message_attached(
                        s,
                        "`-mb <text>` (space) or `-mb=<text>` (equals) — the bordered message \
                         shorthand",
                    );
                }
                _ => {
                    die_message_attached(s, "`-m <text>` (space) or `-m=<text>` (equals)");
                }
            }
        }

        expanded.push(arg.clone());
        i += 1;
    }
    expanded
}

/// Pure routing predicate for the message-family attached-refusal
/// guard (unit-testable; the actual call exits the process like the
/// other pre-clap guards).
///
/// Returns the lane whose attached form the token carries:
/// `"mfs"`, `"mb"`, or `"m"`. Bare flags and the equals forms return
/// `None` (they pass through their own arms). Tokens outside the
/// family — including the validated value-taking shorts (`-f60`,
/// `-S12`), boolean clusters (`-vq`), the uppercase `-M` lane, and
/// double-dash longs — return `None` untouched. Non-UTF8 args never
/// reach this predicate (the caller's `to_str()` guard passes them
/// through for clap to handle).
fn message_attached_refusal_token(s: &str) -> Option<&'static str> {
    if s.starts_with("-mfs") && s != "-mfs" && !s.starts_with("-mfs=") {
        return Some("mfs");
    }
    if s.starts_with("-mb") && s != "-mb" && !s.starts_with("-mb=") {
        return Some("mb");
    }
    // Generic -m lane: attached, and NOT any mfs/mb-lane token — the
    // bare and equals forms of those lanes ("-mfs", "-mfs=X", "-mb",
    // "-mb=X") pass through their own arms, and without this exclusion
    // an equals token like "-mfs=fade" would leak into the generic lane
    // (its "-mf…" prefix looks attached to -m even though it never
    // reaches this check inside the expansion loop).
    if s.starts_with("-m")
        && s != "-m"
        && !s.starts_with("-m=")
        && !s.starts_with("-mfs")
        && !s.starts_with("-mb")
    {
        return Some("m");
    }
    None
}

/// Refuse an attached value on a message shorthand with a clap-format
/// error so the UX matches the unknown-argument path: the real usage
/// line and help footer come via `ux::die_input_with_usage`, and the
/// line-aware renderer paints the `tip:` line suggestion-white
/// automatically.
///
/// `accepted` names the lane's two accepted value forms. The tip
/// carries the WHY, not just the what: a flag-shaped token must never
/// silently become message text (see the module doc for the full
/// validation-asymmetry rationale).
fn die_message_attached(token: &str, accepted: &str) -> ! {
    crate::ux::die_input_with_usage(format!(
        "unexpected argument '{token}' found\n\n  tip: the message shorthands refuse \
         attached values — a flag-shaped token must never silently become message text. \
         Use {accepted}"
    ))
}

/// Refuse arguments after the `sig` subcommand with a clap-format
/// error so the UX matches the unknown-argument path. `sig` is a
/// zero-argument command (zejtron parity): `cosmostrix sig --help` and
/// `cosmostrix sig -m hello` are both refusals, not silent partials.
fn die_sig_extra_arg(token: &str) -> ! {
    crate::ux::die_input_with_usage(format!(
        "unexpected argument '{token}' found\n\n  tip: `sig` is a subcommand and takes no \
         arguments — run `cosmostrix sig`"
    ))
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

    fn refusal(token: &str) -> Option<&'static str> {
        message_attached_refusal_token(token)
    }

    #[test]
    fn mfs_space_form_expands_to_long_flag() {
        let out = expand(&["cosmostrix", "-mfs", "fade"]);
        assert_eq!(out, vec!["cosmostrix", "--msg-fill-style", "fade"]);
    }

    #[test]
    fn mfs_equals_form_expands() {
        // S-master-HUNT-16: changed test value from "pulse" (removed
        // MFS style — see CHANGELOG v50 pulse removal) to "radar"
        // (current live style). The test only verifies the equals-form
        // expansion mechanics (`-mfs=X` → `--msg-fill-style X`), not
        // the value's validity — but using a removed style as the
        // fixture is misleading to future readers.
        let out = expand(&["cosmostrix", "-mfs=radar"]);
        assert_eq!(out, vec!["cosmostrix", "--msg-fill-style", "radar"]);
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
    fn mb_bare_form_emits_flag_plus_m_for_clap_error() {
        // night-dragon-fly-2: bare `-mb` must NOT leak the raw token (clap
        // would parse it as `-m` with the attached value "b" — a bordered
        // overlay spelling "b"). Emit `--message-border -m` so clap
        // reports the missing message value instead.
        let out = expand(&["cosmostrix", "-mb"]);
        assert_eq!(out, vec!["cosmostrix", "--message-border", "-m"]);
    }

    #[test]
    fn mb_equals_form_precedes_the_refusal_guard() {
        // The equals arm is checked BEFORE the attached-refusal guard:
        // "-mb=x" expands (message "x" with border), while "-mba=b" —
        // where the "=" is INSIDE attached text — routes to the mb
        // refusal lane.
        let out = expand(&["cosmostrix", "-mb=x"]);
        assert_eq!(out, vec!["cosmostrix", "--message-border", "-m", "x"]);
        assert_eq!(refusal("-mba=b"), Some("mb"));
    }

    #[test]
    fn attached_refusal_covers_the_message_family() {
        // night-cafe-mclaren-4: every attached blob on the message
        // family routes to its lane — mangled (the owner probe),
        // side-effect-shaped, and valid-style suffixes alike. The
        // actual refusal exits the process (die_message_attached);
        // the end-to-end exit code and message are verified by a
        // binary run in the same commit, like the sig lane.
        assert_eq!(refusal("-mbxxsaadadala"), Some("mb"));
        assert_eq!(refusal("-mbhello"), Some("mb"));
        assert_eq!(refusal("-mblaze"), Some("mb"));
        assert_eq!(refusal("-mfsfade"), Some("mfs"));
        assert_eq!(refusal("-mfsradar"), Some("mfs"));
        assert_eq!(refusal("-mfss"), Some("mfs"));
        assert_eq!(refusal("-msbg"), Some("m"));
        assert_eq!(refusal("-mhello"), Some("m"));
        assert_eq!(refusal("-mfoo"), Some("m"));
        assert_eq!(refusal("-m-x"), Some("m"));
    }

    #[test]
    fn attached_refusal_spares_bare_equals_and_validated_lanes() {
        // Bare flags and equals forms pass through their own arms.
        assert_eq!(refusal("-m"), None);
        assert_eq!(refusal("-mb"), None);
        assert_eq!(refusal("-mfs"), None);
        assert_eq!(refusal("-m=sbg"), None);
        assert_eq!(refusal("-m="), None);
        assert_eq!(refusal("-mb=x"), None);
        assert_eq!(refusal("-mfs=fade"), None);
        // The validated value-taking shorts keep their getopt attached
        // convention — garbage dies loudly in validation, so no
        // refusal is needed (audit: -f/-S/-d/-c/-C/-b/-g/-l/-M).
        assert_eq!(refusal("-f60"), None);
        assert_eq!(refusal("-S12"), None);
        assert_eq!(refusal("-d0.75"), None);
        // Boolean clusters, the uppercase -M lane, double-dash longs,
        // and non-flag words are untouched.
        assert_eq!(refusal("-v"), None);
        assert_eq!(refusal("-vq"), None);
        assert_eq!(refusal("-vsfoo"), None);
        assert_eq!(refusal("-Mzzz"), None);
        assert_eq!(refusal("--mblaze"), None);
        assert_eq!(refusal("--message"), None);
        assert_eq!(refusal("hello"), None);
        assert_eq!(refusal("sig"), None);
    }

    #[test]
    fn m_space_form_passes_through() {
        // The space-separated -m lane is the primary message form —
        // untouched, including messages that start with "fss" or "b"
        // (the old attached-form side effects are retired: attached is
        // refused, space is free).
        let out = expand(&["cosmostrix", "-m", "fss message"]);
        assert_eq!(out, vec!["cosmostrix", "-m", "fss message"]);
        let out = expand(&["cosmostrix", "-m", "blaze"]);
        assert_eq!(out, vec!["cosmostrix", "-m", "blaze"]);
    }

    #[test]
    fn sig_word_in_value_position_passes_through() {
        // A "sig" meant as message TEXT must survive untouched: only
        // argv[1] dispatches, and there no value can precede the token.
        // (argv[1] itself is NOT fed through expand() in tests — the
        // subcommand dispatch EXITS the process, like the attached-value
        // refusal guard; the exit codes are pinned by the install gate.)
        let out = expand(&["cosmostrix", "-m", "sig"]);
        assert_eq!(out, vec!["cosmostrix", "-m", "sig"]);
        let out = expand(&["cosmostrix", "-mb", "sig"]);
        assert_eq!(out, vec!["cosmostrix", "--message-border", "-m", "sig"]);
        // Not the first token: untouched (never a command word there).
        let out = expand(&["cosmostrix", "-v", "sig"]);
        assert_eq!(out, vec!["cosmostrix", "-v", "sig"]);
        // The retired --sig spelling passes through the rewrite lane
        // untouched (expand never rewrites flag-looking tokens); clap
        // reports it as unknown — subcommand only, by directive.
        let out = expand(&["cosmostrix", "--sig"]);
        assert_eq!(out, vec!["cosmostrix", "--sig"]);
        // Other bare words are not ours to rewrite.
        let out = expand(&["cosmostrix", "sigx"]);
        assert_eq!(out, vec!["cosmostrix", "sigx"]);
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
}
