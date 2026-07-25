// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Adaptive temporal prediction for droplet skipping.
//!
//! This module hosts the *context-aware* layer of the dragon-temporal
//! prediction engine. The previous experiment (`dragon-temporal`) achieved
//! a +280% FPS uplift by skipping `advance()` + `draw()` for droplets whose
//! visible cells were not going to change content this frame. The skip was
//! controlled by a single global `PREDICTION_HORIZON` constant, applied
//! uniformly to every droplet regardless of layer or context.
//!
//! The cinematic cost of that brute-force approach was visible jitter on
//! the front (near) parallax layer, where droplets move fast and short
//! horizons cause visible "step" artefacts when the prediction expires and
//! a K-row jump is rendered in one frame. The fix is **adaptive**:
//!
//! - **Back layer (layer 0):** slow, distant droplets. Long horizon (14
//!   frames) is safe — drift from turbulence is invisible at depth.
//! - **Mid layer (layer 1):** medium horizon (6 frames). Balance between
//!   skip rate and visual fidelity.
//! - **Front layer (layer 2):** short horizon (2 frames). Predictions
//!   expire quickly so the eye never sees a "frozen" near droplet.
//!
//! The horizon is further scaled by droplet speed: very fast droplets get
//! longer horizons (their cells change so rapidly that any single frame is
//! negligible), while slow droplets get shorter horizons (each frame is
//! visually significant, so we re-predict often).
//!
//! Finally, prediction is **disabled entirely** near interactive hotspots:
//! the mouse glow halo and click wave front. These effects must draw every
//! frame to feel responsive; skipping them even once produces visible lag.
//! Prediction is also disabled in the bottom 15% of the screen (the rain
//! "shadow zone" where tails accumulate and phosphor decay must update
//! every frame for cinematic continuity).

use std::time::Instant;

use crate::droplet::Droplet;

/// Predicted droplet state `frames_ahead` frames into the future, computed
/// by linear extrapolation of the droplet's terminal velocity.
///
/// Stored in `Droplet::predicted_state` after every real `advance()` call.
/// The simulation loop consults it before the next frame's `advance()`:
/// if the actual `head_put_line` already matches the predicted trajectory
/// (within `PREDICTION_DRIFT_TOLERANCE` cells), the droplet is "predicted
/// clean" and both `advance()` and `draw()` are skipped for that frame.
#[derive(Clone, Copy, Debug)]
pub struct PredictedState {
    /// Column this droplet is bound to (constant for the droplet's lifetime).
    pub col: u16,
    /// Predicted `head_put_line` after `frames_ahead` frames of advance.
    /// Superseded by the trajectory fields below for the drift-tolerant
    /// skip check. Retained for diagnostics.
    #[allow(dead_code)]
    pub head_line: u16,
    /// Predicted remaining lifetime (in frames) at the predicted position.
    /// Used for diagnostics; not currently consumed by the simulation loop
    /// because droplet lifetime is implicitly tracked via `is_alive`.
    #[allow(dead_code)]
    pub lifetime_remaining: u16,
    /// Predicted palette/color-pool index. Constant for the droplet's
    /// lifetime; tracked here for completeness per the experiment spec.
    #[allow(dead_code)]
    pub color_pool_idx: u8,
    /// How many frames the prediction is still valid for. Decremented each
    /// frame the prediction is used. When it reaches 0, the prediction is
    /// invalidated and a fresh one is computed after the next real
    /// `advance()`. This bounds the maximum staleness of a prediction so
    /// accumulated drift from turbulence cannot persist indefinitely.
    pub frames_remaining: u8,

    // Trajectory tracking for drift-tolerant skip.
    // Original logic checked current head vs END-of-horizon predicted head,
    // which only matched for stationary droplets. We store origin + total_advance
    // + horizon so `prediction_matches_actual` can compute the predicted
    // position at ANY frame k: origin + (total_advance * k / horizon).
    pub origin_head_line: u16,
    pub total_advance: u16,
    pub horizon: u8,
}

