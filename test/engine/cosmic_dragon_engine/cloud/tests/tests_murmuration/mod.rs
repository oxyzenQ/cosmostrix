// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-research-7 tests: the murmuration rain style (twelfth
//! style, the rain is a flock). The contracts here pin the five
//! laws of the flock at both levels: the boids physics in
//! isolation (the speed clamps, the separation floor, the
//! startle impulse, the hash window) and the full orchestration
//! (the staggered entry, the bounds, the wall banking, the
//! breathing oscillator, the startle cycle, the drawn bounds, the
//! diff cleanup, pause, transitions, speed scaling, sustained
//! boundedness).

mod boids;
mod core;

pub(crate) use crate::cloud::Cloud;
pub(crate) use crate::frame::Frame;
pub(crate) use crate::rain_style::RainStyle;
pub(crate) use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};
pub(crate) use std::time::{Duration, Instant};

// -- Shared test helpers (mirrors tests_dna_helix / tests_solar_flare) --

pub(crate) fn make_murm_cloud(cols: u16, lines: u16) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Gold,
        RainStyle::Murmuration,
    );
    cloud.init_chars(vec!['V']);
    cloud.set_droplet_density(0.55);
    cloud.set_chars_per_sec(18.0);
    cloud.reset(cols, lines);
    // Set max_sim_delta so the advance pass integrates the flock
    // (the family harness note). One frame step at 60 FPS.
    cloud.set_max_sim_delta(Duration::from_millis(16));
    cloud.clear_redraw_flags_for_test();
    cloud
}

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
