// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! v4.8.0 Zactrix lab integration audit doc guards.

#[test]
fn integration_audit_doc_exists() {
    let docs = include_str!("../../docs/ZACTRIX_INTEGRATION_AUDIT.md");
    assert!(
        docs.contains("Zactrix Integration Audit"),
        "ZACTRIX_INTEGRATION_AUDIT.md must exist and describe the audit"
    );
}

#[test]
fn fifty_k_lab_doc_exists() {
    let docs = include_str!("../../docs/ZACTRIX_50K_LAB.md");
    assert!(
        docs.contains("Zactrix 50k Performance Lab") && docs.contains("zactrix-50k-lab"),
        "ZACTRIX_50K_LAB.md must exist as boundary evidence"
    );
}

#[test]
fn audit_doc_says_fifty_k_was_not_reached() {
    let audit = include_str!("../../docs/ZACTRIX_INTEGRATION_AUDIT.md");
    let lab = include_str!("../../docs/ZACTRIX_50K_LAB.md");
    assert!(
        audit.contains("50k FPS was not reached") || audit.contains("50k was not reached"),
        "audit doc must say 50k was not reached"
    );
    assert!(
        lab.contains("Gold (`50k+`): not reached")
            && lab.contains("This pass did not find a safe code change"),
        "50k lab doc must state that 50k was not reached"
    );
}

#[test]
fn audit_doc_keeps_rejected_optimizations_rejected() {
    let docs = include_str!("../../docs/ZACTRIX_INTEGRATION_AUDIT.md");
    for rejected in [
        "Frame dirty epoch stamps",
        "Monolith stale-only cleanup",
        "Edge-fade line cache",
        "Non-TTY benchmark progress elapsed gate",
    ] {
        assert!(
            docs.contains(rejected),
            "audit doc must list rejected optimization: {rejected}"
        );
    }
    assert!(
        docs.contains("Rejected 50k attempts stay rejected"),
        "audit doc must keep 50k rejected attempts rejected"
    );
}

#[test]
fn audit_doc_forbids_direct_lab_merge() {
    let docs = include_str!("../../docs/ZACTRIX_INTEGRATION_AUDIT.md");
    assert!(
        docs.contains("no direct merge from lab branches"),
        "audit doc must forbid direct lab branch merges"
    );
    assert!(
        docs.contains("cherry-pick or adapt only clean changes"),
        "audit doc must require cherry-pick/adapt integration"
    );
}

#[test]
fn audit_doc_preserves_runtime_invariants() {
    let docs = include_str!("../../docs/ZACTRIX_INTEGRATION_AUDIT.md");
    for invariant in [
        "actual_execution single-threaded-renderer",
        "terminal_writer single-owner",
        "compute_parallelism disabled",
        "active_frame_ratio 100%",
        "active_streams_avg roughly 40-42",
        "dirty ratio roughly 6.8%-7.6%",
    ] {
        assert!(
            docs.contains(invariant),
            "audit doc must preserve invariant: {invariant}"
        );
    }
}

#[test]
fn audit_doc_names_twenty_k_lab_as_candidate_source() {
    let docs = include_str!("../../docs/ZACTRIX_INTEGRATION_AUDIT.md");
    assert!(
        docs.contains("zactrix-20k-lab")
            && docs.contains("Accepted candidate source")
            && docs.contains("reduce redundant color pipeline work"),
        "audit doc must name zactrix-20k-lab as the accepted candidate source"
    );
}

#[test]
fn roadmap_marks_v48_active_and_phase4b_current() {
    let docs = include_str!("../../docs/ROADMAP.md");
    assert!(
        docs.contains("v4.8.0") && docs.contains("[ACTIVE]"),
        "ROADMAP.md must mark v4.8.0 active"
    );
    assert!(
        docs.contains("Phase 4B") && docs.contains("current"),
        "ROADMAP.md must mark v4.8 Phase 4B as current"
    );
    assert!(
        docs.contains("50k was not reached") && docs.contains("No fake benchmark progress"),
        "ROADMAP.md must document honest 50k boundary evidence"
    );
}

#[test]
fn audit_doc_mentions_phase2a_completed() {
    let docs = include_str!("../../docs/ZACTRIX_INTEGRATION_AUDIT.md");
    assert!(
        docs.contains("Phase 2A") && docs.contains("Code Integration (COMPLETE)"),
        "audit doc must record Phase 2A code integration as complete"
    );
}

#[test]
fn audit_doc_mentions_integration_commit_ce8dc81() {
    let docs = include_str!("../../docs/ZACTRIX_INTEGRATION_AUDIT.md");
    assert!(
        docs.contains("ce8dc81"),
        "audit doc must mention integration commit ce8dc81"
    );
    assert!(
        docs.contains("e7253e7"),
        "audit doc must mention source commit e7253e7"
    );
}

#[test]
fn audit_doc_mentions_terminal_writer_single_owner() {
    let docs = include_str!("../../docs/ZACTRIX_INTEGRATION_AUDIT.md");
    assert!(
        docs.contains("terminal_writer") && docs.contains("single-owner"),
        "audit doc must mention terminal_writer single-owner"
    );
}

#[test]
fn audit_doc_mentions_compute_parallelism_disabled() {
    let docs = include_str!("../../docs/ZACTRIX_INTEGRATION_AUDIT.md");
    assert!(
        docs.contains("compute_parallelism") && docs.contains("disabled"),
        "audit doc must mention compute_parallelism disabled"
    );
}

