// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Unified report formatting engine for all Cosmostrix diagnostics output.
//!
//! Provides consistent, premium-quality formatting across --info, --doctor,
//! --perf-stats, --benchmark, and any future diagnostics.

use std::io::IsTerminal;

/// A structured report with a title and named sections.
pub struct Report {
    title: String,
    sections: Vec<Section>,
}

/// A named section within a report, containing key-value fields.
pub struct Section {
    name: String,
    fields: Vec<Field>,
    /// Optional advisory lines (printed as `  - message`).
    advice: Vec<String>,
}

/// A single key-value field within a section.
pub struct Field {
    key: String,
    value: String,
}

impl Report {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            sections: Vec::new(),
        }
    }

    /// Add a section and return a mutable reference for chaining fields.
    pub fn section(&mut self, name: &str) -> &mut Section {
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
    pub fn print(&self) {
        let supports_ansi = std::io::stdout().is_terminal()
            && std::env::var_os("NO_COLOR").is_none()
            && !matches!(std::env::var("CLICOLOR").ok().as_deref(), Some("0"));

        let rule: String = "\u{2500}".repeat(self.title.len());

        if supports_ansi {
            println!(
                "{}{}{}",
                crate::output::BRAND_BOLD,
                self.title,
                crate::output::RESET
            );
        } else {
            println!("{}", self.title);
        }
        println!("{}", rule);

        let mut first = true;
        for section in &self.sections {
            if !first {
                println!();
            }
            first = false;

            println!("{}", section.name);
            for field in &section.fields {
                println!("  {}: {}", field.key, field.value);
            }
            for advice in &section.advice {
                println!("  - {}", advice);
            }
        }
    }
}

impl Section {
    /// Add a key-value field. Returns `&mut Self` for chaining.
    pub fn field(&mut self, key: &str, value: &str) -> &mut Self {
        self.fields.push(Field {
            key: key.to_string(),
            value: value.to_string(),
        });
        self
    }

    /// Add a conditional field (only if `condition` is true).
    pub fn field_if(&mut self, key: &str, value: &str, condition: bool) -> &mut Self {
        if condition {
            self.field(key, value);
        }
        self
    }

    /// Add an advisory line (printed as `  - message`).
    pub fn advice(&mut self, message: &str) -> &mut Self {
        self.advice.push(message.to_string());
        self
    }

    /// Returns true if no advisory lines have been added.
    pub fn has_advice(&self) -> bool {
        !self.advice.is_empty()
    }
}
