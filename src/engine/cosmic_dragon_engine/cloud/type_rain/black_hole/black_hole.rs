// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only
// LOC_EXEMPT: NIGHT-special-1 stages 2.1/2.2 added the rim spin
// (spin_phase + conveyor bookkeeping), the entry spiral gate and the
// formation intro (formation clock, formed flag, seed-dot rendering,
// horizon-bloom filter) on top of the ball/ring orchestrator, stage
// 2.4 added the see-saw roll scheduler field, stage 2.6 added the
// halo stream pool arms (a second lane pool with its own
// spawn/advance/draw passes), and stage 2.7 added the upper-lane
// toggle field of the double-crown stream split plus the tier
// head-floor call in the draw pass — pushing this file over the 800-LOC
// cap. The per-mote physics + roll scheduler (ring.rs), the halo
// stream physics (halo.rs), the infall layer's whole third pool
// (infall.rs — stage 3), the ball cell helpers (ball_helpers.rs)
// and the formation phase math (formation.rs) are already split
// out; what remains is one impl whose draw/spawn/advance passes
// share the private field set — a further split would need
// pub(super) field exposure across sibling modules, a worse
// encapsulation trade than the cap debt (same call as dragon.rs's
// entry-reveal exemption). The stage-3 infall stream owns its pool
// outright in its own module, so the orchestrator grows only its
// pass calls — the cap debt does not compound with the third pool.

//! Black hole rain for the sorgonemous_intrascals scene (NIGHT-special-1,
//! the eighth rain style — stage 3: the ball, the orbital ring stack
//! with its halo streams, and the glyph infall).
//!
//! Motion DNA — 100% distinct from every existing style: the screen hosts
//! a single gravitating body, not a particle field. Stage 1 rendered the
//! body itself: a medium round ball centered on the viewport with a black
//! empty core (the event horizon) and a bright photon-ring rim that fades
//! outward into the dark (the visual reference is the intro logo emblem —
//! a density-shaded round mark — re-expressed through the engine's own
//! brightness ladder and the active charset pool). Stage 2 orbits a ring
//! of glyphs around it: RK4-Lorenz-turbulent motes on a tilted Keplerian
//! ellipse, far side passing behind the hole, near side crossing in
//! front of the core (the per-mote physics lives in `ring.rs`).
//!
//! Stage 2.3 (owner 9.5/10 feedback, the Gargantua reference): the disk
//! is now an equatorial crossing — the near side's sine squashed to half
//! the minor axis so the solid line hugs the vertical MIDDLE of the core,
//! the near/far occlusion keyed on the orbit side instead of screen
//! height (near-side cells the z-tilt lifts above the equator used to be
//! eaten by the old rule — half of why the line read below center), the
//! active-mote floor raised to 0.55 of the pool so the band knits into
//! the near-continuous bright line of the Interstellar imagery, and the
//! disk gained a radial brightness profile (inner-zone bump across the
//! shadow, rung-fade at the line's ends — the smooth sparse transition).
//!
//! Stage 2.4 (owner 9.7/10 feedback, three reads): the brightness key
//! moved from the orbital angle to the projected DISTANCE from the hole
//! (`proximity_level` in ring.rs — two rungs up near the hole for the
//! white-hot "head white", the stage-2.3 fade ladder far away); the
//! disk stack now carries three tiers (the Interstellar ladder — the
//! long equatorial band, a shorter band a snug step above it, the
//! shortest band one more snug step up; the upper tiers draw in front
//! at any height and orbit visibly faster); and the whole stack
//! see-saws around the hole (`RingRoll` in ring.rs — the flat
//! horizontal line holds the single longest pose, eased excursions
//! sweep the shallow-to-mid 15-60 degree attitude window with the
//! whole 70-110 degree near-vertical band excluded, alternating
//! sign, sometimes chaining tilt to tilt through the rest line).
//!
//! Stage 2.5 (owner 9.8/10 feedback): the three-tier stack tightened
//! into the snug family the owner asked for — the upper two lines now
//! sit a small step above the equatorial band and each other (his
//! one-meter-gap analogy: the old layout read ten meters apart), and
//! the see-saw roll's long dwell improved to a more special 30 s-or-
//! more hold across the whole attitude window.
//!
//! Stage 2.6 (owner 9.9/10 feedback): the stack closes
//! into the one-compact-family read — the main disk drops a little
//! below its default center position, the upper two bands pull down
//! with it (the almost-fused grouping, the family reads dense), and
//! the two longest bands widen a little more. The halo streams join
//! the system (physics in `halo.rs`): a second mote pool whose
//! riders orbit the arc circle over and under the shadow — the
//! upper stream doubles the upward-curving particle density by
//! co-riding the lensing arc, the lower stream mirrors it under the
//! hole slightly sparser, both circulating in the disk's rotational
//! sense. Geometry stays fraction-based end to end, so the whole
//! system scales with any screen size (the dynamic-size contract
//! the owner pinned this round).
//!
//! Stage 2.7 (owner 9.95/10 feedback, rated 10/10 masterpiece): the
//! halo split became the DOUBLE UPWARD STREAM — a second upper lane
//! on the 1.48-radius arc joined the 1.30 lensing circle (two
//! distinct crowns of the same rider population) while the lower
//! stream dropped to a RARE echo under the shadow. The stack family
//! descended below the viewport center (crown above, disk line
//! below, the real Gargantua composition), the main band stretched a
//! quarter longer, and the snug upper stacks' heads burned white.
//!
//! Stage 3 (owner green light after the 10/10): the GLYPH INFALL —
//! the rain itself becomes the accretion material (the whole layer
//! lives in `infall.rs`: the third mote pool plus its physics,
//! spawn/advance/draw passes). Glyphs spawn above the viewport and
//! fall through the hole's inverse-square gravitational field,
//! blended to zero just past the disk's reach — straight ambient
//! rain far from the system, elegant arcs near it. Inside the
//! capture radius an accretion brake decays the tangential velocity
//! (angular momentum radiated into the disk), so passing glyphs
//! whip around and decay into tightening inspirals instead of
//! flying by; a mote that crosses the event horizon is eaten, its
//! final flash on the photon ring. Brightness is graded by the
//! radial approach speed (kinetic heat — the plunge component)
//! composed with the shared proximity ladder: the far rain reads
//! Ghost, the whip Core white.
//! Geometry is fraction-based end to end, so the system scales with
//! any screen size (the dynamic-size contract the owner pinned).
//!
//! NIGHT-research-9 (the masterclass physics pass, owner mandate
//! 2026-09-11): three reads, three fixes. (1) The disk's rotation
//! direction now reads coherent end to end — every infalling glyph
//! is born corotating with the disk (a sampled specific angular
//! momentum with the disk's sign; see `activate_infall_mote`), so
//! no capture ever whips against the disk's rotational sense — the
//! ambient rain field reads as one vorticity feeding the hole.
//! (2) The composition adapts to the viewport's width: the ball is
//! width-capped (30% of the terminal width wherever the cap binds —
//! the narrow-screen read: a small shadow, a long disk), and the
//! disk's scale unit stretches to fill the width on wide terminals
//! (the majestic full-width read). (3) The see-saw's attitude window
//! is re-cut: the 70-110 degree near-vertical band is excluded
//! outright (the disk never parks or sweeps where the terminal
//! screen would clip it), with a dynamic tilt cap lowering the
//! ceiling further on viewports whose vertical budget cannot host
//! the menu's 60-degree rung.
//!
//! Geometry: terminal cells are roughly 1:2 (width:height), so a circle
//! on the physical screen is an ellipse in cell space. All radius math
//! runs in line-height units: a cell offset (dx cols, dy lines) sits at
//! screen distance sqrt((dx / 2)^2 + dy^2). The ball outer radius is a
//! fraction of the viewport's limiting half-extent, so it scales with any
//! screen size from 80x24 to 400x100 without touching the constants.
//!
//! Radial brightness: the annulus between the core radius and the outer
//! radius is banded like the vortex drain, but inverted — the brightest
//! zone hugs the event horizon (the accretion photon ring) and dims
//! toward the outer rim, the way a real black hole silhouette reads:
//! a dark core wrapped in a thin blazing edge.
//!
//! Stage roadmap (owner-approved staged rollout, one commit per stage):
//! - stage 1: the ball — owner visual verification (rated 10/10).
//! - stage 2 (this file + `ring.rs`/`halo.rs`, revisions 2.1-2.7): the
//!   orbital ring stack + the double halo stream — owner visual
//!   verification (stage 2.7 rated 10/10, masterpiece).
//! - stage 3 (`infall.rs`, shipped): glyph rain infall — falling
//!   glyphs that bend elegantly into the core when they approach the
//!   ring's capture radius, spiraling through the accretion brake
//!   and vanishing at the event horizon.
//!
//! The glyph pool for each cell re-rolls through a low-probability
//! shimmer gate (matrix DNA: mutation is the engine's life sign), and a
//! full re-roll is armed whenever the charset changes so the ball never
//! carries stale glyphs from a previous pool.
//!
//! Cleanup follows the monolith/vortex three-pass diff pattern: draw
//! into `current_cells`, tag with the `drawn_gen` generation counter,
//! then clear only previous cells NOT redrawn this frame (phosphor
//! metadata and frame blank). The ball is static so its cells are all
//! redrawn each frame, but the stage-2 ring motes vacate cells as they
//! orbit — exactly the case the diff cleanup exists for (stage 3's
//! infalling glyphs will lean on the same contract).

