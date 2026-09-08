// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-special-4 tests: the solar flare rain style (tenth style,
//! the invented corona arcade — the rain rides the magnetism). The
//! contracts here pin the five laws of the corona at both levels:
//! the arcade in isolation (carpet spread, width breath, the
//! bounded flux charge, the flare cycle) and the full orchestration
//! (spawn, the energy-conserving descent, the footpoint deposition,
//! the eruption ejecta, pause, transitions, speed scaling).

mod core;
mod loops;

pub(crate) use crate::cloud::Cloud;
pub(crate) use crate::frame::Frame;
pub(crate) use crate::rain_style::RainStyle;
pub(crate) use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};
pub(crate) use std::time::{Duration, Instant};

// -- Shared test helpers (mirrors tests_aeolian / tests_aurora) --

pub(crate) fn make_solar_cloud(cols: u16, lines: u16) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Sun,
        RainStyle::SolarFlare,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.set_droplet_density(0.70);
    cloud.set_chars_per_sec(14.0);
    cloud.reset(cols, lines);
    // Set max_sim_delta so the advance pass integrates the corona
    // (without this, max_sim_delta defaults to ZERO and the advance
    // dt clamps to zero — the same harness note the dragon, flux,
    // aeolian and aurora-era test trees carried). One frame step at
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
