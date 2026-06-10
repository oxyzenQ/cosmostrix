// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Monolith Rain tests.
//!
//! v4.5.0 Phase 3 adds Depth Regression Lab tests that protect the
//! v4.0.1/v4.5 Monolith Rain visual identity. These tests ensure cinematic
//! depth, sparse density, distinct brightness hierarchy, and clean transitions.
//!
//! v4.5.0 Phase 4 splits this module into focused submodules for
//! maintainability. No test behavior changed.

mod charset;
mod core;
mod depth;
mod residue;
mod transitions;

// Re-exports for submodule access via `super::`
pub(super) use super::make_cloud;
pub(super) use crate::charset::{build_chars, charset_from_str};
pub(super) use crate::cloud::monolith::DrawnCellKind;
pub(super) use crate::cloud::Cloud;
pub(super) use crate::constants::CHARSET_TRANSITION_DURATION_MS;
pub(super) use crate::frame::Frame;
pub(super) use crate::rain_style::RainStyle;
pub(super) use crate::runtime::{BoldMode, ColorMode, ColorScheme, MonolithSize, ShadingMode};
pub(super) use std::collections::BTreeSet;
pub(super) use std::time::{Duration, Instant};

// -- Shared test helpers --

pub(super) fn make_monolith_cloud(cols: u16, lines: u16) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::Mono,
        false,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::BlackHole,
        RainStyle::Monolith,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.set_droplet_density(0.75);
    cloud.set_chars_per_sec(10.0);
    cloud.reset(cols, lines);
    cloud.clear_redraw_flags_for_test();
    cloud
}

pub(super) fn run_frames(cloud: &mut Cloud, frame: &mut Frame, frames: u32, step_ms: u64) {
    let start = Instant::now();
    cloud.last_spawn_time = start - Duration::from_secs(1);
    cloud.last_phosphor_time = start;
    for idx in 0..frames {
        let now = start + Duration::from_millis(idx as u64 * step_ms);
        cloud.rain_at(frame, now);
        frame.clear_dirty();
    }
}

pub(super) fn visible_cell_count(frame: &Frame) -> usize {
    let mut count = 0usize;
    for line in 0..frame.height {
        for col in 0..frame.width {
            let cell = frame.get(col, line).expect("cell in bounds");
            if cell.ch != ' ' || cell.fg.is_some() {
                count += 1;
            }
        }
    }
    count
}

pub(super) fn visible_chars(frame: &Frame) -> Vec<char> {
    let mut chars = Vec::new();
    for line in 0..frame.height {
        for col in 0..frame.width {
            let cell = frame.get(col, line).expect("cell in bounds");
            if cell.ch != ' ' {
                chars.push(cell.ch);
            }
        }
    }
    chars
}

pub(super) fn visible_char_signature(frame: &Frame) -> BTreeSet<char> {
    visible_chars(frame).into_iter().collect()
}

pub(super) fn average_head_delta(before: &[f32], after: &[f32]) -> f32 {
    let len = before.len().min(after.len()).max(1);
    before
        .iter()
        .zip(after.iter())
        .take(len)
        .map(|(a, b)| b - a)
        .sum::<f32>()
        / len as f32
}

pub(super) fn segment_draw_count(cloud: &Cloud) -> usize {
    cloud
        .monolith_rain
        .drawn_cells_for_test()
        .iter()
        .filter(|cell| matches!(cell.kind, DrawnCellKind::Segment))
        .count()
}

pub(super) fn seed_stale_phosphor(cloud: &mut Cloud) {
    cloud.phosphor.fill(180);
    cloud
        .phosphor_base_fg
        .fill(Some(crossterm::style::Color::Grey));
    cloud.phosphor_base_ch.fill('x');
}

pub(super) fn seeded_residue_count(cloud: &Cloud) -> usize {
    cloud
        .phosphor_base_ch
        .iter()
        .filter(|&&ch| ch == 'x')
        .count()
}

pub(super) fn disable_monolith_spawning(cloud: &mut Cloud) {
    cloud.resume_blend = 0.0;
    cloud.resume_start = None;
    cloud.spawn_remainder = 0.0;
    cloud.monolith_rain.deactivate_all_for_test();
}

pub(super) fn phosphor_index(cloud: &Cloud, col: u16, line: u16) -> usize {
    col as usize * cloud.lines as usize + line as usize
}