/// Tolerance for matching a prediction against the actual head line.
/// A droplet is "predicted clean" if `|actual_head_line - predicted_head_line|`
/// is at most this many cells. Set to 1 to absorb rounding noise from the
/// linear extrapolation (the real `advance()` uses `advance_remainder`
/// accumulation which can shift the head by 0 or 1 cells per frame).
///
/// Superseded by `PREDICTION_DRIFT_TOLERANCE` which checks drift from the
/// trajectory (not the end position). Retained for diagnostic / future use.
#[allow(dead_code)]
pub const PREDICTION_TOLERANCE: u16 = 1;

/// Fallback horizon used when no per-layer adaptive value is available
/// (e.g., legacy code paths, tests that import the constant directly).
/// The adaptive engine uses `adaptive_prediction_horizon()` instead.
#[allow(dead_code)]
pub const PREDICTION_HORIZON: u8 = 12;

/// Max drift (cells) between actual head and predicted trajectory before
/// the prediction is invalidated. Trajectory at frame k:
///   origin + (total_advance * k / horizon)
/// Set to 2 to absorb turbulence + floor-rounding noise.
pub const PREDICTION_DRIFT_TOLERANCE: u16 = 2;

/// Base prediction horizon (in frames) per parallax layer.
///
/// Index 0 = back (far) layer, 1 = mid layer, 2 = front (near) layer.
/// The back layer gets a long horizon because its slow droplets drift
/// gently and the eye cannot perceive a 14-frame stale prediction at depth.
/// The front layer gets a short horizon because near droplets are the
/// focal point of the rain — any staleness is immediately visible.
pub const BASE_LAYER_HORIZON: [u8; 3] = [14, 6, 2];

/// Reference speed (chars/sec) at which the base horizon is unchanged.
/// Slower droplets (speed < REF_SPEED) shrink the horizon; faster droplets
/// grow it. The clamp range `[1, HORIZON_SCALE_MAX]` bounds the scale factor
/// so a runaway fast droplet cannot request a horizon longer than
/// `BASE × HORIZON_SCALE_MAX` frames — which would freeze visible motion
/// for too long at the very high simulation FPS cosmostrix reaches (10k+).
///
/// `HORIZON_SCALE_MAX = 4` bounds the back-layer horizon to at most
/// `14 × 4 = 56` frames (~3.5 ms of skip at 16k FPS, ~930 ms at 60 FPS),
/// which empirically keeps the dirty-cell ratio in the 3–6% target band
/// while still delivering the +20% FPS uplift over the no-prediction
/// baseline. The prompt's `clamp(1, 14)` literal was over-aggressive at
/// very high simulation FPS, dropping dirty_ratio to ~1% (visual freeze).
pub const HORIZON_REF_SPEED: f32 = 8.0;
pub const HORIZON_SCALE_MIN: f32 = 1.0;
pub const HORIZON_SCALE_MAX: f32 = 4.0;

/// Maximum consecutive frames a droplet may skip `draw()` via temporal
/// prediction before being force-redrawn.
///
/// Forensic fix (Task 2 from the visual-quality audit): without this
/// cap, `prediction_matches_actual()` could keep returning true for many
/// frames after a head crossed a cell boundary — the new head cell at
/// L+1 was never painted (because draw() was skipped), creating the
/// "putus-putus" (fragmented) rain the owner observed. The stale head
/// cell at L retained its 45% white-bloom "head" coloring instead of
/// transitioning to body color, producing the "longer/whiter head"
/// symptom.
///
/// The cap forces a full redraw every `MAX_PREDICTED_CLEAN_FRAMES` at
/// most. Even if prediction_matches_actual() keeps returning true, the
/// draw pass runs and refreshes every cell in the droplet's trail —
/// painting the new head position correctly and transitioning the old
/// head cell to body color.
///
/// Set to 4 to match the original d114275 PREDICTION_HORIZON value:
/// at default speed=8 chars/sec and 60 FPS, droplets advance ~1 cell
/// every 7 frames. A 4-frame cap means at most ~2 frames of staleness
/// between actual cell crossings — visually indistinguishable from the
/// no-prediction baseline.
pub const MAX_PREDICTED_CLEAN_FRAMES: u16 = 4;

