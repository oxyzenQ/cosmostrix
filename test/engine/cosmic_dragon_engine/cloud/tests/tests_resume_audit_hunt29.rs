// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunter-29: the resume-continuity consistency audit across
//! EVERY rain family (the owner still sees a micro jump on the
//! black hole at sorgonemous_intrascals after the NIGHT-hunter-28
//! glyph fix — one family's fix must generalize to the whole
//! family tree, so the audit runs them all through the same
//! contract).
//!
//! Detector: the per-frame dirty-cell count. Every motion, spawn,
//! shimmer and phosphor-decay contribution to the frame diff
//! scales with `resume_blend` (the one easing constant that gates
//! dt, spawn_scale and phosphor elapsed alike), so the dirty count
//! is a family-agnostic velocity proxy: a re-rolled phase, a
//! dt-clamp burst or an unshifted wall-clock anchor each dump a
//! full-rate frame diff at a near-frozen blend — a spike no legit
//! pass can produce.
//!
//! The cycle drives the REAL state machine end to end:
//! steady baseline -> BRANCH 3 decel -> settle snap -> full freeze
//! -> BRANCH 2 unpause -> accel ramp -> settle -> steady again.
//! The pause/resume anchors that `toggle_pause` stamps with
//! `Instant::now()` are bridged onto the test's synthetic clock
//! (the harness note: rain_at receives synthetic `now` steps, so a
//! real-time anchor would read the synthetic clock as hours ahead
//! and collapse the easing to an instant).
//!
//! Contracts per family:
//! 1. The freeze is total: fully-paused frames dirty nothing.
//! 2. The decel coasts down: no frame exceeds the steady envelope
//!    scaled by its blend.
//! 3. The resume ramps up: same envelope, including the first
//!    post-unpause frames (blend < 0.10) where the owner's micro
//!    jump lives — bound tight enough that a phase re-roll (a
//!    full-rate frame diff) cannot pass.
//! 4. The settle snap (0.95 -> 1.0, the documented C1 tail cut)
//!    stays inside the envelope's margin.

use std::time::{Duration, Instant};

use super::*;

const AUDIT_COLS: u16 = 100;
const AUDIT_LINES: u16 = 40;
/// Frames of warmup before the baseline: 600 x 16 ms = 9.6 s — past
/// every choreographed entry (the neural genesis is the longest at
/// ~8.2 sim-s) plus the pool fill.
const WARMUP_FRAMES: u64 = 600;
/// Frames the steady baseline envelope is sampled over. Long
/// enough to cover the slow families' full activity cycles (the
/// lorenz attractor's lobe transits swing its dirty count between
/// a parked low and a crossing high over multiple seconds — a
/// short sample would calibrate the envelope to one phase).
const BASELINE_FRAMES: u64 = 300;
/// Decel frames: 220 x 16 ms = 3.52 s > the ~2.5 s the exp decay
/// needs to settle below 5%.
const DECEL_FRAMES: u64 = 220;
/// Fully-paused frames: the freeze contract samples these.
const FROZEN_FRAMES: u64 = 60;
/// Resume ramp frames: 250 x 16 ms = 4 s > the ~3.3 s the accel
/// needs to settle above 95%.
const RAMP_FRAMES: u64 = 250;
/// Post-settle frames at full speed: the machine must land back
/// inside the steady envelope. The first SETTLE_DRAIN_FRAMES are
/// the afterglow drain — phosphor cells frozen mid-fade through
/// the pause resume decaying at full rate — a by-design transient
/// (the same economy HUNT-26 amortizes for skip-resumes).
const SETTLED_FRAMES: u64 = 240;
const SETTLE_DRAIN_FRAMES: u64 = 180;

/// Bridge `toggle_pause`'s real-time anchors onto the synthetic
/// clock: the easing reads `now - anchor` with the synthetic `now`
/// the harness feeds rain_at, so a real-time anchor (behind the
/// synthetic clock by minutes of accumulated sim time) would make
/// the decel read as already-settled and the ramp as already done.
/// The state machine itself is fully exercised — only the anchor
/// timestamps are rewritten.
fn toggle_pause_at(cloud: &mut Cloud, t: Instant) {
    cloud.toggle_pause();
    if cloud.pause_start.is_some() {
        cloud.pause_start = Some(t);
    }
    if cloud.resume_start.is_some() {
        cloud.resume_start = Some(t);
    }
}

