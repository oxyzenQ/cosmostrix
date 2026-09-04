// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Shared intro infrastructure tests (pool, RNG, lerp, skip policy)
//! for the `intro_style` dispatcher module — per-style tests live in
//! their style files (`logo_tests.rs`, inline in `cosmic.rs`).

// use super::*; // not needed — tests are self-contained

use super::*;

#[test]
fn min_intro_size_allows_responsive_scaling() {
    // v25 responsive: MIN_INTRO lowered from 80×24 to 10×5 so the
    // intros can play on small terminals via dynamic art scaling
    // (see logo::scale_art). The hard floor is only for
    // absurdly tiny terminals where even a scaled-down logo would
    // be unreadable.
    assert_eq!(MIN_INTRO_COLS, 10);
    assert_eq!(MIN_INTRO_LINES, 5);
}

#[test]
fn xorshift_provides_varied_values() {
    let mut rng = XorShift::new(42);
    let a = rng.next_u32();
    let b = rng.next_u32();
    let c = rng.next_u32();
    assert_ne!(a, b, "consecutive u32 must differ");
    assert_ne!(b, c, "consecutive u32 must differ");
}

#[test]
fn xorshift_next_f32_in_unit_range() {
    let mut rng = XorShift::new(7);
    for _ in 0..1000 {
        let f = rng.next_f32();
        assert!(
            (0.0..1.0).contains(&f),
            "next_f32 returned {f}, out of [0,1)"
        );
    }
}

#[test]
fn xorshift_handles_zero_seed() {
    // Zero seed must not lock the generator.
    let mut rng = XorShift::new(0);
    let a = rng.next_u32();
    let b = rng.next_u32();
    assert_ne!(a, b);
}

#[test]
fn lerp_interpolates_correctly() {
    assert!((lerp(0.0, 10.0, 0.5) - 5.0).abs() < 1e-6);
    assert!((lerp(0.0, 10.0, 0.0) - 0.0).abs() < 1e-6);
    assert!((lerp(0.0, 10.0, 1.0) - 10.0).abs() < 1e-6);
}

#[test]
fn lerp_rgb_interpolates_correctly() {
    // With OKLab perceptual blend, the midpoint is NOT the sRGB
    // linear midpoint (50, 100, 25). But endpoints are preserved
    // and the result is between a and b.
    let a = (0u8, 0u8, 0u8);
    let b = (100u8, 200u8, 50u8);
    let mid = lerp_rgb(a, b, 0.5);
    // Must be strictly between black and b (not equal to either).
    assert!(mid.0 > 0 && mid.0 < 100);
    assert!(mid.1 > 0 && mid.1 < 200);
    assert!(mid.2 > 0 && mid.2 < 50);
}

#[test]
fn lerp_rgb_clamps_to_endpoints() {
    let a = (10u8, 20u8, 30u8);
    let b = (200u8, 100u8, 50u8);
    assert_eq!(lerp_rgb(a, b, 0.0), a);
    assert_eq!(lerp_rgb(a, b, 1.0), b);
}

#[test]
fn particle_pool_starts_full_free_list() {
    let pool = ParticlePool::new();
    assert_eq!(pool.free.len(), PARTICLE_POOL_SIZE);
    assert_eq!(pool.active_count(), 0);
}

#[test]
fn particle_pool_spawn_and_kill_roundtrip() {
    let mut pool = ParticlePool::new();
    let initial_free = pool.free.len();
    let p = Particle {
        x: 1.0,
        y: 2.0,
        vx: 0.0,
        vy: 1.0,
        ch: '*',
        r: 255,
        g: 100,
        b: 50,
        life: 0.5,
        max_life: 0.5,
        angle: 0.0,
        speed: 10.0,
        spiral_rate: 1.0,
        active: true,
    };
    assert!(pool.spawn(p));
    assert_eq!(pool.free.len(), initial_free - 1);
    assert_eq!(pool.active_count(), 1);
    pool.kill(initial_free - 1);
    assert_eq!(pool.free.len(), initial_free);
    assert_eq!(pool.active_count(), 0);
}

#[test]
fn particle_pool_spawn_fails_when_full() {
    let mut pool = ParticlePool::new();
    // Drain the entire free-list.
    for _ in 0..PARTICLE_POOL_SIZE {
        assert!(pool.spawn(Particle::INACTIVE));
    }
    assert_eq!(pool.free.len(), 0);
    // Next spawn should fail (returns false).
    assert!(!pool.spawn(Particle::INACTIVE));
}

#[test]
fn rng_freehand_returns_unit_range() {
    for _ in 0..1000 {
        let f = rng_freehand();
        assert!((0.0..1.0).contains(&f), "rng_freehand returned {f}");
    }
}

// ── skip-key policy ──────────────────────────────────────────────────
//
// The intro must ONLY skip on `q` / `Q` (case-insensitive quit) —
// every other key is drained and ignored. These tests pin the policy
// so a future "make any key skip" refactor would fail loudly.

