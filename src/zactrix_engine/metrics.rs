// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Zactrix diagnostic labels — single-thread reality.

#![allow(dead_code)]

pub(crate) const ENGINE_ARCHITECTURE_LABEL: &str =
    "single-thread renderer, single-owner terminal writer";
pub(crate) const TERMINAL_WRITER_SINGLE_OWNER: &str = "single-owner";
pub(crate) const ACTUAL_EXECUTION_SINGLE_THREADED: &str = "single-threaded-renderer";
pub(crate) const RENDER_PLAN_SINGLE_OWNER: &str = "single-owner";
pub(crate) const SYSTEM_ARCHITECTURE_SUMMARY: &str = "v5.0.4: single-thread architecture. No parallel compute. Terminal writer single-owner. Optimized for single-core execution.";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostic_labels_are_non_empty() {
        assert!(!ENGINE_ARCHITECTURE_LABEL.is_empty());
        assert!(!TERMINAL_WRITER_SINGLE_OWNER.is_empty());
        assert!(!ACTUAL_EXECUTION_SINGLE_THREADED.is_empty());
        assert!(!RENDER_PLAN_SINGLE_OWNER.is_empty());
        assert!(!SYSTEM_ARCHITECTURE_SUMMARY.is_empty());
    }
    #[test]
    fn engine_label_mentions_single_thread() {
        assert!(ENGINE_ARCHITECTURE_LABEL.contains("single-thread"));
    }
    #[test]
    fn terminal_writer_single_owner_label_is_correct() {
        assert!(TERMINAL_WRITER_SINGLE_OWNER.contains("single-owner"));
    }
    #[test]
    fn actual_execution_label_is_single_threaded() {
        assert!(ACTUAL_EXECUTION_SINGLE_THREADED.contains("single-threaded"));
    }
    #[test]
    fn system_summary_preserves_single_thread_semantics() {
        assert!(SYSTEM_ARCHITECTURE_SUMMARY.contains("single-thread"));
        assert!(SYSTEM_ARCHITECTURE_SUMMARY.contains("single-owner"));
    }
}
