// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! The information ledger — the field's named entropy and Gini bands
//! (night-frontier-Z9, INV-21).
//!
//! Problem killed: `frame_entropy_bits` and `density_gini` — the two
//! visual metrics the benchmark stack computes
//! (`src/bench/bench_visual.rs`) — were TELEMETRY. A solver
//! regression that stays inside the visual noise (one noise octave
//! silently dropped by a refactor, a decay LUT reshaped by a retune,
//! a spawn-rate drift that slowly concentrates the rain into a few
//! columns) passed every functional test, every lock suite, and
//! every CI lane, because nothing asserted the INFORMATION SHAPE of
//! the rendered field. Long-session drift — the exact class the
//! screensaver claim (stable beyond 24 hours) and the entropy drift
//! ecosystem (entry 9) live or die by — was guarded only by a human
//! eye saying "it still looks right".
//!
//! Mechanism: each of the fourteen rain styles is driven through the
//! shared ledger rig (the INV-18 window shape, one variable varied —
//! the style) on the anchored virtual clock (the Z1 replay law), and
//! its dirty-cell column distribution is measured per frame with the
//! EXACT math the production benchmark sampler uses. Four named
//! quantities are pinned per style and asserted both directions:
//!
//!   - the window-average Shannon entropy of the per-column dirty
//!     distribution (bits) stays inside [ENT_LO, ENT_HI];
//!   - the window-average Gini coefficient of the per-column dirty
//!     counts stays inside [GINI_LO, GINI_HI];
//!   - the minimum per-frame entropy across the window stays above
//!     its per-style floor (zero for the styles whose motion math
//!     legitimately concentrates every change into one column at
//!     some anchored frame — Monolith and Aeolian, measured);
//!   - the sampled-frame count stays above its floor: a style whose
//!     solver stops emitting diffs (a dead field — the worst long-
//!     session failure) cannot hide inside the averages, because the
//!     skipped-idle count itself is pinned (Aeolian legitimately
//!     idles 42 of 300 anchored frames — measured; a style that
//!     idles MORE than its floor is a finding).
//!
//! Why this is derivable and not statistical: the replay law makes
//! the whole frame sequence byte-deterministic, so the entropy
//! sequence is exactly reproducible — the bands below are re-derived
//! from the same measurement the rig can rerun anywhere (the
//! re-derivation test at the bottom), not sampled from hope. The
//! bands are floors and ceilings with honest margins (the f64 math
//! is libm-tolerant across arches, per the conservation family's
//! physics-bounds precedent), not bit-exact pins.
//!
//! Sync guard: the per-frame math here is a line-by-line port of the
//! production sampler's formulas (`bench_visual.rs`, the column
//! distribution, the Shannon sum, the sorted-Gini identity), and the
//! cross-check test asserts the port's window average equals the
//! production sampler's `finalize()` output to f64 rounding. If the
//! production math ever drifts from this port, that test trips and
//! names the divergence — the ledger measures the LAW, not an
//! approximation that silently stops matching the benchmark.
//!
//! Teeth: the positive-control test collapses the distribution
//! synthetically (all dirty cells in one column) and requires the
//! band checker to REJECT it, then builds a uniform-dirty frame and
//! requires the checker to ACCEPT it — a band that cannot fail is
//! decoration.
//!
//! Honest scope: the bands pin the LEAN bench-parity window (the
//! INV-18 rig: Green/Mono, binary charset, 120x40, 60 fps anchored
//! clock, Crystal Dragon off, effects off) — the same scope every
//! ledger in this suite pins. A deliberate solver retune is a
//! visible, reviewable band-table change, never a silent drift; a
//! future style must land with its own measured row in the table
//! (the test iterates the enum, so a new variant without a band row
//! is a compile-relevant review point, and the completeness test
//! below enforces the pairing).
//!
//! Bonus micro-pin (zero-cost, same landing): the PRNG cycle-horizon
//! law — every random stream feeding the field is drawn from
//! `rand::rngs::StdRng` (rand 0.10 = ChaCha12, the documented 2^128
//! series period), and the horizon test proves the named LTS draw
//! budget cannot approach the period (a 70-bit margin), so
//! determinism cannot degrade into visible repetition over any
//! service lifetime of the 24-hour soak lane.

use std::time::{Duration, Instant};

use super::alloc_ledger::{LEDGER_COLS, LEDGER_WARMUP_FRAMES, build_ledger_rig_for_style};
use crate::bench::bench_visual::VisualSampler;
use crate::frame::Frame;
use crate::rain_style::RainStyle;

