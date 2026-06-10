// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Bounded generation-aware cache policy for Cosmostrix.
//!
//! Zactrix Cache is an internal deterministic cache discipline. It provides
//! bounded, generation-aware caching with explicit invalidation events.
//! It is not a public API. It does not cache terminal output strings.
//! It is a policy module tested through unit tests.

// Phase 1: Module-level dead_code allow is required because all cache types
// are pub(crate) API contracts consumed only in tests — not yet wired into the
// hot rendering path. When the cache is integrated, most items will become
// live and individual allows can be removed.
#![allow(dead_code)]

// ── Invalidation Events ────────────────────────────────────────────────────

/// Events that trigger cache generation invalidation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use]
pub(crate) enum InvalidationEvent {
    /// Terminal window dimensions changed.
    Resize,
    /// Color scheme or color mode changed.
    ColorChange,
    /// Character set or preset changed.
    CharsetChange,
    /// Active scene changed (x/X cycle).
    SceneSwitch,
    /// User profile was applied.
    ProfileApply,
    /// Terminal mode changed (e.g., mouse enable/disable).
    TerminalModeChange,
    /// Atmosphere regime transitioned (future use).
    AtmosphereRegimeChange,
}

impl InvalidationEvent {
    /// Human-readable label for diagnostics.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Resize => "resize",
            Self::ColorChange => "color-change",
            Self::CharsetChange => "charset-change",
            Self::SceneSwitch => "scene-switch",
            Self::ProfileApply => "profile-apply",
            Self::TerminalModeChange => "terminal-mode-change",
            Self::AtmosphereRegimeChange => "atmosphere-regime-change",
        }
    }
}

// ── Cache Generation ──────────────────────────────────────────────────────

/// Generation identifier for cache entries.
///
/// Each invalidation event bumps the generation. Cached entries from older
/// generations are considered stale and not reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CacheGeneration {
    id: u64,
}

impl CacheGeneration {
    /// Create the initial generation (id = 0).
    pub(crate) const fn initial() -> Self {
        Self { id: 0 }
    }

    /// Create a generation from a raw id. Used for testing.
    pub(crate) const fn from_id(id: u64) -> Self {
        Self { id }
    }

    /// Get the raw generation id.
    pub(crate) const fn id(self) -> u64 {
        self.id
    }

    /// Advance to the next generation. Returns the new generation.
    #[must_use]
    pub(crate) fn advance(&mut self) -> Self {
        self.id = self.id.saturating_add(1);
        *self
    }
}

// ── Cache Policy ──────────────────────────────────────────────────────────

/// Bounded cache policy configuration.
///
/// Defines the rules for cache admission and validity. The policy is
/// deterministic and generation-aware.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CachePolicy {
    /// Hard upper bound on cache entries.
    pub max_entries: usize,
    /// Current generation identifier.
    pub generation: CacheGeneration,
    /// Current number of entries in the cache.
    pub entry_count: usize,
}

impl CachePolicy {
    /// Create a new cache policy with the given max_entries.
    pub(crate) const fn new(max_entries: usize) -> Self {
        Self {
            max_entries,
            generation: CacheGeneration::initial(),
            entry_count: 0,
        }
    }

    /// Default cache policy with 256 max entries.
    pub(crate) const fn default_policy() -> Self {
        Self::new(256)
    }

    /// Check whether a cached entry with the given generation is still valid.
    pub(crate) fn is_generation_current(&self, cached_generation: CacheGeneration) -> bool {
        cached_generation.id() == self.generation.id()
    }

    /// Check whether a new entry can be admitted without exceeding bounds.
    pub(crate) fn should_admit(&self, current_entry_count: usize) -> bool {
        current_entry_count < self.max_entries
    }

    /// Invalidate the cache for the given event. Bumps generation and
    /// conceptually clears entries. Returns the previous generation id.
    pub(crate) fn invalidate(&mut self, _event: InvalidationEvent) -> u64 {
        let previous = self.generation.id();
        let _ = self.generation.advance();
        self.entry_count = 0;
        previous
    }

    /// Get the number of evictions needed to admit one entry at the given
    /// count. Returns 0 if admission is possible, or the excess count.
    pub(crate) fn evictions_needed(&self, current_entry_count: usize) -> usize {
        if current_entry_count < self.max_entries {
            0
        } else {
            current_entry_count - self.max_entries + 1
        }
    }