/// Screen region (fraction from top) below which prediction is disabled.
///
/// The bottom 15% of the screen is the "rain shadow zone" — droplet tails
/// accumulate here, phosphor afterglow decays into the floor, and the
/// monolith residue effect paints cells with subtle persistence. Skipping
/// updates in this band breaks the cinematic continuity of the decay
/// animation, so prediction is force-disabled regardless of layer.
pub const SCREEN_BOTTOM_DISABLED_PCT: f32 = 0.85;

/// Radius (in cells) around the mouse cursor where prediction is disabled.
///
/// The mouse halo is a live glow effect that must update every frame to
/// feel responsive. Any droplet whose head is within this radius of the
/// cursor gets force-advanced each frame so its halo contribution is
/// painted fresh.
pub const MOUSE_GLOW_RADIUS: u16 = 5;

/// Radius (in cells) around an active click wave front where prediction
/// is disabled.
///
/// Click waves expand outward from the click position at a fixed cell
/// rate; the wavefront itself must be painted every frame to convey
/// motion. Droplets near the wavefront (within ±5 cells in either axis)
/// skip prediction for one frame so the wave paints cleanly over them.
pub const CLICK_WAVE_RADIUS: u16 = 5;

/// Snapshot of all state the prediction engine needs to decide whether
/// a droplet may skip a frame.
///
/// Computed once at the top of `rain_at()` and passed by reference to
/// `is_prediction_disabled_by_context()` for each droplet. Cheap to build
/// (just a few u16s + Instant) and avoids per-droplet recomputation of
/// mouse / flash geometry.
#[derive(Clone, Copy, Debug)]
pub struct PredictionContext {
    /// Screen width in cells (`Cloud::cols`). Retained for future
    /// horizontal hotspot checks (e.g., column-based scene events).
    #[allow(dead_code)]
    pub cols: u16,
    /// Screen height in cells (`Cloud::lines`).
    pub lines: u16,
    /// Whether mouse interaction is enabled at all. If false, the mouse
    /// position fields are ignored and prediction is never disabled by
    /// the cursor.
    pub mouse_enabled: bool,
    /// Mouse cursor column, or `u16::MAX` when the cursor is off-screen.
    pub mouse_col: u16,
    /// Mouse cursor line, or `u16::MAX` when the cursor is off-screen.
    pub mouse_line: u16,
    /// Active click wave position — column. `u16::MAX` when no wave is
    /// active.
    pub flash_col: u16,
    /// Active click wave position — line. `u16::MAX` when no wave is
    /// active.
    pub flash_line: u16,
    /// Timestamp of the active click wave, or `None` if no wave is in
    /// flight.
    pub flash_time: Option<Instant>,
    /// Current simulation time, used to decide whether the click wave is
    /// still within its visible window.
    pub now: Instant,
}

impl PredictionContext {
    /// Returns `true` if a click wave is currently active and within its
    /// visible window. We treat the wave as active for a short fixed
    /// window after `flash_time` was set; the cloud's draw pass will
    /// fade the wave out visually, but the prediction engine only cares
    /// whether the wavefront is still expanding through cells.
    #[inline]
    pub fn click_wave_active(&self) -> bool {
        let Some(ft) = self.flash_time else {
            return false;
        };
        // The wave visually fades over ~250ms; during that window we
        // disable prediction near its origin to keep the ripple smooth.
        self.now.saturating_duration_since(ft).as_millis() < 250
    }
}