use std::time::{Duration, Instant};

use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
};

use crate::frame::Frame;

use super::super::super::render::DrawCtx;
use super::super::monolith::monolith_helpers::{
    clear_cell, clear_phosphor_metadata, pick_pool_char,
};
use super::super::monolith::{BrightnessLevel, MonolithCleanup};
use super::ball_helpers::{bump_level, conveyor_char, draw_ball_cell, level_for_ring_band};
use super::formation::{
    cross_active, formation_phase, horizon_visibility, seed_center_level, seed_cross_level,
    FormationPhase,
};
use super::halo::{activate_halo_mote, advance_halo_mote, halo_mote_visible, project_halo_mote};
use super::infall::InfallStream;
use super::ring::{
    activate_ring_mote, advance_ring_mote, floor_head_base_at_hot, level_for_ring_z,
    occludes_ring_cell, project_ring_mote, proximity_level, step_down_level, BlackHoleRandom,
    BlackHoleSpawnParams, BlackHoleStep, RingMote,
};
use super::roll::RingRoll;
use super::RollFrame;

/// One drawn ball cell: grid position plus its radial brightness band.
/// Own struct instead of reusing monolith's `DrawnCell` because the ball
/// has no Segment/Spine kind distinction (same shape as `VortexCell`,
/// plus the precomputed band level — the band is static geometry, so it
/// is computed once at reset instead of per frame).
#[derive(Clone, Copy, Debug)]
pub(crate) struct BlackHoleCell {
    pub(crate) col: u16,
    pub(crate) line: u16,
    pub(crate) level: BrightnessLevel,
}

/// Cell-space aspect divisor: terminal cells are ~1:2 (width:height), so
/// one column of travel covers half a line-height of screen distance.
/// Shared with `ring.rs` (the orbital projection runs in the same
/// line-height units as the ball raster).
pub(crate) const CELL_ASPECT_DIVISOR: f32 = 2.0;

/// Shorthand rank for the brightness ladder (Ghost = 0 ... Core = 4).
/// BrightnessLevel carries no PartialEq, so band tests compare ranks.
#[cfg(test)]
pub(crate) fn level_rank(level: BrightnessLevel) -> u8 {
    match level {
        BrightnessLevel::Ghost => 0,
        BrightnessLevel::Dim => 1,
        BrightnessLevel::Mid => 2,
        BrightnessLevel::Hot => 3,
        BrightnessLevel::Core => 4,
    }
}

pub(crate) struct BlackHoleRain {
    /// Precomputed ring annulus cells (geometry is static, so it is
    /// built once per reset instead of rescanned per frame).
    ring_cells: Vec<BlackHoleCell>,
    /// Parallel glyph per ring cell, drawn from the active charset pool.
    glyphs: Vec<char>,
    /// Set when the glyph pool may have changed (charset switch, style
    /// entry); the next draw re-rolls every glyph from the live pool.
    glyphs_stale: bool,
    /// Palette slot adopted at entry / palette transition (the ball is a
    /// single body, so one slot covers every cell).
    palette_slot: u8,
    /// Stage 2 orbital ring: one mote per column (the family lane
    /// model). The per-mote physics (RK4 Lorenz turbulence + Keplerian
    /// advance + projection) lives in `ring.rs`.
    motes: Vec<RingMote>,
    /// Active mote count (the spawn target's deficit baseline).
    active_motes: usize,
    /// Rotating scan cursor for amortized O(1) free-slot search
    /// (mirrors `VortexRain::spawn_scan_idx`).
    spawn_scan_idx: usize,
    /// Stage-2.6 halo streams: the arc-riding companion pool (one
    /// mote per column, the lane model — the physics lives in
    /// `halo.rs`). The stage-2.7 double upward stream: the inner
    /// upper riders co-ride the lensing arc (1.30), the outer upper
    /// riders the wider crown (1.48), the lower riders the rare
    /// mirrored circle under the shadow.
    halo_motes: Vec<RingMote>,
    /// Active halo mote count (the halo spawn target's deficit
    /// baseline).
    active_halo: usize,
    /// Rotating scan cursor for the halo pool's free-slot search
    /// (same amortization contract as the ring's cursor).
    halo_scan_idx: usize,
    /// Fractional spawn-remainder carry of the halo pool (the ring's
    /// counterpart lives in the cloud layer's shared field; the halo
    /// keeps its own because the two pools budget independently).
    halo_spawn_remainder: f32,
    /// Stage 3 glyph infall: the third mote pool — falling glyphs
    /// bending through the hole's gravitational field into the core
    /// (the whole layer — pool, physics, spawn/advance/draw passes —
    /// lives in `infall.rs`; this field is the orchestrator's only
    /// handle on it, keeping the LOC cap debt of this file flat as
    /// the system grows its third pool).
    infall: InfallStream,
    /// Bresenham stream-split accumulator: each halo activation adds
    /// the upper FAMILY's combined share (the two upper crowns, 0.82)
    /// and the running fractional part decides upper family vs the
    /// rare lower stream — the split holds EXACTLY on every pool fill
    /// (a random pick would only hold on average, and a small pool
    /// can land a visibly inverted split on an unlucky seed — the
    /// lower stream must read rare EVERY run, the deterministic
    /// splitter guarantees it).
    halo_tag_acc: f32,
    /// Upper-family lane toggle (stage 2.7, the double upward
    /// stream): alternates every upper-family activation between the
    /// inner crown (1.30 lensing circle) and the outer crown (1.48
    /// arc) — the strict alternation splits the family exactly in
    /// half, matching the two streams' equal spawn weights.
    halo_upper_lane: bool,
    /// Alternating lobe selector for the Lorenz-state seeding (parity
    /// with the lorenz style's spawn — balanced wobble distribution).
    next_lobe: u8,
    /// Global motion clock. dt = now - last_step clamped by
    /// max_sim_delta, scaled by resume_blend (the anti-teleport
    /// contract shared with the structured family).
    last_step: Option<Instant>,
    /// Cached ball anchor: integer center cell (the discrete midpoint
    /// the ball raster scans around). Rebuilt at reset.
    center_col: i32,
    center_line: i32,
    /// Cached ball outer radius in line-height units — the ring
    /// radii are multiples of it (scales with any screen size).
    /// NIGHT-research-9: the effective radius is the SMALLER of the
    /// ball fraction of the viewport's limiting half-extent and the
    /// width cap (`BLACK_HOLE_BALL_WIDTH_MAX` of the half-width), so
    /// narrow viewports shrink the shadow and the disk dominates.
    ball_outer_r: f32,
    /// The disk's scale unit (line-height units) — the LARGER of the
    /// viewport's limiting half-extent and
    /// `BLACK_HOLE_DISK_WIDTH_FRACTION` of the half-width
    /// (NIGHT-research-9). The tier semi-major axes key on this, not
    /// on the ball: wide terminals stretch the disk toward the 92%
    /// half-width clamp (the majestic full-width read) while the
    /// ball/halo family stays proportionally compact.
    disk_unit: f32,
    /// The ring proximity ladder's gain (dimensionless, 1.0 at the
    /// canonical ball-to-disk proportion): the fade/hot keypoints
    /// scale with the disk unit's stretch so the brightness profile
    /// rides the disk's real reach at every viewport class.
    proximity_gain: f32,
    /// The see-saw's dynamic attitude ceiling (radians, positive) —
    /// the menu pick clamp, recomputed at reset from the vertical
    /// budget (92% of the half-height over the tier-0 semi-major
    /// with wobble headroom). Stored so style re-entry can re-apply
    /// it to a freshly constructed scheduler.
    roll_tilt_cap: f32,
    /// Ball rim spin phase (radians, unbounded — read through cos so
    /// no wrapping bookkeeping). Advanced by the same clock and
    /// omega as the ring's mean motion: the hole visibly rotates
    /// with its disk (stage 2.1, owner feedback).
    spin_phase: f32,
    /// Stage-2.4 see-saw roll scheduler: the disk stack's attitude
    /// angle (0 = the flat horizontal rest line, positive lifts the
    /// left end). Ticked on the advance pass's wall clock; the draw
    /// pass feeds the live angle to every ring projection so the
    /// whole stack pivots rigidly around the hole. Reset on style
    /// entry (fresh choreography with the formation); a pure resize
    /// keeps the current attitude.
    roll: RingRoll,
    /// Per-cell aspect-corrected angle around the ball center
    /// (parallel to `ring_cells` — the conveyor's input geometry,
    /// computed once at reset).
    cell_angles: Vec<f32>,
    /// Last conveyor bucket index per cell (parallel to `ring_cells`
    /// — motion-gated glyph re-roll bookkeeping).
    cell_buckets: Vec<i32>,
    /// Per-cell normalized radius (0.0 at the event horizon, 1.0 at
    /// the outer rim; parallel to `ring_cells`) — the formation
    /// intro's horizon-bloom filter input.
    cell_dist_norm: Vec<f32>,
    /// Formation clock (seconds since formation start; rides the
    /// advance pass's dt-wall — pause stops the birth mid-phase and
    /// resume continues it, exactly like the motes).
    formation_t: f32,
    /// Set once the horizon bloom completes — the steady-state gate
    /// for mote spawning (accretion begins when the hole is whole).
    /// A pure resize keeps it set (the hole does not re-form on a
    /// resize); style re-entry clears it via `begin_formation`.
    formed: bool,
    /// The seed glyph (the singularity dot's character — persisted
    /// so the dot doesn't flicker, re-rolled on entry/shimmer).
    seed_glyph: char,
    /// Arms a seed glyph re-roll on the next draw (style entry / draw
    /// history invalidation).
    seed_stale: bool,
    /// Last frame's drawn cells (diff cleanup input).
    previous_cells: Vec<BlackHoleCell>,
    /// This frame's drawn cells (diff cleanup output).
    current_cells: Vec<BlackHoleCell>,
    /// Generation tags for the diff cleanup pass (flat: col * lines +
    /// line), rebuilt when the viewport dimensions change.
    drawn_gen: Vec<u32>,
    drawn_gen_counter: u32,
    drawn_gen_dims: (u16, u16),
}

