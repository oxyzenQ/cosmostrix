// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Chroma Dragon — Tuning Constants
//!
//! Phase 4 (Dragon Awakening) activates shader innovations that were
//! plumbed into `chroma::shaders::base::resolve_cell_color` during Phase 3
//! but left dormant (hardcoded to `None` in the `DrawCtx → ShaderCtx`
//! builder):
//!
//!   - Innovation C (temporal column hue coherence) — Phase 4-A
//!   - Innovation E (subpixel hue jitter)           — Phase 4-B
//!   - Innovation D (head halo via background blend) — Phase 4-D
//!
//! Phase 5 adds perceptual L smoothing at the palette transition wave
//! line, killing the hard brightness step that occurs when the two
//! palettes have different perceptual luminance at corresponding stop
//! indices.
//!
//!   - Phase 5 — perceptual L smoothing at transition wave
//!
//! Phase 6 extends Phase 3-I's "palette-aware ghost" pattern to anomaly
//! halos: `cloud::phosphor::apply_anomalies` now derives the halo target
//! color from the active palette instead of hardcoding pure white.
//!
//!   - Phase 6 — palette-aware anomaly halos (LuminanceSurge + PulseWave)
//!
//! Phase 7 replaces the v17 global brightness floor (`MIN_RGB_SUM = 180`)
//! with a palette-relative floor derived from each palette's own brightness
//! profile. Dark themes (Cosmos, Nebula, Mercury, Moon) no longer have
//! their intentionally dark trail stops washed out to sum 180.
//!
//!   - Phase 7 — palette-relative brightness floor
//!
//! All three Phase 4 innovations are always-on in production. Phase 5 is
//! conditionally-on (only during the 300 ms transition window). Phase 6 is
//! always-on for anomaly frames (rare — ~5% of frames). Phase 7 is always-on
//! at palette build time (zero runtime cost). The constants
//! below tune their amplitudes; see the doc comments on
//! `ShaderCtx::column_coherence_lut`, `ShaderCtx::subpixel_jitter_amplitude`,
//! `ShaderCtx::head_halo_factor`, `ShaderCtx::transition_l_table`,
//! `anomaly_halo_target`, and `colors_from_stops` for the full rationale.
//!
//! ## Why a separate module?
//!
//! These constants are Chroma Dragon-specific — they tune the coloring
//! engine, not the rendering engine or the cloud simulation. Keeping them
//! in `src/constants.rs` pushed that file over the 800-LOC cap (1480 +
//! 51 = 1531). Moving them here keeps each file under the cap and groups
//! all chroma tuning in one auditable place. Future Chroma Dragon
//! innovations should add their tuning constants here too.

/// Phase 4-A: angular frequency (rad/s) of the temporal column hue
/// coherence oscillation.
///
/// The shader's `column_coherence_perturbation(phase, col)` computes
/// `sin(phase + col * 0.05)` and rounds to `{-1, 0, +1}`. The `phase`
/// argument advances at this rate so the per-column shimmer drifts
/// slowly over time.
///
/// `0.105` rad/s → period `2π / 0.105 ≈ 59.8 s` (~1 minute). Slow
/// enough to read as atmospheric rather than animated, fast enough
/// that a user watching for ~10 s perceives the columns breathing
/// through adjacent palette stops.
///
/// Spatial frequency is fixed at `0.05` rad/col (period ~125 cols)
/// inside the shader — that value is not exposed because it is
/// coupled to the `{-1, 0, +1}` rounding amplitude and changing it
/// in isolation would either quantize to 0 everywhere (too low) or
/// strobe per-cell (too high).
pub(crate) const COLUMN_COHERENCE_FREQ: f32 = 0.105;

/// Phase 4-B: amplitude of the per-cell subpixel hue jitter.
///
/// Each Middle cell's resolved RGB is perturbed by an independent
/// signed offset in `[-amp, +amp]` per channel, derived from a
/// deterministic FNV-1a hash of `(line, col)`. The same cell always
/// gets the same jitter (no strobing across frames); neighboring
/// cells get uncorrelated jitter (film-grain texture).
///
/// `3` is the conservative production default — at typical viewing
/// distance it reads as subtle organic texture rather than noise.
/// Higher values (6–8) produce a visible "static" effect; lower
/// values (1–2) are imperceptible on most terminals.
///
/// The jitter is applied AFTER the palette decision and BEFORE
/// atmospheric, so it does not interfere with the head→body→tail
/// hierarchy or the atmospheric luminance/saturation math.
pub(crate) const SUBPIXEL_JITTER_AMPLITUDE: u8 = 3;

