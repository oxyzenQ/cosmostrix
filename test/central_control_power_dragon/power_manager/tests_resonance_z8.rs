// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The control-loop resonance census — the no-limit-cycle proof for
//! the Power Dragon's coupled backoff axes (night-frontier-Z8).
//!
//! Entry 46's wave-field ledger FOUND a real resonance: under
//! alternating frame pacing the solver's amplification matrices
//! composed a ~15 percent growth per 4-frame cycle, and the record
//! names the production reach — the Power Dragon's throttle cycles
//! and adaptive pacing oscillate the frame delta. The solver got its
//! Lyapunov functional. The CONTROLLERS did not: the measured drain
//! backoff (S-master-HUNT-23), the predictive reader backoff (entry
//! 39), and the perf-pressure accumulator rise and fall on the same
//! cadence the resonance rides, and nothing asserted their
//! interaction cannot compose a limit cycle.
//!
//! This census drives the coupled loops through the adversarial
//! scenario matrix — the pacing sawtooth entry 46 measured, the byte
//! sawtooth the reader model damps, the combined storm, and the
//! thermal pulse train — and asserts the discrete-control contract
//! at every frame:
//!
//!   1. BOUNDEDNESS: every emitted control signal stays inside its
//!      documented closed interval (both backoffs in [0, 1], the
//!      pressure accumulator in [0, 1], effective_fps between the
//!      documented floor and the base) — the integrators cannot
//!      wind up, however adversarial the input.
//!   2. RELEASE: when the storm stops, every axis decays to its
//!      healthy band within the closed-form frame count the
//!      documented fall rate gives (1.0 / OUTPUT_DRAIN_BACKOFF_FALL
//!      plus margin) — the loops settle, they do not hunt.
//!   3. NO AMPLIFICATION: under sustained alternation the backoff
//!      saturates at its ceiling and stays saturated — a controlled
//!      saturation, not a growing oscillation; the amplitude cannot
//!      ratchet past the clamp.
//!
//! Honest scope, stated once: the census drives PowerManager
//! directly (the unit level, where the scenario matrix is exact and
//! deterministic); the cloud-side consumers of these signals are
//! covered by their own suites. The teeth are at the bottom: the
//! storm must be strong enough that the RAW unclamped accumulation
//! would exceed the ceiling — proving the clamp is load-bearing and
//! the boundedness assertion measures something.

use std::time::{Duration, Instant};

use crate::central_control_power_dragon::power_manager::PowerManager;
use crate::central_control_power_dragon::reader_cost::TerminalFamily;
use crate::central_control_power_dragon::{
    OUTPUT_DRAIN_BACKOFF_FALL, OUTPUT_DRAIN_BACKOFF_RISE, OUTPUT_DRAIN_FPS_FLOOR,
};

/// The census base cadence: 60 fps, the canonical virtual clock.
const BASE_FPS: f64 = 60.0;
/// One storm frame's period step in the pacing sawtooth: the cycle
/// entry 46 measured (60 -> 45 -> 30 -> 90), as milliseconds.
const SAWTOOTH_PERIODS_MS: [u64; 4] = [16, 22, 33, 11];
/// The storm's over-budget work multiplier: the frame's work is 1.3x
/// its period, so the pressure overshoot sits at 0.3 every frame.
const STORM_WORK_MULT: f32 = 1.3;
/// The storm's write overshoot (the measured-latency axis): the
/// terminal cannot drain, every storm frame.
const STORM_WRITE_OVERSHOOT: f32 = 0.5;
/// The storm's reader byte volume: twice the family's per-frame
/// budget, so the predicted parse time overshoots every storm frame.
const STORM_BYTE_MULT: u64 = 2;
/// The settle window: the documented fall rate needs at most
/// 1.0 / 0.002 = 500 frames to drain a saturated backoff; 650 gives
/// the closed-form bound 1.3x of headroom.
const SETTLE_FRAMES: u32 = 650;
/// The healthy band a settled axis must reach.
const SETTLED_BAND: f32 = 0.02;

/// One census frame: advance the clock, sample the controllers.
struct CensusFrame {
    now: Instant,
    period_s: f32,
}

/// Drive one storm frame: pacing from the sawtooth, over-budget work
/// and writes, over-budget reader bytes, plus an optional thermal
/// pulse.
fn storm_frame(
    pm: &mut PowerManager,
    frame: &CensusFrame,
    thermal: Option<f32>,
    budget_bytes: u64,
) {
    if let Some(t) = thermal {
        pm.set_thermal_pressure(t);
    }
    pm.begin_frame(frame.now);
    pm.observe_frame_end(
        frame.period_s * STORM_WORK_MULT,
        frame.period_s,
        STORM_WRITE_OVERSHOOT,
    );
    pm.observe_reader_cost(budget_bytes * STORM_BYTE_MULT, frame.period_s);
}

