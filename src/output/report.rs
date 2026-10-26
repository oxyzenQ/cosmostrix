// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Unified report formatting engine for all cosmostrix diagnostics output.
//!
//! Provides the zelynic output canon (night-dragon-maintenance-1) across
//! --doctor, --perf-stats, --benchmark, and any future diagnostics:
//!
//! - a `━━━ TITLE ━━━` banner in bold brand purple — the heavy-rule
//!   canon; the all-caps title text itself stays untouched (the
//!   BRANDING.md all-caps display-banner law, and the release CI greps
//!   for the literal title)
//! - lean single-column fields: two-space indent, colon-attached key,
//!   values aligned on one padded column per section — zelynic's
//!   `  {:<11} {}` shape, widened per section when its keys run long
//! - `! message` advisory bullets (zelynic's warnings grammar), the
//!   mark painted in bold warning yellow when ANSI is available
//!
//! The render is machine-friendly by construction: piped surfaces lose
//! every escape (the capability ladder returns plain text), and the
//! `  key:` line prefix survives untouched, so text scrapers keep
//! working across the restyle.

use std::io::IsTerminal;

/// The banner's heavy-rule glyph (U+2501, the zelynic canon box
/// drawing weight — three glyphs per side, exactly as the sibling
/// paints its flagship titles).
const BANNER_GLYPH: &str = "\u{2501}";

/// The label-column floor: zelynic's `  {:<11} {}` field shape pads
/// the colon-attached label to 11 columns, then one separating space
/// puts the value 14 columns in. Sections whose longest key+colon
/// exceeds the floor widen to fit — alignment stays exact either way.
const LABEL_WIDTH_FLOOR: usize = 11;

/// Build the `━━━ TITLE ━━━` banner line (plain text; the renderer
/// wraps it in bold brand purple when the surface supports ANSI).
fn banner_line(title: &str) -> String {
    format!(
        "{BANNER_GLYPH}{BANNER_GLYPH}{BANNER_GLYPH} {title} {BANNER_GLYPH}{BANNER_GLYPH}{BANNER_GLYPH}"
    )
}

/// Build one aligned field line: two-space indent, colon-attached key
/// padded to `width`, then the value.
fn field_line(key: &str, value: &str, width: usize) -> String {
    format!("  {:<width$} {}", format!("{key}:"), value)
}

/// Compute a section's label column from its keys: zelynic's 11-column
/// floor, widened to the longest `key:` plus two spaces when a section
/// runs longer keys.
fn section_width(fields: &[Field]) -> usize {
    fields
        .iter()
        .map(|f| f.key.len() + 1 + 2) // "key:" plus two trailing spaces
        .max()
        .unwrap_or(0)
        .max(LABEL_WIDTH_FLOOR)
}

/// A structured report with a title and named sections.
pub(crate) struct Report {
    title: String,
    sections: Vec<Section>,
}

/// A named section within a report, containing key-value fields.
pub(crate) struct Section {
    name: String,
    fields: Vec<Field>,
    /// Optional advisory lines (printed as `  ! message`).
    advice: Vec<String>,
}

/// A single key-value field within a section.
pub(crate) struct Field {
    key: String,
    value: String,
}