/// Phase 4-D: blend factor for the head halo (background-aware dissolve).
///
/// The shader's `CharLoc::Head` branch resolves the brightest palette stop
/// and sets `bold = true`. Phase 4-D blends this head color toward the
/// scene background by this factor, softening the hard bright pixel
/// against the dark background. On a dark-cosmos bg, the head becomes
/// slightly dimmer and bg-tinted — a "dissolve into the scene" rather
/// than a stark white smear. On a light bg, the head brightens slightly
/// toward the bg, maintaining contrast.
///
/// `0.15` = 15% blend toward bg. Conservative — the head stays clearly
/// the brightest cell in the droplet, but the transition from head to
/// body reads as a soft glow rather than a hard edge. Higher values
/// (0.3–0.5) produce a visible "aura" but risk washing out the head
/// on very dark backgrounds. Lower values (0.05–0.10) are barely
/// perceptible.
///
/// Applied ONLY to `CharLoc::Head` cells. Middle and Tail stops are
/// pinned by the palette hierarchy and must not be haloed. Applied
/// AFTER palette resolution and BEFORE subpixel jitter + atmospheric,
/// so downstream effects compose on the haloed color.
///
/// `None` in `ShaderCtx` disables (matches pre-Phase-4-D dormant
/// behavior — `blend_toward_bg` existed since Phase 3-D but had zero
/// production callers). `Color::Reset` bg is a no-op (no RGB to blend
/// toward), so the halo auto-disables when no explicit bg is set.
pub(crate) const HEAD_HALO_FACTOR: f32 = 0.15;

/// Phase 5: smoothing window (in lines) for perceptual L smoothing at
/// the palette transition wave line.
///
/// During a palette transition, `color_wave_line` sweeps top-to-bottom
/// over `COLOR_TRANSITION_DURATION_MS` (300 ms). Cells within ±this
/// many lines of the wave get their OKLab L channel blended toward
/// the opposite palette's L for that stop index. The blend peaks at
/// 0.5 at the wave line (50% midpoint — no palette swap) and falls
/// off linearly to 0 at ±window.
///
/// `3.0` lines = a 7-line smoothing band (wave ± 3). At 60 fps and
/// 300 ms transition duration, the wave sweeps ~1 line per frame on
/// a 50-line display, so the smoothing band covers ~7 frames of
/// visible transition — long enough to perceive the dissolve, short
/// enough to not blur the cascade effect.
///
/// Higher values (5–8) produce a softer, more gradual dissolve but
/// risk washing out the cascade direction (the wave becomes less
/// visible as a top-to-bottom sweep). Lower values (1–2) keep the
/// cascade crisp but the brightness step remains partially visible.
///
/// The window is in floating-point lines so the shader can compute
/// `|distance| / window` without integer rounding. The actual cell
/// lines affected are `ceil(wave_line - window)` to `floor(wave_line
/// + window)` inclusive.
pub(crate) const TRANSITION_L_SMOOTHING_WINDOW: f32 = 3.0;

/// Phase 6: hue-cycle rate (palette stops per second) for the PulseWave
/// anomaly halo target color.
///
/// `cloud::phosphor::apply_anomalies` calls
/// `chroma::post::anomaly::anomaly_halo_target(palette_colors, kind, elapsed)`
/// to derive the halo target color from the active palette. For
/// `AnomalyKind::PulseWave`, the target is a hue-cycled palette stop —
/// `(elapsed * ANOMALY_HALO_CYCLE_RATE) as usize % palette_colors.len()`.
/// The expanding ring's color cycles through palette stops as it expands,
/// giving PulseWave a distinct visual identity from LuminanceSurge (which
/// uses the palette's brightest stop as a static target).
///
/// `4.0` stops/sec → on a typical 9-stop palette, a full cycle takes
/// ~2.25 sec. Anomaly lifetime is `ANOMALY_DURATION_SECS` (1.5 sec), so
/// the ring cycles through ~6 of 9 stops during its lifetime — long
/// enough to clearly perceive the hue cycle (covers red→green→blue in
/// a Rainbow palette), short enough that the ring doesn't strobe (each
/// stop is held for ~250 ms, well above flicker fusion threshold).
///
/// Lower values (1–2) make the cycle barely perceptible within one
/// anomaly's lifetime. Higher values (8+) make the ring strobe through
/// stops too quickly to read as a hue cycle — it becomes noise.
///
/// `LuminanceSurge` is unaffected by this constant — it uses
/// `palette_colors.last()` (the brightest stop) as a static target, on
/// the rationale that a "luminous surge" should lift cells toward the
/// palette's natural ceiling rather than cycling through hues.
pub(crate) const ANOMALY_HALO_CYCLE_RATE: f32 = 4.0;

