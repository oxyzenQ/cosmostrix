// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Internal types for the cloud simulation engine.

use std::time::Duration;

use crate::droplet::Droplet;

/// Per-column tracking for spawn control and speed scaling.
#[derive(Clone, Debug)]
pub(crate) struct ColumnStatus {
    pub(crate) max_speed_pct: f32,
    pub(crate) num_droplets: u8,
    pub(crate) can_spawn: bool,
}

/// Spawn-time state sampled from `Cloud` before mutably borrowing the droplet pool.
pub(crate) struct DropletSpawnSpec {
    pub(crate) col: u16,
    pub(crate) end_line: u16,
    pub(crate) char_pool_idx: u16,
    pub(crate) length: u16,
    pub(crate) chars_per_sec: f32,
    pub(crate) time_to_linger: Duration,
    pub(crate) layer: u8,
    /// Dynamic tail cell count for front-layer droplets (1 for mid/back).
    /// See `Droplet::tail_cells` for semantics.
    pub(crate) tail_cells: u8,
    pub(crate) palette_slot: u8,
    pub(crate) turb_phase: f32,
}

impl DropletSpawnSpec {
    pub(crate) fn apply_to(self, d: &mut Droplet) {
        d.bound_col = self.col;
        d.end_line = self.end_line;
        d.char_pool_idx = self.char_pool_idx;
        d.length = self.length;
        d.chars_per_sec = self.chars_per_sec;
        d.time_to_linger = self.time_to_linger;
        d.layer = self.layer;
        d.tail_cells = self.tail_cells;
        d.palette_slot = self.palette_slot;
        d.head_put_line = 0;
        d.head_cur_line = 0;
        d.prev_head_put_line = 0;
        d.tail_put_line = None;
        d.tail_cur_line = 0;
        d.head_stop_time = None;
        d.turb_phase = self.turb_phase;
        d.turb_time = 0.0;
        // Phase jitter: leave advance_remainder at its current value.
        // activate() will reset it to 0.0 unless SPAWN_PHASE_JITTER is true,
        // in which case a random offset is applied after activation.
    }
}

/// A single character in the overlay message box (position + glyph).
pub(crate) struct MsgChr {
    pub(crate) line: u16,
    pub(crate) col: u16,
    pub(crate) val: char,
}

/// RAIN_BORDER_TOUCH_GLOW (Option C+D): an active touch pulse on a
/// message-overlay border cell. Each entry records:
/// - the `MsgChr` index in `Cloud::message` that was touched,
/// - the column (for the halo above the border, Option D),
/// - the `head_rgb` of the droplet that touched (so the glow is
///   dynamic per-droplet, not a static white),
/// - the birth instant (for smoothstep envelope decay).
///
/// See `docs/research/RAIN_BORDER_TOUCH_GLOW_AUDIT.md` for the design.
#[derive(Clone, Copy)]
pub(crate) struct BorderPulse {
    pub(crate) msg_idx: usize,
    pub(crate) col: u16,
    pub(crate) head_rgb: (u8, u8, u8),
    pub(crate) birth: std::time::Instant,
}

/// Kind of rare atmospheric anomaly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum AnomalyKind {
    /// Brief luminance surge in a localized area.
    LuminanceSurge,
    /// Stream glyph corruption/mutation.
    GlyphCorruption,
    /// Faint expanding pulse wave.
    PulseWave,
}

/// An active anomaly zone on the screen.
#[derive(Clone, Debug)]
pub(crate) struct AnomalyZone {
    pub(crate) col: u16,
    pub(crate) line: u16,
    pub(crate) radius: u16,
    pub(crate) kind: AnomalyKind,
    pub(crate) start_time: std::time::Instant,
}