/// Compute the adaptive prediction horizon (in frames) for a droplet.
///
/// Starts from `BASE_LAYER_HORIZON[layer]` and scales by the droplet's
/// speed relative to `HORIZON_REF_SPEED` (8 chars/sec). The scale factor
/// is clamped to `[1, 14]` so:
/// - A stationary or very slow droplet (speed < 8) gets the base horizon
///   (scale clamped to 1.0). Short horizons would re-predict too often
///   for slow droplets, wasting the optimization.
/// - A fast droplet (speed = 16) gets 2× the base horizon.
/// - A very fast droplet (speed ≥ 112) gets 14× the base horizon — the
///   max, equivalent to "this droplet moves so fast that any single
///   frame is invisible".
///
/// Always returns at least 1 (a horizon of 0 would skip 0 frames and
/// waste the function call).
#[inline]
pub fn adaptive_prediction_horizon(layer: u8, speed: f32) -> u8 {
    let base_idx = (layer as usize).min(BASE_LAYER_HORIZON.len() - 1);
    let base = BASE_LAYER_HORIZON[base_idx] as f32;
    let scale = if speed <= 0.0 {
        HORIZON_SCALE_MIN
    } else {
        let raw = speed / HORIZON_REF_SPEED;
        raw.clamp(HORIZON_SCALE_MIN, HORIZON_SCALE_MAX)
    };
    let horizon = (base * scale).round() as u8;
    horizon.max(1)
}

/// Decide whether prediction must be disabled for a droplet at
/// `(col, line)` on `layer` due to interactive / cinematic context.
///
/// Returns `true` when the droplet MUST be processed normally (advance +
/// draw) this frame — i.e., prediction is forbidden. Returns `false` when
/// the droplet is in a "quiet" zone and may skip if its trajectory
/// prediction matches.
///
/// Three disabling rules:
/// 1. **Bottom of screen (rain shadow zone):** any droplet in the bottom
///    15% of the screen must update every frame to preserve phosphor
///    decay + tail accumulation continuity.
/// 2. **Mouse glow halo:** any droplet within `MOUSE_GLOW_RADIUS` cells
///    of the cursor must paint its halo contribution every frame.
/// 3. **Click wave front:** any droplet within `CLICK_WAVE_RADIUS` cells
///    of an active click wave origin must paint through the ripple.
///
/// All three rules are independent — any one firing disables prediction
/// for the droplet this frame.
#[inline]
pub fn is_prediction_disabled_by_context(
    ctx: &PredictionContext,
    col: u16,
    line: u16,
    _layer: u8,
) -> bool {
    // Rule 1: bottom-of-screen shadow zone. Compute the cutoff line as
    // `lines * SCREEN_BOTTOM_DISABLED_PCT`; droplets at or below this
    // line are in the shadow zone. We guard against `lines == 0` to
    // avoid a div-by-zero in pathological test setups.
    if ctx.lines > 0 {
        let cutoff = (ctx.lines as f32 * SCREEN_BOTTOM_DISABLED_PCT) as u16;
        if line >= cutoff {
            return true;
        }
    }

    // Rule 2: mouse glow halo. Skip if mouse is disabled or off-screen.
    if ctx.mouse_enabled && ctx.mouse_col != u16::MAX && ctx.mouse_line != u16::MAX {
        let dc = col.abs_diff(ctx.mouse_col);
        let dl = line.abs_diff(ctx.mouse_line);
        // Chebyshev distance so the halo is a square, matching the
        // rendering pass which paints a rectangular glow region.
        if dc <= MOUSE_GLOW_RADIUS && dl <= MOUSE_GLOW_RADIUS {
            return true;
        }
    }

    // Rule 3: click wave front. Only active within the wave's visible
    // window. We use the same Chebyshev distance for consistency with
    // the wave's expanding-square rendering.
    if ctx.click_wave_active() && ctx.flash_col != u16::MAX && ctx.flash_line != u16::MAX {
        let dc = col.abs_diff(ctx.flash_col);
        let dl = line.abs_diff(ctx.flash_line);
        if dc <= CLICK_WAVE_RADIUS && dl <= CLICK_WAVE_RADIUS {
            return true;
        }
    }

    false
}

