// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Core neural-style behavior contracts (NIGHT-research-9):
//! scene resolution, the genesis assembly, the capture economy
//! and its starvation-free bookkeeping, the integrate-and-fire
//! dynamics, the pulse rides and deliveries, the thought-burst
//! cycle, the plasticity rewiring, the drawn bounds, resize on
//! the trained machine, speed scaling, and the sustained
//! boundedness integration.

use super::*;

use crate::constants::NEUR_POT_CAP;

#[test]
fn neural_scene_resolves_style_and_fields() {
    let s = crate::scene::get_scene("neural").expect("neural scene exists");
    assert_eq!(s.config.rain_style, RainStyle::Neural);
    assert_eq!(s.config.color, Some("cyan"));
    assert_eq!(s.config.charset, Some("binary"));
    assert_eq!(s.config.fps, Some(60.0));
    assert_eq!(s.config.speed, Some(16.0));
    assert_eq!(s.config.density, Some(0.55));
    assert_eq!(
        s.config.glitch_level,
        Some(crate::config::GlitchLevel::None)
    );
    // Style dispatch sanity: structured family, accumulator spawn.
    assert!(!RainStyle::Neural.is_droplet_family());
    assert!(RainStyle::Neural.uses_spawn_remainder());
    // Label round-trip (the scene-custom `rain` field surface).
    assert_eq!(RainStyle::Neural.as_str(), "neural");
    assert_eq!(RainStyle::from_label("neural"), Some(RainStyle::Neural));
    assert_eq!(RainStyle::from_label("Neural"), Some(RainStyle::Neural));
    assert_eq!(RainStyle::from_label("nn"), Some(RainStyle::Neural));
    assert_eq!(
        RainStyle::from_label("neural_network"),
        Some(RainStyle::Neural)
    );
    assert!(RainStyle::valid_labels_hint().contains("neural"));
    // The scene joins the interactive cycle (a new scene that
    // forgets to join fails the scene tests' coverage contract).
    assert!(crate::scene::SCENE_ORDER.contains(&"neural"));
}

#[test]
fn neur_machine_assembles_through_genesis() {
    // The birth contract: the genesis completes over its window
    // and the steady machine carries the full population with
    // every wire complete at the no-seam handoff (in the steady
    // state the plasticity economy keeps at most one wire
    // mid-lifecycle — the bounded-learning contract below).
    let mut cloud = make_neur_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    // The genesis is one-shot choreography: ~8.2 sim-s at the
    // reference dial, then the steady state. Run well past it so
    // the capture economy and the wire sweep complete.
    let genesis_frames = (8.2 / DT_SIM_PER_FRAME).ceil() as u32;
    // The handoff probe rides the run's own time anchor: a
    // second run_frames call would re-anchor on a fresh
    // Instant::now, and the machine's dt would freeze at zero
    // until the synthetic clock caught back up.
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    // The first plasticity rewire cannot fire before the rewire
    // clock's jitter floor (NEUR_REWIRE_CLOCK_MEAN * 0.6 = 4.5
    // sim-s after lit), so probing at +60 frames (1.28 sim-s)
    // still sees the machine exactly as the genesis left it.
    let handoff_frame = genesis_frames + 60;
    for idx in 0..(genesis_frames + 900) {
        let now = start + Duration::from_millis(idx as u64 * 16);
        cloud.rain_at(&mut frame, now);
        frame.clear_dirty();
        if idx == handoff_frame {
            // The no-seam handoff: the Thought seam force-completes
            // every wire, so the whole population stands complete
            // here — any incomplete wire would be a seam.
            assert!(
                cloud
                    .neural_rain
                    .synapses
                    .iter()
                    .all(|s| !s.active || s.grown >= 1.0),
                "a genesis wire was still growing at the steady handoff"
            );
        }
    }
    assert!(
        cloud.neural_rain.lit_for_test(),
        "the machine never lit (t = {:.2})",
        cloud.neural_rain.genesis_t_for_test()
    );
    assert_eq!(
        cloud.neural_rain.node_active_for_test(),
        cloud.neural_rain.nodes.len()
    );
    // The steady machine: the plasticity economy keeps at most
    // ONE wire mid-growth at any instant (the rewire clock's
    // jitter floor, 4.5 sim-s, exceeds the full
    // retire-plus-regrow chain, 2.5 sim-s, so two successors can
    // never overlap). The exact rewire schedule is not
    // platform-stable — libm ulp differences in the leak and
    // kick factors shift the fire timing, which shifts the
    // shared RNG stream — so a mid-growth successor at this
    // arbitrary instant is the economy working, not a seam.
    let mid_growth = cloud
        .neural_rain
        .synapses
        .iter()
        .filter(|s| s.active && s.grown < 1.0)
        .count();
    assert!(
        mid_growth <= 1,
        "{mid_growth} wires mid-growth at once — the bounded-learning contract allows one"
    );
    // The architecture: layered, with the output band present.
    let layers = cloud.neural_rain.geom.nodes_per_layer.clone();
    assert!(layers.len() >= 3);
    assert!(layers.iter().all(|&n| n >= 2));
}

