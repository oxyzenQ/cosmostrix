// Copyright (c) 2026 rezky_nightky

//! Internal types for the cloud simulation engine.

use std::time::Duration;

use crate::droplet::Droplet;

/// Per-column tracking for spawn control and speed scaling.
#[derive(Clone, Debug)]
pub(super) struct ColumnStatus {
    pub(super) max_speed_pct: f32,
    pub(super) num_droplets: u8,
    pub(super) can_spawn: bool,
}

/// Spawn-time state sampled from `Cloud` before mutably borrowing the droplet pool.
pub(super) struct DropletSpawnSpec {
    pub(super) col: u16,
    pub(super) end_line: u16,
    pub(super) char_pool_idx: u16,
    pub(super) length: u16,
    pub(super) chars_per_sec: f32,
    pub(super) time_to_linger: Duration,
    pub(super) layer: u8,
    pub(super) palette_slot: u8,
    pub(super) turb_phase: f32,
}

impl DropletSpawnSpec {
    pub(super) fn apply_to(self, d: &mut Droplet) {
        d.bound_col = self.col;
        d.end_line = self.end_line;
        d.char_pool_idx = self.char_pool_idx;
        d.length = self.length;
        d.chars_per_sec = self.chars_per_sec;
        d.time_to_linger = self.time_to_linger;
        d.layer = self.layer;
        d.palette_slot = self.palette_slot;
        d.head_put_line = 0;
        d.head_cur_line = 0;
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
pub(super) struct MsgChr {
    pub(super) line: u16,
    pub(super) col: u16,
    pub(super) val: char,
}

/// Kind of rare atmospheric anomaly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum AnomalyKind {
    /// Brief luminance surge in a localized area.
    LuminanceSurge,
    /// Stream glyph corruption/mutation.
    GlyphCorruption,
    /// Faint expanding pulse wave.
    PulseWave,
}

/// An active anomaly zone on the screen.
#[derive(Clone, Debug)]
pub(super) struct AnomalyZone {
    pub(super) col: u16,
    pub(super) line: u16,
    pub(super) radius: u16,
    pub(super) kind: AnomalyKind,
    pub(super) start_time: std::time::Instant,
}
