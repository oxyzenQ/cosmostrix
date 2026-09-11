// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-research-26 tests: the glyph rain style — the classic
//! per-column rain (the droplet family's sole member, the "matrix"
//! scene's style). The tree the masterclass audit called for: the
//! only major style without its own contract file. Pins the head
//! self-bloom's in-hue cap (the front layer's retired white-wash
//! clamp), the chroma/legacy boost parity, the cinematic layer
//! distribution, the length/tail clamps, and the sparse warm-start
//! pool lifecycle.

mod core;

pub(crate) use crate::cloud::Cloud;
pub(crate) use crate::frame::Frame;
pub(crate) use crate::rain_style::RainStyle;
pub(crate) use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};
pub(crate) use std::time::{Duration, Instant};

// -- Shared test helpers (the family pattern) --

pub(crate) fn make_glyph_cloud(cols: u16, lines: u16) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::NeonGreen,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.set_droplet_density(0.65);
    cloud.set_chars_per_sec(18.0);
    cloud.reset(cols, lines);
    cloud.clear_redraw_flags_for_test();
    cloud
}

pub(crate) fn run_frames(cloud: &mut Cloud, frame: &mut Frame, frames: u32, step_ms: u64) {
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    cloud.set_max_sim_delta(Duration::from_millis(step_ms));
    for idx in 0..frames {
        let now = start + Duration::from_millis(idx as u64 * step_ms);
        cloud.rain_at(frame, now);
        frame.clear_dirty();
    }
}