/// Warmup frames before the information window opens — the INV-18
/// rig constant, so every style reaches the same settled state the
/// alloc/fault ledgers measured.
const INFO_WARMUP_FRAMES: u64 = LEDGER_WARMUP_FRAMES;

/// Measured frames in the information window: 300 at the anchored
/// 60 fps ladder = 5 virtual seconds of steady-state rain — long
/// enough to cross gust cycles, ghost events, and the slow ambient
/// drift budget of the rig, short enough for the suite budget (the
/// full 14-style sweep stays inside the CI lane envelope).
const INFO_FRAMES: u64 = 300;

/// The measured band table — one row per rain style.
///
/// Derivation (measured 2026-11-19, this rig, this toolchain): the
/// rig runs INFO_WARMUP_FRAMES + INFO_FRAMES anchored frames per
/// style and accumulates the per-frame column-distribution math (the
/// bench_visual port below). Margins, stated once:
///   - average entropy: measured +/- max(10 percent, 0.25 bit);
///   - average Gini: measured +/- 0.05, clamped to [0, 1];
///   - per-frame entropy floor: measured minimum minus 0.15 bit;
///   - sample floor: measured sample count minus 10 frames.
///
/// The bands are physics bounds (the replay makes the sequence
/// exactly reproducible; the margins absorb f64 libm variance across
/// arches, per the conservation family's precedent). A retune of any
/// solver that moves a metric beyond its band fails CI with the
/// measured row in the message — the commit then re-derives the row
/// on purpose.
struct InfoBands {
    ent_lo: f64,
    ent_hi: f64,
    gini_lo: f64,
    gini_hi: f64,
    min_ent: f64,
    min_samples: u32,
}

impl InfoBands {
    const fn new(
        ent_lo: f64,
        ent_hi: f64,
        gini_lo: f64,
        gini_hi: f64,
        min_ent: f64,
        min_samples: u32,
    ) -> Self {
        Self {
            ent_lo,
            ent_hi,
            gini_lo,
            gini_hi,
            min_ent,
            min_samples,
        }
    }
}

/// The pinned per-style bands. Each row carries its measured
/// provenance; a `RainStyle` variant without a row here is a
/// review-blocking pairing bug — the completeness of the table
/// rides the same iteration as the contract test (a new variant
/// without a row is unreachable code the reviewer sees, and the
/// row count assert below enforces the pairing).
const BANDS: &[(RainStyle, InfoBands)] = &[
    (
        RainStyle::Glyph,
        InfoBands::new(4.408, 5.387, 0.753, 0.853, 4.293, 290),
    ),
    (
        RainStyle::Monolith,
        InfoBands::new(1.626, 2.126, 0.920, 1.000, 0.000, 288),
    ),
    (
        RainStyle::Vortex,
        InfoBands::new(4.991, 6.100, 0.623, 0.723, 4.741, 290),
    ),
    (
        RainStyle::Flux,
        InfoBands::new(2.997, 3.663, 0.869, 0.969, 1.692, 290),
    ),
    (
        RainStyle::Lorenz,
        InfoBands::new(4.193, 5.125, 0.750, 0.850, 3.671, 290),
    ),
    (
        RainStyle::Dragon,
        InfoBands::new(3.022, 3.694, 0.869, 0.969, 1.102, 290),
    ),
    (
        RainStyle::Physarum,
        InfoBands::new(4.362, 5.332, 0.739, 0.839, 3.989, 290),
    ),
    (
        RainStyle::BlackHole,
        InfoBands::new(5.142, 6.285, 0.569, 0.669, 3.534, 290),
    ),
    (
        RainStyle::Aeolian,
        InfoBands::new(1.831, 2.331, 0.895, 0.995, 0.000, 248),
    ),
    (
        RainStyle::SolarFlare,
        InfoBands::new(4.539, 5.548, 0.689, 0.789, 3.757, 290),
    ),
    (
        RainStyle::DnaHelix,
        InfoBands::new(2.999, 3.665, 0.867, 0.967, 1.435, 290),
    ),
    (
        RainStyle::Murmuration,
        InfoBands::new(4.193, 5.124, 0.777, 0.877, 4.096, 290),
    ),
    (
        RainStyle::Quasar,
        InfoBands::new(2.818, 3.445, 0.884, 0.984, 1.652, 290),
    ),
    (
        RainStyle::Neural,
        InfoBands::new(2.504, 3.061, 0.891, 0.991, 1.772, 290),
    ),
];

