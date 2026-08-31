// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Atmospheric vignette + shadow constants — extracted from `mod.rs`
//! to keep that file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns: depth fog vignette (top/bottom row dim), cinematic CRT
//! vignette (edge dim), radial vignette (edge darkening), rain shadow
//! (bottom quadratic fade-out). These control the "cinematic framing"
//! of the rain — dimming at viewport edges to focus the eye.

// Import PARALLAX_LAYERS from the parallax sibling module (re-exported
// via mod.rs's `pub(crate) use parallax::*`).
use super::PARALLAX_LAYERS;

// ─── Depth fog vignette (top/bottom row dim) ───────────────────────────────

/// Number of rows at top and bottom affected by depth fog.
/// v30 (visual mode): reduced from 4 → 3 per owner request — shorter
/// dimmer zones at top and bottom borders.
///
/// v50 (alpha.2): disabled (FOG_MIN_FACTOR = 1.0). Depth fog was redundant with
/// viewport_edge_fade + CRT vignette — all three dim the same top/bottom
/// rows and compound destructively. With fog at 0.45, the compounded
/// top row reached 0.24 (76% dim) and bottom row 0.07 (93% dim).
/// Disabling fog leaves edge_fade + CRT vignette as the sole edge dim
/// pair, matching the masterclass calibration target of 0.533 top / 0.369
/// bottom. FOG_ROWS kept at 3 for code structure; the factor=1.0 gate
/// in droplet.rs skips the brightness multiply entirely (zero cost).
pub(crate) const FOG_ROWS: u16 = 3;

/// Minimum brightness factor at the extreme edge row.
/// v50 (alpha.2): set to 1.0 (disabled). See FOG_ROWS comment for rationale.
/// When 1.0, the fog_factor == 1.0 gate in droplet.rs skips the
/// brightness multiply entirely — zero runtime cost, zero visual impact.
pub(crate) const FOG_MIN_FACTOR: f32 = 1.0;

// ─── Cinematic CRT vignette (top & bottom edge dim) ────────────────────────

/// Height (in rows) of the CRT vignette band at top and bottom.
/// v30 (visual mode): reduced from 5 → 3 per owner request — shorter
/// dimmer zones at top and bottom borders.
pub(crate) const CRT_VIGNETTE_HEIGHT: u16 = 3;

/// Brightness factor at the extreme edge row of the CRT vignette.
///
/// ## Deep Focus preset (2026-08-23, battle round 2 champion)
/// Set to 0.87 (13% dim) — warm CRT glass, lighter than noir.
/// The Deep Focus philosophy: gentle vignette, cinematic lens,
/// reduced fatigue. Edges dim softly like a classic film frame
/// — rain fades into shadow at the borders.
///
/// Reference points:
/// - 0.87 (Deep Focus): 13% dim — warm CRT glass, lighter
/// - 0.85 (Cinema Noir, superseded): 15% dim — warm CRT glass
/// - 0.82 (masterclass): 18% dim — calibrated when fog was active
/// - 0.50 (v30): 50% dim — destructive when compounded
///
/// See `docs/VISUAL_IDENTITY.md` for the preset lineage and
/// `docs/research/VISUAL_MODE_AUDIT.md` for the full master audit.
pub(crate) const CRT_VIGNETTE_EDGE_FACTOR: f32 = 0.87;

/// Perf-pressure threshold below which the CRT vignette is skipped
/// (perf optimization — skip on slow systems).
pub(crate) const CRT_VIGNETTE_PERF_THRESHOLD: f32 = 0.5;

/// M1 (internal independent QA): phosphor decay pressure gate with hysteresis.
/// When pressure rises above `PHOSPHOR_SKIP_HIGH` (0.70), phosphor decay is
/// skipped entirely. It stays skipped until pressure drops below
/// `PHOSPHOR_SKIP_LOW` (0.50), preventing strobing when pressure fluctuates
/// around the threshold. The hysteresis gap (0.20) is wider than typical
/// run-to-run pressure noise, so the effect fades smoothly in/out rather
/// than hard-cutting.
pub(crate) const PHOSPHOR_SKIP_HIGH: f32 = 0.70;
pub(crate) const PHOSPHOR_SKIP_LOW: f32 = 0.50;

// ─── Cinematic radial vignette (edge darkening) ────────────────────────────

/// Intensity of the radial vignette (0.0 = none, 1.0 = full black at edges).
///
/// Deep Focus preset (battle round 2 champion): 0.14 (14% corner —
/// lighter photographic lens than noir's 20%). Cinematic lens
/// philosophy — dark corners, deep-focus frame. Corners dim
/// into shadow.
pub(crate) const VIGNETTE_INTENSITY: f32 = 0.14;

/// Inner radius (as fraction of half-screen) where vignette starts.
///
/// Deep Focus preset (battle round 2 champion): 0.75 — vignette
/// starts later than noir (0.70). Combined with INTENSITY=0.14,
/// radial vignette is lighter and starts further out.
pub(crate) const VIGNETTE_INNER_RADIUS: f32 = 0.75;

/// Per-layer vignette multiplier (0.0 = no dimming, 1.0 = full dimming).
///
/// Front layer (2) is exempt — vignette is a depth effect that should
/// only push mid/back deeper into the background.
pub(crate) const VIGNETTE_LAYER_MULT: [f32; PARALLAX_LAYERS] = [1.0, 1.0, 0.0];

// ─── Rain shadow (bottom quadratic fade-out) ───────────────────────────────

/// Percentage of screen height (from bottom) affected by rain shadow.
///
/// Deep Focus preset (battle round 2 champion): 0.13 (13% of screen
/// height) — smaller bottom shadow zone than noir (15%). On a
/// 40-line terminal this is ~5 rows. Rain fades gently into shadow.
pub(crate) const RAIN_SHADOW_PCT: f32 = 0.13;

/// Per-layer rain shadow multiplier (front layer exempt, same as vignette).
pub(crate) const RAIN_SHADOW_LAYER_MULT: [f32; PARALLAX_LAYERS] = [1.0, 1.0, 0.0];

/// Minimum brightness floor for the rain shadow quadratic. The fade curve
/// never drops below this value, even at the very last row.
///
/// ## Deep Focus preset (2026-08-23, battle round 2 champion)
/// Set to 0.58 (42% dim floor) — lifted shadow floor. Rain gently
/// dissolves toward the bottom border. Combined with PCT=0.13, the
/// shadow is lighter than noir: a clear depth gradient in the
/// bottom rows with less squeeze.
///
/// Reference points:
/// - 0.58 (Deep Focus): 42% dim floor — lifted shadow floor
/// - 0.55 (Cinema Noir, superseded): 45% dim floor — visible fade
/// - 0.50 (v50 alpha.2): 50% dim floor — visible depth
/// - 0.00 (previously): full quadratic to black — destructive
///
/// See `docs/VISUAL_IDENTITY.md` for the preset lineage and
/// `docs/research/VISUAL_MODE_AUDIT.md` for the full 4-effect
/// compounding math.
pub(crate) const RAIN_SHADOW_FLOOR: f32 = 0.58;
