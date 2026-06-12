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
fn roadmap_marks_v48_phase2b_current() {
    let docs = include_str!("../../docs/ROADMAP.md");
    assert!(
        docs.contains("v4.8.0") && docs.contains("[ACTIVE]"),
        "ROADMAP.md must mark v4.8.0 active"
    );
    assert!(
        docs.contains("Phase 2B (current): Validation Lock"),
        "ROADMAP.md must mark v4.8 Phase 2B as current"
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
