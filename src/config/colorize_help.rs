// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Help text colorizer — extracted from `config/mod.rs` to keep that
//! file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns `colorize_help()` — applies brand purple bold to --flag names
//! + section headers in the --help output.
//!
//! NIGHT-boost-2 (2026-09-24): example command lines (indented
//! `cosmostrix ...` invocations) render in bold Matrix green so
//! runnable examples are visually distinct from option definitions
//! (bold white) and section headings (bold brand purple). The `#`
//! annotation lines that sit ABOVE each example stay plain — position,
//! not color, marks them as commentary.

use crate::output::color_capability;
use crate::output::ColorCapability;

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
            // Bold white for short flags (-c, -S, etc.)
            out.push_str("  \x1b[1m-");
            out.push_str(rest);
            out.push_str("\x1b[0m");
            out.push_str(nl);
            continue;
        }

        if let Some(rest) = line.strip_prefix("  --") {
            // Bold white for long flags (--color, --fps, etc.)
            out.push_str("  \x1b[1m--");
            out.push_str(rest);
            out.push_str("\x1b[0m");
            out.push_str(nl);
            continue;
        }

        out.push_str(line);
        out.push_str(nl);
    }
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

    /// Headings and flag lines keep their pre-boost styles: a heading
    /// is brand-bold-wrapped and a short flag line is bold white.
    #[test]
    fn heading_and_flag_styles_are_unchanged() {
        let heading = colorize_help("DIAGNOSTICS:\n");
        assert!(heading.starts_with(crate::output::brand_bold_open()));
        assert!(heading.ends_with(":\n"));

        let flag = colorize_help("  -c, --color <name>\n");
        assert_eq!(flag, "  \x1b[1m-c, --color <name>\x1b[0m\n");
    }
}