/// Phase 7: ratio of palette head brightness used as the brightness floor
/// for trail stops. The floor is `palette_max_sum * PALETTE_FLOOR_RATIO`,
/// clamped to `[ABSOLUTE_MIN_FLOOR, GLOBAL_MAX_FLOOR]`.
///
/// Pre-Phase-7 (v17): global `MIN_RGB_SUM = 180` boosted any color with
/// sum < 180 to sum = 180. This fixed a "dim/dark" complaint but caused
/// washout on dark themes — e.g. Cosmos `(3, 3, 18)` (sum 24, intentional
/// "void" trail) became `(22, 22, 135)` (sum 180), destroying the deep-space
/// aesthetic. Mercury `(5, 5, 5)` (sum 15) became `(60, 60, 60)` (sum 180),
/// turning a near-black trail into medium gray.
///
/// Phase 7 derives the floor from the palette's own brightness profile:
/// the brightest stop (head) defines the palette's "ceiling", and trail
/// stops are required to be at least `PALETTE_FLOOR_RATIO` as bright as
/// that ceiling. This preserves the head→body→trail hierarchy while
/// preventing true invisibility.
///
/// `0.20` = 20% of head brightness. On a typical palette with head sum 655,
/// the floor is `655 * 0.20 = 131` (capped at `GLOBAL_MAX_FLOOR = 180`).
/// Trail stops at sum 13 (Green) boost to sum 131 → `(0, 121, 3)` — clearly
/// visible dark green, less aggressive than v17's `(0, 165, 14)`. Trail
/// stops at sum 24 (Cosmos) boost to sum 131 → `(16, 16, 99)` — visible
/// void blue, much less aggressive than v17's `(22, 22, 135)`.
///
/// History: Phase 7 originally shipped with `0.15` (trail sum ~98 across
/// most themes). User visual testing at speed 100 reported trails as "too
/// dark" — the 0.15 floor produced dim trails that, while aesthetically
/// preserving dark themes, hurt readability at high rain speed. The
/// `phase7_print_ratio_sweep_audit` test in `palette/tests_floor_audit.rs`
/// verified that 0.20 doubles trail brightness to ~130 across most themes
/// with **zero** themes hitting the `GLOBAL_MAX_FLOOR` cap (no v17-style
/// washout). 0.25 would push 4 themes (Spectrum20, Stars, Pluto, Moon)
/// into the cap; 0.30 maxes out at 180 for 42/44 themes (full v17
/// regression). 0.20 is the empirical sweet spot.
///
/// Lower values (0.08–0.15) preserve dark themes more aggressively but
/// regress on the "dim/dark" complaint. Higher values (0.25–0.30) make
/// trails brighter but increasingly cap-hit, risking v17-style washout.
pub(crate) const PALETTE_FLOOR_RATIO: f32 = 0.20;

/// Phase 7: absolute minimum brightness floor. Any stop with RGB sum
/// below this gets boosted to this value, regardless of palette profile.
///
/// This catches true invisibility — a stop at `(0, 0, 0)` (sum 0) is
/// invisible against any background, even on a palette with a dim head.
/// `30` corresponds to `(10, 10, 10)` — dark gray, visible against pure
/// black bg. Below this, stops are imperceptible at typical viewing
/// distance and terminal contrast.
///
/// See `PALETTE_FLOOR_RATIO` for the full Phase 7 rationale.
pub(crate) const ABSOLUTE_MIN_FLOOR: u16 = 30;

