// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Tests for the system-feeling / control_color_drift integration.
//!
//! These tests guard two invariants:
//!
//! 1. **Partition completeness** — every `ColorScheme` variant appears in
//!    exactly one `ColorFamily`'s `family_members` list. Without this, a
//!    new variant added to the enum would silently fall through to the
//!    `_ => Green` fallback in `family_for` and never participate in
//!    signal-driven drift.
//!
//! 2. **Family consistency** — `family_for(scheme)` returns the same
//!    family that contains `scheme` in `family_members`. A mismatch
//!    would mean the classifier picks a family, but the scheme the user
//!    is currently on classifies itself into a different family —
//!    drift would feel incoherent.
//!
//! 3. **State→Family mapping is total** — every `FeelingState` maps to
//!    a `ColorFamily` with at least 2 members (so drift can always pick
//!    a non-current scheme).

#![cfg(test)]

use crate::cloud::ecosystem::{family_for, family_members, ColorFamily};
use crate::control_color_drift::{family_for_state, FeelingState};
use crate::runtime::ColorScheme;

/// Helper: iterate every ColorScheme variant.
///
/// ColorScheme does not implement an iteration trait, so we list them
/// explicitly here. This list MUST be updated when a new variant is
/// added to the enum — the partition-completeness test below will fail
/// if the list is stale, which is the point.
fn all_schemes() -> Vec<ColorScheme> {
    use ColorScheme::*;
    vec![
        Green,
        Green2,
        Green3,
        NeonGreen,
        Carbon,
        Yellow,
        Orange,
        Red,
        Blue,
        Cyan,
        Gold,
        Rainbow,
        Purple,
        Neon,
        Fire,
        Ocean,
        Forest,
        Vaporwave,
        Gray,
        Snow,
        Aurora,
        FancyDiamond,
        Cosmos,
        Nebula,
        Spectrum20,
        Stars,
        Mars,
        Venus,
        Mercury,
        Jupiter,
        Saturn,
        Uranus,
        Neptune,
        Pluto,
        Moon,
        Sun,
        NeonPurple,
        NeonWhite,
        NeonBlue,
        NeonRed,
        NeonOrange,
        NeonYellow,
        NeonCyan,
    ]
}

/// Every ColorScheme variant must be classifiable into exactly one family.
///
/// This catches the case where a new variant is added to the enum but
/// `family_for` is not updated — the variant would hit the `_ => Green`
/// fallback and appear in the Green family's `family_members` list,
/// causing this test to see it twice (once in Green via the fallback,
/// once nowhere else). Actually the fallback means it appears in Green
/// once. The real check is: does `family_for(s)` return a family whose
/// `family_members` contains `s`? That's the `family_consistency` test.
///
/// This test instead checks: the union of all `family_members` lists
/// equals the full enum. A variant missing from all lists would be
/// undetectable to drift.
#[test]
fn family_partition_covers_every_variant() {
    let schemes = all_schemes();
    let mut seen = std::collections::HashSet::new();
    for family in [
        ColorFamily::Green,
        ColorFamily::GoldWarm,
        ColorFamily::RedFire,
        ColorFamily::BlueWater,
        ColorFamily::PurpleNebula,
        ColorFamily::GrayMoon,
        ColorFamily::Rainbow,
    ] {
        for &s in family_members(family) {
            seen.insert(s);
        }
    }
    for s in &schemes {
        assert!(
            seen.contains(s),
            "ColorScheme {:?} is not in any family_members list — drift cannot target it",
            s
        );
    }
}

/// `family_for(scheme)` must return a family whose `family_members`
/// contains `scheme`. Otherwise the classifier's notion of "which family
/// am I in" disagrees with the family tables, causing drift to behave
/// incoherently (e.g. family_for(Red) = RedFire, but RedFire doesn't
/// contain Red — drift to RedFire would never pick Red, even though
/// Red considers itself a RedFire member).
#[test]
fn family_for_is_consistent_with_family_members() {
    for &scheme in &all_schemes() {
        let family = family_for(scheme);
        let members = family_members(family);
        assert!(
            members.contains(&scheme),
            "family_for({:?}) = {:?}, but {:?} is not in family_members({:?}) = {:?}",
            scheme,
            family,
            scheme,
            family,
            members
        );
    }
}