impl BlackHoleRain {
    pub(crate) fn new() -> Self {
        Self {
            ring_cells: Vec::new(),
            glyphs: Vec::new(),
            glyphs_stale: true,
            palette_slot: 0,
            motes: Vec::new(),
            active_motes: 0,
            spawn_scan_idx: 0,
            halo_motes: Vec::new(),
            active_halo: 0,
            halo_scan_idx: 0,
            halo_spawn_remainder: 0.0,
            halo_tag_acc: 0.0,
            halo_upper_lane: false,
            infall: InfallStream::new(),
            next_lobe: 0,
            last_step: None,
            center_col: 0,
            center_line: 0,
            ball_outer_r: 0.0,
            disk_unit: 0.0,
            proximity_gain: 1.0,
            roll_tilt_cap: RingRoll::MENU_MAX_RADIANS,
            spin_phase: 0.0,
            roll: RingRoll::new(),
            cell_angles: Vec::new(),
            cell_buckets: Vec::new(),
            cell_dist_norm: Vec::new(),
            formation_t: 0.0,
            formed: false,
            seed_glyph: '0',
            seed_stale: true,
            previous_cells: Vec::new(),
            current_cells: Vec::new(),
            drawn_gen: Vec::new(),
            drawn_gen_counter: 0,
            drawn_gen_dims: (0, 0),
        }
    }

    /// Rebuild the ball geometry for a new viewport (or style entry).
    ///
    /// The outer radius is the SMALLER of the ball fraction of the
    /// viewport's limiting half-extent (line-height units: `unit =
    /// min(cols / 4, lines / 2)` — `cols / 4` is the half-width
    /// expressed in line units through the cell aspect, `lines / 2`
    /// the half-height) and the width cap share of the half-width
    /// (NIGHT-research-9, `BLACK_HOLE_BALL_WIDTH_MAX`): the cap binds
    /// on every viewport up to roughly 1.8:1 aspect (the common
    /// terminal classes — the shadow reads 30% of the terminal width,
    /// the disk dominates the composition, the owner's narrow-screen
    /// read); beyond that the height bound takes over.
    ///
    /// The disk's scale unit (stored as `disk_unit`) is the LARGER of
    /// the limiting half-extent and
    /// `BLACK_HOLE_DISK_WIDTH_FRACTION` of the half-width: on wide
    /// viewports the disk stretches toward the projection's
    /// 92%-of-half-width clamp (the majestic full-width read) while
    /// the ball/halo family stays ball-keyed. The ring proximity
    /// ladder's gain (`proximity_gain`) and the see-saw's dynamic
    /// tilt cap (`roll_tilt_cap`, from the vertical budget over the
    /// tier-0 semi-major) are recomputed in the same pass.
    pub(crate) fn reset(&mut self, cols: u16, lines: u16) {
        self.ring_cells.clear();
        if cols == 0 || lines == 0 {
            self.glyphs.clear();
            self.glyphs_stale = true;
            self.reset_mote_pools(0, 0, 0.0);
            self.center_col = 0;
            self.center_line = 0;
            self.ball_outer_r = 0.0;
            self.disk_unit = 0.0;
            self.proximity_gain = 1.0;
            self.cell_angles.clear();
            self.cell_buckets.clear();
            self.cell_dist_norm.clear();
            self.clear_draw_history();
            return;
        }

        let half_w = cols as f32 / (CELL_ASPECT_DIVISOR * 2.0);
        let half_h = lines as f32 / 2.0;
        let unit = half_w.min(half_h);
        // NIGHT-research-9 sizing: width-capped ball (the shadow never
        // exceeds BALL_WIDTH_MAX of the half-width, so narrow viewports
        // shrink it and the disk reads long), width-stretched disk unit
        // (wide viewports fill the width), and the derived gain/cap.
        let outer_r = (unit * crate::constants::BLACK_HOLE_BALL_FRACTION)
            .min(half_w * crate::constants::BLACK_HOLE_BALL_WIDTH_MAX);
        let core_r = outer_r * crate::constants::BLACK_HOLE_CORE_FRACTION;
        self.disk_unit = unit.max(half_w * crate::constants::BLACK_HOLE_DISK_WIDTH_FRACTION);
        // The proximity ladder's gain: the disk unit's stretch factor
        // vs the canonical ball-derived unit (1.0 whenever the ball is
        // neither width-capped nor the disk width-stretched).
        self.proximity_gain =
            self.disk_unit * crate::constants::BLACK_HOLE_BALL_FRACTION / outer_r.max(0.05);
        // The see-saw's dynamic tilt cap: the tier-0 semi-major (with
        // wobble headroom) times the tilt's sine must stay inside 92%
        // of the half-height — the NIGHT-research-9 anti-clip guard
        // (the 70-110 degree window is excluded outright by the menu;
        // this cap lowers the ceiling for viewports whose vertical
        // budget cannot host even the 60-degree rung).
        let a_tier0 = crate::constants::BLACK_HOLE_RING_MAJOR_FRACTION
            * crate::constants::BLACK_HOLE_RING_TIERS[0].major_scale
            * self.disk_unit
            * 1.10;
        self.roll_tilt_cap = ((0.92 * half_h / a_tier0.max(0.05)).clamp(0.0, 1.0))
            .asin()
            .min(RingRoll::MENU_MAX_RADIANS);
        self.roll.set_tilt_cap(self.roll_tilt_cap);

        // Degenerate viewport guard: a ball thinner than one cell of
        // annulus width draws nothing (prevents a zero-width division
        // below and an empty-shell flash on tiny terminals). No ball,
        // no ring anchor — the mote pool stays empty so nothing
        // degenerate projects around a zero-radius hole.
        if outer_r - core_r < 1.0 {
            self.glyphs.clear();
            self.glyphs_stale = true;
            self.reset_mote_pools(0, lines, 0.0);
            self.center_col = 0;
            self.center_line = 0;
            self.ball_outer_r = 0.0;
            self.disk_unit = 0.0;
            self.proximity_gain = 1.0;
            self.cell_angles.clear();
            self.cell_buckets.clear();
            self.cell_dist_norm.clear();
            self.clear_draw_history();
            return;
        }

        // Integer center cell (the grid's discrete midpoint). Even
        // dimensions land the true center between cells; (n - 1) / 2
        // picks the lower-mid cell so the scan offsets are exact
        // integers and the distance math stays consistent with the
        // rasterized geometry.
        let cx = ((cols - 1) / 2) as i32;
        let cy = ((lines - 1) / 2) as i32;
        let half_w_cells = (outer_r * CELL_ASPECT_DIVISOR).ceil() as i32;
        let half_h_cells = outer_r.ceil() as i32;
        let annulus_width = outer_r - core_r;

        // Cache the ball anchor: the ring projects its ellipse around
        // this exact center and radius every frame (line-height units,
        // same math as the raster above).
        self.center_col = cx;
        self.center_line = cy;
        self.ball_outer_r = outer_r;

        for dy in -half_h_cells..=half_h_cells {
            for dx in -half_w_cells..=half_w_cells {
                let col = (cx + dx) as u16;
                let line = (cy + dy) as u16;
                if col >= cols || line >= lines {
                    continue;
                }
                let dist = ((dx as f32 / CELL_ASPECT_DIVISOR).powi(2) + (dy as f32).powi(2)).sqrt();
                if dist < core_r || dist > outer_r {
                    continue;
                }
                let t = ((dist - core_r) / annulus_width).clamp(0.0, 1.0);
                self.ring_cells.push(BlackHoleCell {
                    col,
                    line,
                    level: level_for_ring_band(t),
                });
            }
        }

        self.glyphs.clear();
        self.glyphs.resize_with(self.ring_cells.len(), || '0');
        self.glyphs_stale = true;

        // Rim conveyor geometry: each cell's angle around the center
        // (aspect-corrected, the same units the ring projection uses)
        // drives the sliding glyph buckets — the visible surface
        // rotation of the ball. Buckets start at 0 so the first draw
        // re-rolls every glyph to its bucket's character. The
        // normalized radius (formation's horizon-bloom filter) is
        // captured in the same pass.
        self.cell_angles = self
            .ring_cells
            .iter()
            .map(|c| {
                let dx = (c.col as f32 - cx as f32) / CELL_ASPECT_DIVISOR;
                let dy = c.line as f32 - cy as f32;
                dy.atan2(dx)
            })
            .collect();
        self.cell_buckets = vec![0; self.ring_cells.len()];
        self.cell_dist_norm = self
            .ring_cells
            .iter()
            .map(|c| {
                let dx = (c.col as f32 - cx as f32) / CELL_ASPECT_DIVISOR;
                let dy = c.line as f32 - cy as f32;
                ((dx * dx + dy * dy).sqrt() - core_r) / annulus_width
            })
            .collect();

        self.reset_mote_pools(cols, lines, outer_r);
        self.clear_draw_history();
    }

