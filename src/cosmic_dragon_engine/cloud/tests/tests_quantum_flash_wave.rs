// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Bounded flash wave pool regression tests.
//!
//! Extracted from `tests_quantum.rs` as a pre-emptive split to keep both
//! files below the 1500-LOC guard (`scripts/check-rs-loc.sh`). The 4 tests
//! here cover the v30 bounded-pool fix: `Cloud::set_mouse_click` activates
//! a pool slot instead of overwriting a single `Option<Instant>`.
//!
//! The `make_truecolor_cloud` helper is duplicated from
//! `tests_quantum.rs` because each test module compiles its own private
//! copy; cross-module sharing would require a `tests_common.rs` shim
//! (deferred — the helper is small and stable).

use super::super::Cloud;
use crate::constants::{MOUSE_FLASH_DURATION_SECS, MOUSE_FLASH_POOL_SIZE};
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

/// Build a TrueColor cloud so palette body colors are distinct per scheme.
/// Duplicated from `tests_quantum.rs::make_truecolor_cloud` (kept private
/// to each test module to avoid a shared `tests_common` dependency).
fn make_truecolor_cloud(scheme: ColorScheme) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::TrueColor,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        scheme,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(20, 10);
    cloud
}

// ─── v30 fix: bounded flash wave pool regression tests ──────────────────────
//
// Before the v30 fix, `Cloud::set_mouse_click` overwrote a single
// `flash_time: Option<Instant>` slot on every click — the second click of a
// rapid double-click reset the in-flight wave's elapsed clock to zero,
// restarting the wave from its origin instead of letting it complete its
// release. The fix replaces the single slot with a bounded pool of
// `MOUSE_FLASH_POOL_SIZE` slots; each click activates a new slot (or evicts
// the oldest active slot when the pool is full).
//
// These tests verify the pool-state invariants directly on `Cloud::flash_waves`
// (a `[FlashWave; MOUSE_FLASH_POOL_SIZE]` array). The visual renderer
// (`droplet.rs`) iterates this array via `DrawCtx::flash_waves` — its
// correctness is covered by existing visual-depth tests, which now build
// `DrawCtx` with `flash_waves: &[]` (no active waves) per the v30 fix.

/// Single click activates exactly one pool slot.
#[test]
fn flash_wave_pool_single_click_activates_one_slot() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    cloud.set_mouse_click(5, 5);
    let active = cloud.flash_waves.iter().filter(|w| w.active).count();
    assert_eq!(
        active, 1,
        "single click must activate exactly 1 slot (got {active})"
    );
    let w = cloud
        .flash_waves
        .iter()
        .find(|w| w.active)
        .expect("at least one active slot");
    assert_eq!(w.col, 5);
    assert_eq!(w.line, 5);
}

/// Double-click activates TWO slots — the regression that motivated the fix.
/// Old behavior: second click overwrote the first slot → only 1 active wave.
/// New behavior: second click activates a new slot → 2 active waves coexist.
#[test]
fn flash_wave_pool_double_click_keeps_both_waves() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    cloud.set_mouse_click(5, 5);
    cloud.set_mouse_click(10, 10);
    let active = cloud.flash_waves.iter().filter(|w| w.active).count();
    assert_eq!(
        active, 2,
        "double-click must keep both waves active (got {active}) — this is the v30 fix regression"
    );
    // Both waves should have distinct click origins.
    let active_origins: Vec<(u16, u16)> = cloud
        .flash_waves
        .iter()
        .filter(|w| w.active)
        .map(|w| (w.col, w.line))
        .collect();
    assert!(
        active_origins.contains(&(5, 5)),
        "first click origin (5,5) must still be active: {active_origins:?}"
    );
    assert!(
        active_origins.contains(&(10, 10)),
        "second click origin (10,10) must be active: {active_origins:?}"
    );
}

/// Rapid clicks up to the pool cap fill every slot; cap+1 evicts OLDEST.
#[test]
fn flash_wave_pool_overflow_evicts_oldest() {
    let mut cloud = make_truecolor_cloud(ColorScheme::Green);
    // Fill the pool exactly.
    for i in 0..MOUSE_FLASH_POOL_SIZE {
        cloud.set_mouse_click(i as u16, i as u16);
    }
    let active = cloud.flash_waves.iter().filter(|w| w.active).count();
    assert_eq!(
        active, MOUSE_FLASH_POOL_SIZE,
        "pool must be full after {MOUSE_FLASH_POOL_SIZE} clicks (got {active})"
    );
    // The oldest wave (origin (0,0)) must still be present.
    assert!(
        cloud
            .flash_waves
            .iter()
            .any(|w| w.active && w.col == 0 && w.line == 0),
        "oldest wave (0,0) must still be active before overflow"
    );
    // One more click — overflow. Oldest (0,0) must be evicted.
    cloud.set_mouse_click(99, 99);
    let active = cloud.flash_waves.iter().filter(|w| w.active).count();
    assert_eq!(
        active, MOUSE_FLASH_POOL_SIZE,
        "pool must still be at cap after overflow (got {active})"
    );
    assert!(
        !cloud
            .flash_waves
            .iter()
            .any(|w| w.active && w.col == 0 && w.line == 0),
        "oldest wave (0,0) must be evicted after overflow"
    );
    assert!(
        cloud
            .flash_waves
            .iter()
            .any(|w| w.active && w.col == 99 && w.line == 99),
        "new click (99,99) must be active after overflow"
    );
}

/// Pool cap matches the documented constant.
#[test]
fn flash_wave_pool_size_constant_is_reasonable() {
    // Sanity: pool must hold at least 2 (else double-click always evicts)
    // and at most 8 (more than enough for any rapid-click scenario within
    // the 1.8s window — beyond that the visual would be unreadable).
    assert!(
        (2..=8).contains(&MOUSE_FLASH_POOL_SIZE),
        "MOUSE_FLASH_POOL_SIZE = {MOUSE_FLASH_POOL_SIZE} is outside [2, 8] — adjust if intentional"
    );
    // Compile-time check: duration must be positive. Using `const _: ()`
    // pattern avoids clippy::assertions_on_constants while still catching
    // accidental zero/negative values at build time.
    const _: () = assert!(MOUSE_FLASH_DURATION_SECS > 0.0);
}