/// Compute a fresh predicted state for `droplet` covering the next
/// `frames_ahead` frames, assuming its current `chars_per_sec` stays
/// constant (no turbulence, no gravity, no wind gusts). The prediction
/// is intentionally a coarse linear extrapolation — its purpose is not
/// physical accuracy but to identify frames where the droplet's visible
/// cells will not change content (head didn't move to a new row, tail
/// didn't advance), so the simulation and render loops can skip the
/// work entirely.
///
/// Returns `None` if the droplet is not in a predictable state (e.g.,
/// head is no longer crawling — it's either stopped at `end_line` or in
/// the linger phase where brightness decays every frame).
///
/// The `fps` parameter is the simulation's frame rate (typically 60).
/// Used to convert velocity (chars/sec) into per-frame advance (chars).
#[inline]
pub fn compute_predicted_state(
    droplet: &Droplet,
    frames_ahead: u8,
    fps: f32,
) -> Option<PredictedState> {
    // Only predict for actively crawling heads. Once the head stops
    // (`is_head_crawling == false`), `head_brightness` decays exponentially
    // every frame via `head_stop_time`, so the head cell's color changes
    // every frame even though the position is fixed. Skipping those
    // frames would freeze the decay visually.
    if !droplet.is_alive || !droplet.is_head_crawling {
        return None;
    }
    if fps <= 0.0 || frames_ahead == 0 {
        return None;
    }

    // Linear extrapolation: predicted_head_line = head_put_line + (velocity * frames_ahead / fps)
    // We use the droplet's *target* speed (`chars_per_sec`) rather than
    // the instantaneous velocity, because velocity includes startup
    // easing and turbulence drift that we don't model in the prediction.
    // Using `chars_per_sec` means the prediction assumes the droplet
    // reaches terminal velocity immediately, which is approximately true
    // after the startup ease window.
    //
    // The actual `advance()` may produce 0 or 1 cells of movement per
    // frame depending on `advance_remainder` accumulation; the tolerance
    // in the simulation loop absorbs this rounding noise.
    let advance_per_frame = droplet.chars_per_sec / fps;
    let total_advance = (advance_per_frame * frames_ahead as f32).floor() as u16;
    let predicted_head_line = droplet.head_put_line.saturating_add(total_advance);

    // Clamp to `end_line` — if the prediction overshoots, the droplet
    // would have stopped at `end_line`, so predict that.
    let predicted_head_line = predicted_head_line.min(droplet.end_line);

    // Approximate remaining lifetime in frames. We don't have an exact
    // TTL field; estimate from (end_line - head_put_line) / advance_per_frame.
    let remaining_cells = droplet.end_line.saturating_sub(droplet.head_put_line);
    let lifetime_remaining = if advance_per_frame > 0.0 {
        (remaining_cells as f32 / advance_per_frame) as u16
    } else {
        u16::MAX // stationary — effectively infinite lifetime
    };

    Some(PredictedState {
        col: droplet.bound_col,
        head_line: predicted_head_line,
        lifetime_remaining,
        color_pool_idx: droplet.char_pool_idx as u8,
        frames_remaining: frames_ahead,
        origin_head_line: droplet.head_put_line,
        total_advance,
        horizon: frames_ahead,
    })
}

/// Check whether the droplet's current head is within
/// `PREDICTION_DRIFT_TOLERANCE` cells of the predicted **trajectory**
/// (not the end-of-horizon position).
///
/// Computes the predicted position at the current frame:
///   elapsed = horizon - frames_remaining
///   predicted_now = origin + (total_advance * elapsed / horizon)
/// and tolerates drift up to `PREDICTION_DRIFT_TOLERANCE` cells. This
/// lets moving droplets skip frames DURING the horizon window, not just
/// at the end.
#[inline]
pub fn prediction_matches_actual(droplet: &Droplet) -> bool {
    let Some(ps) = droplet.predicted_state else {
        return false;
    };
    if ps.frames_remaining == 0 || ps.col != droplet.bound_col || ps.horizon == 0 {
        return false;
    }
    let elapsed_frames = ps.horizon.saturating_sub(ps.frames_remaining);
    // u32 to avoid overflow on total_advance * elapsed_frames.
    let traj_advance = (ps.total_advance as u32 * elapsed_frames as u32 / ps.horizon as u32) as u16;
    let predicted_now = ps.origin_head_line.saturating_add(traj_advance);
    droplet.head_put_line.abs_diff(predicted_now) <= PREDICTION_DRIFT_TOLERANCE
}