    /// Replay the formation intro (stage 2.2): rewind the formation
    /// clock and clear the formed flag — the next frames run the
    /// birth sequence (seed dot -> collapse flare -> horizon bloom ->
    /// accretion). Called on style ENTRY only (scene switches and
    /// first launch); a pure resize keeps the steady state. Stage
    /// 2.4: the roll scheduler resets too — the stack enters flat
    /// and holds the horizontal Gargantua line for the first flat
    /// hold before its first tilt. NIGHT-research-9: the fresh
    /// scheduler re-applies the stored dynamic tilt cap (the
    /// viewport's vertical budget survives the re-entry).
    pub(crate) fn begin_formation(&mut self) {
        self.formation_t = 0.0;
        self.formed = false;
        self.seed_stale = true;
        self.roll = RingRoll::new();
        self.roll.set_tilt_cap(self.roll_tilt_cap);
    }

    /// Rebuild the ring, halo and infall pools: one mote per column
    /// each (the family lane model — a wider viewport hosts a longer
    /// ring AND a denser stream population, so both pools resize),
    /// all vacant. Pass 0 to leave the pools empty (degenerate
    /// viewports with no ball anchor). The shared clock resets once
    /// for all pools (one motion clock per body system), and the
    /// infall layer's spawn/exit envelope re-caches from the ball
    /// anchor (stage 3).
    fn reset_mote_pools(&mut self, cols: u16, lines: u16, ball_outer_r: f32) {
        self.motes.clear();
        self.halo_motes.clear();
        if cols > 0 {
            self.motes.resize_with(cols as usize, RingMote::vacant);
            self.halo_motes.resize_with(cols as usize, RingMote::vacant);
        }
        self.active_motes = 0;
        self.spawn_scan_idx = 0;
        self.active_halo = 0;
        self.halo_scan_idx = 0;
        self.halo_spawn_remainder = 0.0;
        self.halo_tag_acc = 0.0;
        self.halo_upper_lane = false;
        self.infall.reset_pool(cols, lines, ball_outer_r);
        self.next_lobe = 0;
        self.last_step = None;
    }

    /// Steady-state drawn-glyph count (the HUD active metric): every
    /// ball annulus cell plus every active ring mote head plus every
    /// active halo stream rider plus every falling infall glyph — the
    /// honest figure of what the style animates each frame.
    pub(crate) fn active_count(&self) -> usize {
        self.ring_cells.len() + self.active_motes + self.active_halo + self.infall.active_count()
    }

    /// Palette transition completion: the ball adopts the new slot,
    /// and so does every active ring mote, halo stream rider and
    /// infalling glyph (family contract — parity with the
    /// lorenz/vortex mote adoption).
    pub(crate) fn adopt_palette_slot(&mut self, palette_slot: u8) {
        self.palette_slot = palette_slot;
        for m in &mut self.motes {
            if m.active {
                m.palette_slot = palette_slot;
            }
        }
        for m in &mut self.halo_motes {
            if m.active {
                m.palette_slot = palette_slot;
            }
        }
        self.infall.adopt_palette_slot(palette_slot);
    }

    /// Steady-state active-mote target from pool size + density
    /// (mirrors `LorenzRain::target_active_count` with the ring's
    /// ratios — stage 2.3 raises the floor to 0.55 and the cap to
    /// the full pool: the ring must read as a solid band, not a
    /// sparse stream, per the owner's Gargantua reference).
    fn target_active_motes(lanes: usize, density: f32) -> usize {
        if lanes == 0 {
            return 0;
        }
        let ratio = (crate::constants::BLACK_HOLE_RING_ACTIVE_BASE
            + density.clamp(0.01, 5.0) * crate::constants::BLACK_HOLE_RING_ACTIVE_DENSITY_MULT)
            .clamp(0.02, crate::constants::BLACK_HOLE_RING_ACTIVE_MAX);
        ((lanes as f32 * ratio).round() as usize).clamp(1, lanes)
    }

    /// Steady-state active-halo target from pool size + density (the
    /// halo twin of `target_active_motes`: the base is lower because
    /// each rider is visible only through its own semicircle — the
    /// pool carries both streams' traffic).
    fn target_active_halo(lanes: usize, density: f32) -> usize {
        if lanes == 0 {
            return 0;
        }
        let ratio = (crate::constants::BLACK_HOLE_HALO_ACTIVE_BASE
            + density.clamp(0.01, 5.0) * crate::constants::BLACK_HOLE_HALO_ACTIVE_DENSITY_MULT)
            .clamp(0.02, crate::constants::BLACK_HOLE_HALO_ACTIVE_MAX);
        ((lanes as f32 * ratio).round() as usize).clamp(1, lanes)
    }

    /// Amortized free-slot scan over a mote pool (rotating cursor —
    /// mirrors vortex's `find_inactive_mote`); shared by the ring and
    /// halo pools so both spawn passes amortize identically.
    fn find_inactive_in(pool: &[RingMote], cursor: &mut usize) -> Option<usize> {
        let len = pool.len();
        if len == 0 {
            return None;
        }
        for step in 0..len {
            let idx = (*cursor + step) % len;
            if !pool[idx].active {
                *cursor = (idx + 1) % len;
                return Some(idx);
            }
        }
        None
    }

    /// Amortized free-slot scan (rotating cursor — mirrors vortex's
    /// `find_inactive_mote`).
    fn find_inactive_mote(&mut self) -> Option<usize> {
        Self::find_inactive_in(&self.motes, &mut self.spawn_scan_idx)
    }

    /// The halo pool's free-slot scan (the ring twin's cursor).
    fn find_inactive_halo(&mut self) -> Option<usize> {
        Self::find_inactive_in(&self.halo_motes, &mut self.halo_scan_idx)
    }

