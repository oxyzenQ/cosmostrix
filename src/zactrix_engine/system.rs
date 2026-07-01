// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Zactrix System diagnostic model for Cosmostrix.
//!
//! Cosmostrix is single-thread by design. Compute parallelism is never
//! activated. The diagnostic model reflects this immutable architecture.

#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub(crate) enum RuntimeMode {
    Calm,
    Normal,
    Stress,
}

impl RuntimeMode {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Calm => "calm",
            Self::Normal => "normal",
            Self::Stress => "stress",
        }
    }
    pub(crate) const fn default() -> Self {
        Self::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub(crate) enum IdlePolicy {
    AdaptiveSleep,
}

impl IdlePolicy {
    pub(crate) const fn as_str(self) -> &'static str {
        "adaptive-sleep"
    }
    pub(crate) const fn default() -> Self {
        Self::AdaptiveSleep
    }
}

#[derive(Debug, Clone, Copy)]
#[must_use]
pub(crate) struct ZactrixSystemConfig {
    pub runtime_mode: RuntimeMode,
    pub idle_policy: IdlePolicy,
}

impl ZactrixSystemConfig {
    pub(crate) const fn default() -> Self {
        Self {
            runtime_mode: RuntimeMode::Normal,
            idle_policy: IdlePolicy::AdaptiveSleep,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn runtime_mode_default_is_normal() {
        assert_eq!(RuntimeMode::default(), RuntimeMode::Normal);
    }
    #[test]
    fn runtime_mode_labels_are_non_empty() {
        for m in [RuntimeMode::Calm, RuntimeMode::Normal, RuntimeMode::Stress] {
            assert!(!m.as_str().is_empty());
        }
    }
    #[test]
    fn idle_policy_default_is_adaptive_sleep() {
        assert_eq!(IdlePolicy::default(), IdlePolicy::AdaptiveSleep);
    }
    #[test]
    fn system_config_defaults_are_conservative() {
        let c = ZactrixSystemConfig::default();
        assert_eq!(c.runtime_mode, RuntimeMode::Normal);
        assert_eq!(c.idle_policy, IdlePolicy::AdaptiveSleep);
    }
}