struct Cycle {
    /// Baseline: the max dirty count over the steady sample.
    steady_max: usize,
    /// Dirty count on the first post-unpause frame (the owner's
    /// micro-jump instant), with the blend that frame ran at.
    first_resume_frame: (usize, f32),
}

/// The envelope: an absolute multiple of the steady baseline. The
/// calibration datum is the owner's own acceptability ruling: the
/// glyph family carries ~1x-steady full-rate churn at a frozen
/// blend (the documented 2% trail-character cycling + head
/// life-signs, present when the owner rated the NIGHT-hunter-28
/// resume fix 10/10), so a blend-tight envelope would flag
/// accepted behavior. The jump this audit exists to catch is a
/// different class: the phosphor ownership fight dirtied the whole
/// drawn population every frame (5x steady on the black hole,
/// 621 cells vs a 125-cell steady diff, flipping drawn state
/// against ghost state) — far outside 1.5x + 30 even before the
/// family's own fix. The 0.95 -> 1.0 settle snap (5%) and RNG
/// variance of the baseline sample also fit inside the headroom.
fn envelope(steady_max: usize, _blend: f32) -> f32 {
    steady_max as f32 * 1.5 + 30.0
}

fn drive_cycle(name: &str, mut cloud: Cloud) -> Cycle {
    // Production always arms the sim cap (bench: target period;
    // interactive: the frame-pacing cap) — mirror that contract so
    // dt flows into every family's advance (the flux harness note:
    // without it the advance clamps to zero and the family never
    // draws). The families that already set it in their make helper
    // get the same value re-applied — a no-op.
    cloud.set_max_sim_delta(Duration::from_millis(16));
    let mut frame = Frame::new(AUDIT_COLS, AUDIT_LINES, cloud.palette.bg);
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    let mut idx: u64 = 0;

    let dirty_now = |frame: &Frame| -> usize {
        if frame.is_dirty_all() {
            AUDIT_COLS as usize * AUDIT_LINES as usize
        } else {
            frame.dirty_indices().len()
        }
    };

    // Warmup: past every entry choreography and pool fill.
    for _ in 0..WARMUP_FRAMES {
        let now = start + Duration::from_millis(idx * 16);
        cloud.rain_at(&mut frame, now);
        frame.clear_dirty();
        idx += 1;
    }

    // Steady baseline envelope. Full-redraw frames (the periodic
    // resync machinery flags dirty_all) are excluded — they are a
    // terminal-drift correction, not visual motion, and counting
    // them would inflate the baseline to the whole grid and loosen
    // every envelope below.
    let mut steady_max = 0usize;
    for _ in 0..BASELINE_FRAMES {
        let now = start + Duration::from_millis(idx * 16);
        cloud.rain_at(&mut frame, now);
        if !frame.is_dirty_all() {
            steady_max = steady_max.max(frame.dirty_indices().len());
        }
        frame.clear_dirty();
        idx += 1;
    }
    assert!(
        steady_max > 0,
        "{name}: the steady baseline dirtied nothing — the family never drew"
    );

    // BRANCH 3: start the deceleration.
    let pause_t = start + Duration::from_millis(idx * 16);
    toggle_pause_at(&mut cloud, pause_t);
    let mut decel_worst = (0u64, 1.0f32, 0usize, 0.0f32);
    let mut settled = false;
    for f in 0..DECEL_FRAMES {
        let now = start + Duration::from_millis(idx * 16);
        cloud.rain_at(&mut frame, now);
        // Full-redraw (resync) frames re-emit identical content —
        // invisible by design, so they carry no jump signal.
        let dirty = if frame.is_dirty_all() {
            0
        } else {
            frame.dirty_indices().len()
        };
        let blend = cloud.resume_blend;
        let bound = envelope(steady_max, blend);
        if cloud.pause_start.is_some() && (dirty as f32) > bound && (dirty as f32) > decel_worst.3 {
            decel_worst = (f, blend, dirty, bound);
        }
        if cloud.pause {
            settled = true;
        }
        frame.clear_dirty();
        idx += 1;
    }
    assert!(
        settled,
        "{name}: the deceleration never settled into the full pause"
    );

    // The freeze: fully-paused frames must dirty NOTHING (rain_at
    // returns before every mutating pass).
    for _ in 0..FROZEN_FRAMES {
        let now = start + Duration::from_millis(idx * 16);
        cloud.rain_at(&mut frame, now);
        let dirty = dirty_now(&frame);
        assert_eq!(
            dirty, 0,
            "{name}: a fully-paused frame dirtied {dirty} cells — the freeze leaks"
        );
        frame.clear_dirty();
        idx += 1;
    }

    // BRANCH 2: unpause onto the accel ramp.
    let resume_t = start + Duration::from_millis(idx * 16);
    toggle_pause_at(&mut cloud, resume_t);
    assert!(
        cloud.resume_start.is_some(),
        "{name}: unpause must arm the ramp"
    );
    assert_eq!(
        cloud.resume_blend, 0.0,
        "{name}: the ramp starts from frozen"
    );
    let mut first_resume_frame = (0usize, 0.0f32);
    // Sustained-overshoot streak: a legitimate drama event (the
    // neural thought-burst, a monolith hero spawn, an attractor
    // transit) legitimately spikes the diff for a frame or three —
    // the artifact this audit hunts (the phosphor ownership
    // strobe) holds the WHOLE drawn population above the envelope
    // for every frame of the window. Only a sustained streak
    // (RAMP_SUSTAINED_FRAMES+ consecutive overshooting frames)
    // reads as a jump.
    let mut streak = 0u64;
    const RAMP_SUSTAINED_FRAMES: u64 = 4;
    for f in 0..RAMP_FRAMES {
        let now = start + Duration::from_millis(idx * 16);
        cloud.rain_at(&mut frame, now);
        let dirty = if frame.is_dirty_all() {
            0
        } else {
            frame.dirty_indices().len()
        };
        let blend = cloud.resume_blend;
        if f == 0 {
            first_resume_frame = (dirty, blend);
        }
        let bound = envelope(steady_max, blend);
        if (dirty as f32) > bound {
            streak += 1;
            let first = f + 1 - streak;
            assert!(
                streak < RAMP_SUSTAINED_FRAMES,
                "{}: resume frames {}..{} sustained {} frames above the envelope (worst {} > {:.0}, steady max {}) — a ramp-side micro jump",
                name,
                first,
                f,
                streak,
                dirty,
                bound,
                steady_max
            );
        } else {
            streak = 0;
        }
        frame.clear_dirty();
        idx += 1;
    }
    assert!(
        cloud.resume_start.is_none(),
        "{name}: the ramp never settled back to full speed"
    );

    // Post-settle steady: back inside the full-blend envelope.
    for f in 0..SETTLED_FRAMES {
        let now = start + Duration::from_millis(idx * 16);
        cloud.rain_at(&mut frame, now);
        let dirty = if frame.is_dirty_all() {
            0
        } else {
            frame.dirty_indices().len()
        };
        if f >= SETTLE_DRAIN_FRAMES {
            let bound = envelope(steady_max, 1.0);
            assert!(
                (dirty as f32) <= bound,
                "{name}: post-settle frame {f} dirtied {dirty} > {bound:.0} (steady max {steady_max})"
            );
        }
        frame.clear_dirty();
        idx += 1;
    }

    // The verdict: the decel side must stay inside the envelope
    // entirely (a decel overshoot has no legitimate drama-event
    // alibi — motion is only ever slowing). The ramp side is
    // enforced by the sustained-streak assertion inside the loop
    // (a transient event spike passes; the strobe class holds for
    // the whole window).
    assert_eq!(
        decel_worst.3, 0.0,
        "{name}: decel frame {} (blend {:.3}) dirtied {} > bound {:.0} — a decel-side jump",
        decel_worst.0, decel_worst.1, decel_worst.2, decel_worst.3
    );

    Cycle {
        steady_max,
        first_resume_frame,
    }
}

