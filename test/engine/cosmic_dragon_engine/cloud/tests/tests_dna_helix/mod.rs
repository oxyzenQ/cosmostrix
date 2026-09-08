// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-research-7 tests: the DNA helix rain style (eleventh
//! style, the rain writes the genome). The contracts here pin the
//! five laws of the ladder at both levels: the genome in isolation
//! (the turn, the pairing, the recency decay, the replication
//! fork's travel, dissolve and re-synthesis) and the full
//! orchestration (spawn, the nucleotide fall, the rung absorption
//! and mutation, the drawn bounds, the diff cleanup, pause,
//! transitions, speed scaling, sustained boundedness).

mod core;
mod helix;

pub(crate) use crate::cloud::Cloud;
pub(crate) use crate::frame::Frame;
pub(crate) use crate::rain_style::RainStyle;
pub(crate) use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};
pub(crate) use std::time::{Duration, Instant};

// -- Shared test helpers (mirrors tests_solar_flare / tests_aeolian) --

pub(crate) fn make_dna_cloud(cols: u16, lines: u16) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Neptune,
        RainStyle::DnaHelix,
    );
    cloud.init_chars(vec!['A', 'T', 'G', 'C']);
    cloud.set_droplet_density(0.70);
    cloud.set_chars_per_sec(14.0);
    cloud.reset(cols, lines);
    // Set max_sim_delta so the advance pass integrates the molecule
    // (without this, max_sim_delta defaults to ZERO and the advance
    // dt clamps to zero — the same harness note the dragon, flux,
    // aeolian and solar test trees carried). One frame step at
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