fn key(code: crossterm::event::KeyCode) -> crossterm::event::KeyEvent {
    crossterm::event::KeyEvent::new(code, crossterm::event::KeyModifiers::NONE)
}

#[test]
fn skip_key_accepts_lowercase_q() {
    assert!(is_skip_key(&key(crossterm::event::KeyCode::Char('q'))));
}

#[test]
fn skip_key_accepts_uppercase_q() {
    assert!(is_skip_key(&key(crossterm::event::KeyCode::Char('Q'))));
}

#[test]
fn skip_key_rejects_space() {
    assert!(!is_skip_key(&key(crossterm::event::KeyCode::Char(' '))));
}

#[test]
fn skip_key_rejects_enter() {
    assert!(!is_skip_key(&key(crossterm::event::KeyCode::Enter)));
}

#[test]
fn skip_key_rejects_escape() {
    assert!(!is_skip_key(&key(crossterm::event::KeyCode::Esc)));
}

#[test]
fn skip_key_rejects_arrows() {
    use crossterm::event::KeyCode::*;
    assert!(!is_skip_key(&key(Up)));
    assert!(!is_skip_key(&key(Down)));
    assert!(!is_skip_key(&key(Left)));
    assert!(!is_skip_key(&key(Right)));
}

#[test]
fn skip_key_rejects_other_letters() {
    // Sanity: only `q` / `Q` skip — not any other letter.
    for c in ['a', 'A', 'z', 'Z', 'x', 'X', 'p', 'P'] {
        assert!(
            !is_skip_key(&key(crossterm::event::KeyCode::Char(c))),
            "char {c:?} should NOT skip the intro"
        );
    }
}

#[test]
fn skip_key_rejects_function_keys() {
    use crossterm::event::KeyCode::*;
    assert!(!is_skip_key(&key(F(1))));
    assert!(!is_skip_key(&key(F(12))));
}

#[test]
fn skip_key_rejects_tab_and_backspace() {
    use crossterm::event::KeyCode::*;
    assert!(!is_skip_key(&key(Tab)));
    assert!(!is_skip_key(&key(Backspace)));
}

// ── modifier guard (allowlist: NONE | SHIFT only) ──────────────────
//
// Super+Q / Ctrl+Q / Alt+Q must NOT skip the intro. Only bare 'q'
// or Shift+'Q' are allowed. This matches the main event loop's
// is_unmodified_or_shift() guard.

fn key_with_mod(
    code: crossterm::event::KeyCode,
    mods: crossterm::event::KeyModifiers,
) -> crossterm::event::KeyEvent {
    crossterm::event::KeyEvent::new(code, mods)
}

#[test]
fn skip_key_rejects_super_q() {
    assert!(!is_skip_key(&key_with_mod(
        crossterm::event::KeyCode::Char('q'),
        crossterm::event::KeyModifiers::SUPER
    )));
}

#[test]
fn skip_key_rejects_ctrl_q() {
    assert!(!is_skip_key(&key_with_mod(
        crossterm::event::KeyCode::Char('q'),
        crossterm::event::KeyModifiers::CONTROL
    )));
}

#[test]
fn skip_key_rejects_alt_q() {
    assert!(!is_skip_key(&key_with_mod(
        crossterm::event::KeyCode::Char('q'),
        crossterm::event::KeyModifiers::ALT
    )));
}

#[test]
fn skip_key_accepts_shift_q() {
    // Shift+Q → 'Q' with SHIFT modifier — this IS allowed (CapsLock
    // equivalent on physical keyboard).
    assert!(is_skip_key(&key_with_mod(
        crossterm::event::KeyCode::Char('Q'),
        crossterm::event::KeyModifiers::SHIFT
    )));
}

#[test]
fn skip_key_rejects_ctrl_shift_q() {
    // Ctrl+Shift+Q must NOT skip — CONTROL bit is present.
    assert!(!is_skip_key(&key_with_mod(
        crossterm::event::KeyCode::Char('Q'),
        crossterm::event::KeyModifiers::CONTROL | crossterm::event::KeyModifiers::SHIFT
    )));
}

// ── intro type bypass ────────────────────────────────────────────────
//
// `IntroType::None` must short-circuit before any rendering happens.
// This is the contract `--benchmark` and friends rely on implicitly
// (they bypass `run_interactive` entirely, but the intro entry point
// must still be a no-op for `None` so a misconfigured config file
// cannot accidentally invoke the cinematic).

#[test]
fn intro_type_none_equality_short_circuits() {
    // The run_intro early-out uses `==`. Confirm the enum supports it
    // and that `None` matches itself.
    assert!(IntroType::None == IntroType::None);
    assert!(IntroType::Logo != IntroType::None);
    assert!(IntroType::Cosmic != IntroType::None);
}
