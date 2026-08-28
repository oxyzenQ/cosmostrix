// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Help text colorizer — extracted from `config/mod.rs` to keep that
//! file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns `colorize_help()` — applies brand purple bold to --flag names
//! + section headers in the --help output.

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

        if let Some(rest) = line.strip_prefix("  cosmostrix") {
            // Bold white for command examples
            out.push_str("  \x1b[1mcosmostrix\x1b[0m");
            out.push_str(rest);
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