impl Report {
    pub(crate) fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            sections: Vec::new(),
        }
    }

    /// Add a section and return a mutable reference for chaining fields.
    pub(crate) fn section(&mut self, name: &str) -> &mut Section {
        self.sections.push(Section {
            name: name.to_string(),
            fields: Vec::new(),
            advice: Vec::new(),
        });
        self.sections.last_mut().expect("just pushed")
    }

    /// Render the report lines (pure — no I/O, no capability sniffing
    /// when `ansi` is false, so tests pin the exact shape).
    ///
    /// Format:
    /// ```text
    /// ━━━ TITLE ━━━
    ///
    /// SECTION
    ///   key:   value
    ///   key:   value
    ///   ! advice
    ///
    /// SECTION
    ///   key:   value
    /// ```
    fn render_lines(&self, ansi: bool) -> Vec<String> {
        let mut lines = Vec::new();

        let banner = banner_line(&self.title);
        lines.push(if ansi {
            crate::output::brand_bold(&banner)
        } else {
            banner
        });

        for section in &self.sections {
            lines.push(String::new());

            let header = section.name.clone();
            lines.push(if ansi {
                crate::output::brand_bold(&header)
            } else {
                header
            });

            let width = section_width(&section.fields);
            for field in &section.fields {
                lines.push(field_line(&field.key, &field.value, width));
            }
            for advice in &section.advice {
                let mark = if ansi {
                    crate::output::warn_bold("!")
                } else {
                    "!".to_string()
                };
                lines.push(format!("  {mark} {advice}"));
            }
        }

        lines
    }

    /// Print the report to stdout with the zelynic output canon.
    pub(crate) fn print(&self) {
        self.emit(false);
    }

    /// Print the report to stderr.
    ///
    /// Use this when the report must survive an alt-screen restore (e.g.
    /// `--perf-stats` post-loop output). stdout is captured by the alt
    /// screen buffer and lost when `Terminal::drop()` restores the main
    /// screen; stderr is not. This matches the AB-10 rain-screen
    /// cleanliness pattern used by `final_fps_line` and `lr_trace!`.
    pub(crate) fn eprint(&self) {
        self.emit(true);
    }

    /// Shared emit logic for `print()` (stdout) and `eprint()` (stderr).
    fn emit(&self, to_stderr: bool) {
        let supports_ansi = if to_stderr {
            std::io::stderr().is_terminal()
        } else {
            std::io::stdout().is_terminal()
        } && std::env::var_os("NO_COLOR").is_none()
            && !matches!(std::env::var("CLICOLOR").ok().as_deref(), Some("0"));

        let out_fn: fn(&str) = if to_stderr {
            |s: &str| eprintln_safe!("{s}")
        } else {
            |s: &str| println_safe!("{s}")
        };

        for line in self.render_lines(supports_ansi) {
            out_fn(&line);
        }
    }
}

impl Section {
    /// Add a key-value field. Returns `&mut Self` for chaining.
    pub(crate) fn field(&mut self, key: &str, value: &str) -> &mut Self {
        self.fields.push(Field {
            key: key.to_string(),
            value: value.to_string(),
        });
        self
    }

    /// Add an advisory line (printed as `  ! message`).
    pub(crate) fn advice(&mut self, message: &str) -> &mut Self {
        self.advice.push(message.to_string());
        self
    }

    /// Returns true if no advisory lines have been added.
    pub(crate) fn has_advice(&self) -> bool {
        !self.advice.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_uses_the_heavy_rule_canon() {
        // Three glyphs per side, spaces around the title — the exact
        // zelynic flagship shape.
        assert_eq!(
            banner_line("COSMOSTRIX DIAGNOSTICS REPORT"),
            "\u{2501}\u{2501}\u{2501} COSMOSTRIX DIAGNOSTICS REPORT \u{2501}\u{2501}\u{2501}"
        );
    }

    #[test]
    fn short_keys_pad_to_the_zelynic_column() {
        // "build:" is 6 chars; the 11-rung label floor plus the
        // separating space puts the value at column 14 — byte-for-byte
        // where zelynic's `  {:<11} {}` puts it.
        assert_eq!(
            field_line("build", "freebsd-amd64", 11),
            "  build:      freebsd-amd64"
        );
    }

    #[test]
    fn long_keys_widen_without_breaking_the_column() {
        // "color_pipeline_detail:" is 22 chars — padded to 24, the
        // separating space keeps two gaps after the pad.
        assert_eq!(
            field_line("color_pipeline_detail", "chroma", 24),
            "  color_pipeline_detail:   chroma"
        );
    }

    #[test]
    fn section_width_floors_at_the_zelynic_rung() {
        let fields = vec![Field {
            key: "lto".to_string(),
            value: "fat".to_string(),
        }];
        assert_eq!(section_width(&fields), 11);
    }

    #[test]
    fn section_width_widens_for_long_keys() {
        let fields = vec![Field {
            key: "color_pipeline_detail".to_string(), // 21 + 3 = 24
            value: "x".to_string(),
        }];
        assert_eq!(section_width(&fields), 24);
    }

    #[test]
    fn render_is_tidy_end_to_end() {
        let mut report = Report::new("R");
        {
            let s = report.section("BUILD");
            s.field("build", "freebsd-amd64");
            s.field("lto", "fat");
            s.advice("check the linker flags");
        }
        let lines = report.render_lines(false);
        assert_eq!(
            lines[0],
            "\u{2501}\u{2501}\u{2501} R \u{2501}\u{2501}\u{2501}"
        );
        assert_eq!(lines[1], "");
        assert_eq!(lines[2], "BUILD");
        // Both values align on one column (14 chars in).
        assert_eq!(lines[3], "  build:      freebsd-amd64");
        assert_eq!(lines[4], "  lto:        fat");
        // The advisory rides the zelynic warnings grammar.
        assert_eq!(lines[5], "  ! check the linker flags");
        assert_eq!(lines.len(), 6);
    }
}
