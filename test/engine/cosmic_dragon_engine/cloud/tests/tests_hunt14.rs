// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunter-14 regression tests: deep audit of the two ORIGINAL rain
//! styles — Glyph (droplet family) and Monolith (structured family).
//!
//! Two defect classes found and closed:
//!
//! 1. Glyph warm-start free-list violation (spawn.rs
//!    `ensure_glyph_pool_and_warm_start`). The free-list contract is
//!    "contains exactly the dead droplet indices" (type_rain/glyph/spawn_logic.rs); the
//!    warm-start loop seeded ALIVE droplets by direct index
//!    (`&mut self.droplets[i]`) without popping their slots, so the
//!    invariant broke on every glyph scene entry. Under pool pressure a
//!    later spawn popped an alive index and silently overwrote a live
//!    droplet mid-fall; the old column's `col_stat.num_droplets` budget
//!    leaked permanently (death decrements only the OVERWRITTEN
//!    droplet's new column), thinning that column's rain density until
//!    the next reset or scene switch.
//!
//! 2. Monolith drawn-gen u32 wrap (monolith.rs Pass 2). The per-frame
//!    tag counter had no wrap guard — unlike its twin in frame.rs
//!    (GEN_RESET_THRESHOLD). After a 2.2-year continuous wrap the
//!    counter re-enters stale tag values; a false "redrawn this frame"
//!    match in Pass 3 skips a needed clear_cell and drops the position
//!    from the diff history with no production recovery path (the
//!    stuck-cell sweep is debug-gated).

use std::time::{Duration, Instant};

use super::make_cloud;
use crate::frame::Frame;
use crate::rain_style::RainStyle;

/// The warm-start free-list invariant: after glyph entry, no alive
/// droplet index may remain in the free list. This pin FAILS on the
/// pre-NIGHT-hunter-14 code (seeded indices 0..seed_limit stay in the
/// list while their droplets are alive).
#[test]
fn hunt14_warm_start_keeps_free_list_invariant() {
    let mut cloud = make_cloud();
    // The production trigger is a rain-style switch TO Glyph (the 'x'
    // scene cycle, or a live-reload scene change).
    cloud.transition_rain_style(RainStyle::Glyph);

    for &di in &cloud.droplet_free_list {
        assert!(
            !cloud.droplets[di].is_alive,
            "alive droplet at index {di} still occupies a free-list slot — \
             the spawn path can overwrite it and leak its column budget"
        );
    }

    // The complement: every alive droplet index is absent from the list.
    for (di, d) in cloud.droplets.iter().enumerate() {
        if d.is_alive {
            assert!(
                !cloud.droplet_free_list.contains(&di),
                "alive droplet {di} present in free list"
            );
        }
    }
}

/// Pool conservation across warm start + sustained spawn pressure:
/// free-list length + alive count == pool size, at every round. This is
/// the exact statement of the "the list holds exactly the dead indices"
/// contract — pre-fix it broke the moment the warm start seeded (30 list
/// entries + 3 alive > 30 pool slots).
#[test]
fn hunt14_warm_start_column_budget_matches_alive_count() {
    let mut cloud = make_cloud();
    cloud.transition_rain_style(RainStyle::Glyph);

    let pool_size = cloud.droplets.len();
    let sum: usize = cloud.col_stat.iter().map(|c| c.num_droplets as usize).sum();
    let alive = cloud.droplets.iter().filter(|d| d.is_alive).count();
    assert_eq!(
        sum, alive,
        "budget must equal alive count right after warm start"
    );
    assert!(
        alive >= 3,
        "warm start must seed visible rain (got {alive})"
    );
    assert_eq!(
        cloud.droplet_free_list.len() + alive,
        pool_size,
        "free-list + alive must conserve the pool right after warm start"
    );

    // Pool pressure: hammer the spawn path with a huge rate budget for
    // many rounds (no advance pass runs, so nothing dies and the free
    // list only drains). The conservation law must hold after every
    // round; column budgets must always equal the alive count.
    let start = Instant::now();
    for round in 0..64 {
        cloud.last_spawn_time = start - Duration::from_secs(1);
        cloud.droplets_per_sec = 10_000.0;
        cloud.spawn_droplets(start, 1.0);

        for &di in &cloud.droplet_free_list {
            assert!(
                !cloud.droplets[di].is_alive,
                "round {round}: alive droplet {di} re-entered the free list"
            );
        }
        assert_eq!(
            cloud.droplet_free_list.len() + cloud.droplets.iter().filter(|d| d.is_alive).count(),
            pool_size,
            "round {round}: pool conservation broken"
        );
        let sum: usize = cloud.col_stat.iter().map(|c| c.num_droplets as usize).sum();
        let alive = cloud.droplets.iter().filter(|d| d.is_alive).count();
        assert_eq!(
            sum, alive,
            "round {round}: column budget drifted from alive count"
        );
    }
}

