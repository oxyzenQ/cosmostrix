// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-special-2 tests: the aeolian rain style (ninth style, the
//! invented string weave — the rain plays the instrument). The
//! contracts here pin the six laws of the weave at both levels:
//! the string field in isolation (conduction, reflection, decay,
//! the L1 boundedness proof) and the full orchestration (spawn,
//! fall, capture, pause, transitions, speed scaling).

mod core;
mod strings;

pub(crate) use crate::cloud::Cloud;
pub(crate) use crate::frame::Frame;
pub(crate) use crate::rain_style::RainStyle;
pub(crate) use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};
pub(crate) use std::time::{Duration, Instant};

// -- Shared test helpers (mirrors tests_lorenz) --

pub(crate) fn make_aeolian_cloud(cols: u16, lines: u16) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Aurora,
        RainStyle::Aeolian,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.set_droplet_density(0.70);
    cloud.set_chars_per_sec(16.0);
    cloud.reset(cols, lines);
    // Set max_sim_delta so the advance pass integrates the weave
    // (without this, max_sim_delta defaults to ZERO and the advance
    // dt clamps to zero — the same harness note the dragon, flux and
    // black-hole test trees carry). One frame step at 60 FPS.
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