/// Drive one clean frame: half-budget work (no overshoot), a clean
/// write, under-budget reader bytes.
fn clean_frame(pm: &mut PowerManager, frame: &CensusFrame, budget_bytes: u64) {
    pm.begin_frame(frame.now);
    pm.observe_frame_end(frame.period_s * 0.5, frame.period_s, 0.0);
    pm.observe_reader_cost(budget_bytes / 4, frame.period_s);
}

/// Assert the boundedness contract for every emitted signal.
fn assert_bounded(pm: &PowerManager, base: f64, frame_idx: u32) {
    let drain = pm.drain_backoff();
    let reader = pm.reader_backoff_for_test();
    let pressure = pm.effective_pressure();
    let visual = pm.visual_pressure();
    let fps = pm.effective_fps(false, true);
    assert!(
        (0.0..=1.0).contains(&drain),
        "frame {frame_idx}: the measured drain backoff left [0, 1] (got \
         {drain}) — the integrator wound up; the no-limit-cycle contract \
         is broken"
    );
    assert!(
        (0.0..=1.0).contains(&reader),
        "frame {frame_idx}: the predictive reader backoff left [0, 1] (got \
         {reader}) — the reader integrator wound up"
    );
    assert!(
        (0.0..=1.0).contains(&pressure),
        "frame {frame_idx}: the pressure accumulator left [0, 1] (got \
         {pressure}) — the throttle integrator wound up"
    );
    assert!(
        (0.0..=1.0).contains(&visual),
        "frame {frame_idx}: the visual-pressure EMA left [0, 1] (got \
         {visual}) — the low-pass filter produced an out-of-band sample"
    );
    assert!(
        (OUTPUT_DRAIN_FPS_FLOOR..=base).contains(&fps),
        "frame {frame_idx}: effective_fps {fps} left the documented band \
         [{OUTPUT_DRAIN_FPS_FLOOR}, {base}] — the pacing output escaped \
         its floor/base clamp"
    );
    assert!(
        drain.is_finite() && reader.is_finite() && pressure.is_finite(),
        "frame {frame_idx}: a control signal went non-finite (drain \
         {drain}, reader {reader}, pressure {pressure}) — the adversarial \
         input found a numerical hole"
    );
}

/// INV-20-adjacent census (night-frontier-Z8): the pacing sawtooth
/// entry 46 measured — applied to the controllers themselves —
/// saturates the backoff axes inside their clamps and releases them
/// cleanly when the storm stops. No limit cycle, no windup.
#[test]
fn lock_resonance_pacing_sawtooth_settles_without_limit_cycle() {
    let family = TerminalFamily::CpuRendered;
    let budget = family.budget_bytes_per_frame(BASE_FPS);
    let t0 = Instant::now();
    let mut pm = PowerManager::new(BASE_FPS, t0);
    pm.set_reader_family(family);

    // The storm: 400 frames of the measured sawtooth pacing with
    // over-budget work, writes, and reader bytes (16 ms virtual
    // steps ride the wall clock the controllers sample).
    let mut now = t0 + Duration::from_millis(1000);
    for i in 0..400u32 {
        let period_ms = SAWTOOTH_PERIODS_MS[(i % 4) as usize];
        let period_s = period_ms as f32 / 1000.0;
        now += Duration::from_millis(16);
        let frame = CensusFrame { now, period_s };
        storm_frame(&mut pm, &frame, None, budget);
        assert_bounded(&pm, BASE_FPS, i);
    }

    // Saturation, not oscillation: under sustained alternation both
    // backoff axes sit pinned at their ceiling (a controlled
    // saturation is the bounded response; a limit cycle would hunt
    // around it).
    let drain = pm.drain_backoff();
    let reader = pm.reader_backoff_for_test();
    assert!(
        (drain - 1.0).abs() <= 1.0e-4,
        "the measured axis did not saturate under the sustained storm \
         (drain {drain}) — the rise rate or the clamp changed and the \
         census's saturation premise no longer holds"
    );
    assert!(
        (reader - 1.0).abs() <= 1.0e-4,
        "the predictive axis did not saturate under the sustained storm \
         (reader {reader}) — the reader rise rate or the clamp changed"
    );

    // The release: clean frames decay every axis into the healthy
    // band within the closed-form frame count the documented fall
    // rate gives.
    let budget_now = family.budget_bytes_per_frame(BASE_FPS);
    for i in 0..SETTLE_FRAMES {
        now += Duration::from_millis(16);
        let frame = CensusFrame {
            now,
            period_s: 1.0 / BASE_FPS as f32,
        };
        clean_frame(&mut pm, &frame, budget_now);
        assert_bounded(&pm, BASE_FPS, 400 + i);
    }
    assert!(
        pm.drain_backoff() <= SETTLED_BAND,
        "the measured axis did not release after the storm ({} against a \
         band of {SETTLED_BAND}) — the fall rate broke or the loop hunts",
        pm.drain_backoff()
    );
    assert!(
        pm.reader_backoff_for_test() <= SETTLED_BAND,
        "the predictive axis did not release after the storm ({}) — the \
         fall rate broke or the reader loop hunts",
        pm.reader_backoff_for_test()
    );
    assert!(
        pm.effective_pressure() <= SETTLED_BAND,
        "the pressure accumulator did not release after the storm ({}) — \
         the pressure decay broke or the loop hunts",
        pm.effective_pressure()
    );
    let fps = pm.effective_fps(false, true);
    assert!(
        (fps - BASE_FPS).abs() <= 0.5,
        "effective_fps did not return to the base cadence after release \
         (got {fps} against {BASE_FPS}) — the pacing loop hunts"
    );
}