    /// Spawn pass — accumulator pattern identical to
    /// `LorenzRain::spawn` (deficit-bounded budget + fractional
    /// remainder carry). New motes enter at a uniform random orbital
    /// phase, so the stream populates around the full circumference
    /// instead of clumping at one angle. The stage-2.6 halo pool and
    /// the stage-3 infall pool spawn through the same contract right
    /// after the ring block (their remainders are internal: the three
    /// pools budget independently, the cloud layer's shared field
    /// stays the ring's).
    pub(crate) fn spawn(
        &mut self,
        elapsed: Duration,
        spawn_remainder: &mut f32,
        params: &BlackHoleSpawnParams,
        random: &mut BlackHoleRandom<'_>,
    ) {
        if params.cols == 0
            || params.lines == 0
            || self.motes.is_empty()
            || self.halo_motes.is_empty()
            || self.ball_outer_r < 1.0
        {
            *spawn_remainder = 0.0;
            self.halo_spawn_remainder = 0.0;
            self.infall.hold_spawn_budget();
            return;
        }

        // Formation gate (stage 2.2): accretion begins only when the
        // hole is whole — no motes orbit a half-born horizon, and no
        // rain falls into it. The remainders are zeroed so the
        // post-formation spawn budgets start clean instead of banking
        // pre-formation elapsed time.
        if !self.formed {
            *spawn_remainder = 0.0;
            self.halo_spawn_remainder = 0.0;
            self.infall.hold_spawn_budget();
            return;
        }

        let target = Self::target_active_motes(self.motes.len(), params.density);
        if self.active_motes >= target {
            *spawn_remainder = (*spawn_remainder).min(crate::constants::SPAWN_REMAINDER_CAP);
        } else {
            let deficit = target - self.active_motes;
            let spawn_rate = (target as f32 * crate::constants::BLACK_HOLE_RING_SPAWN_RATE_MULT
                + crate::constants::BLACK_HOLE_RING_SPAWN_RATE_FLOOR)
                * params.spawn_scale;
            let budget = elapsed.as_secs_f32() * spawn_rate
                + (*spawn_remainder).min(crate::constants::SPAWN_REMAINDER_CAP);
            if !budget.is_finite() || budget <= 0.0 {
                *spawn_remainder = 0.0;
            } else {
                let to_spawn = (budget.floor() as usize).min(deficit);
                *spawn_remainder =
                    (budget - to_spawn as f32).min(crate::constants::SPAWN_REMAINDER_CAP);
                for _ in 0..to_spawn {
                    let Some(idx) = self.find_inactive_mote() else {
                        break;
                    };
                    let lobe_sign = if self.next_lobe == 0 { 1.0 } else { -1.0 };
                    self.next_lobe = (self.next_lobe + 1) % 2;
                    activate_ring_mote(
                        &mut self.motes[idx],
                        lobe_sign,
                        params.active_palette_slot,
                        random.rand_chance,
                        random.rng,
                    );
                    self.active_motes += 1;
                }
            }
        }

        // Stage 3 glyph infall: the falling-rain pool spawns on the
        // same deficit-bounded accumulator contract (its own internal
        // remainder, the cached spawn envelope from the ball anchor).
        // Runs BEFORE the halo block so the halo's early returns skip
        // nothing of the rain.
        self.infall.spawn(elapsed, params, random);

        // Halo streams (stage 2.6, re-split stage 2.7): the arc pool
        // spawns on the same deficit-bounded accumulator contract —
        // the three lane tags ride the weighted pick inside the
        // activation, so the double crown and its rare mirror fill
        // at the same gradual pace as the disk bands.
        let halo_target = Self::target_active_halo(self.halo_motes.len(), params.density);
        if self.active_halo >= halo_target {
            self.halo_spawn_remainder = self
                .halo_spawn_remainder
                .min(crate::constants::SPAWN_REMAINDER_CAP);
            return;
        }
        let halo_deficit = halo_target - self.active_halo;
        let halo_rate = (halo_target as f32 * crate::constants::BLACK_HOLE_HALO_SPAWN_RATE_MULT
            + crate::constants::BLACK_HOLE_HALO_SPAWN_RATE_FLOOR)
            * params.spawn_scale;
        let halo_budget = elapsed.as_secs_f32() * halo_rate
            + self
                .halo_spawn_remainder
                .min(crate::constants::SPAWN_REMAINDER_CAP);
        if !halo_budget.is_finite() || halo_budget <= 0.0 {
            self.halo_spawn_remainder = 0.0;
            return;
        }
        let halo_to_spawn = (halo_budget.floor() as usize).min(halo_deficit);
        self.halo_spawn_remainder =
            (halo_budget - halo_to_spawn as f32).min(crate::constants::SPAWN_REMAINDER_CAP);
        for _ in 0..halo_to_spawn {
            let Some(idx) = self.find_inactive_halo() else {
                break;
            };
            // The stage-2.7 three-way split: the Bresenham accumulator
            // walks the upper FAMILY's combined share (inner + outer
            // crowns, 0.82); a fill below 1.0 lands the rare lower
            // stream, a wrap lands the upper family — and within the
            // family the lane toggle alternates inner/outer at
            // exactly the two crowns' equal shares, so the double
            // upward stream fills evenly and the lower stream stays
            // rare on every seed, every pool fill.
            self.halo_tag_acc += crate::constants::BLACK_HOLE_HALO_UPPER_WEIGHT
                + crate::constants::BLACK_HOLE_HALO_OUTER_WEIGHT;
            let stream_tag = if self.halo_tag_acc >= 1.0 {
                self.halo_tag_acc -= 1.0;
                if self.halo_upper_lane {
                    self.halo_upper_lane = false;
                    super::halo::HALO_STREAM_TAG_UPPER
                } else {
                    self.halo_upper_lane = true;
                    super::halo::HALO_STREAM_TAG_UPPER_OUTER
                }
            } else {
                super::halo::HALO_STREAM_TAG_LOWER
            };
            let lobe_sign = if self.next_lobe == 0 { 1.0 } else { -1.0 };
            self.next_lobe = (self.next_lobe + 1) % 2;
            activate_halo_mote(
                &mut self.halo_motes[idx],
                stream_tag,
                lobe_sign,
                params.active_palette_slot,
                random.rand_chance,
                random.rng,
            );
            self.active_halo += 1;
        }
    }

    /// Motion pass — the clock owner for the ring, the halo streams
    /// and the glyph infall (the per-mote RK4 Lorenz step + Keplerian
    /// advance live in `ring.rs` and `halo.rs`; the infall's
    /// gravitational integration lives in `infall.rs`). dt = now -
    /// last_step clamped by max_sim_delta and scaled by resume_blend
    /// (the anti-teleport contract shared with the structured family);
    /// a fully-paused run simply stops integrating. The ball's rim
    /// spin advances on the same clock and the same mean omega as
    /// the motes — the co-rotation contract — even while no mote is
    /// active (the hole spins on its own phase from the first
    /// frame).
    pub(crate) fn advance(&mut self, step: &BlackHoleStep) {
        let dt_wall = match self.last_step {
            Some(last) => {
                step.now
                    .saturating_duration_since(last)
                    .as_secs_f32()
                    .min(step.max_sim_delta.as_secs_f32())
                    .max(0.0)
                    * step.resume_blend.clamp(0.0, 1.0)
            }
            None => 0.0,
        };
        self.last_step = Some(step.now);
        if dt_wall <= 0.0 {
            return;
        }

        // Integration dt: chars_per_sec mapped onto attractor time
        // (RK4 stability regime — same mapping as the lorenz style),
        // plus the mean orbital rate in radians per wall second.
        let dt_lorenz_base =
            step.chars_per_sec.max(0.0) * crate::constants::BLACK_HOLE_RING_DT_PER_CPS * dt_wall;
        let omega_base =
            step.chars_per_sec.max(0.0) * crate::constants::BLACK_HOLE_RING_OMEGA_PER_CPS;

        // Ball rim co-rotation: the spin phase rides the same clock
        // and omega as the ring's mean motion, scaled by SPIN_RATE
        // (1.0 = lockstep with the disk's phase — the hole rotates
        // with its ring, per the owner's stage-2 feedback).
        self.spin_phase += omega_base * dt_wall * crate::constants::BLACK_HOLE_RING_SPIN_RATE;

        // See-saw roll (stage 2.4, the lever): the stack's attitude
        // rides the same wall clock — pause freezes the lever
        // mid-swing, resume continues the pivot. Ticked even with no
        // active motes (the schedule is the disk's, not any single
        // mote's), so the first excursion timing is anchored to the
        // formation, not to the spawn luck.
        self.roll.tick(dt_wall);

        // Formation clock (stage 2.2): the birth sequence rides the
        // same wall-clock dt as everything else — pause freezes the
        // hole mid-birth, resume continues it. The clock stops for
        // good once the horizon bloom completes (the formed flag
        // then opens the spawn gate).
        if !self.formed {
            let total = super::formation::formation_total_secs();
            self.formation_t = (self.formation_t + dt_wall).min(total);
            if self.formation_t >= total {
                self.formed = true;
            }
        }

        if self.active_motes == 0 && self.active_halo == 0 && self.infall.active_count() == 0 {
            return;
        }

        let mut absorbed = 0usize;
        // The disk gain (NIGHT-research-9): the disk unit's stretch
        // factor, threaded to the Keplerian shear normalization so the
        // advance physics tracks the real semi-major-to-ball ratio.
        let disk_gain = self.proximity_gain;
        for m in &mut self.motes {
            if !m.active {
                continue;
            }
            if advance_ring_mote(m, dt_wall, dt_lorenz_base, omega_base, disk_gain) {
                absorbed += 1;
            }
        }
        if absorbed > 0 {
            self.active_motes = self.active_motes.saturating_sub(absorbed);
        }

        // Halo streams (stage 2.6): the arc riders integrate on the
        // same clock, the same attractor time and the same base
        // omega — only their Keplerian pace (the outer-lane scaling)
        // and their projection differ from the ring motes.
        if self.active_halo > 0 {
            let mut halo_absorbed = 0usize;
            for m in &mut self.halo_motes {
                if !m.active {
                    continue;
                }
                if advance_halo_mote(m, dt_wall, dt_lorenz_base, omega_base) {
                    halo_absorbed += 1;
                }
            }
            if halo_absorbed > 0 {
                self.active_halo = self.active_halo.saturating_sub(halo_absorbed);
            }
        }

        // Glyph infall (stage 3): the falling rain integrates on the
        // same wall clock through its own sim-time coupling — one
        // field, one clock, three projections. Pause freezes a glyph
        // mid-plunge, resume continues the fall.
        self.infall.advance(dt_wall, step.chars_per_sec);
    }

