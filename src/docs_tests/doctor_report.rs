// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Static docs and source tests for doctor/report polish (v4.9.0 Phase 4).
//!
//! These tests verify that `--doctor` output, help text, and docs
//! correctly describe terminal lifecycle semantics without requiring
//! a TTY or subprocess.

/// Doctor source code — tested for embedded lifecycle contract strings.
const DOCTOR_SRC: &str = include_str!("../doctor.rs");

/// CLI config source — tested for help text accuracy.
const CONFIG_SRC: &str = include_str!("../config.rs");

/// Release guard doc.
const RELEASE_GUARD: &str = include_str!("../../docs/RELEASE_GUARD.md");

/// Roadmap doc.
const ROADMAP: &str = include_str!("../../docs/ROADMAP.md");

/// Terminal lifecycle matrix doc.
const LIFECYCLE: &str = include_str!("../../docs/TERMINAL_LIFECYCLE_MATRIX.md");

// ---------------------------------------------------------------------------
// Doctor output wording guards (tested via source string inclusion)
// ---------------------------------------------------------------------------

#[test]
fn doctor_mentions_destructive_reset_recovery() {
    assert!(
        DOCTOR_SRC.contains("destructive recovery"),
        "doctor source must mention destructive reset recovery"
    );
}

#[test]
fn doctor_mentions_normal_cleanup_non_destructive() {
    assert!(
        DOCTOR_SRC.contains("non-destructive"),
        "doctor source must mention normal cleanup is non-destructive"
    );
}

#[test]
fn doctor_mentions_sigkill_cannot_be_guaranteed() {
    // The doctor advice for disabled fork guard says SIGKILL may leave
    // terminal broken; the field says "cannot be caught or guaranteed"
    let src = DOCTOR_SRC.to_lowercase();
    assert!(
        src.contains("cannot be caught") || src.contains("cannot be guaranteed"),
        "doctor source must mention SIGKILL cannot be caught or guaranteed"
    );
}

#[test]
fn doctor_mentions_single_owner_terminal_writer() {
    assert!(
        DOCTOR_SRC.contains("single-owner"),
        "doctor source must mention single-owner terminal writer"
    );
}

#[test]
fn doctor_mentions_lifecycle_matrix_reference() {
    assert!(
        DOCTOR_SRC.contains("TERMINAL_LIFECYCLE_MATRIX.md"),
        "doctor source should reference the lifecycle matrix doc"
    );
}

// ---------------------------------------------------------------------------
// Help text guards
// ---------------------------------------------------------------------------

#[test]
fn reset_terminal_help_mentions_destructive() {
    let lower = CONFIG_SRC.to_lowercase();
    assert!(
        lower.contains("destructive"),
        "--reset-terminal help text must mention destructive recovery"
    );
}

// ---------------------------------------------------------------------------
// Cross-document guards
// ---------------------------------------------------------------------------

#[test]
fn docs_mention_doctor_report_polish_phase() {
    // Roadmap must have Phase 4 for Doctor/report polish
    let phase4_line = ROADMAP
        .lines()
        .find(|l| l.contains("Phase 4") && l.contains("Doctor"));
    assert!(
        phase4_line.is_some(),
        "roadmap must have Phase 4 row for Doctor/report polish"
    );
    let line = phase4_line.unwrap();
    assert!(
        line.contains("current") || line.contains("`"),
        "roadmap Phase 4 must be marked current or have a commit hash"
    );
}

#[test]
fn release_guard_links_doctor_to_lifecycle_matrix() {
    // Gate 7 step 1 should mention --doctor and lifecycle contract fields
    let gate7 = RELEASE_GUARD
        .lines()
        .skip_while(|l| !l.contains("Gate 7"))
        .take_while(|l| !l.starts_with("### Gate 8") && !l.starts_with("### Gate 9"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        gate7.contains("--doctor") && gate7.contains("lifecycle contract"),
        "release guard Gate 7 must link --doctor to lifecycle contract fields"
    );
}

#[test]
fn lifecycle_matrix_mentions_doctor_diagnostic_only() {
    assert!(
        LIFECYCLE.contains("--doctor") || LIFECYCLE.contains("doctor"),
        "lifecycle matrix must mention --doctor"
    );
    assert!(
        LIFECYCLE.to_lowercase().contains("diagnostic")
            || LIFECYCLE.to_lowercase().contains("report-only"),
        "lifecycle matrix must describe --doctor as diagnostic/report-only"
    );
}
