// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Zactrix diagnostic labels and metric types.

#![allow(dead_code)]

pub(crate) const PARALLEL_COMPUTE_SINGLE_OWNER_LABEL: &str =
    "parallel compute, single-owner terminal writer";
pub(crate) const TERMINAL_WRITER_SINGLE_OWNER: &str = "single-owner";
pub(crate) const NO_PARALLEL_TERMINAL_WRITING: &str = "no real parallel terminal writing";
pub(crate) const ACTUAL_EXECUTION_SINGLE_THREADED: &str = "single-threaded-renderer";
pub(crate) const RENDER_PLAN_SINGLE_OWNER: &str = "single-owner";
pub(crate) const COMPUTE_PARALLELISM_DISABLED: &str = "disabled";
pub(crate) const PHASE_1_SYSTEM_SUMMARY: &str = "v4.5.0 Phase 1: architecture split / boundary definition only. No real parallel rendering. Terminal writer remains single-owner. v4.0.1 visual behavior is preserved.";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_labels_are_non_empty() {
        assert!(!PARALLEL_COMPUTE_SINGLE_OWNER_LABEL.is_empty());
        assert!(!TERMINAL_WRITER_SINGLE_OWNER.is_empty());
        assert!(!NO_PARALLEL_TERMINAL_WRITING.is_empty());
        assert!(!ACTUAL_EXECUTION_SINGLE_THREADED.is_empty());
        assert!(!RENDER_PLAN_SINGLE_OWNER.is_empty());
        assert!(!COMPUTE_PARALLELISM_DISABLED.is_empty());
        assert!(!PHASE_1_SYSTEM_SUMMARY.is_empty());
    }
    #[test]
    fn parallel_compute_label_mentions_single_owner() {
        assert!(PARALLEL_COMPUTE_SINGLE_OWNER_LABEL.contains("single-owner"));
    }
    #[test]
    fn no_parallel_terminal_writing_label_is_honest() {
        assert!(NO_PARALLEL_TERMINAL_WRITING.contains("no real parallel"));
    }
    #[test]
    fn actual_execution_label_is_single_threaded() {
        assert!(ACTUAL_EXECUTION_SINGLE_THREADED.contains("single-threaded"));
    }
    #[test]
    fn phase_1_summary_preserves_v401_behavior() {
        assert!(PHASE_1_SYSTEM_SUMMARY.contains("v4.0.1 visual behavior is preserved"));
    }
}
