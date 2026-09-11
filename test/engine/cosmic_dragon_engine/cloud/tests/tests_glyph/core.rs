// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Core glyph-style behavior contracts (NIGHT-research-26): scene
//! resolution + family classification, the head self-bloom's in-hue
//! cap (the audit's front-layer white-wash edge, retired), the
//! chroma/legacy boost parity, the cinematic layer distribution,
//! the length and tail clamps, and the sparse warm-start pool
//! lifecycle.

use super::*;

#[test]
fn glyph_scene_resolves_style_and_fields() {
    let s = crate::scene::get_scene("matrix").expect("matrix scene exists");
    assert_eq!(s.config.rain_style, RainStyle::Glyph);
    // Family classification: the droplet family's sole member (the
    // classic per-column pool renders it; every other style is a
    // structured state machine).
    assert!(RainStyle::Glyph.is_droplet_family());
    // Label round-trip (the scene-custom `rain` field surface).
    assert_eq!(RainStyle::Glyph.as_str(), "glyph");
    assert_eq!(RainStyle::from_label("glyph"), Some(RainStyle::Glyph));
    assert_eq!(RainStyle::from_label("Glyph"), Some(RainStyle::Glyph));
}

// -- NIGHT-research-26: the head self-bloom's in-hue cap --

#[test]
fn glyph_head_selfbloom_caps_in_hue() {
    // The audit's finding: the head is the soft kind by construction
    // (the palette's last stop, the self-bloom deliberately in-hue),
    // but the front layer's 0.234 x 1.20 boost clamped the
    // sub-dominant channels onto their saturated sibling — the
    // NeonGreen head stop (195, 255, 205) washed to (250, 255, 255),
    // exactly the white edge the themes' own "head stays tinted"
    // principle forbids. The NR26 cap renormalizes the boost's scale
    // against the source's own max channel: the ratios hold by
    // construction, a head already at the display edge has nowhere
    // in-hue to go, and grey sources stay bit-identical to the
    // retired equation.
    use crate::chroma_dragon_engine::legacy::boost_rgb;
    use crate::constants::{HEAD_SELFBLOOM_BASE, PARALLAX_HEAD_SELFBLOOM_MULT};

    let front = HEAD_SELFBLOOM_BASE * PARALLAX_HEAD_SELFBLOOM_MULT[2];
    // The audit's exact case: the tinted head at the display edge
    // composes to identity — no wash, no flattening, the head reads
    // its own palette stop.
    assert_eq!(
        boost_rgb(195, 255, 205, front),
        (195, 255, 205),
        "the edge-saturated tinted head must boost to identity"
    );
    // A dimmer tinted head lifts in-hue: the ratios hold, the max
    // channel anchors at the edge, the sub-dominant channels keep
    // their distance (the retired clamp would have flattened them).
    assert_eq!(boost_rgb(41, 218, 76, front), (48, 255, 89));
    // White stays white, black stays black, at every layer factor.
    for &mult in PARALLAX_HEAD_SELFBLOOM_MULT.iter() {
        let f = HEAD_SELFBLOOM_BASE * mult;
        assert_eq!(boost_rgb(255, 255, 255, f), (255, 255, 255));
        assert_eq!(boost_rgb(0, 0, 0, f), (0, 0, 0));
    }
    // Grey sources stay bit-identical to the retired clamp equation
    // (the continuity contract — the grey case clamps the same way
    // both equations).
    for c in [0u8, 40, 100, 160, 200, 250, 255] {
        for &mult in PARALLAX_HEAD_SELFBLOOM_MULT.iter() {
            let f = HEAD_SELFBLOOM_BASE * mult;
            let legacy = (c as f32 * (1.0 + f)).round().clamp(0.0, 255.0) as u8;
            let (out, _, _) = boost_rgb(c, c, c, f);
            assert_eq!(out, legacy, "grey parity broken at c={c}");
        }
    }
    // The in-hue sweep: 512 LCG-walked tinted colors, every layer
    // factor — the channel ratios survive (within one LSB of
    // rounding), no channel flattens onto its sibling, and a tinted
    // source never composes pure white.
    let mut seed: u32 = 0x26_a1_26;
    for _ in 0..512 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let r = (seed & 0xff) as u8;
        let g = ((seed >> 8) & 0xff) as u8;
        let b = ((seed >> 16) & 0xff) as u8;
        if r == g && g == b {
            continue; // grey sources carry the separate parity above
        }
        for &mult in PARALLAX_HEAD_SELFBLOOM_MULT.iter() {
            let f = HEAD_SELFBLOOM_BASE * mult;
            let (or, og, ob) = boost_rgb(r, g, b, f);
            assert_ne!(
                (or, og, ob),
                (255, 255, 255),
                "a tinted head washed to pure white"
            );
            // Ratio survival: the cross products agree within the
            // rounding slack (each output channel is the rounded
            // product, so one LSB of the larger channel is the
            // honest bound; the retired clamp's flattening broke
            // this by thousands).
            let in_min = r.min(g).min(b) as i32;
            let in_max = r.max(g).max(b) as i32;
            let out_min = or.min(og).min(ob) as i32;
            let out_max = or.max(og).max(ob) as i32;
            if in_max > 0 && out_max > 0 {
                assert!(
                    (out_min * in_max - in_min * out_max).abs() <= 2 * in_max,
                    "the boost flattened the hue: r={r} g={g} b={b} f={f}"
                );
            }
            // No channel may pass the display edge unless it WAS the
            // max channel (the renormalization anchor).
            if or == 255 {
                assert!(r == r.max(g).max(b), "a sub-dominant channel saturated");
            }
            if og == 255 {
                assert!(g == r.max(g).max(b), "a sub-dominant channel saturated");
            }
            if ob == 255 {
                assert!(b == r.max(g).max(b), "a sub-dominant channel saturated");
            }
        }
    }
}

