// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Help text colorizer — extracted from `config/mod.rs` to keep that
//! file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns `colorize_help()` — paints the --help output with the
//! three-tier semantic grammar: section headings in bold brand
//! purple, flag spellings (definition rows and inline prose
//! mentions) in the calm grey grammar tier, and runnable example
//! command lines in bold Matrix green.
//!
//! NIGHT-boost-2 (2026-09-24): example command lines (indented
//! `cosmostrix ...` invocations) render in bold Matrix green so
//! runnable examples are visually distinct from option definitions
//! (bold white) and section headings (bold brand purple). The `#`
//! annotation lines that sit ABOVE each example stay plain — position,
//! not color, marks them as commentary.

use crate::output::ColorCapability;
use crate::output::color_capability;

/// Bold Matrix green open sequence for --help example command lines,
/// capability-aware (the same four-rung ladder as
/// `crate::output::brand_bold_open`).
///
/// Color provenance: the default Green theme's body stop
/// (80, 255, 110) from `src/engine/chroma_dragon_engine/catalog/
/// themes.rs`, quantized per capability — 256-color index 84
/// (#5fff87, the theme's own ansi ramp body entry) and ANSI green 32
/// (the theme's c16 body anchor). Green means "runnable command";
/// every other help surface keeps its existing hue.
pub(crate) fn matrix_green_bold_open() -> &'static str {
    matrix_green_bold_open_for(color_capability())
}

/// Capability-parameterized form — unit-testable without env sniffing.
#[must_use]
fn matrix_green_bold_open_for(cap: ColorCapability) -> &'static str {
    match cap {
        ColorCapability::TrueColor => "\x1b[1;38;2;80;255;110m",
        ColorCapability::Color256 => "\x1b[1;38;5;84m",
        ColorCapability::Color16 => "\x1b[1;32m",
        ColorCapability::Mono => "",
    }
}

pub(crate) fn colorize_help(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    for chunk in text.split_inclusive('\n') {
        let (line, nl) = chunk
            .strip_suffix('\n')
            .map(|l| (l, "\n"))
            .unwrap_or((chunk, ""));

        let is_heading =
            !line.starts_with(' ') && line.ends_with(':') && line == line.to_ascii_uppercase();

        if is_heading {
            // Bold brand purple for section headings (matches --help USAGE:).
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

        // NIGHT-boost-2: example command lines — any indented line whose
        // first non-space token is `cosmostrix` (the 2-space COMMON
        // OPTIONS examples, the 6-space inline examples, and the 8-space
        // --dump-config block) — render the WHOLE line in bold Matrix
        // green. One rule, one semantic: green = copy-pasteable command.
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

    /// The green ladder must match the provenance contract exactly:
    /// truecolor body stop (80,255,110), the theme's own 256-color
    /// ramp entry 84, the theme's c16 body anchor 32, and nothing on
    /// mono. If the Green theme is ever retuned, this test plus the
    /// doc comment pin where the new values must be mirrored.
    #[test]
    fn green_open_is_capability_laddered() {
        assert_eq!(
            matrix_green_bold_open_for(ColorCapability::TrueColor),
            "\x1b[1;38;2;80;255;110m"
        );
        assert_eq!(
            matrix_green_bold_open_for(ColorCapability::Color256),
            "\x1b[1;38;5;84m"
        );
        assert_eq!(
            matrix_green_bold_open_for(ColorCapability::Color16),
            "\x1b[1;32m"
        );
        assert_eq!(matrix_green_bold_open_for(ColorCapability::Mono), "");
    }

    /// Example command lines are wrapped green-line-reset, verbatim
    /// body, at every indent level the help text uses (2, 6, 8 spaces)
    /// — and the USAGE invocation line belongs to the same class.
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