#[test]
fn neur_streamers_assemble_and_absorb() {
    // Law 3: the data falls (the staggered accumulator entry) and
    // the machine eats it (the capture economy's bookkeeping —
    // every absorption flows through the counter).
    let mut cloud = make_neur_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    let mut saw_streamer = false;
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    for idx in 0..240 {
        let now = start + Duration::from_millis(idx as u64 * 16);
        cloud.rain_at(&mut frame, now);
        frame.clear_dirty();
        if cloud.neural_rain.streamer_active_for_test() > 0 {
            saw_streamer = true;
        }
    }
    assert!(saw_streamer, "the data never fell");
    assert!(
        cloud.neural_rain.absorptions_for_test() > 0,
        "the machine never ate a streamer"
    );
    // The starvation-free bookkeeping: the counter never exceeds
    // the pool, and the pool rides the target band.
    assert!(cloud.neural_rain.streamer_active_for_test() <= cloud.neural_rain.streamers.len());
}

#[test]
fn neur_fires_and_launches_pulses() {
    // Laws 2 + 4: the input band fires (the threshold crossing)
    // and the pulses ride the wires (the pool bookkeeping stays
    // honest).
    let mut cloud = make_neur_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    let genesis_frames = (8.2 / DT_SIM_PER_FRAME).ceil() as u32;
    let mut saw_pulse = false;
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    for idx in 0..(genesis_frames + 600) {
        let now = start + Duration::from_millis(idx as u64 * 16);
        cloud.rain_at(&mut frame, now);
        frame.clear_dirty();
        if cloud.neural_rain.pulse_active_for_test() > 0 {
            saw_pulse = true;
        }
    }
    assert!(
        cloud.neural_rain.fires_for_test() > 0,
        "the machine never fired"
    );
    assert!(saw_pulse, "no pulse ever rode a wire");
    assert!(cloud.neural_rain.pulse_active_for_test() <= cloud.neural_rain.pulses.len());
}

#[test]
fn neur_burst_clock_fires_volleys() {
    // Law 4's drama event: the thought burst force-fires a clump
    // of inputs — a whole wave crosses the machine.
    let mut cloud = make_neur_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    // Assemble to steady first (only a lit machine thinks in
    // volleys).
    let genesis_frames = (8.2 / DT_SIM_PER_FRAME).ceil() as u32;
    run_frames(&mut cloud, &mut frame, genesis_frames + 60, 16);
    assert!(cloud.neural_rain.lit_for_test());
    let fires_before = cloud.neural_rain.fires_for_test();
    cloud.neural_rain.arm_burst_for_test(0.01);
    run_frames(&mut cloud, &mut frame, 8, 16);
    assert!(cloud.neural_rain.bursts_for_test() >= 1);
    // The volley: the burst's force-fires show up as real fires.
    assert!(cloud.neural_rain.fires_for_test() > fires_before);
}

