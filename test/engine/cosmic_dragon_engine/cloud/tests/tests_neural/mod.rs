// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-research-9 tests: the neural network rain style (the
//! fourteenth style, the rain trains the network). The contracts
//! here pin the five laws of the network at both levels: the
//! genesis phase math in isolation (the phase ordering, the
//! luminosity ramp, the no-seam handoff) and the full
//! orchestration (the staggered data entry, the capture economy
//! and the starvation-free bookkeeping, the integrate-and-fire
//! dynamics, the pulse rides and deliveries, the thought-burst
//! cycle, the plasticity rewiring, the drawn bounds, the diff
//! cleanup, resize, speed scaling, sustained boundedness).

mod core;
mod genesis;

pub(crate) use crate::cloud::Cloud;
pub(crate) use crate::frame::Frame;
pub(crate) use crate::rain_style::RainStyle;
pub(crate) use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};
pub(crate) use std::time::{Duration, Instant};

// -- Shared test helpers (mirrors tests_quasar / tests_murmuration) --

pub(crate) fn make_neur_cloud(cols: u16, lines: u16) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Cyan,
        RainStyle::Neural,
    );
    // A binary bit pool (the scene's data read — bits, not
    // letters).
    cloud.init_chars(vec!['0', '1']);
    cloud.set_droplet_density(0.55);
    cloud.set_chars_per_sec(16.0);
    cloud.reset(cols, lines);
    // The scene entry re-arms the genesis (the birth replays on
    // entry, not on reset).
    cloud.neural_rain.begin_genesis();
    // Set max_sim_delta so the advance pass integrates the
    // machine (the family harness note). One frame step at
    // 60 FPS.
    cloud.set_max_sim_delta(Duration::from_millis(16));
    cloud.clear_redraw_flags_for_test();
    cloud
}

/// Sim-seconds per 16 ms frame at the reference dial (16 cps).
pub(crate) const DT_SIM_PER_FRAME: f32 = 0.016 * 16.0 / 12.0;

pub(crate) fn run_frames(cloud: &mut Cloud, frame: &mut Frame, frames: u32, step_ms: u64) {
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    for idx in 0..frames {
        let now = start + Duration::from_millis(idx as u64 * step_ms);
        cloud.rain_at(frame, now);
        frame.clear_dirty();
    }
}
