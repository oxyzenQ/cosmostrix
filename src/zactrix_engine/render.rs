// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Render planning boundary types for Zactrix Engine.

#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub(crate) enum TerminalWriterPolicy {
    SingleOwner,
}

impl TerminalWriterPolicy {
    pub(crate) const fn as_str(self) -> &'static str {
        "single-owner"
    }
    pub(crate) const fn default() -> Self {
        Self::SingleOwner
    }
}

#[derive(Debug, Clone, Copy)]
#[must_use]
pub(crate) struct RenderPlan {
    pub writer_policy: TerminalWriterPolicy,
    pub compute_enabled: bool,
}

impl RenderPlan {
    pub(crate) const fn default() -> Self {
        Self {
            writer_policy: TerminalWriterPolicy::SingleOwner,
            compute_enabled: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminal_writer_policy_default_is_single_owner() {
        assert_eq!(
            TerminalWriterPolicy::default(),
            TerminalWriterPolicy::SingleOwner
        );
    }
    #[test]
    fn terminal_writer_policy_label_is_single_owner() {
        assert_eq!(TerminalWriterPolicy::SingleOwner.as_str(), "single-owner");
    }
    #[test]
    fn render_plan_default_is_single_owner_no_compute() {
        let p = RenderPlan::default();
        assert_eq!(p.writer_policy, TerminalWriterPolicy::SingleOwner);
        assert!(!p.compute_enabled);
    }
}