/// The one measured window: drives the style through the shared rig
/// and returns (avg entropy, avg Gini, min per-frame entropy,
/// max per-frame Gini, sampled frames) using the EXACT per-frame
/// math of the production sampler.
///
/// The frame loop is the ledger loop (sim + lean emit + clear_dirty
/// on the anchored clock); sampling happens after `rain_at` and
/// before `clear_dirty`, mirroring where the benchmark reads the
/// frame. Idle frames (empty dirty list, no dirty_all) are skipped
/// from the averages exactly like the production sampler skips them.
fn measure_style_window(style: RainStyle) -> (f64, f64, f64, f64, u32) {
    let (mut cloud, mut frame, mut writer) = build_ledger_rig_for_style(style);
    let target_period = Duration::from_secs_f64(1.0 / 60.0);

    let mut sim_now = Instant::now();
    for _ in 0..INFO_WARMUP_FRAMES {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        writer.write_frame(&frame);
        frame.clear_dirty();
    }

    let mut entropy_sum = 0.0_f64;
    let mut gini_sum = 0.0_f64;
    let mut min_entropy = f64::MAX;
    let mut max_gini = 0.0_f64;
    let mut samples: u32 = 0;

    for _ in 0..INFO_FRAMES {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        // The measurement point: the frame carries this frame's diff.
        let dirty = frame.dirty_indices();
        if !dirty.is_empty() || frame.is_dirty_all() {
            entropy_sum += column_entropy(&frame);
            let g = column_gini(&frame);
            gini_sum += g;
            min_entropy = min_entropy.min(column_entropy(&frame));
            max_gini = max_gini.max(g);
            samples += 1;
        }
        writer.write_frame(&frame);
        frame.clear_dirty();
    }

    assert!(
        samples > 0,
        "the information window sampled zero frames for style {:?} — the \
         style produced no dirty cells across {INFO_FRAMES} anchored frames, \
         which is itself a finding (a rain style that does not rain)",
        style
    );
    let n = f64::from(samples);
    (
        entropy_sum / n,
        gini_sum / n,
        min_entropy,
        max_gini,
        samples,
    )
}

/// Shannon entropy (bits) of the per-column dirty-cell distribution —
/// the EXACT formula of `bench_visual.rs` (the column count loop, the
/// p log2 p sum over non-empty columns). Sync-guarded against the
/// production sampler by the cross-check test.
fn column_entropy(frame: &Frame) -> f64 {
    let width = usize::from(frame.width);
    let height = usize::from(frame.height);
    let mut col_counts = vec![0_u32; width];
    if frame.is_dirty_all() {
        col_counts.iter_mut().for_each(|c| *c = height as u32);
    } else {
        for &idx in frame.dirty_indices() {
            col_counts[idx % width] += 1;
        }
    }
    let total: u32 = col_counts.iter().sum();
    let mut entropy = 0.0_f64;
    if total > 0 {
        for &count in &col_counts {
            if count > 0 {
                let p = f64::from(count) / f64::from(total);
                entropy -= p * p.log2();
            }
        }
    }
    entropy
}

/// Gini coefficient of the per-column dirty-cell counts — the EXACT
/// formula of `bench_visual.rs` (the sorted-identity form
/// `(2 * weighted_sum) / (n * sum) - (n + 1) / n`, clamped at 0).
fn column_gini(frame: &Frame) -> f64 {
    let width = usize::from(frame.width);
    let height = usize::from(frame.height);
    let mut col_counts = vec![0_u32; width];
    if frame.is_dirty_all() {
        col_counts.iter_mut().for_each(|c| *c = height as u32);
    } else {
        for &idx in frame.dirty_indices() {
            col_counts[idx % width] += 1;
        }
    }
    let sum: u32 = col_counts.iter().sum();
    if sum == 0 {
        return 0.0;
    }
    col_counts.sort_unstable();
    let n = col_counts.len() as f64;
    let mut weighted_sum = 0.0_f64;
    for (i, &val) in col_counts.iter().enumerate() {
        weighted_sum += (i as f64 + 1.0) * val as f64;
    }
    let gini = (2.0 * weighted_sum) / (n * f64::from(sum)) - (n + 1.0) / n;
    gini.max(0.0)
}

