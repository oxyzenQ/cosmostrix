// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-special-3 tests: the aurora rain style (tenth style, the
//! invented polar veil — the rain paints the light). The contracts
//! here pin the five laws of the veil at both levels: the ray
//! lattice in isolation (repulsion spread, wind advection, the
//! two-band depth breath, the bounded glow charge) and the full
//! orchestration (spawn, fall, the funnel, absorption charging the
//! fringe, pause, transitions, speed scaling).

mod core;
mod rays;

pub(crate) use crate::cloud::Cloud;
pub(crate) use crate::frame::Frame;
pub(crate) use crate::rain_style::RainStyle;
pub(crate) use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};
pub(crate) use std::time::{Duration, Instant};

// -- Shared test helpers (mirrors tests_aeolian) --

pub(crate) fn make_aurora_cloud(cols: u16, lines: u16) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Aurora,
        RainStyle::Aurora,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.set_droplet_density(0.70);
    cloud.set_chars_per_sec(14.0);
    cloud.reset(cols, lines);
    // Set max_sim_delta so the advance pass integrates the veil
    // (without this, max_sim_delta defaults to ZERO and the advance
    // dt clamps to zero — the same harness note the dragon, flux,
    // aeolian and black-hole test trees carry). One frame step at
    // 60 FPS.
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
