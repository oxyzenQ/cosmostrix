// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Task-18 tests: the ripple rain style (fourth style, water surface).

mod core;

pub(crate) use crate::cloud::Cloud;
pub(crate) use crate::frame::Frame;
pub(crate) use crate::rain_style::RainStyle;
pub(crate) use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};
pub(crate) use std::time::{Duration, Instant};

// -- Shared test helpers --

pub(crate) fn make_ripple_cloud(cols: u16, lines: u16) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Ocean,
        RainStyle::Ripple,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.set_droplet_density(0.70);
    cloud.set_chars_per_sec(20.0);
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