#[test]
fn audit_doc_mentions_phase2b_validation_lock() {
    let docs = include_str!("../../docs/ZACTRIX_INTEGRATION_AUDIT.md");
    assert!(
        docs.contains("Phase 2B") && docs.contains("Validation Lock"),
        "audit doc must record Phase 2B validation lock"
    );
}

#[test]
fn audit_doc_confirms_no_direct_lab_merge() {
    let docs = include_str!("../../docs/ZACTRIX_INTEGRATION_AUDIT.md");
    assert!(
        docs.contains("No direct merge from"),
        "audit doc must confirm no direct merge from lab branches"
    );
}

#[test]
fn audit_doc_mentions_merge_prep() {
    let docs = include_str!("../../docs/ZACTRIX_INTEGRATION_AUDIT.md");
    assert!(
        docs.contains("Main Merge Prep") && docs.contains("Conflict Audit"),
        "audit doc must mention Phase 3 merge prep / conflict audit"
    );
}

#[test]
fn audit_doc_mentions_no_version_bump_until_release_prep() {
    let docs = include_str!("../../docs/ZACTRIX_INTEGRATION_AUDIT.md");
    assert!(
        docs.contains("No version bump") || docs.contains("no version bump"),
        "audit doc must mention no version bump until release prep"
    );
}

#[test]
fn audit_doc_mentions_locked_benchmark_27900() {
    let docs = include_str!("../../docs/ZACTRIX_INTEGRATION_AUDIT.md");
    assert!(
        docs.contains("27,900.4") || docs.contains("27.9k"),
        "audit doc must mention locked 27,900.4 FPS benchmark"
    );
}

#[test]
fn audit_doc_mentions_merge_readiness_and_no_conflicts() {
    let docs = include_str!("../../docs/ZACTRIX_INTEGRATION_AUDIT.md");
    assert!(
        docs.contains("zero conflicts") || docs.contains("fast-forward"),
        "audit doc must mention merge-readiness (zero conflicts or fast-forward)"
    );
}

#[test]
fn terminal_kill_cleanup_doc_exists() {
    let docs = include_str!("../../docs/TERMINAL_KILL_CLEANUP.md");
    assert!(
        docs.contains("pkill -f cosmostrix")
            && docs.contains("SIGTERM")
            && docs.contains("SIGKILL"),
        "TERMINAL_KILL_CLEANUP.md must document pkill, SIGTERM, and SIGKILL"
    );
}

#[test]
fn terminal_kill_cleanup_doc_mentions_recovery() {
    let docs = include_str!("../../docs/TERMINAL_KILL_CLEANUP.md");
    assert!(
        docs.contains("printf '\\\\033c'") || docs.contains("printf '\\033c'"),
        "doc must mention printf escape recovery"
    );
    assert!(
        docs.contains("stty sane"),
        "doc must mention stty sane recovery"
    );
    assert!(
        docs.contains("--reset-terminal"),
        "doc must mention --reset-terminal recovery"
    );
}

#[test]
fn terminal_kill_cleanup_doc_separates_normal_from_reset() {
    let docs = include_str!("../../docs/TERMINAL_KILL_CLEANUP.md");
    assert!(
        docs.contains("Normal Exit") && docs.contains("--reset-terminal"),
        "doc must distinguish normal exit from --reset-terminal"
    );
}

#[test]
fn terminal_kill_cleanup_doc_mentions_no_screen_clear_on_normal_exit() {
    let docs = include_str!("../../docs/TERMINAL_KILL_CLEANUP.md");
    assert!(
        docs.contains("does NOT clear the screen") || docs.contains("does not clear the screen"),
        "doc must state normal exit does not clear screen"
    );
}

#[test]
fn terminal_kill_cleanup_doc_mentions_watchdog_fallback() {
    let docs = include_str!("../../docs/TERMINAL_KILL_CLEANUP.md");
    assert!(
        docs.contains("watchdog") && docs.contains("20"),
        "doc must mention watchdog as stuck-loop fallback"
    );
}

#[test]
fn terminal_kill_cleanup_doc_mentions_signal_exit_viewport_clear() {
    let docs = include_str!("../../docs/TERMINAL_KILL_CLEANUP.md");
    assert!(
        docs.contains("signal-exit") && docs.contains("viewport"),
        "doc must mention signal-exit viewport clear behavior"
    );
}

#[test]
fn terminal_kill_cleanup_doc_mentions_fork_guard_ppid_check() {
    let docs = include_str!("../../docs/TERMINAL_KILL_CLEANUP.md");
    assert!(
        docs.contains("ppid") || docs.contains("getppid"),
        "doc must mention fork guard ppid check to prevent race"
    );
}

#[test]
fn terminal_kill_cleanup_doc_normal_exit_non_destructive() {
    let docs = include_str!("../../docs/TERMINAL_KILL_CLEANUP.md");
    assert!(
        docs.contains("does NOT clear") || docs.contains("does not clear"),
        "doc must confirm normal exit is non-destructive"
    );
}

#[test]
fn roadmap_marks_v48_phase4b() {
    let docs = include_str!("../../docs/ROADMAP.md");
    assert!(
        docs.contains("Phase 4B") && docs.contains("current"),
        "ROADMAP.md must mark v4.8 Phase 4B as current"
    );
}
