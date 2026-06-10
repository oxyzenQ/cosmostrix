// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Zactrix System diagnostic model for Cosmostrix.

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
pub(crate) enum CpuBudget {
    Low,
    Balanced,
    Stress,
}

impl CpuBudget {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Balanced => "balanced",
            Self::Stress => "stress",
        }
    }
    pub(crate) const fn default() -> Self {
        Self::Balanced
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub(crate) enum ComputeParallelism {
    Disabled,
    Planned,
    Active,
}

impl ComputeParallelism {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Planned => "planned",
            Self::Active => "active",
        }
    }
    pub(crate) const fn default() -> Self {
        Self::Disabled
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
    pub cpu_budget: CpuBudget,
    pub compute_parallelism: ComputeParallelism,
    pub idle_policy: IdlePolicy,
}

impl ZactrixSystemConfig {
    pub(crate) const fn default() -> Self {
        Self {
            runtime_mode: RuntimeMode::Normal,
            cpu_budget: CpuBudget::Balanced,
            compute_parallelism: ComputeParallelism::Disabled,
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
    fn cpu_budget_default_is_balanced() {
        assert_eq!(CpuBudget::default(), CpuBudget::Balanced);
    }
    #[test]
    fn cpu_budget_labels_are_non_empty() {
        for b in [CpuBudget::Low, CpuBudget::Balanced, CpuBudget::Stress] {
            assert!(!b.as_str().is_empty());
        }
    }
    #[test]
    fn compute_parallelism_default_is_disabled() {
        assert_eq!(ComputeParallelism::default(), ComputeParallelism::Disabled);
    }
    #[test]
    fn compute_parallelism_default_is_not_active() {
        assert_ne!(ComputeParallelism::default(), ComputeParallelism::Active);
    }
    #[test]
    fn idle_policy_default_is_adaptive_sleep() {
        assert_eq!(IdlePolicy::default(), IdlePolicy::AdaptiveSleep);
    }
    #[test]
    fn system_config_defaults_are_conservative() {
        let c = ZactrixSystemConfig::default();
        assert_eq!(c.runtime_mode, RuntimeMode::Normal);
        assert_eq!(c.cpu_budget, CpuBudget::Balanced);
        assert_eq!(c.compute_parallelism, ComputeParallelism::Disabled);
        assert_eq!(c.idle_policy, IdlePolicy::AdaptiveSleep);
    }
}