/// Phase 7: maximum brightness jump allowed between adjacent palette stops
/// (post-floor). If stop[i+1] is more than BODY_TAIL_MAX_GAP_RATIO times
/// brighter than stop[i], stop[i] is scaled up to maintain continuity.
///
/// Pre-Phase-7-b: Phase 7's basic floor (PALETTE_FLOOR_RATIO=0.15) made
/// trails visible but left a large brightness gap between trail (sum ~100)
/// and body (sum 250-640). At high rain speed (>= 80), this gap becomes
/// perceptually a hard brightness step — the eye sees two distinct bands
/// instead of a continuous gradient, creating a horizontal-line illusion
/// across all columns.
///
/// Phase 7-b fixes this by enforcing continuity: iterate head→trail, and
/// if any adjacent pair has gap > BODY_TAIL_MAX_GAP_RATIO, scale up the
/// darker stop to gap = BODY_TAIL_MAX_GAP_RATIO. Hue is preserved (RGB
/// ratio scaling, same as the basic floor).
///
/// `2.0` = trail must be at least 50% as bright as the next-brighter stop.
/// Empirically verified across all 44 built-in themes — tightens the
/// body-tail step from 2.5x to 2.0x (20% reduction), killing the
/// horizontal-line illusion at speed 100 while preserving head→body→trail
/// hierarchy (head still 2-3x brighter than trail after continuity).
///
/// History: Phase 7-b originally shipped with `2.5` (trail = 40% of next).
/// User visual testing at speed 100 reported a persistent horizontal-line
/// illusion — the 2.5x brightness step was still perceptible as a hard
/// band at high rain speed. The `phase7b_print_gap_ratio_sweep_audit`
/// test in `palette/tests_floor_audit.rs` verified the impact of each candidate
/// gap target across all 44 themes:
///
///   gap=2.5 (was):  trail ~130, max_step 2.51x — horizontal-line visible
///   gap=2.0 (now):  trail ~130, max_step 2.01x — step 20% tighter (verified)
///   gap=1.8:         some themes jump to 160-194 — NeonWhite exceeds v17's
///                    180 ceiling (regression on dark-aesthetic preservation)
///   gap=1.5:         many themes 200-281 — trails nearly as bright as body,
///                    losing the cinematic trail-fade effect
///
/// 2.0 is the empirical sweet spot — measurable step reduction with zero
/// themes exceeding v17's 180 ceiling.
///
/// Lower values (1.5-1.8) compress the dynamic range too aggressively —
/// trails become as bright as body, losing the cinematic trail-fade effect.
/// Higher values (2.5-3.0) leave visible gaps that re-introduce the
/// horizontal-line illusion at high speed.
///
/// This is applied AFTER the basic floor and BEFORE quantization. The
/// GLOBAL_MAX_FLOOR cap (180) still applies to the basic floor; continuity
/// itself is uncapped (can boost above 180 if needed to maintain the gap
/// contract — see `apply_body_tail_continuity` doc comment).
pub(crate) const BODY_TAIL_MAX_GAP_RATIO: f32 = 2.0;

/// Phase 7: maximum brightness floor. The derived floor is never higher
/// than this, even for palettes with extremely bright heads.
///
/// This caps the v17 behavior — `180` was the original global floor, and
/// Phase 7 never exceeds it. For palettes where the derived floor would
/// naturally exceed 180 (e.g. a palette with head sum 1500 → derived floor
/// 225), the floor is clamped to 180, preserving the v17 upper bound.
///
/// In practice, this constant is only hit by palettes with very bright
/// heads (head sum > 1200), which is rare — most palettes have head sums
/// in the 500–800 range, so the derived floor is well below 180.
///
/// See `PALETTE_FLOOR_RATIO` for the full Phase 7 rationale.
pub(crate) const GLOBAL_MAX_FLOOR: u16 = 180;

/// RAIN_BORDER_TOUCH_GLOW (Option C+D, owner-approved 2026-08-26):
/// when a droplet's head crosses the top border of the `-mb` overlay,
/// the touched border cell briefly glows toward the droplet's `head_rgb`.
///
/// Owner insight: *"the top-left border gets hit by rain, changes from
/// black to white, then fades after a few seconds. When rain touches
/// it again, it reappears. But the color is not just white — it is
/// dynamic"* — dynamic color from the touching droplet's head, not
/// static white.
///
/// LTS invariant override: the existing "top corners stay dark / no lone
/// bright heads at top corners" rule (mod.rs §1018-1027) is RELAXED for
/// transient touch events. Corners (`╭╮`) are now eligible for pulse
/// blending along with mid-edge (`─`) cells; the bottom-corner bright
/// anchor rule still applies.
///
/// Pulse envelope: smoothstep over `BORDER_TOUCH_PULSE_LIFETIME_MS`,
/// peaking at `BORDER_TOUCH_PULSE_MAX` (1.0 = full head color) and
/// decaying to 0 at the end of the lifetime.
///
/// Owner spec (translated from the original Indonesian): *"from black to
/// white, then it fades away after a few seconds"* — peak immediately on
/// touch, decay over a few seconds. 1500 ms sits in the "a few seconds"
/// range while staying short enough that subsequent touches in the same
/// column (typical at 5-10 drops/sec) re-trigger fresh pulses rather
/// than saturating the cell.
pub(crate) const BORDER_TOUCH_PULSE_LIFETIME_MS: u32 = 1500;
pub(crate) const BORDER_TOUCH_PULSE_MAX: f32 = 1.0;

/// RAIN_BORDER_TOUCH_GLOW (Option D, halo above border): a single-row
/// halo above the top border, with per-column decay modulated by the
/// same touch events. The halo uses the same `head_rgb` color, blended
/// at a lower max factor (0.3) so it does not compete with the message
/// text for the eye.
pub(crate) const BORDER_TOUCH_HALO_MAX: f32 = 0.3;
/// Halo lifetime is shorter than the border pulse — the halo is the
/// "splash up" cue, not the sustained glow on the border itself.
pub(crate) const BORDER_TOUCH_HALO_LIFETIME_MS: u32 = 400;
