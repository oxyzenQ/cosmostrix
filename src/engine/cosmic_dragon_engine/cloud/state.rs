// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Internal types for the cloud simulation engine.

use std::time::Duration;

use crate::constants::QUANTUM_RIPPLE_TRAIL_LEN;
use crate::droplet::Droplet;

/// A quantum particle (mouse-click ripple / border-touch splash crown
/// spark) in the shared pre-allocated pool. Lives in `state.rs` with
/// the other Cloud state types — moved out of `cloud/mod.rs` pure code
/// motion (mod.rs was at the 800-line hard LOC cap).
#[derive(Clone, Copy, Debug)]
pub(crate) struct QuantumParticle {
    pub active: bool,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub birth: std::time::Instant,
    pub ch: char,
    /// Palette body color at spawn (crossfade on palette switch).
    pub r: u8,
    pub g: u8,
    pub b: u8,
    // v50 (2026-08-17) trail particles masterclass effect: ring-buffer
    // of the last QUANTUM_RIPPLE_TRAIL_LEN positions, rendered with
    // diminishing brightness + cycled color (from C7). The trail is
    // pushed every frame in apply_quantum_ripple BEFORE the position
    // update, creating a streaking "comet trail" behind the moving
    // particle.
    //
    // Layout: trail_x[i] / trail_y[i] store the i-th most recent past
    // position. trail_count is the number of valid entries (0..=TRAIL_LEN).
    // The trail is rendered oldest-first so the most recent past position
    // (closest to the current particle) is drawn LAST and overrides any
    // older trail cells with its (brighter) value.
    pub trail_x: [f32; QUANTUM_RIPPLE_TRAIL_LEN],
    pub trail_y: [f32; QUANTUM_RIPPLE_TRAIL_LEN],
    pub trail_count: u8,
    /// Max trail entries for this particle. Quantum ripples use
    /// `QUANTUM_RIPPLE_TRAIL_LEN` (6); border-touch sparks use 1
    /// (F2 Splash Crown — see `docs/research/RAIN_BORDER_TOUCH_SPARK_RESEARCH.md`).
    /// The trail push in `apply_quantum_ripple` caps at this value.
    pub max_trail: u8,
    /// Per-particle lifetime in seconds. Quantum ripples use
    /// `QUANTUM_RIPPLE_LIFETIME_SECS` (4.0); border-touch sparks use
    /// `BORDER_SPARK_LIFETIME_SECS` (0.35). The age check + brightness
    /// curve in `apply_quantum_ripple` use this instead of the constant.
    pub lifetime: f32,
    /// S-master-HUNT-21: accumulated simulation age (seconds). Incremented
    /// by the SAME dt that drives motion — so particles age at exactly
    /// the rate they move. S-master-HUNT-22: that dt is now REAL elapsed
    /// time (bounded by `PARTICLE_MAX_FRAME_DT_SECS`), so `sim_age`
    /// advances at wall-clock speed on every terminal: motion, aging,
    /// and the co-spawned flash wave share one clock and an effect
    /// completes in its intended real-world duration regardless of
    /// frame rate. The `birth: Instant` field is retained for
    /// diagnostics.
    pub sim_age: f32,
}

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
///
/// `is_border` is the POSITIONAL classification: true iff the overlay
/// layout itself placed a border glyph at this cell (the perimeter of a
/// bordered `-mb` box). It is the single source of truth for the
/// border-vs-content split in `draw_message`, `build_border_order`, and
/// the word-ordinal builder — the old glyph-based test
/// (`is_border_char(val)`, which also matched `' '`, `'+'`, `'-'`, `'|'`
/// and box-drawing glyphs) swallowed user text characters that happen to
/// collide with border glyphs: `-m "v80.0.0-alpha.1"` rendered the dash
/// as a blank cell, reading "v80.0.0 alpha.1" (owner bug, v80.0.0-alpha.1 (S-master-HUNT-3)).
/// User text is ALWAYS content, whatever glyph it carries.
pub(crate) struct MsgChr {
    pub(crate) line: u16,
    pub(crate) col: u16,
    pub(crate) val: char,
    pub(crate) is_border: bool,
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
