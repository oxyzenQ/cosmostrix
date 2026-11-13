// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! Help text colorizer — extracted from `config/mod.rs` to keep that
//! file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns `colorize_help()` — paints the --help output with the
//! zelynic tier grammar: section headings in bold brand purple,
//! the Usage: synopsis and flag spellings (definition rows and
//! inline prose mentions) in the calm grey grammar tier, and
//! runnable example command lines in bold ok green (the shared
//! zelynic status-green seat, `crate::output::ok_bold_open`).
//!
//! NIGHT-boost-2 (2026-09-24): example command lines (indented
//! `cosmostrix ...` invocations) render in bold ok green (the shared
//! zelynic status-green seat) so runnable examples are visually
//! distinct from option definitions (bold white) and section
//! headings (bold brand purple). The `#`
//! annotation lines that sit ABOVE each example stay plain — position,
//! not color, marks them as commentary.

/// Bold ok green open sequence for --help example command lines —
/// the shared zelynic status-green seat (`crate::output::ok_bold_open`):
/// truecolor #50FA7B (80,250,123), the 256-color 84 rung, ANSI green
/// (32) on classic 16-color terminals, plain text on mono.
///
/// night-cafe-1 (the zelynic 100-percent color-parity pass): the
/// private ladder this surface used to carry — the Green theme's body
/// stop (80,255,110), a hue the zelynic Ok slot never rode — is
/// retired. One green, one seat: the help's runnable-command tier is
/// now byte-identical to every other CLI surface that paints a
/// runnable verdict, and the hue can never drift again. Green means
/// "runnable command"; every other help surface keeps its existing
/// hue.
pub(crate) fn matrix_green_bold_open() -> &'static str {
    crate::output::ok_bold_open()
}