    /// Drop the diff-cleanup history and arm a full glyph re-roll
    /// (semantic invalidation: charset switch, palette change, forced
    /// redraw). The next draw pass rebuilds the baseline from scratch.
    pub(crate) fn clear_draw_history(&mut self) {
        self.previous_cells.clear();
        self.current_cells.clear();
        self.drawn_gen.clear();
        self.drawn_gen_counter = 0;
        self.drawn_gen_dims = (0, 0);
        self.glyphs_stale = true;
        self.seed_stale = true;
    }

    /// Draw pass — full annulus render + ring mote render +
    /// monolith-style diff cleanup.
    ///
    /// Stage 2 motion budget: the ball redraws its full annulus every
    /// frame (static body), the ring motes draw head + comet trail on
    /// their projected cells (far-side cells inside the ball silhouette
    /// are occluded — passing behind the hole), and the diff cleanup
    /// clears the cells the orbiting motes vacated — the contract this
    /// pattern has carried since stage 1, now load-bearing.
    pub(crate) fn draw(
        &mut self,
        ctx: &DrawCtx<'_>,
        frame: &mut Frame,
        cleanup: &mut MonolithCleanup<'_>,
        rng: &mut StdRng,
        rand_chance: &Uniform<f32>,
    ) {
        let lines_us = ctx.lines as usize;
        self.current_cells.clear();
        self.current_cells.reserve(self.ring_cells.len());

        // Charset switches and style entries arm a full re-roll so the
        // ball never carries glyphs from a stale pool.
        if self.glyphs_stale {
            for g in &mut self.glyphs {
                *g = pick_pool_char(ctx.char_pool, rand_chance, rng);
            }
            self.glyphs_stale = false;
        }

        // Formation intro (stage 2.2): during the dot phases only the
        // singularity seed (plus its collapse cross flare) draws — the
        // annulus and the ring do not exist yet. During the horizon
        // bloom the annulus draws from the inside out (photon-ring
        // cells first, outer rim last) on the ease-out visibility; the
        // steady state draws everything. The dot cells flow through
        // the same current_cells stream so the diff cleanup clears
        // them when the bloom replaces them.
        match formation_phase(self.formation_t) {
            FormationPhase::Seed | FormationPhase::Collapse => {
                self.draw_seed_dot(ctx, frame, rand_chance, rng);
            }
            FormationPhase::Horizon | FormationPhase::Steady => {
                let visibility = horizon_visibility(self.formation_t);
                for (idx, cell) in self.ring_cells.iter().enumerate() {
                    if cell.col >= ctx.cols || cell.line >= ctx.lines {
                        // Viewport shrank without a reset (live resize window):
                        // skip out-of-bounds geometry this frame; the resize
                        // handler calls reset() and rebuilds the annulus.
                        continue;
                    }
                    // Horizon bloom filter: cells beyond the current bloom
                    // radius are not drawn yet (the annulus grows from the
                    // inside out). The filter is geometry-static — the cell
                    // list never reorders, only the cut advances.
                    if self.cell_dist_norm.len() > idx && self.cell_dist_norm[idx] > visibility {
                        continue;
                    }

                    // Rim conveyor (stage 2.1 — the visible ball rotation): the
                    // glyph pattern is bucketed by angle and the buckets slide
                    // around the annulus with the spin phase. When a cell's
                    // bucket changes the glyph re-rolls to the new bucket's
                    // deterministic character — motion-gated mutation, the same
                    // life-sign DNA as the family's shimmer gates.
                    if self.cell_angles.len() > idx && self.cell_buckets.len() > idx {
                        let bucket = ((self.cell_angles[idx] - self.spin_phase)
                            / crate::constants::BLACK_HOLE_RING_CONVEYOR_ARC)
                            .floor() as i32;
                        if bucket != self.cell_buckets[idx] {
                            self.cell_buckets[idx] = bucket;
                            self.glyphs[idx] = conveyor_char(ctx.char_pool, bucket);
                        }
                    }
                    // Matrix shimmer: low-probability glyph mutation is the
                    // engine's life sign (every style carries one); the ball
                    // stays calm, a slow surface flicker at the event horizon.
                    if self.glyphs.len() > idx
                        && rand_chance.sample(rng) < crate::constants::BLACK_HOLE_SHIMMER_CHANCE
                    {
                        self.glyphs[idx] = pick_pool_char(ctx.char_pool, rand_chance, rng);
                    }
                    let ch = if self.glyphs.len() > idx {
                        self.glyphs[idx]
                    } else {
                        '0'
                    };

                    // Doppler-style lobe (stage 2.1): a bright band sweeps
                    // around the rim with the spin phase — cells near the lobe
                    // peak brighten one rung, cells near the opposite point dim
                    // one. The radial band structure is untouched (bump applied
                    // at draw time only, clamped to the ladder), so the
                    // approved photon-ring read stays intact while the surface
                    // visibly rotates left-to-right in lockstep with the ring.
                    let level = if self.cell_angles.len() > idx {
                        let lobe = (self.cell_angles[idx] - self.spin_phase).cos();
                        if lobe > crate::constants::BLACK_HOLE_RING_LOBE_GAIN {
                            bump_level(cell.level, 1)
                        } else if lobe < -crate::constants::BLACK_HOLE_RING_LOBE_GAIN {
                            bump_level(cell.level, -1)
                        } else {
                            cell.level
                        }
                    } else {
                        cell.level
                    };
                    draw_ball_cell(
                        ctx,
                        frame,
                        cell.col,
                        cell.line,
                        ch,
                        self.palette_slot,
                        level,
                    );
                    self.current_cells.push(*cell);
                }
            }
        }

        // The cached ball anchor as floats — one read for the ring,
        // the halo and the infall draw arms below (the projection
        // families all key on the same center).
        let cx_f = self.center_col as f32;
        let cy_f = self.center_line as f32;
        // The frame's see-saw snapshot (NIGHT-lts-1 stage 1): one
        // trig pair per frame feeds both pools' projections — the
        // attitude angle is rigid across the whole stack within the
        // frame, so the per-mote projections used to re-evaluate it
        // per mote.
        let roll_frame = RollFrame::from_angle(self.roll.angle());

        // Stage 2: the orbital ring — project every active mote onto
        // its tier band's ellipse around the cached ball anchor, rolled
        // by the live see-saw angle, draw the head + comet trail
        // (occluded far-side tier-0 cells skipped; the upper tiers are
        // lensed bands that read in front at any height), and record
        // the drawn cells into the same diff-cleanup stream as the ball
        // (one unified current_cells / drawn_gen pipeline).
        if self.active_motes > 0 && self.ball_outer_r >= 1.0 {
            let outer_r = self.ball_outer_r;
            // The disk unit + proximity gain (NIGHT-research-9): the
            // semi-major axes key on the width-stretched disk unit, the
            // fade/hot keypoints ride the stretch gain.
            let disk_unit = self.disk_unit;
            let prox_gain = self.proximity_gain;
            // Semi-major clamp: 92% of the viewport's half-width (in
            // line-height units) so the disk extremes never clip on
            // narrow terminals — the wide-disk read survives resize.
            let major_limit = 0.92 * ctx.cols as f32 / (CELL_ASPECT_DIVISOR * 2.0);
            // The live see-saw angle rides the frame's RollFrame
            // snapshot (stage 2.4: one read per frame, every mote of
            // every tier pivots on it — the rigid-lever read the
            // owner asked for).
            let roll = roll_frame;
            for m in &mut self.motes {
                if !m.active {
                    continue;
                }
                let (col_f, line_f) =
                    project_ring_mote(m, cx_f, cy_f, outer_r, disk_unit, major_limit, roll);
                let col = col_f.round() as i32;
                let line = line_f.round() as i32;
                if col < 0 || line < 0 || col >= ctx.cols as i32 || line >= ctx.lines as i32 {
                    // Off-screen: skip the draw AND the trail push —
                    // the trail keeps its last in-bounds positions, so
                    // the streak resumes from the last visible cell
                    // when the mote re-enters the viewport (lorenz
                    // parity: no phantom trail cells).
                    continue;
                }
                let (col, line) = (col as u16, line as u16);

                // The mote's side is a property of the orbit (the sign
                // of sin phi), known exactly — the stage-2.3 occlusion
                // rule keys on it: near-side motes draw in front of
                // the hole at any height (the z-tilt breathes them
                // above the equator without vanishing), far-side
                // motes hide only while inside the silhouette. The
                // stage-2.4 upper tiers are lensed-image bands — they
                // read in front of the hole at any height, so they
                // take the near-side path unconditionally. Shared
                // by the head and the trail cells below (the side can
                // only flip at the extremes, which sit outside the
                // silhouette — no flip artifact is visible).
                let near_side = m.tier > 0 || m.phi.sin() >= 0.0;

                // Proximity grade (stage 2.4, the owner's 9.7/10
                // feedback): the head's projected distance from the
                // hole's center, in ball radii — distance, not orbital
                // angle, because the roll preserves it: the glow rides
                // the hole at every tilt. Near the hole the head steps
                // two rungs up (Mid/Hot bases land at Core — the white
                // "head white" the owner asked for), far out it steps
                // down the fade ladder (the ends dissolve into dim
                // wisps). Stage 2.7 (owner 9.95/10 feedback): the snug
                // upper stacks' bases floor at Hot, so stacks 2 and 3
                // read Core (white) across their reach — the more
                // bright/white head ruling.
                let head_dx = (col_f - cx_f) / CELL_ASPECT_DIVISOR;
                let head_dy = line_f - cy_f;
                let head_dist_norm = (head_dx * head_dx + head_dy * head_dy).sqrt() / outer_r;
                let head_base = if m.tier >= 1 {
                    floor_head_base_at_hot(level_for_ring_z(m.z))
                } else {
                    level_for_ring_z(m.z)
                };
                // NIGHT-research-9: the ladder's input rides the stretch
                // gain, so the hot/warm/fade keypoints track the disk's
                // real reach (a width-stretched disk fades at its tips,
                // not at the canonical ball-relative distances).
                let head_level = proximity_level(head_base, head_dist_norm * prox_gain);

                // Matrix shimmer: mutate the glyph when the head lands
                // on a new cell (previous trail head differs), gated
                // by the family chance constant.
                if m.trail_len > 0 {
                    let (prev_col, prev_line) = m.trail[(m.trail_len - 1) as usize];
                    if (prev_col != col || prev_line != line)
                        && rand_chance.sample(rng)
                            < crate::constants::BLACK_HOLE_RING_SHIMMER_CHANCE
                    {
                        m.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                    }
                } else {
                    m.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                }

                // Head: the graded level (white-hot near the hole,
                // fading at the outer disk) through the side-aware
                // occlusion (near side always in front, far side
                // hidden inside the silhouette).
                if !occludes_ring_cell(
                    col,
                    line,
                    self.center_col,
                    self.center_line,
                    outer_r,
                    near_side,
                ) {
                    draw_ball_cell(ctx, frame, col, line, m.ch, m.palette_slot, head_level);
                    self.current_cells.push(BlackHoleCell {
                        col,
                        line,
                        level: head_level,
                    });
                }

                // Comet trail: previously occupied cells, one brightness
                // rung dimmer each, drawn only while in bounds and not
                // behind the ball (a trail visibly vanishes into the
                // silhouette and resumes on emergence).
                for t in 0..m.trail_len as usize {
                    let (tc, tl) = m.trail[t];
                    if tc >= ctx.cols || tl >= ctx.lines {
                        continue;
                    }
                    if occludes_ring_cell(
                        tc,
                        tl,
                        self.center_col,
                        self.center_line,
                        outer_r,
                        near_side,
                    ) {
                        continue;
                    }
                    let depth = (m.trail_len as usize - t).min(4) as u8;
                    let trail_level = step_down_level(head_level, depth);
                    draw_ball_cell(ctx, frame, tc, tl, m.ch, m.palette_slot, trail_level);
                    self.current_cells.push(BlackHoleCell {
                        col: tc,
                        line: tl,
                        level: trail_level,
                    });
                }

                m.push_trail(col, line);
            }
        }

        // Stage 2.7 halo streams: project every active rider onto its
        // arc circle around the cached ball anchor (rolled by the
        // live see-saw angle with the rest of the system), draw the
        // head + comet trail through the stream-visibility filter
        // (each mote draws only on its own semicircle — the two
        // upper crowns over the shadow, the rare lower echo under
        // it), and record the drawn cells into the same
        // diff-cleanup stream. No occlusion rule: the arc bands
        // (1.20-1.40 outer radii for the inner crowns, 1.38-1.58 for
        // the outer) never enter the ball silhouette. Bounds-checked
        // per cell so a live resize window (geometry rebuilt on
        // reset) never paints outside the viewport — the
        // dynamic-screen-size contract.
        if self.active_halo > 0 && self.ball_outer_r >= 1.0 {
            let outer_r = self.ball_outer_r;
            // The frame's RollFrame snapshot — same angle the ring
            // projected through (the whole system pivots as one
            // rigid body).
            let roll = roll_frame;
            for m in &mut self.halo_motes {
                if !m.active {
                    continue;
                }
                // The stream filter: hidden motes retire their trail
                // (the handoff at the extreme clears the streak so
                // the re-emergence on the opposite limb never paints
                // a teleporting tail) and skip the frame entirely.
                if !halo_mote_visible(m) {
                    m.trail_len = 0;
                    continue;
                }
                let (col_f, line_f) = project_halo_mote(m, cx_f, cy_f, outer_r, roll);
                let col = col_f.round() as i32;
                let line = line_f.round() as i32;
                if col < 0 || line < 0 || col >= ctx.cols as i32 || line >= ctx.lines as i32 {
                    // Off-screen: skip the draw AND the trail push —
                    // the trail keeps its last in-bounds positions
                    // (the entry spiral drifts young motes beyond the
                    // arc; they re-enter as they settle).
                    continue;
                }
                let (col, line) = (col as u16, line as u16);

                // Proximity grade: same distance key as the ring
                // heads — the arc circle sits at 1.30 outer radii,
                // just inside the hot radius, so the stream riders
                // burn white-hot like the lensing arc they share the
                // road with (the whole-crown glow the owner approved).
                let head_dx = (col_f - cx_f) / CELL_ASPECT_DIVISOR;
                let head_dy = line_f - cy_f;
                let head_dist_norm = (head_dx * head_dx + head_dy * head_dy).sqrt() / outer_r;
                let head_level = proximity_level(level_for_ring_z(m.z), head_dist_norm);

                // Matrix shimmer: the same motion-gated mutation gate
                // the ring heads carry (mutation tied to motion —
                // the family life sign).
                if m.trail_len > 0 {
                    let (prev_col, prev_line) = m.trail[(m.trail_len - 1) as usize];
                    if (prev_col != col || prev_line != line)
                        && rand_chance.sample(rng)
                            < crate::constants::BLACK_HOLE_RING_SHIMMER_CHANCE
                    {
                        m.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                    }
                } else {
                    m.ch = pick_pool_char(ctx.char_pool, rand_chance, rng);
                }

                // Head: the graded level, always drawn (no occlusion
                // path for the arc band).
                draw_ball_cell(ctx, frame, col, line, m.ch, m.palette_slot, head_level);
                self.current_cells.push(BlackHoleCell {
                    col,
                    line,
                    level: head_level,
                });

                // Comet trail: previously occupied cells, one
                // brightness rung dimmer each, drawn only while in
                // bounds (the streak follows the arc's curvature —
                // the comet read on the crown and under the foot).
                for t in 0..m.trail_len as usize {
                    let (tc, tl) = m.trail[t];
                    if tc >= ctx.cols || tl >= ctx.lines {
                        continue;
                    }
                    let depth = (m.trail_len as usize - t).min(4) as u8;
                    let trail_level = step_down_level(head_level, depth);
                    draw_ball_cell(ctx, frame, tc, tl, m.ch, m.palette_slot, trail_level);
                    self.current_cells.push(BlackHoleCell {
                        col: tc,
                        line: tl,
                        level: trail_level,
                    });
                }

                m.push_trail(col, line);
            }
        }

        // Stage 3 glyph infall: project every active falling glyph
        // through the hole's gravitational field onto the screen (the
        // cached ball anchor + outer radius), grade its head by speed
        // and proximity (kinetic heat composed with the family
        // ladder), draw the head + comet trail, and record the cells
        // into the same unified diff-cleanup stream. The empty core is
        // never painted (the absorption retires a mote exactly at the
        // horizon); bounds-checked per cell so a live resize window
        // never paints outside the viewport — the dynamic-screen-size
        // contract.
        self.infall.draw(super::infall::InfallDrawArgs {
            ctx,
            frame,
            rng,
            rand_chance,
            cx: cx_f,
            cy: cy_f,
            ball_outer_r: self.ball_outer_r,
            current_cells: &mut self.current_cells,
        });

        // Pass 2: generation-tag every drawn cell (monolith pattern —
        // u32 counter bump instead of clearing the array). Rebuilt when
        // the viewport dimensions change; sized to the full grid because
        // the flat index col * lines + line spans exactly cols * lines.
        let need_dims = (ctx.cols, ctx.lines);
        let need_len = ctx.cols as usize * lines_us.max(1);
        if self.drawn_gen_dims != need_dims || self.drawn_gen.len() != need_len {
            self.drawn_gen.resize(need_len, 0);
            self.drawn_gen_dims = need_dims;
        }
        self.drawn_gen_counter = self.drawn_gen_counter.wrapping_add(1);
        let gen = self.drawn_gen_counter;
        for cell in &self.current_cells {
            let idx = cell.col as usize * lines_us + cell.line as usize;
            if idx < self.drawn_gen.len() {
                self.drawn_gen[idx] = gen;
            }
            // NIGHT-hunter-29 (the phosphor ownership rule — the doc
            // on clear_phosphor_metadata): a cell drawn this frame is
            // owned by the draw; zeroing its phosphor state every
            // frame kills the period-2 ghost-vs-draw strobe that was
            // the owner's resume micro jump on this style (the whole
            // drawn population — ball annulus, ring tiers, halo
            // crowns, infall streaks — flickered between the drawn
            // state and the A20 orphan ghost's dim palette tint at
            // full rate, blend-independent, most visible exactly
            // when the resume ramp freezes every other motion).
            clear_phosphor_metadata(cleanup, cell.col, cell.line);
        }

        // Pass 3: clear previous cells NOT redrawn this frame.
        let drawn_gen = &self.drawn_gen[..];
        for cell in &self.previous_cells {
            let idx = cell.col as usize * lines_us + cell.line as usize;
            if idx < drawn_gen.len() && drawn_gen[idx] == gen {
                continue;
            }
            clear_cell(frame, cleanup, cell.col, cell.line);
        }

        std::mem::swap(&mut self.previous_cells, &mut self.current_cells);
    }