#[test]
fn neur_plasticity_rewires_constant_count() {
    // Law 5: the rewire clock retires a wire and grows the
    // successor in the SAME slot — the wire count never changes.
    let mut cloud = make_neur_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    let genesis_frames = (8.2 / DT_SIM_PER_FRAME).ceil() as u32;
    run_frames(&mut cloud, &mut frame, genesis_frames + 60, 16);
    assert!(cloud.neural_rain.lit_for_test());
    let syn_count = cloud.neural_rain.synapses.len();
    assert!(syn_count > 0);
    cloud.neural_rain.arm_rewire_for_test(0.01);
    // The retire fade (1.1 s) + the successor's growth: run past
    // both.
    run_frames(&mut cloud, &mut frame, 120, 16);
    assert!(cloud.neural_rain.rewires_for_test() >= 1);
    assert_eq!(
        cloud.neural_rain.synapses.len(),
        syn_count,
        "the wire count changed — the successor must reuse the slot"
    );
}

#[test]
fn neur_population_and_potentials_stay_bounded() {
    // The stability contract: every state variable bounded by
    // direct construction, pinned over a long run.
    let mut cloud = make_neur_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    let genesis_frames = (8.2 / DT_SIM_PER_FRAME).ceil() as u32;
    run_frames(&mut cloud, &mut frame, genesis_frames + 2000, 16);
    for (potential, refract, flash) in cloud.neural_rain.node_states_for_test() {
        assert!(
            (0.0..=NEUR_POT_CAP).contains(&potential),
            "potential out of bounds: {potential}"
        );
        assert!(refract >= 0.0, "refractory went negative: {refract}");
        assert!((0.0..=1.0).contains(&flash), "flash out of bounds: {flash}");
    }
    assert_eq!(
        cloud.neural_rain.node_active_for_test(),
        cloud.neural_rain.nodes.len()
    );
    assert!(cloud.neural_rain.pulse_active_for_test() <= cloud.neural_rain.pulses.len());
    assert!(cloud.neural_rain.streamer_active_for_test() <= cloud.neural_rain.streamers.len());
}

#[test]
fn neur_drawn_cells_stay_in_bounds() {
    // The drawn-bounds contract: every drawn cell lands inside the
    // viewport (the degenerate-terminal guard's steady-state
    // sibling).
    let mut cloud = make_neur_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    let genesis_frames = (8.2 / DT_SIM_PER_FRAME).ceil() as u32;
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    for idx in 0..(genesis_frames + 300) {
        let now = start + Duration::from_millis(idx as u64 * 16);
        cloud.rain_at(&mut frame, now);
        frame.clear_dirty();
        for cell in cloud.neural_rain.drawn_cells_for_test() {
            assert!(cell.col < 120, "cell col out of bounds: {}", cell.col);
            assert!(cell.line < 40, "cell line out of bounds: {}", cell.line);
        }
    }
}

#[test]
fn neur_degenerate_viewport_is_valid() {
    // The degenerate-terminal contract: a 1x1 viewport produces a
    // collapsed but valid machine, never a panic.
    let mut cloud = make_neur_cloud(1, 1);
    let mut frame = Frame::new(1, 1, cloud.palette.bg);
    run_frames(&mut cloud, &mut frame, 120, 16);
    // The machine may not have lit (the collapsed layout's clocks
    // still run) — the contract is validity, not assembly.
    assert!(cloud.neural_rain.node_active_for_test() <= cloud.neural_rain.nodes.len());
}

