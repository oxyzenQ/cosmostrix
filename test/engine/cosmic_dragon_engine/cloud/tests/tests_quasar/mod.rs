// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-research-8 tests: the quasar rain style (the thirteenth
//! style, the rain feeds the engine). The contracts here pin the
//! five laws of the engine at both levels: the ignition phase
//! math in isolation (the phase ordering, the luminosity and jet
//! fronts, the no-seam handoff) and the full orchestration (the
//! staggered infall, the capture economy and the starvation-free
//! bookkeeping, the Kepler shear, the doppler asymmetry, the jet
//! ride, the flare cycle, the drawn bounds, the diff cleanup,
//! pause, transitions, resize, speed scaling, sustained
//! boundedness).

mod core;
mod ignition;

pub(crate) use crate::cloud::Cloud;
pub(crate) use crate::frame::Frame;
pub(crate) use crate::rain_style::RainStyle;
pub(crate) use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};
pub(crate) use std::time::{Duration, Instant};

// -- Shared test helpers (mirrors tests_murmuration / tests_dna_helix) --

pub(crate) fn make_quas_cloud(cols: u16, lines: u16) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Stars,
        RainStyle::Quasar,
    );
    // A braille dot pool (the scene's granulated-light read).
    cloud.init_chars(vec!['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧']);
    cloud.set_droplet_density(0.60);
    cloud.set_chars_per_sec(18.0);
    cloud.reset(cols, lines);
    // The scene entry re-arms the ignition (the birth replays on
    // entry, not on reset).
    cloud.quasar_rain.begin_ignition();
    // Set max_sim_delta so the advance pass integrates the engine
    // (the family harness note). One frame step at 60 FPS.
    cloud.set_max_sim_delta(Duration::from_millis(16));
    cloud.clear_redraw_flags_for_test();
    cloud
}

/// Sim-seconds per 16 ms frame at the reference dial (18 cps).
pub(crate) const DT_SIM_PER_FRAME: f32 = 0.016 * 18.0 / 12.0;

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
