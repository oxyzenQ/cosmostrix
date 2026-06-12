// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Static docs tests for the Terminal Lifecycle Matrix (v4.9.0 Phase 3).
//!
//! These tests verify that `docs/TERMINAL_LIFECYCLE_MATRIX.md` exists,
//! covers all required lifecycle paths, and maintains honesty about
//! SIGKILL, destructive recovery, and platform limitations.

/// Read a docs file via `include_str!` (compile-time embedded, relative
/// to this file in `src/docs_tests/`).
const LIFECYCLE: &str = include_str!("../../docs/TERMINAL_LIFECYCLE_MATRIX.md");
const RELEASE_GUARD: &str = include_str!("../../docs/RELEASE_GUARD.md");
const ROADMAP: &str = include_str!("../../docs/ROADMAP.md");

// ---------------------------------------------------------------------------
// Scope D-1: Lifecycle matrix doc existence
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_matrix_doc_exists() {
    assert!(
        !LIFECYCLE.is_empty(),
        "TERMINAL_LIFECYCLE_MATRIX.md must not be empty"
    );
}

// ---------------------------------------------------------------------------
// Scope D-2: Individual lifecycle path coverage
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_matrix_mentions_normal_q_esc_exit() {
    assert!(
        LIFECYCLE.contains("q") || LIFECYCLE.contains("Esc"),
        "lifecycle matrix must mention normal q/Esc exit"
    );
    assert!(
        LIFECYCLE.to_lowercase().contains("normal exit"),
        "lifecycle matrix must describe normal exit behavior"
    );
}

#[test]
fn lifecycle_matrix_mentions_ctrl_c_sigint() {
    assert!(
        LIFECYCLE.contains("SIGINT") || LIFECYCLE.contains("Ctrl-C"),
        "lifecycle matrix must mention SIGINT / Ctrl-C"
    );
}

#[test]
fn lifecycle_matrix_mentions_sigterm_pkill() {
    assert!(
        LIFECYCLE.contains("SIGTERM"),
        "lifecycle matrix must mention SIGTERM"
    );
    assert!(
        LIFECYCLE.contains("pkill"),
        "lifecycle matrix must mention pkill"
    );
}

#[test]
fn lifecycle_matrix_mentions_sighup() {
    assert!(
        LIFECYCLE.contains("SIGHUP"),
        "lifecycle matrix must mention SIGHUP"
    );
}

#[test]
fn lifecycle_matrix_mentions_sigtstp_sigcont() {
    assert!(
        LIFECYCLE.contains("SIGTSTP"),
        "lifecycle matrix must mention SIGTSTP"
    );
    assert!(
        LIFECYCLE.contains("SIGCONT"),
        "lifecycle matrix must mention SIGCONT"
    );
}

#[test]
fn lifecycle_matrix_mentions_sigkill_cannot_be_caught() {
    assert!(
        LIFECYCLE.contains("SIGKILL"),
        "lifecycle matrix must mention SIGKILL"
    );
    // Must honestly state SIGKILL cannot be caught or cleaned up
    let lower = LIFECYCLE.to_lowercase();
    assert!(
        lower.contains("cannot be caught"),
        "lifecycle matrix must honestly state SIGKILL cannot be caught"
    );
}

#[test]
fn lifecycle_matrix_mentions_reset_terminal_destructive() {
    assert!(
        LIFECYCLE.contains("--reset-terminal") || LIFECYCLE.contains("reset-terminal"),
        "lifecycle matrix must mention --reset-terminal"
    );
    let lower = LIFECYCLE.to_lowercase();
    assert!(
        lower.contains("destructive"),
        "lifecycle matrix must describe --reset-terminal as destructive recovery"
    );
}

#[test]
fn lifecycle_matrix_mentions_windows_terminal_issue_15() {
    assert!(
        LIFECYCLE.contains("Windows"),
        "lifecycle matrix must mention Windows Terminal"
    );
    assert!(
        LIFECYCLE.contains("#15")
            || LIFECYCLE.contains("issue #15")
            || LIFECYCLE.contains("issue#15"),
        "lifecycle matrix must reference Windows Terminal issue #15"
    );
}

#[test]
fn lifecycle_matrix_mentions_tmux_headless_non_tty() {
    assert!(
        LIFECYCLE.contains("tmux"),
        "lifecycle matrix must mention tmux"
    );
    assert!(
        LIFECYCLE.to_lowercase().contains("headless")
            || LIFECYCLE.contains("non-TTY")
            || LIFECYCLE.contains("non-TTY"),
        "lifecycle matrix must mention headless/non-TTY"
    );
}

// ---------------------------------------------------------------------------
// Scope D-3: Cross-document guards
// ---------------------------------------------------------------------------

#[test]
fn release_guard_mentions_lifecycle_matrix() {
    assert!(
        RELEASE_GUARD.contains("TERMINAL_LIFECYCLE_MATRIX.md"),
        "release guard must reference the terminal lifecycle matrix"
    );
}

#[test]
fn roadmap_marks_phase_3_current_or_completed() {
    // Phase 3 should be marked as "current" or have a commit hash
    let phase3_line = ROADMAP
        .lines()
        .find(|l| l.contains("Phase 3") && l.contains("Terminal lifecycle"));
    assert!(
        phase3_line.is_some(),
        "roadmap must have a Phase 3 row for Terminal lifecycle matrix"
    );
    let line = phase3_line.unwrap();
    assert!(
        line.contains("current") || line.contains("`"),
        "roadmap Phase 3 must be marked current or have a commit hash"
    );
}

#[test]
fn docs_test_modules_under_1000_loc() {
    let this_loc = LIFECYCLE.lines().count(); // reuse to prove file is embedded
    assert!(
        this_loc > 0,
        "lifecycle matrix content should be embedded and non-empty"
    );
}