    /// The seed dot phases' renderer: the singularity glyph at the
    /// viewport center (brightness ramped by the formation phase)
    /// plus, during the collapse, the four-cell cross flare around
    /// it. Bounds-checked like every other draw arm; the cells join
    /// the unified diff-cleanup stream so the horizon bloom cleanly
    /// erases them when it takes over.
    fn draw_seed_dot(
        &mut self,
        ctx: &DrawCtx<'_>,
        frame: &mut Frame,
        rand_chance: &Uniform<f32>,
        rng: &mut StdRng,
    ) {
        // The seed glyph persists (no per-frame flicker); it re-rolls
        // on entry (seed_stale) and at the calm shimmer rate — the
        // same life-sign contract the annulus carries.
        if self.seed_stale {
            self.seed_glyph = pick_pool_char(ctx.char_pool, rand_chance, rng);
            self.seed_stale = false;
        } else if rand_chance.sample(rng) < crate::constants::BLACK_HOLE_SHIMMER_CHANCE {
            self.seed_glyph = pick_pool_char(ctx.char_pool, rand_chance, rng);
        }

        let t = self.formation_t;
        let level = seed_center_level(t);
        let (cc, cl) = (self.center_col, self.center_line);
        if cc >= 0 && cl >= 0 && (cc as u16) < ctx.cols && (cl as u16) < ctx.lines {
            let (col, line) = (cc as u16, cl as u16);
            draw_ball_cell(
                ctx,
                frame,
                col,
                line,
                self.seed_glyph,
                self.palette_slot,
                level,
            );
            self.current_cells.push(BlackHoleCell { col, line, level });
        }

        // Collapse flare: the four cross cells around the seed.
        if cross_active(t) {
            let cross_level = seed_cross_level(t);
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let col = cc + dx;
                let line = cl + dy;
                if col < 0 || line < 0 || col >= ctx.cols as i32 || line >= ctx.lines as i32 {
                    continue;
                }
                let (col, line) = (col as u16, line as u16);
                draw_ball_cell(
                    ctx,
                    frame,
                    col,
                    line,
                    self.seed_glyph,
                    self.palette_slot,
                    cross_level,
                );
                self.current_cells.push(BlackHoleCell {
                    col,
                    line,
                    level: cross_level,
                });
            }
        }
    }

    // -- Test-only diagnostics (mirrors the monolith/vortex *_for_test API) --

    #[cfg(test)]
    pub(crate) fn ring_cells_for_test(&self) -> &[BlackHoleCell] {
        &self.ring_cells
    }

    #[cfg(test)]
    pub(crate) fn drawn_cells_for_test(&self) -> &[BlackHoleCell] {
        &self.previous_cells
    }

    #[cfg(test)]
    pub(crate) fn motes_for_test(&self) -> &[RingMote] {
        &self.motes
    }

    #[cfg(test)]
    pub(crate) fn active_motes_for_test(&self) -> usize {
        self.active_motes
    }

    #[cfg(test)]
    /// The stage-2.6 halo stream pool (the arc riders).
    pub(crate) fn halo_motes_for_test(&self) -> &[RingMote] {
        &self.halo_motes
    }

    #[cfg(test)]
    /// Active halo stream rider count.
    pub(crate) fn active_halo_for_test(&self) -> usize {
        self.active_halo
    }

    #[cfg(test)]
    /// The stage-3 infall pool (the falling glyph motes).
    pub(crate) fn infall_motes_for_test(
        &self,
    ) -> &[crate::cloud::type_rain::black_hole::infall::InfallMote] {
        self.infall.motes_for_test()
    }

    #[cfg(test)]
    /// Active falling-glyph count (the rain layer's population).
    pub(crate) fn active_infall_for_test(&self) -> usize {
        self.infall.active_infall_for_test()
    }

    #[cfg(test)]
    /// Ball rim spin phase (the co-rotation contract's observable).
    pub(crate) fn spin_phase_for_test(&self) -> f32 {
        self.spin_phase
    }

    #[cfg(test)]
    /// See-saw roll angle (radians; 0 = the flat horizontal rest
    /// line, positive lifts the left end of the stack).
    pub(crate) fn roll_angle_for_test(&self) -> f32 {
        self.roll.angle()
    }

    #[cfg(test)]
    /// Ball outer radius (line-height units) — the width-capped
    /// effective radius (NIGHT-research-9's dynamic-size observable).
    pub(crate) fn ball_outer_r_for_test(&self) -> f32 {
        self.ball_outer_r
    }

    #[cfg(test)]
    /// Disk scale unit (line-height units) — the width-stretched
    /// scale base of the tier semi-major axes.
    pub(crate) fn disk_unit_for_test(&self) -> f32 {
        self.disk_unit
    }

    #[cfg(test)]
    /// The see-saw's dynamic tilt cap (radians, positive) — the
    /// menu pick clamp derived from the viewport's vertical budget.
    pub(crate) fn roll_tilt_cap_for_test(&self) -> f32 {
        self.roll_tilt_cap
    }

    #[cfg(test)]
    /// Formation clock (seconds since formation start).
    pub(crate) fn formation_t_for_test(&self) -> f32 {
        self.formation_t
    }

    #[cfg(test)]
    /// Steady-state gate (true once the horizon bloom completed).
    pub(crate) fn formed_for_test(&self) -> bool {
        self.formed
    }
}