pub(crate) fn colorize_help(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    // night-cafe-1 (the zelynic synopsis law): the invocation line
    // under the Usage: heading is grammar, not a runnable example -
    // it rides the calm grey tier. The slot opens when the heading
    // branch emits Usage: and is consumed by the first non-blank
    // line that follows, so exactly one synopsis line is exempted
    // from the green example matcher per Usage: block.
    let mut pending_synopsis = false;
    for chunk in text.split_inclusive('\n') {
        let (line, nl) = chunk
            .strip_suffix('\n')
            .map(|l| (l, "\n"))
            .unwrap_or((chunk, ""));

        // night-ux-2 (2026-11-11): the banner rail (the ━━━ title line)
        // rides the heading tier too — the zelynic helper-output canon
        // renders it bold brand purple, and the owner's re-run report
        // caught it arriving white.
        let is_banner = line.starts_with('\u{2501}');

        // night-T-1 (2026-11-11): the zelynic-shaped help head uses
        // Title case section labels ("Usage:", "Examples:") — they ride
        // the same heading tier as the all-caps sections.
        // night-ux-2: the heading test now catches the Title case shape
        // itself (short, letters-and-spaces, colon-terminated, not
        // indented) instead of only the all-caps shape plus two
        // hand-listed labels — the owner's re-run report caught
        // "Common options:" (and every other renamed section) still
        // arriving white. The length guard keeps wrapped prose lines
        // that merely end in a colon out of the tier.
        let body = line.trim_end_matches(':');
        let is_heading = !line.starts_with(' ')
            && line.ends_with(':')
            && !body.is_empty()
            && body.len() <= 40
            && body.chars().all(|c| c.is_ascii_alphabetic() || c == ' ');

        if is_banner || is_heading {
            // Bold brand purple for the banner rail and section
            // headings (matches --help USAGE:).
            // body is the heading with its trailing colon(s) stripped
            // (trim_end_matches above), so the comparison carries none.
            pending_synopsis = is_heading && body == "Usage";
            out.push_str(crate::output::brand_bold_open());
            out.push_str(line);
            out.push_str(crate::output::reset());
            out.push_str(nl);
            continue;
        }

        if let Some(rest) = line.strip_prefix("      Example:") {
            // Bold white for "Example:" labels
            out.push_str("      \x1b[1mExample:\x1b[0m");
            out.push_str(rest);
            out.push_str(nl);
            continue;
        }

        // night-cafe-1 (the zelynic synopsis law, NIGHT-hunt-35): the
        // invocation line under the Usage: heading rides the calm grey
        // grammar tier — the same tier zelynic paints its
        // `zelynic <command> [target] [value] [options]` synopsis with.
        // The indent stays outside the span (the flag-row law), and the
        // slot is consumed exactly once per Usage: block, so a blank
        // line after the heading never leaks grey into the next tier.
        if pending_synopsis {
            if !line.trim().is_empty() {
                pending_synopsis = false;
                if line.starts_with(' ') && line.trim_start().starts_with("cosmostrix") {
                    let indent_len = line.len() - line.trim_start().len();
                    out.push_str(&line[..indent_len]);
                    out.push_str(crate::output::grey_open());
                    out.push_str(line.trim_start());
                    out.push_str(crate::output::reset());
                    out.push_str(nl);
                    continue;
                }
            }
        }

        // NIGHT-boost-2: example command lines — any indented line whose
        // first non-space token is `cosmostrix` (the 2-space COMMON
        // OPTIONS examples, the 6-space inline examples, and the 8-space
        // --dump-config block) — render the WHOLE line in bold ok green
        // (the shared zelynic status-green seat). One rule, one
        // semantic: green = copy-pasteable command. The Usage: synopsis
        // never reaches this branch (the grey slot above consumes it —
        // the zelynic hierarchy: bold purple headings, grey synopsis
        // and flag grammar, green runnable examples).
        // Prose that merely mentions cosmostrix mid-sentence ("Then:
        // cosmostrix ...") does not lead with the binary name and stays
        // plain, so the matcher cannot false-positive on narrative text.
        if line.starts_with(' ') && line.trim_start().starts_with("cosmostrix") {
            out.push_str(matrix_green_bold_open());
            out.push_str(line);
            out.push_str(crate::output::reset());
            out.push_str(nl);
            continue;
        }

        if let Some(rest) = line.strip_prefix("  sig ") {
            // night-signature-advanced-4: the `sig` SUBCOMMAND's
            // definition row rides the same calm grey grammar tier as
            // the flag rows — a subcommand is grammar too (the same
            // zelynic "context, not content" law; the grey span keeps
            // definition rows visually distinct from their prose).
            out.push_str("  ");
            out.push_str(crate::output::grey_open());
            out.push_str("sig");
            out.push_str(rest);
            out.push_str(crate::output::reset());
            out.push_str(nl);
            continue;
        }

        if let Some(rest) = line.strip_prefix("  -") {
            // Calm grey for the flag spelling (night-improve-2
            // grey law: flags are grammar — "context, not
            // content", the zelynic flag_row tier). The spelling
            // covers the whole definition line ("-c, --color
            // <name>"): value hints ride inside the span.
            out.push_str("  ");
            out.push_str(crate::output::grey_open());
            out.push('-');
            out.push_str(rest);
            out.push_str(crate::output::reset());
            out.push_str(nl);
            continue;
        }

        if let Some(rest) = line.strip_prefix("  --") {
            // Calm grey for the long spelling — same tier, same
            // span law as the short branch above.
            out.push_str("  ");
            out.push_str(crate::output::grey_open());
            out.push_str("--");
            out.push_str(rest);
            out.push_str(crate::output::reset());
            out.push_str(nl);
            continue;
        }

        // night-improve-2: inline flag mentions inside prose
        // ("See --list-colors.", "(e.g. -mfss) is rejected")
        // ride the same grey grammar tier — a flag spelling is
        // grammar wherever it appears. The matcher anchors on a
        // word boundary and requires a letter after the dashes, so
        // kill -9, ranges like 0.0-3.0, and undashed names stay
        // plain. Lines painted above (headings, examples, spelling
        // rows) never reach this branch.
        out.push_str(&grey_inline_mentions(line));
        out.push_str(nl);
    }
    out
}