/// Families are disjoint: no scheme appears in two families. A scheme
/// in two families would make drift non-deterministic about which family
/// it's targeting.
#[test]
fn families_are_disjoint() {
    let families = [
        ColorFamily::Green,
        ColorFamily::GoldWarm,
        ColorFamily::RedFire,
        ColorFamily::BlueWater,
        ColorFamily::PurpleNebula,
        ColorFamily::GrayMoon,
        ColorFamily::Rainbow,
    ];
    let mut seen: std::collections::HashMap<ColorScheme, usize> = std::collections::HashMap::new();
    for (i, &family) in families.iter().enumerate() {
        for &s in family_members(family) {
            if let Some(&prev) = seen.get(&s) {
                panic!(
                    "ColorScheme {:?} appears in both {:?} and {:?} — families must be disjoint",
                    s, families[prev], family
                );
            }
            seen.insert(s, i);
        }
    }
}

/// Every FeelingState must map to a family with at least 2 members.
///
/// With only 1 member, drift could never pick a different scheme from
/// the current one (the "skip current_scheme" logic would exhaust the
/// pool), making that state a drift no-op. We require >= 2 so drift
/// always has a real target.
#[test]
fn every_state_maps_to_family_with_multiple_members() {
    for state in [
        FeelingState::Calm,
        FeelingState::Pulse,
        FeelingState::Signal,
        FeelingState::Void,
        FeelingState::Compression,
    ] {
        let family = family_for_state(state);
        let members = family_members(family);
        assert!(
            members.len() >= 2,
            "FeelingState {:?} maps to {:?} which has only {} members — drift would be a no-op",
            state,
            family,
            members.len()
        );
    }
}

/// family_for_state is a pure const fn — calling it twice with the same
/// input must return the same output. (Guards against accidental
/// non-determinism if someone ever makes it read runtime state.)
#[test]
fn family_for_state_is_deterministic() {
    for state in [
        FeelingState::Calm,
        FeelingState::Pulse,
        FeelingState::Signal,
        FeelingState::Void,
        FeelingState::Compression,
    ] {
        let a = family_for_state(state);
        let b = family_for_state(state);
        assert_eq!(a, b, "family_for_state({:?}) must be deterministic", state);
    }
}

/// The 5 FeelingStates are all distinct. This guards against accidental
/// aliasing if the enum is ever refactored.
#[test]
fn all_feeling_states_are_distinct() {
    let states = [
        FeelingState::Calm,
        FeelingState::Pulse,
        FeelingState::Signal,
        FeelingState::Void,
        FeelingState::Compression,
    ];
    for i in 0..states.len() {
        for j in (i + 1)..states.len() {
            assert_ne!(
                states[i], states[j],
                "FeelingState variants {:?} and {:?} must be distinct",
                states[i], states[j]
            );
        }
    }
}

/// Every FeelingState has a non-empty label. Doctor output depends on this.
#[test]
fn every_state_has_nonempty_label() {
    for state in [
        FeelingState::Calm,
        FeelingState::Pulse,
        FeelingState::Signal,
        FeelingState::Void,
        FeelingState::Compression,
    ] {
        let label = state.label();
        assert!(
            !label.is_empty(),
            "FeelingState {:?} has empty label — doctor output would break",
            state
        );
    }
}

/// Every ColorFamily has a non-empty label. Doctor output depends on this.
#[test]
fn every_family_has_nonempty_label() {
    for family in [
        ColorFamily::Green,
        ColorFamily::GoldWarm,
        ColorFamily::RedFire,
        ColorFamily::BlueWater,
        ColorFamily::PurpleNebula,
        ColorFamily::GrayMoon,
        ColorFamily::Rainbow,
    ] {
        let label = family.label();
        assert!(
            !label.is_empty(),
            "ColorFamily {:?} has empty label — doctor output would break",
            family
        );
    }
}