/// INV-20-adjacent census (night-frontier-Z8): the byte sawtooth —
/// alternate over-budget and under-budget reader frames, the shape
/// the proactive reader model (entry 39) exists to damp — ratchets
/// the predictive axis into its clamp and releases it, with the
/// measured axis untouched (its input stays clean; the axes decay
/// independently by design).
#[test]
fn lock_resonance_byte_sawtooth_ratchets_bounded_and_releases() {
    let family = TerminalFamily::CpuRendered;
    let budget = family.budget_bytes_per_frame(BASE_FPS);
    let t0 = Instant::now();
    let mut pm = PowerManager::new(BASE_FPS, t0);
    pm.set_reader_family(family);

    let mut now = t0 + Duration::from_millis(1000);
    let mut reader_trace: Vec<f32> = Vec::new();
    for i in 0..300u32 {
        let period_s = 1.0 / BASE_FPS as f32;
        now += Duration::from_millis(16);
        let frame = CensusFrame { now, period_s };
        pm.begin_frame(frame.now);
        pm.observe_frame_end(frame.period_s * 0.5, frame.period_s, 0.0);
        // The sawtooth: over budget on odd frames, quarter budget on
        // even frames.
        let bytes = if i % 2 == 1 { budget * 3 } else { budget / 4 };
        pm.observe_reader_cost(bytes, frame.period_s);
        assert_bounded(&pm, BASE_FPS, i);
        reader_trace.push(pm.reader_backoff_for_test());
    }

    // The ratchet's honest shape: under sustained alternation the
    // rise clamps at the ceiling while the fall keeps stepping, so
    // the axis rides a PERIOD-2 RIPPLE inside [ceiling - FALL,
    // ceiling]. That ripple is the designed bounded response, not a
    // runaway limit cycle: its amplitude is exactly one fall step
    // (OUTPUT_DRAIN_BACKOFF_FALL = 0.002), it cannot grow (the rise
    // has nowhere to go past the clamp), and it collapses to zero
    // the moment the input cleans. The contract: every late sample
    // sits inside the ripple band, and the band's amplitude is
    // NON-AMPLIFYING — the late window's spread must not exceed the
    // fall step the input forces.
    let fall_step = OUTPUT_DRAIN_BACKOFF_FALL;
    let late = &reader_trace[reader_trace.len() - 40..];
    let late_spread = late.iter().cloned().fold(f32::MIN, f32::max)
        - late.iter().cloned().fold(f32::MAX, f32::min);
    for (k, v) in late.iter().enumerate() {
        assert!(
            (1.0 - fall_step - 1.0e-4..=1.0).contains(v),
            "the ratchet left the ripple band at sample {k} (backoff {v} \
             against [{} , 1.0]) — the integrator's bounded response \
             changed; re-derive the census bounds",
            1.0 - fall_step
        );
    }
    assert!(
        late_spread <= fall_step + 1.0e-4,
        "the ceiling ripple amplified (late spread {late_spread} against \
         the fall step {fall_step}) — the alternation now composes a \
         growing oscillation, the exact class this census exists to kill"
    );

    // The measured axis stayed clean (its input was clean): the
    // axes decay independently, the contract the max() merge relies
    // on.
    assert!(
        pm.drain_backoff() <= SETTLED_BAND,
        "the measured axis moved on clean writes ({}) — the axis \
         independence broke",
        pm.drain_backoff()
    );

    // The release on clean reader input.
    for i in 0..SETTLE_FRAMES {
        now += Duration::from_millis(16);
        let frame = CensusFrame {
            now,
            period_s: 1.0 / BASE_FPS as f32,
        };
        clean_frame(&mut pm, &frame, budget);
        assert_bounded(&pm, BASE_FPS, 300 + i);
    }
    assert!(
        pm.reader_backoff_for_test() <= SETTLED_BAND,
        "the predictive axis did not release on clean input ({}) — the \
         reader fall rate broke",
        pm.reader_backoff_for_test()
    );
}