#[test]
fn glyph_selfbloom_parity_chroma_legacy() {
    // The chroma-audit A4 contract carried forward through the NR26
    // cap: both pipelines compute the same renormalized scale —
    // bit-identical output at any color and any factor.
    use crate::chroma_dragon_engine::legacy;
    use crate::chroma_dragon_engine::palette;
    let factors = [
        0.0f32,
        HEAD_FACTOR_BACK,
        HEAD_FACTOR_MID,
        HEAD_FACTOR_FRONT,
        0.5,
        1.0,
        4.0,
    ];
    let colors = [
        (0u8, 0u8, 0u8),
        (255, 255, 255),
        (195, 255, 205),
        (235, 195, 255),
        (41, 218, 76),
        (128, 128, 200),
        (10, 0, 250),
    ];
    for &(r, g, b) in colors.iter() {
        for &f in factors.iter() {
            assert_eq!(
                legacy::boost_rgb(r, g, b, f),
                palette::boost_rgb(r, g, b, f),
                "chroma/legacy parity broken at ({r},{g},{b}) x {f}"
            );
        }
    }
}

// The three layers' effective self-bloom factors (the audit's
// 0.234 x {0.38, 0.68, 1.20} arithmetic, computed from the live
// constants so a re-tune can not silently break the pins).
use crate::constants::{HEAD_SELFBLOOM_BASE, PARALLAX_HEAD_SELFBLOOM_MULT};
const HEAD_FACTOR_BACK: f32 = HEAD_SELFBLOOM_BASE * PARALLAX_HEAD_SELFBLOOM_MULT[0];
const HEAD_FACTOR_MID: f32 = HEAD_SELFBLOOM_BASE * PARALLAX_HEAD_SELFBLOOM_MULT[1];
const HEAD_FACTOR_FRONT: f32 = HEAD_SELFBLOOM_BASE * PARALLAX_HEAD_SELFBLOOM_MULT[2];

// -- The spawn contracts (the family's real behavior) --

#[test]
fn glyph_spec_layers_lengths_and_tails_hold_their_contracts() {
    // The cinematic depth distribution ([0.35, 0.30, 0.35] — back,
    // mid, front), the length clamps (every droplet a recognizable
    // head-body-tail streak, never a degenerate bare head, never a
    // screen-saturating column), and the front layer's proportional
    // tail allocation (45 percent, capped).
    use crate::constants::{
        FRONT_LAYER_TAIL_MAX_CELLS, FRONT_LAYER_TAIL_PCT, MAX_DROPLET_LENGTH_CAP,
        MIN_DROPLET_LENGTH,
    };
    let mut cloud = make_glyph_cloud(80, 40);
    let mut layer_counts = [0usize; 3];
    let samples = 3000;
    for i in 0..samples {
        let col = (i % 80) as u16;
        let spec = cloud.build_droplet_spec(col);
        layer_counts[spec.layer as usize] += 1;
        assert!(
            (MIN_DROPLET_LENGTH..=MAX_DROPLET_LENGTH_CAP).contains(&spec.length),
            "length escaped the band: {}",
            spec.length
        );
        let expected_tail: u8 = if spec.layer == 2 {
            ((spec.length as f32 * FRONT_LAYER_TAIL_PCT).round() as u32)
                .clamp(1, FRONT_LAYER_TAIL_MAX_CELLS as u32) as u8
        } else {
            1
        };
        assert_eq!(
            spec.tail_cells, expected_tail,
            "tail contract broken at layer {}",
            spec.layer
        );
    }
    // The cinematic distribution: equal back/front share, mid
    // slightly thinner (the 20 percent band is far beyond binomial
    // noise at n = 3000).
    let total = samples as f32;
    let (back, mid, front) = (
        layer_counts[0] as f32 / total,
        layer_counts[1] as f32 / total,
        layer_counts[2] as f32 / total,
    );
    assert!(
        (back - 0.35).abs() < 0.08 && (mid - 0.30).abs() < 0.08 && (front - 0.35).abs() < 0.08,
        "layer distribution drifted: back={back} mid={mid} front={front}"
    );
}

