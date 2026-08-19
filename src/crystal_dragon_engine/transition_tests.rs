// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! transition tests, extracted from inline `mod tests { ... }` block in
//! transition.rs (Pattern D → Pattern C unification).
//!
//! Uses `use super::*;` to access transition.rs's private items unchanged.

use super::*;
#[test]
fn no_drift_returns_none() {
    assert_eq!(CrystalDragonDrift::NoDrift.scheme(), None);
}

#[test]
fn drift_returns_scheme() {
    let drift = CrystalDragonDrift::Drift(ColorScheme::Snow);
    assert_eq!(drift.scheme(), Some(ColorScheme::Snow));
}

#[test]
fn drift_is_copy() {
    let drift = CrystalDragonDrift::Drift(ColorScheme::Fire);
    let copy = drift;
    assert_eq!(copy.scheme(), Some(ColorScheme::Fire));
}