/// INV-20-adjacent census (night-frontier-Z8): the combined storm —
/// pacing sawtooth, byte storm, write overshoot, and a thermal pulse
/// train TOGETHER, the worst case the production cadence can compose.
/// Every signal stays bounded and finite at every frame; the release
/// still reaches the healthy band.
#[test]
fn lock_resonance_combined_storm_stays_bounded() {
    let family = TerminalFamily::CpuRendered;
    let budget = family.budget_bytes_per_frame(BASE_FPS);
    let t0 = Instant::now();
    let mut pm = PowerManager::new(BASE_FPS, t0);
    pm.set_reader_family(family);

    let mut now = t0 + Duration::from_millis(1000);
    for i in 0..400u32 {
        let period_ms = SAWTOOTH_PERIODS_MS[(i % 4) as usize];
        let period_s = period_ms as f32 / 1000.0;
        // The thermal pulse train: an 0.8 pulse every 8th frame, 0.0
        // between (the SMC sampler's worst plausible shape).
        let thermal = if i % 8 == 0 { Some(0.8) } else { Some(0.0) };
        now += Duration::from_millis(16);
        let frame = CensusFrame { now, period_s };
        storm_frame(&mut pm, &frame, thermal, budget);
        assert_bounded(&pm, BASE_FPS, i);
    }

    // The release window runs clean (thermal input included).
    for i in 0..SETTLE_FRAMES {
        now += Duration::from_millis(16);
        let frame = CensusFrame {
            now,
            period_s: 1.0 / BASE_FPS as f32,
        };
        pm.set_thermal_pressure(0.0);
        clean_frame(&mut pm, &frame, budget);
        assert_bounded(&pm, BASE_FPS, 400 + i);
    }
    assert!(
        pm.effective_pressure() <= SETTLED_BAND && pm.visual_pressure() <= SETTLED_BAND,
        "the combined storm did not release: pressure {} visual {} against \
         a band of {SETTLED_BAND} — a loop hunts or the EMA target is stuck",
        pm.effective_pressure(),
        pm.visual_pressure()
    );
    assert!(
        pm.drain_backoff() <= SETTLED_BAND && pm.reader_backoff_for_test() <= SETTLED_BAND,
        "the backoff axes did not release after the combined storm: drain \
         {} reader {} — a fall rate broke",
        pm.drain_backoff(),
        pm.reader_backoff_for_test()
    );
}

/// The census's teeth: the storm must be strong enough that the RAW
/// unclamped accumulation would exceed the ceiling — proving the
/// clamp is load-bearing and the boundedness assertions measure a
/// real guard, not a tautology.
#[test]
fn lock_resonance_clamp_is_load_bearing() {
    let family = TerminalFamily::CpuRendered;
    let t0 = Instant::now();
    let mut pm = PowerManager::new(BASE_FPS, t0);
    pm.set_reader_family(family);

    // 30 storm frames at write overshoot 1.0: the raw rise is
    // 30 * 0.05 = 1.5, far past the 1.0 ceiling. The observed value
    // must read exactly the ceiling — the clamp engaged, and the
    // boundedness assertions in the scenarios above were guarding a
    // real edge, not an unreachable one.
    let mut now = t0 + Duration::from_millis(1000);
    for i in 0..30u32 {
        now += Duration::from_millis(16);
        let frame = CensusFrame {
            now,
            period_s: 1.0 / BASE_FPS as f32,
        };
        pm.begin_frame(frame.now);
        pm.observe_frame_end(frame.period_s * 0.5, frame.period_s, 1.0);
        assert_bounded(&pm, BASE_FPS, i);
    }
    let raw = 30.0_f32 * OUTPUT_DRAIN_BACKOFF_RISE;
    assert!(
        raw > 1.0,
        "the tooth's raw accumulation ({raw}) no longer exceeds the \
         ceiling — the storm is too weak to prove the clamp load-bearing; \
         strengthen the scenario"
    );
    assert!(
        (pm.drain_backoff() - 1.0).abs() <= 1.0e-6,
        "the clamp did not engage exactly at the ceiling (drain {}) — the \
         saturation shape changed and the census's boundedness premise \
         needs re-derivation",
        pm.drain_backoff()
    );
}