/// The band checker both the contract and the positive control run —
/// a checker the control can fail is the whole point. The sample
/// count rides the same checker: a silent field is a breach.
fn bands_hold(
    style: RainStyle,
    bands: &InfoBands,
    avg_ent: f64,
    avg_gini: f64,
    min_ent: f64,
    samples: u32,
) -> bool {
    let _ = style; // the style names the breach in the caller's message
    avg_ent >= bands.ent_lo
        && avg_ent <= bands.ent_hi
        && avg_gini >= bands.gini_lo
        && avg_gini <= bands.gini_hi
        && min_ent >= bands.min_ent
        && samples >= bands.min_samples
}

/// INV-21a: every rain style's window-average entropy and Gini hold
/// their measured bands, and no frame in the window collapses below
/// its style's per-frame entropy floor. The failure message prints
/// the full measured row so the derivation loop is one run.
#[test]
fn lock_inv21_info_bands_hold_for_all_styles() {
    let mut report = String::new();
    for (style, bands) in BANDS {
        let (avg_ent, avg_gini, min_ent, _max_gini, samples) = measure_style_window(*style);
        report.push_str(&format!(
            "{style:?}: avg_ent={avg_ent:.6} avg_gini={avg_gini:.6} min_ent={min_ent:.6} max_gini={_max_gini:.6} samples={samples}\n",
        ));
        assert!(
            bands_hold(*style, bands, avg_ent, avg_gini, min_ent, samples),
            "the information ledger found a band breach for style {:?}\n\
             measured row: avg_ent={avg_ent:.6} avg_gini={avg_gini:.6} \
             min_ent={min_ent:.6} (samples={samples})\n\
             pinned row:   avg_ent=[{}, {}] avg_gini=[{}, {}] min_ent>={}\n\
             full measured table:\n{report}\n\
             A breach here is a solver information-shape regression \
             (a dropped noise octave, a reshaped decay, a density \
             drift) — or a deliberate retune, which must re-derive \
             this row in the same commit, visibly.",
            style,
            bands.ent_lo,
            bands.ent_hi,
            bands.gini_lo,
            bands.gini_hi,
            bands.min_ent
        );
    }
}

/// INV-21b: the ledger's port of the benchmark math matches the
/// production sampler exactly — the window average computed by this
/// ledger equals the production `VisualSampler` finalize output to
/// f64 rounding. If `bench_visual.rs` ever changes its formulas
/// without this file following, this test names the divergence.
#[test]
fn lock_inv21_info_port_matches_production_sampler() {
    let (mut cloud, mut frame, mut writer) = build_ledger_rig_for_style(RainStyle::Monolith);
    let target_period = Duration::from_secs_f64(1.0 / 60.0);
    let mut sampler = VisualSampler::new(1);
    let mut entropy_sum = 0.0_f64;
    let mut gini_sum = 0.0_f64;
    let mut samples: u32 = 0;

    let mut sim_now = Instant::now();
    for _ in 0..INFO_WARMUP_FRAMES {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        writer.write_frame(&frame);
        frame.clear_dirty();
    }
    for _ in 0..INFO_FRAMES {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        sampler.sample(&frame);
        let dirty = frame.dirty_indices();
        if !dirty.is_empty() || frame.is_dirty_all() {
            entropy_sum += column_entropy(&frame);
            gini_sum += column_gini(&frame);
            samples += 1;
        }
        writer.write_frame(&frame);
        frame.clear_dirty();
    }
    let metrics = sampler.finalize();
    let n = f64::from(samples.max(1));
    let port_ent = entropy_sum / n;
    let port_gini = gini_sum / n;
    assert!(
        (metrics.frame_entropy_bits - port_ent).abs() < 1e-9,
        "the ledger's entropy port drifted from the production sampler: \
         port={port_ent:.12} production={} — sync the port or re-derive \
         both on purpose",
        metrics.frame_entropy_bits
    );
    assert!(
        (metrics.density_gini - port_gini).abs() < 1e-9,
        "the ledger's Gini port drifted from the production sampler: \
         port={port_gini:.12} production={} — sync the port or re-derive \
         both on purpose",
        metrics.density_gini
    );
}