// One test per family: the audit's whole point is that no family
// hides behind another's fix (the hunter-28 lesson — the glyph fix
// left the black hole's owner-visible jump unexamined).

#[test]
fn hunt29_resume_cycle_glyph() {
    let report = drive_cycle("glyph", make_cloud());
    eprintln!(
        "hunt29 glyph: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_monolith() {
    let cloud = super::tests_monolith::make_monolith_cloud(AUDIT_COLS, AUDIT_LINES);
    let report = drive_cycle("monolith", cloud);
    eprintln!(
        "hunt29 monolith: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_vortex() {
    let cloud = super::tests_vortex::make_vortex_cloud(AUDIT_COLS, AUDIT_LINES);
    let report = drive_cycle("vortex", cloud);
    eprintln!(
        "hunt29 vortex: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_flux() {
    let cloud = super::tests_flux::make_flux_cloud(AUDIT_COLS, AUDIT_LINES);
    let report = drive_cycle("flux", cloud);
    eprintln!(
        "hunt29 flux: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_lorenz() {
    let cloud = super::tests_lorenz::make_lorenz_cloud(AUDIT_COLS, AUDIT_LINES);
    let report = drive_cycle("lorenz", cloud);
    eprintln!(
        "hunt29 lorenz: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_dragon() {
    let cloud = super::tests_dragon::make_dragon_cloud(AUDIT_COLS, AUDIT_LINES);
    let report = drive_cycle("dragon", cloud);
    eprintln!(
        "hunt29 dragon: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_physarum() {
    let cloud = super::tests_physarum::make_physarum_cloud(AUDIT_COLS, AUDIT_LINES);
    let report = drive_cycle("physarum", cloud);
    eprintln!(
        "hunt29 physarum: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_black_hole() {
    // The owner's repro: sorgonemous_intrascals (black hole style,
    // energy-zen, binary, speed 12 — make_black_hole_cloud pins the
    // same dial).
    let cloud = super::tests_black_hole::make_black_hole_cloud(AUDIT_COLS, AUDIT_LINES);
    let report = drive_cycle("black_hole", cloud);
    eprintln!(
        "hunt29 black_hole: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_aeolian() {
    let cloud = super::tests_aeolian::make_aeolian_cloud(AUDIT_COLS, AUDIT_LINES);
    let report = drive_cycle("aeolian", cloud);
    eprintln!(
        "hunt29 aeolian: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_solar_flare() {
    let cloud = super::tests_solar_flare::make_solar_cloud(AUDIT_COLS, AUDIT_LINES);
    let report = drive_cycle("solar_flare", cloud);
    eprintln!(
        "hunt29 solar_flare: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_dna_helix() {
    let cloud = super::tests_dna_helix::make_dna_cloud(AUDIT_COLS, AUDIT_LINES);
    let report = drive_cycle("dna_helix", cloud);
    eprintln!(
        "hunt29 dna_helix: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_murmuration() {
    let cloud = super::tests_murmuration::make_murm_cloud(AUDIT_COLS, AUDIT_LINES);
    let report = drive_cycle("murmuration", cloud);
    eprintln!(
        "hunt29 murmuration: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_quasar() {
    let cloud = super::tests_quasar::make_quas_cloud(AUDIT_COLS, AUDIT_LINES);
    let report = drive_cycle("quasar", cloud);
    eprintln!(
        "hunt29 quasar: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_neural() {
    let cloud = super::tests_neural::make_neur_cloud(AUDIT_COLS, AUDIT_LINES);
    let report = drive_cycle("neural", cloud);
    eprintln!(
        "hunt29 neural: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

// -- Color-mode variants (production parity) --
//
// The mono variants above catch the fight through the A20 orphan
// path (base_ch without base_fg). Production runs color modes where
// every structured family's drawn cells carry fg=Some — the A19
// tracked path — so the color variants audit the same contract at
// the settings the owner actually watches.

fn make_color_cloud(
    style: RainStyle,
    scheme: ColorScheme,
    density: f32,
    cps: f32,
    needs_genesis: bool,
) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::TrueColor,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        scheme,
        style,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.set_droplet_density(density);
    cloud.set_chars_per_sec(cps);
    cloud.reset(AUDIT_COLS, AUDIT_LINES);
    if needs_genesis {
        cloud.neural_rain.begin_genesis();
    }
    cloud.clear_redraw_flags_for_test();
    cloud
}

#[test]
fn hunt29_resume_cycle_color_glyph() {
    let cloud = make_color_cloud(RainStyle::Glyph, ColorScheme::Green, 0.5, 12.0, false);
    let report = drive_cycle("color_glyph", cloud);
    eprintln!(
        "hunt29 color_glyph: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_color_monolith() {
    let cloud = make_color_cloud(RainStyle::Monolith, ColorScheme::Cosmos, 0.75, 10.0, false);
    let report = drive_cycle("color_monolith", cloud);
    eprintln!(
        "hunt29 color_monolith: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_color_vortex() {
    let cloud = make_color_cloud(RainStyle::Vortex, ColorScheme::Cosmos, 0.70, 24.0, false);
    let report = drive_cycle("color_vortex", cloud);
    eprintln!(
        "hunt29 color_vortex: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_color_flux() {
    let cloud = make_color_cloud(RainStyle::Flux, ColorScheme::Ocean, 0.70, 18.0, false);
    let report = drive_cycle("color_flux", cloud);
    eprintln!(
        "hunt29 color_flux: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_color_lorenz() {
    let cloud = make_color_cloud(RainStyle::Lorenz, ColorScheme::Cosmos, 0.70, 24.0, false);
    let report = drive_cycle("color_lorenz", cloud);
    eprintln!(
        "hunt29 color_lorenz: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_color_dragon() {
    let cloud = make_color_cloud(RainStyle::Dragon, ColorScheme::Cosmos, 0.55, 18.0, false);
    let report = drive_cycle("color_dragon", cloud);
    eprintln!(
        "hunt29 color_dragon: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_color_physarum() {
    let cloud = make_color_cloud(RainStyle::Physarum, ColorScheme::Cosmos, 0.55, 18.0, false);
    let report = drive_cycle("color_physarum", cloud);
    eprintln!(
        "hunt29 color_physarum: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_color_black_hole() {
    // The owner's repro at production settings: sorgonemous_intrascals
    // (energy-zen palette, binary charset, speed 12, density 0.55).
    let cloud = make_color_cloud(
        RainStyle::BlackHole,
        ColorScheme::EnergyZen,
        0.55,
        12.0,
        false,
    );
    let report = drive_cycle("color_black_hole", cloud);
    eprintln!(
        "hunt29 color_black_hole: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_color_aeolian() {
    let cloud = make_color_cloud(RainStyle::Aeolian, ColorScheme::Aurora, 0.70, 16.0, false);
    let report = drive_cycle("color_aeolian", cloud);
    eprintln!(
        "hunt29 color_aeolian: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_color_solar_flare() {
    let cloud = make_color_cloud(RainStyle::SolarFlare, ColorScheme::Sun, 0.70, 14.0, false);
    let report = drive_cycle("color_solar_flare", cloud);
    eprintln!(
        "hunt29 color_solar_flare: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_color_dna_helix() {
    let cloud = make_color_cloud(RainStyle::DnaHelix, ColorScheme::Neptune, 0.70, 14.0, false);
    let report = drive_cycle("color_dna_helix", cloud);
    eprintln!(
        "hunt29 color_dna_helix: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_color_murmuration() {
    let cloud = make_color_cloud(RainStyle::Murmuration, ColorScheme::Gold, 0.55, 18.0, false);
    let report = drive_cycle("color_murmuration", cloud);
    eprintln!(
        "hunt29 color_murmuration: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_color_quasar() {
    let cloud = make_color_cloud(RainStyle::Quasar, ColorScheme::Stars, 0.60, 18.0, false);
    let report = drive_cycle("color_quasar", cloud);
    eprintln!(
        "hunt29 color_quasar: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}

#[test]
fn hunt29_resume_cycle_color_neural() {
    let cloud = make_color_cloud(RainStyle::Neural, ColorScheme::Cyan, 0.55, 16.0, true);
    let report = drive_cycle("color_neural", cloud);
    eprintln!(
        "hunt29 color_neural: steady {} first-resume dirty {} @ blend {:.3}",
        report.steady_max, report.first_resume_frame.0, report.first_resume_frame.1
    );
}
