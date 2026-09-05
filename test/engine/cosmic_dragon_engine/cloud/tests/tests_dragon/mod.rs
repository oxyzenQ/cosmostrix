// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-research-5 tests: the dragon rain style (fifth style,
//! Chinese-mythology serpentine chain — free flight + circling).

mod core;

pub(crate) use crate::cloud::Cloud;
pub(crate) use crate::frame::Frame;
pub(crate) use crate::rain_style::RainStyle;
pub(crate) use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};
pub(crate) use std::time::{Duration, Instant};

// -- Shared test helpers --

pub(crate) fn make_dragon_cloud(cols: u16, lines: u16) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Cosmos,
        RainStyle::Dragon,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.set_droplet_density(0.55);
    cloud.set_chars_per_sec(18.0);
    // Set max_sim_delta so the advance pass integrates motion (without
    // this, max_sim_delta defaults to ZERO and advance returns early
    // every frame — the dragons spawn but never move, producing only
    // 1 dirty frame instead of a live stream).
    cloud.set_max_sim_delta(Duration::from_millis(16));
    cloud.reset(cols, lines);
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