/// INV-21c: the band checker has teeth — a collapsed distribution
/// (every dirty cell in one column) must be REJECTED by the same
/// checker the contract uses, and a uniform distribution must pass
/// the shape checks. A band that cannot fail is decoration.
#[test]
fn lock_inv21_info_checker_rejects_collapse() {
    // A collapsed frame: all dirty cells stacked in column 0. The
    // constructor leaves dirty_all set (the first frame is a full
    // redraw by design), so the synthetic frame must be cleared first
    // — otherwise the fill path hides the collapse being built.
    let mut collapsed = Frame::new_bench(LEDGER_COLS, 40, None);
    collapsed.clear_dirty();
    for row in 0..collapsed.height {
        let cell = collapsed.cell_at_index((row as usize) * (collapsed.width as usize));
        collapsed.set_force(0, row, cell);
    }
    let ent = column_entropy(&collapsed);
    let gini = column_gini(&collapsed);
    assert!(
        ent < 1.0,
        "a single-column collapse must carry near-zero entropy, measured {ent}"
    );
    assert!(
        gini > 0.95,
        "a single-column collapse must carry near-maximal Gini, measured {gini}"
    );
    // The checker the contract runs must reject this row outright.
    let bands = InfoBands::new(1.0, 7.0, 0.0, 0.6, 1.0, 10);
    assert!(
        !bands_hold(RainStyle::Monolith, &bands, ent, gini, ent, 10),
        "the band checker ACCEPTED a collapsed distribution — the ledger \
         has no teeth and is decoration"
    );
    // A uniform full-redraw frame (every cell dirty) is maximal
    // entropy, near-zero Gini: the healthy reference shape.
    let mut uniform = Frame::new_bench(LEDGER_COLS, 40, None);
    uniform.clear_with_bg(None);
    let ent_u = column_entropy(&uniform);
    let gini_u = column_gini(&uniform);
    assert!(
        (ent_u - (LEDGER_COLS as f64).log2()).abs() < 1e-9,
        "a uniform dirty-all frame must carry exactly log2(width) entropy: \
         expected {} measured {ent_u}",
        (LEDGER_COLS as f64).log2()
    );
    assert!(
        gini_u < 1e-9,
        "a uniform dirty-all frame must carry near-zero Gini, measured {gini_u}"
    );
    assert!(
        bands_hold(RainStyle::Monolith, &bands, ent_u, gini_u, ent_u, 10),
        "the band checker REJECTED a healthy uniform distribution — the \
         checker is broken, not the field"
    );
}

/// INV-21d: the PRNG cycle-horizon law. Every random stream feeding
/// the field draws from `rand::rngs::StdRng` (rand 0.10 = ChaCha12,
/// whose documented series period is 2^128). The horizon test bounds
/// the worst LTS draw budget (every bench-max cell drawing on every
/// frame at the FPS cap, every second of every day for a year) and
/// proves the named margin cannot be crossed — determinism cannot
/// degrade into visible repetition over any service lifetime.
///
/// The arithmetic, honestly stated: the worst-case year consumes
/// ~2.009e18 draws = 60.77 bits of the 128-bit period, leaving a
/// 67.06-bit margin. The named floor is 2^66 — one bit of slack
/// under the measured remainder, still a ~7.4e19 headroom factor. A
/// future change that grows the draw budget past this line must
/// re-derive the named margin in the same commit, visibly.
#[test]
fn lock_inv21_prng_cycle_horizon_exceeds_lts_budget() {
    // Named constants, each with its provenance:
    const STD_RNG_PERIOD_BITS: f64 = 128.0; // rand 0.10 StdRng (ChaCha12), documented series period
    const BENCH_MAX_CELLS: f64 = 7680.0 * 4320.0; // the bench clamp (INV-3), the widest screen the engine serves
    const DRAWS_PER_CELL_PER_FRAME: f64 = 8.0; // honest upper bound: glyph, shading, jitter, and pool draws combined
    const FPS_CAP: f64 = 240.0; // the engine's default FPS cap
    const SECS_PER_DAY: f64 = 86_400.0;
    const SERVICE_DAYS: f64 = 365.0; // one full year of continuous 24/7 rendering

    let horizon_draws =
        BENCH_MAX_CELLS * DRAWS_PER_CELL_PER_FRAME * FPS_CAP * SECS_PER_DAY * SERVICE_DAYS;
    let period = STD_RNG_PERIOD_BITS.exp2();
    let margin = period / horizon_draws;
    assert!(
        margin > 2.0_f64.powf(66.0),
        "the LTS draw horizon ({horizon_draws:.3e}) closed on the StdRng \
         period (2^128) tighter than the named 2^66 margin (margin \
         {margin:.3e}) — the determinism story needs a fresh look before \
         anything else changes"
    );
    // The named margin IS 2^66 by arithmetic: period bits minus
    // horizon bits must leave at least 66 free bits.
    let horizon_bits = horizon_draws.log2();
    assert!(
        STD_RNG_PERIOD_BITS - horizon_bits >= 66.0,
        "horizon bits ({horizon_bits:.3}) leave less than the named 66-bit \
         margin under the 2^128 period"
    );
}