/// Wrap every inline flag spelling in `line` with the calm grey tier.
///
/// Scanner, not regex (no new dependency): a candidate starts at a
/// word boundary, carries one or two ASCII dashes, then a LETTER
/// (digits-only tails like `-9` and hyphenated numbers like `0.0-3.0`
/// can never match), and the body runs over letters, digits, and
/// dashes with trailing dashes trimmed back out of the span. Splice
/// indices land on ASCII boundaries, so the `&str` slicing stays
/// UTF-8-safe even though the prose carries em-dashes.
fn grey_inline_mentions(line: &str) -> String {
    let open = crate::output::grey_open();
    if open.is_empty() {
        return line.to_string();
    }
    let bytes = line.as_bytes();
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let boundary = i == 0
            || !(bytes[i - 1].is_ascii_alphanumeric()
                || bytes[i - 1] == b'_'
                || bytes[i - 1] == b'-');
        if boundary && bytes[i] == b'-' {
            let mut j = i;
            while j < bytes.len() && bytes[j] == b'-' {
                j += 1;
            }
            if j - i <= 2 && j < bytes.len() && bytes[j].is_ascii_alphabetic() {
                let mut k = j;
                while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'-') {
                    k += 1;
                }
                let mut end = k;
                while end > j && bytes[end - 1] == b'-' {
                    end -= 1;
                }
                if end > j {
                    spans.push((i, end));
                    i = end;
                    continue;
                }
            }
        }
        i += 1;
    }
    if spans.is_empty() {
        return line.to_string();
    }
    let mut out = String::with_capacity(line.len() + spans.len() * (open.len() + 8));
    let mut last = 0;
    for (s, e) in spans {
        out.push_str(&line[last..s]);
        out.push_str(open);
        out.push_str(&line[s..e]);
        out.push_str(crate::output::reset());
        last = e;
    }
    out.push_str(&line[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// night-cafe-1: the example tier rides the shared ok-bold seat —
    /// byte-identical to `crate::output::ok_bold_open()` at the live
    /// capability, so the help green can never drift from the verdict
    /// green. The per-rung byte pin lives in the Ok slot's own seat
    /// (test/output/output_tests.rs ok_bold_open_is_capability_laddered).
    #[test]
    fn example_green_rides_the_shared_ok_bold_seat() {
        assert_eq!(matrix_green_bold_open(), crate::output::ok_bold_open());
    }

    /// Example command lines are wrapped green-line-reset, verbatim
    /// body, at every indent level the help text uses (2, 6, 8 spaces)
    /// — standalone lines stay green; only the line following the
    /// Usage: heading is exempted (the synopsis slot).
    #[test]
    fn example_command_lines_render_green() {
        let green = matrix_green_bold_open();
        let reset = crate::output::reset();
        for line in [
            "  cosmostrix [OPTIONS]",
            "      cosmostrix --color rainbow",
            "        cosmostrix --dump-config | less",
        ] {
            let out = colorize_help(&format!("{line}\n"));
            assert_eq!(
                out,
                format!("{green}{line}{reset}\n"),
                "example line not green-wrapped verbatim: {line}"
            );
        }
    }

    /// night-cafe-1 (the zelynic synopsis law): the invocation line
    /// under the Usage: heading rides the calm grey tier — indent
    /// outside the span, spelling inside — while the line under the
    /// Examples: heading stays green. The zelynic hierarchy in two
    /// pins: grey grammar under Usage:, green runnable under Examples:.
    #[test]
    fn usage_synopsis_rides_grey_and_examples_stay_green() {
        let g = crate::output::grey_open();
        let r = crate::output::reset();
        let purple = crate::output::brand_bold_open();
        let out = colorize_help("Usage:\n  cosmostrix [options]\n");
        assert_eq!(
            out,
            format!("{purple}Usage:{r}\n  {g}cosmostrix [options]{r}\n")
        );
        let green = matrix_green_bold_open();
        let out = colorize_help("Examples:\n  cosmostrix -c rainbow -v\n");
        assert_eq!(
            out,
            format!("{purple}Examples:{r}\n{green}  cosmostrix -c rainbow -v{r}\n")
        );
    }

    /// The synopsis slot is consumed once: a Usage: block whose
    /// synopsis is absent cannot leak grey onto the next tier (the
    /// following heading or flag row renders per its own law).
    #[test]
    fn synopsis_slot_is_consumed_exactly_once() {
        let g = crate::output::grey_open();
        let r = crate::output::reset();
        let purple = crate::output::brand_bold_open();
        // Blank line between heading and synopsis: the slot survives
        // the blank and still greys the synopsis.
        let out = colorize_help("Usage:\n\n  cosmostrix [options]\n");
        assert_eq!(
            out,
            format!("{purple}Usage:{r}\n\n  {g}cosmostrix [options]{r}\n"),
            "synopsis after a blank line must still ride grey"
        );
        // A Usage: block with no synopsis: the next heading keeps its
        // own tier, and the example line after it is never greyed by
        // the stale slot (the slot only exempts the FIRST non-blank
        // line, and a non-matching line consumes it without painting).
        // Full-output equality, built from the live capability, so the
        // pin holds on every rung (mono included - the contains() probe
        // trap: plain text matches every tier-wrapped needle when the
        // tiers are empty, so a negated contains() lies in mono runs).
        let out = colorize_help("Usage:\nExamples:\n  cosmostrix -c rainbow -v\n");
        let green = matrix_green_bold_open();
        assert_eq!(
            out,
            format!(
                "{purple}Usage:{r}\n{purple}Examples:{r}\n{green}  cosmostrix -c rainbow -v{r}\n"
            ),
            "the example line must ride green, never the stale synopsis slot"
        );
    }

    /// Prose that mentions cosmostrix without leading with it must
    /// stay untouched — the matcher keys on the first non-space token.
    #[test]
    fn prose_mentioning_cosmostrix_stays_plain() {
        let plain = "      Then: cosmostrix --charset zen\n";
        assert_eq!(colorize_help(plain), plain);
    }

    /// The `#` annotation lines above examples are deliberately plain
    /// (position, not color, marks commentary) — they must pass
    /// through byte-identical.
    #[test]
    fn annotation_lines_above_examples_stay_plain() {
        let annotation = "      # custom palette from config\n";
        assert_eq!(colorize_help(annotation), annotation);
    }

    /// Headings stay brand-bold-wrapped (unchanged pre/post grey).
    #[test]
    fn heading_stays_brand_bold() {
        let heading = colorize_help("DIAGNOSTICS:\n");
        assert!(heading.starts_with(crate::output::brand_bold_open()));
        assert!(heading.ends_with(":\n"));
    }

    /// night-T-1: the Title case section labels of the zelynic-shaped
    /// help head ride the same heading tier as the all-caps sections.
    #[test]
    fn title_case_section_labels_stay_brand_bold() {
        for heading in ["Usage:\n", "Examples:\n"] {
            let out = colorize_help(heading);
            assert!(
                out.starts_with(crate::output::brand_bold_open()),
                "{heading} must render as a heading"
            );
        }
    }

    /// night-ux-2 (2026-11-11): the banner rail renders bold brand
    /// purple — the owner's re-run report caught it arriving white.
    #[test]
    fn banner_rail_renders_brand_bold() {
        let banner = "━━━ cosmostrix — cinematic Matrix rain renderer ━━━\n";
        let out = colorize_help(banner);
        assert!(
            out.starts_with(crate::output::brand_bold_open()),
            "the banner rail must render as a heading"
        );
        assert!(out.ends_with(crate::output::reset()));
    }

    /// night-ux-2: every section label of the real manual rides the
    /// heading tier — the shape test replaces the hand-listed
    /// allowlist, so a renamed section can never fall out of sync
    /// again (the exact white-heading failure the owner re-ran).
    #[test]
    fn every_manual_section_label_renders_brand_bold() {
        for heading in [
            "Usage:",
            "Examples:",
            "Common options:",
            "Config:",
            "Diagnostics:",
            "Discovery:",
            "Runtime controls:",
            "Help:",
            "Rendering philosophy:",
        ] {
            let out = colorize_help(&format!("{heading}\n"));
            assert!(
                out.starts_with(crate::output::brand_bold_open()),
                "{heading} must render as a heading"
            );
        }
    }

    /// night-ux-2: the length guard — a long letters-only prose line
    /// that merely ends in a colon stays plain, it is not a heading.
    #[test]
    fn long_prose_colon_line_stays_plain() {
        let line = "accepted keys are sat saturation bright brightness head body tail:\n";
        assert_eq!(colorize_help(line), line);
    }

    /// Flag definition lines render the whole spelling in the calm
    /// grey grammar tier (night-improve-2): the spelling including its
    /// value hint rides inside the span, the two-space indent never
    /// does. Expected values build from the live capability so the
    /// pin holds on every rung (mono included).
    #[test]
    fn flag_spellings_render_grey() {
        let g = crate::output::grey_open();
        let r = crate::output::reset();
        let flag = colorize_help("  -c, --color <name>\n");
        assert_eq!(flag, format!("  {g}-c, --color <name>{r}\n"));
        let long = colorize_help("  --charset [--charset-custom] <name>\n");
        assert_eq!(
            long,
            format!("  {g}--charset [--charset-custom] <name>{r}\n")
        );
    }

    /// Inline flag mentions inside prose ride the same grey tier; the
    /// surrounding prose and the trailing period stay plain.
    #[test]
    fn inline_flag_mentions_render_grey() {
        let g = crate::output::grey_open();
        let r = crate::output::reset();
        let line = "      Color theme or custom palette name. See --list-colors.\n";
        let out = colorize_help(line);
        assert_eq!(
            out,
            format!("      Color theme or custom palette name. See {g}--list-colors{r}.\n")
        );
    }

    /// The inline matcher is conservative: digit tails (kill -9),
    /// hyphenated numeric ranges (0.0-3.0), and undashed names never
    /// grey — lines pass through byte-identical.
    #[test]
    fn inline_matcher_conservative_cases_stay_plain() {
        for line in [
            "      a kill -9 TUI death (sudo-safe, works blind-typed)\n",
            "      Range 0.0-3.0 (1.0 = no change).\n",
            "      Tune theme colors. Accepted keys: sat/saturation.\n",
        ] {
            assert_eq!(colorize_help(line), line, "must stay plain: {line}");
        }
    }
}
