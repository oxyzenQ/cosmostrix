// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Unified report formatting engine for all cosmostrix diagnostics output.
//!
//! Provides consistent, premium-quality formatting across --doctor,
//! --perf-stats, --benchmark, and any future diagnostics.

use std::io::IsTerminal;

/// A structured report with a title and named sections.
pub(crate) struct Report {
    title: String,
    sections: Vec<Section>,
}

/// A named section within a report, containing key-value fields.
pub(crate) struct Section {
    name: String,
    fields: Vec<Field>,
    /// Optional advisory lines (printed as `  - message`).
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

    /// Print the report to stdout with consistent formatting.
    ///
    /// Format:
    /// ```text
    /// TITLE
    /// ─────
    /// SECTION
    ///   key: value
    ///   key: value
    ///
    /// SECTION
    ///   key: value
    /// ```
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

        let rule: String = "\u{2500}".repeat(self.title.len());

        let out_fn: fn(&str) = if to_stderr {
            |s: &str| eprintln!("{s}")
        } else {
            |s: &str| println!("{s}")
        };

        if supports_ansi {
            out_fn(&format!(
                "{}{}{}",
                crate::output::brand_bold_open(),
                self.title,
                crate::output::reset()
            ));
        } else {
            out_fn(&self.title);
        }
        out_fn(&rule);

        let mut first = true;
        for section in &self.sections {
            if !first {
                out_fn("");
            }
            first = false;

            if supports_ansi {
                out_fn(&format!(
                    "{}{}{}",
                    crate::output::brand_bold_open(),
                    section.name,
                    crate::output::reset()
                ));
            } else {
                out_fn(&section.name);
            }
            for field in &section.fields {
                out_fn(&format!("  {}: {}", field.key, field.value));
            }
            for advice in &section.advice {
                out_fn(&format!("  - {}", advice));
            }
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

    /// Add an advisory line (printed as `  - message`).
    pub(crate) fn advice(&mut self, message: &str) -> &mut Self {
        self.advice.push(message.to_string());
        self
    }

    /// Returns true if no advisory lines have been added.
    pub(crate) fn has_advice(&self) -> bool {
        !self.advice.is_empty()
    }
}