#[test]
fn glyph_pool_warm_starts_sparse_and_exact() {
    // The scene-entry contract: the pool re-seeds a sparse set of
    // top-biased droplets (no blank first frame, no instant wall of
    // rain), the free-list keeps its exactness (dead indices only —
    // the NIGHT-hunter-14 invariant), and the column accounting
    // holds (one droplet per seeded column, the column locked).
    use crate::constants::{
        DROPLET_COUNT_FACTOR, WARM_START_SEED_FRACTION, WARM_START_SEED_MAX, WARM_START_SEED_MIN,
    };
    let mut cloud = make_glyph_cloud(120, 40);
    cloud.ensure_glyph_pool_and_warm_start();
    let pool_size = (DROPLET_COUNT_FACTOR * 120.0).round() as usize;
    assert_eq!(cloud.droplets.len(), pool_size, "pool size contract");
    let seed_limit = ((120.0 * WARM_START_SEED_FRACTION).round() as usize)
        .clamp(WARM_START_SEED_MIN, WARM_START_SEED_MAX);
    let alive = cloud.droplets.iter().filter(|d| d.is_alive).count();
    assert_eq!(alive, seed_limit, "the sparse seed count contract");
    // Free-list exactness: every remaining index is a dead droplet.
    assert_eq!(
        cloud.droplet_free_list.len(),
        pool_size - seed_limit,
        "free-list length after the warm start"
    );
    for &idx in cloud.droplet_free_list.iter() {
        assert!(
            !cloud.droplets[idx].is_alive,
            "an alive index leaked into the free list"
        );
    }
    // Column accounting: exactly the seeded columns carry a droplet
    // and are spawn-locked.
    let seeded_cols = cloud
        .col_stat
        .iter()
        .filter(|cs| cs.num_droplets == 1)
        .count();
    assert_eq!(seeded_cols, seed_limit, "one droplet per seeded column");
    // Heads top-biased: every warm-start head sits in the upper
    // quarter band (the fresh-entry read).
    let head_cap = (40u16 / 4).clamp(2, crate::constants::WARM_START_MAX_HEAD);
    for d in cloud.droplets.iter() {
        if d.is_alive {
            assert!(
                d.head_cur_line <= head_cap,
                "warm-start head escaped the top band: {}",
                d.head_cur_line
            );
        }
    }
}

#[test]
fn glyph_entry_ramp_fills_beyond_the_warm_seed() {
    // The scene-entry ramp: after the warm start the spawn rate
    // eases from its floor up to full over ~0.7 s, so the pool
    // fills well past the sparse seed within a couple of seconds —
    // the ramp is what makes the entry read "rain arriving" instead
    // of "a few streaks, then a jump cut".
    let mut cloud = make_glyph_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    cloud.ensure_glyph_pool_and_warm_start();
    let seed_alive = cloud.droplets.iter().filter(|d| d.is_alive).count();
    run_frames(&mut cloud, &mut frame, 180, 16);
    let alive = cloud.droplets.iter().filter(|d| d.is_alive).count();
    assert!(
        alive > seed_alive,
        "the ramp never filled the pool past the seed ({seed_alive} -> {alive})"
    );
    // The fill stays a sparse majority: even ramped, the classic
    // rain holds well below half the pool (the calm-sky family
    // contract — the steady state rides the density dial).
    assert!(
        (alive as f32) < 0.5 * cloud.droplets.len() as f32,
        "the ramp over-filled the pool: {alive}/{}",
        cloud.droplets.len()
    );
}
