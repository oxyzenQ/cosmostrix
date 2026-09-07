// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-special-1 tests: the black hole rain style (eighth style,
//! the sorgonemous_intrascals scene — the event-horizon ball, its
//! stage-2 orbital ring, the stage-2.2 formation intro, and the
//! stage-3 glyph infall).

mod core;
mod formation;
mod infall;
mod ring;

pub(crate) use crate::cloud::Cloud;
pub(crate) use crate::frame::Frame;
pub(crate) use crate::rain_style::RainStyle;
pub(crate) use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};
pub(crate) use std::time::{Duration, Instant};

// -- Shared test helpers --

pub(crate) fn make_black_hole_cloud(cols: u16, lines: u16) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::EnergyZen,
        RainStyle::BlackHole,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.set_droplet_density(0.55);
    cloud.set_chars_per_sec(12.0);
    cloud.reset(cols, lines);
    // Set max_sim_delta so the advance pass integrates the orbital
    // ring (without this, max_sim_delta defaults to ZERO and the
    // advance dt clamps to zero — the same harness note the dragon
    // and flux test trees carry). One frame step at 60 FPS.
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

/// Drive the cloud past the ~3.1 s formation intro (seed dot ->
/// collapse -> horizon bloom) so geometry, mote and draw contracts
/// assert the STEADY state — the formation sequence itself is covered
/// by `formation.rs`. 220 frames x 16 ms = 3.52 s.
pub(crate) fn run_frames_to_steady(cloud: &mut Cloud, frame: &mut Frame) {
    run_frames(cloud, frame, 220, 16);
}