#[test]
fn neur_resize_keeps_the_trained_machine() {
    // The resize contract: a pure resize on the lit machine keeps
    // the steady state (no second genesis — the pools rebuild to
    // the full population immediately).
    let mut cloud = make_neur_cloud(120, 40);
    let mut frame = Frame::new(120, 40, cloud.palette.bg);
    let genesis_frames = (8.2 / DT_SIM_PER_FRAME).ceil() as u32;
    run_frames(&mut cloud, &mut frame, genesis_frames + 60, 16);
    assert!(cloud.neural_rain.lit_for_test());
    cloud.reset(100, 30);
    assert!(
        cloud.neural_rain.lit_for_test(),
        "the resize re-armed the genesis — a pure resize keeps the trained machine"
    );
    assert_eq!(
        cloud.neural_rain.node_active_for_test(),
        cloud.neural_rain.nodes.len()
    );
}

#[test]
fn neur_speed_scaling_rides_the_family_clock() {
    // The family speed contract: the whole machine (the genesis
    // included) rides one sim clock — doubling the speed dial
    // doubles the birth's progress.
    let mut slow = make_neur_cloud(120, 40);
    let mut fast = make_neur_cloud(120, 40);
    fast.set_chars_per_sec(32.0);
    let mut frame_slow = Frame::new(120, 40, slow.palette.bg);
    let mut frame_fast = Frame::new(120, 40, fast.palette.bg);
    run_frames(&mut slow, &mut frame_slow, 100, 16);
    run_frames(&mut fast, &mut frame_fast, 100, 16);
    let t_slow = slow.neural_rain.genesis_t_for_test();
    let t_fast = fast.neural_rain.genesis_t_for_test();
    assert!(
        t_fast > t_slow * 1.8,
        "the speed keys do not scale the machine: {t_fast} vs {t_slow}"
    );
}

// -- NIGHT-research-22: the soft-light round (standing-Core sweep) --

#[test]
fn neur_pulse_heads_compose_at_the_warm_ceiling() {
    // The audit's first standing site: the flaring branch lifted
    // every pulse head to Core for the whole 1.8 s burst window —
    // about ten times the black hole's whip flash. The pulse-head
    // ladder now composes at the warm Hot ceiling under every
    // genesis cap (the ramp included); the machine's white lives
    // on the neurons' fire flashes (the 0.35 s fired-flash tau).
    use crate::cloud::monolith::BrightnessLevel;
    use crate::cloud::type_rain::neural::draw::pulse_head_level;

    for cap_rank in 0u8..=4 {
        assert!(
            !matches!(pulse_head_level(cap_rank), BrightnessLevel::Core),
            "a riding signal head must never land Core (genesis cap {cap_rank})"
        );
    }
    assert!(matches!(pulse_head_level(4), BrightnessLevel::Hot));
}

#[test]
fn neur_step_up_stops_at_the_warm_ceiling() {
    // The audit's second standing site: the output band's step-up
    // lifted every standing Hot answer node to Core (the output
    // is the machine's voice, but the voice reads warm now). The
    // step-up holds at Hot, keeps the sub-ceiling answer read (Mid
    // steps up to Hot), and a fired flash — already Core from the
    // fire moment — keeps its white through the step.
    use crate::cloud::monolith::BrightnessLevel;
    use crate::cloud::type_rain::neural::draw::step_up_level;

    assert!(matches!(
        step_up_level(BrightnessLevel::Ghost),
        BrightnessLevel::Dim
    ));
    assert!(matches!(
        step_up_level(BrightnessLevel::Dim),
        BrightnessLevel::Mid
    ));
    assert!(matches!(
        step_up_level(BrightnessLevel::Mid),
        BrightnessLevel::Hot
    ));
    // The cap: a standing Hot node holds the warm ceiling through
    // the output band's step-up (was Core — the standing offender).
    assert!(matches!(
        step_up_level(BrightnessLevel::Hot),
        BrightnessLevel::Hot
    ));
    // The fired flash keeps its white.
    assert!(matches!(
        step_up_level(BrightnessLevel::Core),
        BrightnessLevel::Core
    ));
}
