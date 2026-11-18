// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! Shared test-support utilities, mounted at the crate root.
//!
//! This module sits deliberately OUTSIDE the engine trees that carry
//! `#![forbid(unsafe_code)]` (night-frontier-Z5c): the one unsafe
//! operation a test harness legitimately needs — the process-global
//! environment mutation that isolates a test seat — is concentrated
//! here, behind one audited helper, instead of scattering unsafe
//! blocks through the forbidden trees. The unsafe-census gate
//! (scripts/gates/unsafe-census.py) does not scan test/ (its pin is
//! the shipped surface), so this file is the single, named home of
//! the test-side env mutation.

/// Set a process-global environment variable for test isolation.
///
/// Used by the test seats that must redirect config discovery (the
/// safepath whitelist) and skip startup validation before the test
/// harness builds a Cloud. Callers run during single-threaded test
/// setup, before any worker thread exists.
///
/// # Safety
///
/// `std::env::set_var` is unsafe under edition 2024 because a
/// concurrent reader of the environment is UB. The test harness only
/// calls this from setup paths that run before the parallel test
/// executor spawns (the same discipline the pre-existing unsafe
/// blocks in the per-seat test files documented); no other thread
/// reads the environment concurrently at that point.
pub(crate) fn set_env_var(key: &str, value: &str) {
    // SAFETY: single-threaded test setup, no concurrent env readers —
    // see the Safety note above.
    unsafe {
        std::env::set_var(key, value);
    }
}