/// Pool exhaustion terminates cleanly (the `else break` arm): no panic,
/// no index reuse while alive, and column budgets stay exact even when
/// the spawn gates saturate (per-column caps stop the drain before the
/// list is empty — the conservation law, not emptiness, is the contract).
#[test]
fn hunt14_warm_start_exhaustion_terminates_cleanly() {
    let mut cloud = make_cloud();
    cloud.transition_rain_style(RainStyle::Glyph);

    let pool_size = cloud.droplets.len();
    let start = Instant::now();
    for _ in 0..32 {
        cloud.last_spawn_time = start - Duration::from_secs(1);
        cloud.droplets_per_sec = 10_000.0;
        cloud.spawn_droplets(start, 1.0);
        assert_eq!(
            cloud.droplet_free_list.len() + cloud.droplets.iter().filter(|d| d.is_alive).count(),
            pool_size,
            "pool conservation broken under spawn pressure"
        );
    }
    let alive = cloud.droplets.iter().filter(|d| d.is_alive).count();
    assert!(alive > 0, "pool should be heavily populated");
    let sum: usize = cloud.col_stat.iter().map(|c| c.num_droplets as usize).sum();
    assert_eq!(sum, alive, "column budgets must equal alive count");
}

/// The drawn-gen wrap guard: forcing the counter to the u32 edge must
/// fold it back to 1 with all stale tags zeroed. Pre-fix, the counter
/// wrapped to 0 while old tags survived at values > 0 — stale tags that
/// could false-match a future counter pass and skip a needed clear.
#[test]
fn hunt14_monolith_drawn_gen_wrap_guard_resets_stale_tags() {
    use super::tests_monolith::{make_monolith_cloud, run_frames};

    let mut cloud = make_monolith_cloud(48, 18);
    let mut frame = Frame::new(48, 18, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 3, 16);

    // Stale tags from the warm-up frames exist (counter 1..=3).
    let counter_before = cloud.monolith_rain.drawn_gen_counter_for_test();
    assert!((1..=4).contains(&counter_before));

    // Force the counter to the wrap edge and draw past it.
    cloud
        .monolith_rain
        .force_drawn_gen_counter_for_test(u32::MAX);
    run_frames(&mut cloud, &mut frame, 2, 16);

    let counter = cloud.monolith_rain.drawn_gen_counter_for_test();
    // u32::MAX -> wrap -> guard folds to 1 -> next frame bumps to 2.
    assert_eq!(counter, 2, "wrap must fold the counter to 1 (then bump)");
    for &tag in cloud.monolith_rain.drawn_gen_tags_for_test() {
        assert!(
            tag <= counter,
            "stale tag {tag} survived the wrap (counter {counter}) — \
             Pass 3 could false-match and skip a needed clear"
        );
    }
}

/// Behavioral pin across the wrap: when every stream deactivates, the
/// next draw must clear ALL previously drawn cells (no false
/// "redrawn this frame" matches — the residue class Pass 3 exists to
/// prevent).
#[test]
fn hunt14_monolith_clears_vacated_cells_across_gen_wrap() {
    use super::tests_monolith::{make_monolith_cloud, run_frames, visible_cell_count};

    let mut cloud = make_monolith_cloud(48, 18);
    let mut frame = Frame::new(48, 18, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 4, 16);
    assert!(
        visible_cell_count(&frame) > 0,
        "precondition: monolith drew content"
    );

    cloud
        .monolith_rain
        .force_drawn_gen_counter_for_test(u32::MAX);
    // Deactivate every stream WITHOUT re-arming the spawner (direct
    // rain_at call with a zero spawn budget — run_frames would respawn
    // streams and redraw content).
    cloud.monolith_rain.deactivate_all_for_test();
    cloud.spawn_remainder = 0.0;
    let now = Instant::now();
    cloud.last_spawn_time = now;
    cloud.last_phosphor_time = now;
    cloud.rain_at(&mut frame, now);
    frame.clear_dirty();
    // Second frame with zero budget: pure decay/cleanup pass.
    cloud.rain_at(&mut frame, now + Duration::from_millis(16));

    assert_eq!(
        visible_cell_count(&frame),
        0,
        "vacated cells must clear across the wrap — diff history residue"
    );
}
