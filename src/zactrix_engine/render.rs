// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Render planning boundary types for Zactrix Engine.
//!
//! Cosmostrix is single-thread: terminal writer is always single-owner.

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
}

impl RenderPlan {
    pub(crate) const fn default() -> Self {
        Self {
            writer_policy: TerminalWriterPolicy::SingleOwner,
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
    fn render_plan_default_is_single_owner() {
        let p = RenderPlan::default();
        assert_eq!(p.writer_policy, TerminalWriterPolicy::SingleOwner);
    }
}