    /// Check if the cache is within bounds (entry_count <= max_entries).
    pub(crate) fn is_within_bounds(&self, current_entry_count: usize) -> bool {
        current_entry_count <= self.max_entries
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_generation_is_zero() {
        let gen = CacheGeneration::initial();
        assert_eq!(gen.id(), 0);
    }

    #[test]
    fn advance_increments_generation() {
        let mut gen = CacheGeneration::initial();
        let _ = gen.advance();
        assert_eq!(gen.id(), 1);
        let _ = gen.advance();
        assert_eq!(gen.id(), 2);
    }

    #[test]
    fn advance_does_not_overflow_on_saturating_add() {
        let mut gen = CacheGeneration::from_id(u64::MAX);
        let _ = gen.advance();
        assert_eq!(gen.id(), u64::MAX);
    }

    #[test]
    fn from_id_creates_correct_generation() {
        let gen = CacheGeneration::from_id(42);
        assert_eq!(gen.id(), 42);
    }

    #[test]
    fn cache_policy_initial_state() {
        let policy = CachePolicy::default_policy();
        assert_eq!(policy.max_entries, 256);
        assert_eq!(policy.generation.id(), 0);
        assert_eq!(policy.entry_count, 0);
    }

    #[test]
    fn should_admit_allows_within_bounds() {
        let policy = CachePolicy::new(4);
        assert!(policy.should_admit(0));
        assert!(policy.should_admit(3));
        assert!(!policy.should_admit(4));
    }

    #[test]
    fn should_admit_prevents_unbounded_growth() {
        let policy = CachePolicy::new(256);
        assert!(!policy.should_admit(256));
        assert!(!policy.should_admit(1000));
    }

    #[test]
    fn invalidate_bumps_generation_and_resets_count() {
        let mut policy = CachePolicy::default_policy();
        policy.entry_count = 100;

        let previous = policy.invalidate(InvalidationEvent::Resize);
        assert_eq!(previous, 0);
        assert_eq!(policy.generation.id(), 1);
        assert_eq!(policy.entry_count, 0);
    }

    #[test]
    fn consecutive_invalidations_advance_generation() {
        let mut policy = CachePolicy::default_policy();

        policy.invalidate(InvalidationEvent::ColorChange);
        assert_eq!(policy.generation.id(), 1);

        policy.invalidate(InvalidationEvent::SceneSwitch);
        assert_eq!(policy.generation.id(), 2);

        policy.invalidate(InvalidationEvent::ProfileApply);
        assert_eq!(policy.generation.id(), 3);
    }

    #[test]
    fn stale_generation_is_not_current() {
        let mut policy = CachePolicy::default_policy();
        let original = policy.generation;

        policy.invalidate(InvalidationEvent::Resize);

        assert!(!policy.is_generation_current(original));
        assert!(policy.is_generation_current(policy.generation));
    }

    #[test]
    fn all_invalidation_events_are_tested() {
        let events = [
            InvalidationEvent::Resize,
            InvalidationEvent::ColorChange,
            InvalidationEvent::CharsetChange,
            InvalidationEvent::SceneSwitch,
            InvalidationEvent::ProfileApply,
            InvalidationEvent::TerminalModeChange,
            InvalidationEvent::AtmosphereRegimeChange,
        ];
        let mut policy = CachePolicy::default_policy();
        for event in events {
            let prev = policy.generation.id();
            policy.invalidate(event);
            assert_eq!(
                policy.generation.id(),
                prev + 1,
                "event {:?} should bump generation",
                event
            );
        }
        assert_eq!(policy.generation.id(), 7);
    }

    #[test]
    fn invalidation_event_labels_are_non_empty() {
        let events = [
            InvalidationEvent::Resize,
            InvalidationEvent::ColorChange,
            InvalidationEvent::CharsetChange,
            InvalidationEvent::SceneSwitch,
            InvalidationEvent::ProfileApply,
            InvalidationEvent::TerminalModeChange,
            InvalidationEvent::AtmosphereRegimeChange,
        ];
        for event in events {
            assert!(!event.as_str().is_empty());
        }
    }

    #[test]
    fn evictions_needed_is_zero_within_bounds() {
        let policy = CachePolicy::new(10);
        assert_eq!(policy.evictions_needed(0), 0);
        assert_eq!(policy.evictions_needed(9), 0);
    }

    #[test]
    fn evictions_needed_is_correct_at_and_beyond_bounds() {
        let policy = CachePolicy::new(10);
        assert_eq!(policy.evictions_needed(10), 1);
        assert_eq!(policy.evictions_needed(11), 2);
        assert_eq!(policy.evictions_needed(15), 6);
    }

    #[test]
    fn is_within_bounds_checks_entry_count() {
        let policy = CachePolicy::new(10);
        assert!(policy.is_within_bounds(0));
        assert!(policy.is_within_bounds(10));
        assert!(!policy.is_within_bounds(11));
    }

    #[test]
    fn new_with_zero_max_entries_blocks_admission() {
        let policy = CachePolicy::new(0);
        assert!(!policy.should_admit(0));
    }

    #[test]
    fn cache_never_grows_unbounded() {
        let policy = CachePolicy::default_policy();
        assert!(!policy.should_admit(usize::MAX));
        assert_eq!(policy.max_entries, 256);
    }
}
