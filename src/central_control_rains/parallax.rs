// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! Parallax depth layer constants — extracted from `mod.rs` to keep
//! that file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! The rain is rendered in 3 parallax layers (back/mid/front). Every
//! per-layer multiplier (speed, brightness, saturation, head bloom,
//! length, density, glyph dim, contrast reduction) lives here.

// ─── Parallax depth layers ─────────────────────────────────────────────────
//
// The rain is rendered in 3 parallax layers (back/mid/front = far/mid/near).
// Each droplet is assigned to a layer at spawn time. Layer controls depth
// via compound multiplicative factors: speed (parallax), brightness, haze,
// glow — all stack to push back-layer rain into atmospheric depth while
// keeping the front layer as the vivid neon focal point.
//
// The layer index is also used to gate per-layer effects in droplet.rs,
// phosphor.rs, monolith.rs, and the spawn path.

/// Number of parallax depth layers.
pub(crate) const PARALLAX_LAYERS: usize = 3;

/// Per-layer speed multiplier (layer 0 = far, 2 = near).
///
/// Back layer moves at 35% of base speed (parallax recession), front layer
/// at 170% (foreground whoosh). Mid matches base speed.
pub(crate) const PARALLAX_SPEED_MULT: [f32; PARALLAX_LAYERS] = [0.35, 1.0, 1.7];

/// Per-layer brightness multiplier (layer 0 = far, 2 = near).
///
/// Deep Focus preset (comparison round 2 champion): field lifted,
/// head peak tamed. Back pushed into shadow (0.56), front gently
/// boosted (1.08) for cinematic presence without glare.
///   - Back  (0): 0.56 (deep shadow — lifted from noir 0.52)
///   - Mid   (1): 0.82 (slightly dim — vivid streaks)
///   - Front (2): 1.08 (gentle boost — tamed from noir 1.10)
pub(crate) const PARALLAX_BRIGHTNESS_MULT: [f32; PARALLAX_LAYERS] = [0.56, 0.82, 1.08];

/// Per-layer saturation multiplier (layer 0 = desaturated, 2 = full).
///
/// Deep Focus preset (comparison round 2 champion): slightly muted head
/// color. Back desaturated (0.52) for shadow haze, front boosted
/// (1.10) for cinematic color richness without overdrive.
///   - Back  (0): 0.52 (shadow haze blend)
///   - Mid   (1): 0.84 (slightly vivid)
///   - Front (2): 1.10 (cinematic richness — tamed from noir 1.12)
pub(crate) const PARALLAX_SATURATION_MULT: [f32; PARALLAX_LAYERS] = [0.52, 0.84, 1.10];

/// Per-layer head-bloom multiplier (layer 0 = suppressed, 2 = full).
///
/// Deep Focus preset (comparison round 2 champion): glare control.
/// Front head blooms gently (1.24 — tamed from noir 1.30) for
/// cinematic trail presence. Front-layer heads glow without glare.
///   - Back  (0): 0.48 (suppressed — stays in shadow)
///   - Mid   (1): 0.74 (moderate pop)
///   - Front (2): 1.24 (CINEMATIC BLOOM — tamed from noir 1.30)
pub(crate) const PARALLAX_HEAD_BLOOM_MULT: [f32; PARALLAX_LAYERS] = [0.48, 0.74, 1.24];

/// Per-layer head self-bloom multiplier (layer 0 = suppressed, 2 = full).
///
/// .0 differential + silent override bug fix: back 0.45→0.38
/// (effective self-bloom ~9%, vs ~27% for front — distant heads read
/// as ambient glow); mid 0.78→0.68 (slightly less self-glow to match
/// lower density); front 1.0→1.15 (full cinematic self-bloom).
///
/// Bug fix in droplet.rs switched from `as i32` truncation (which made
/// ALL multipliers < 1.0 collapse to 0, giving 0% boost for every
/// layer) to f32 math. Now fractional multipliers actually apply:
/// back gets ~9% boost, mid gets ~16%, front gets ~27%.
///   - Back  (0): 0.38 (ambient distant glow, no pinprick)
///   - Mid   (1): 0.68 (clearly present, not flashy)
///   - Front (2): 1.20 (boosted self-glow, Option F)
pub(crate) const PARALLAX_HEAD_SELFBLOOM_MULT: [f32; PARALLAX_LAYERS] = [0.38, 0.68, 1.20];

/// Per-layer length multiplier (layer 0 = short, 2 = long).
///
/// Back layer droplets are 50% of base length (brief streaks). Front layer
/// droplets are 140% (long cinematic rain streaks). Mid matches base.
pub(crate) const PARALLAX_LENGTH_MULT: [f32; PARALLAX_LAYERS] = [0.5, 1.0, 1.4];

// ─── Phosphor persistence (CRT afterglow) ──────────────────────────────────
//
// Phosphor is what gives trails their "fading glow" — when a droplet's
// head passes a cell, that cell gets phosphor energy that decays over
// time, producing the characteristic Matrix afterglow. Per-layer decay
// lets back-layer trails fade fast (don't linger as bright spots) while
// front-layer trails fade slow (smooth body→tail gradient).

/// Per-cell phosphor energy decay rate (higher = faster fade).
///
/// CALIBRATED (night-frontier-Z4c): the rate is no longer a bare
/// hand-tuned number — it derives from the real P1-class phosphor
/// decay constant under one documented aesthetic time dilation:
///
/// ```text
/// rate = 1 / (tau_P1 * time_scale) = 1 / (0.025 s * 5.0) = 8.0 /s
/// ```
///
/// P1 (Zn2SiO4:Mn green willemite, the classic green monochrome
/// monitor phosphor) has a characteristic exponential decay component
/// of tau = 25 ms; a 25 ms afterglow is invisible at terminal scale,
/// so the engine runs phosphor time 5x slower — a documented aesthetic
/// dilation chosen to reproduce THIS rate bit-identically. The
/// calibration gives the number its physical identity ("the P1 curve
/// at 5x dilation, half-life 86.6 ms simulated") without retuning the
/// visuals; `central_control_rains::phosphor_calibration` owns the
/// law, the reference classes (P3 amber, P4 white, P7 radar), and the
/// per-frame decay LUT, and the lock tests pin the half-lives.
///
/// v50.0.0-beta.6 history: increased from 5.5 to 8.0 for cross-terminal
/// consistency. At 5.5, trails lasted ~0.6s — visually longer on
/// VTE-based terminals (gnome-console, gnome-terminal) due to
/// their CPU-rendered sub-pixel blending making dim ghosts more
/// visible. At 8.0, trails last ~0.4s, matching the snappy feel on
/// Alacritty (GPU-rendered). Terminal-aware multiplier
/// (`phosphor_decay_mult` in TerminalCaps) further adjusts per
/// terminal tier.
pub(crate) const PHOSPHOR_DECAY_RATE: f32 = 1.0
    / (super::phosphor_calibration::PHOSPHOR_TAU_P1_SECS
        * super::phosphor_calibration::PHOSPHOR_SIM_TIME_SCALE);

/// Energy level when a cell's tail passes (starts the phosphor glow).
///
/// At 160, trail brightness is ~63% of head — body cells clearly visible
/// as colored rain rather than dim ghosts.
pub(crate) const PHOSPHOR_TAIL_RESIDUAL: u8 = 160;

/// Below this energy, the cell is cleared to blank.
pub(crate) const PHOSPHOR_DEAD_THRESHOLD: u8 = 6;

/// Minimum phosphor energy for rendering the original character glyph in
/// ghost cells. Below this threshold, the ghost cell renders as a blank
/// space (or dim color-only patch). Prevents stale cells from filling
/// the background with dark charset glyphs.
pub(crate) const PHOSPHOR_GLYPH_THRESHOLD: u8 = 96;

/// Per-layer phosphor decay rate multiplier (far=fast, near=slow).
///
/// Deep Focus preset (comparison round 2 champion): back clears faster,
/// front lingers. Back: 1.9 (quick flicker), mid: 1.15 (smooth
/// streaks), front: 0.65 (lingering cinematic trail). Effective
/// rates: back=15.2, mid=9.2, front=5.2. Less residual busyness
/// than noir.
///   - Back  (0): 1.9 (quick flicker — shadow exit)
///   - Mid   (1): 1.15 (smooth — clean dissolve)
///   - Front (2): 0.65 (lingering cinematic trail)
///
/// The multipliers are cosmostrix's own parallax-depth structure,
/// layered ON TOP of the calibrated P1 base rate: each layer's
/// effective tau is the P1 curve's simulated tau (0.125 s = 25 ms x
/// 5.0 dilation) divided by the multiplier (back tau 65.8 ms, mid
/// 108.7 ms, front 192.3 ms — half-lives 45.6 / 75.3 / 133.2 ms;
/// pinned in the Z4c calibration tests).
pub(crate) const PHOSPHOR_LAYER_DECAY_MULT: [f32; PARALLAX_LAYERS] = [1.9, 1.15, 0.65];

/// Number of rows from the bottom of the screen where phosphor decay is
/// accelerated (prevents "concrete wall" residue buildup).
pub(crate) const PHOSPHOR_BOTTOM_ROWS: u16 = 12;

/// Phosphor decay rate multiplier applied to bottom rows.
///
/// Deep Focus preset: 1.8 — dissolve lingers slightly (tamed
/// from noir 2.0). Soft afterglow fade at bottom. Rain trails
/// into shadow.
pub(crate) const PHOSPHOR_BOTTOM_DECAY_MULT: f32 = 1.8;

// ─── Parallax depth layering ────────────────────────────────────────────
//
// Three independent controls stack to push the back layer into atmospheric
// depth: density (fewer spawns), contrast reduction (fg→bg blend = fog),
// and glyph dim (char-level dimming, currently 1.0 = no-op since brightness
// + saturation already cover it).

/// Per-layer spawn density multiplier (far = sparse, near = dense).
///
/// .0 silent override bug fix: front restored from 1.10 → 0.85 to
/// compensate for the spawn-roll fix (commit 9080472) that gave front
/// +40% more density rolls. At spawn_droplets distribution [0.35, 0.30,
/// 0.35] (post-fix), front effective spawn rate = 0.35 × 0.85 × col_mod
/// ≈ 0.208 — matching 5571c0b's 0.25 × 1.0 × col_mod ≈ 0.175 with a
/// slight +19% bump so front reads as more prominent (now that bug
/// fixes actually apply per-droplet boosts).
///
/// Mid reduced 0.75→0.62 (~17% fewer droplets — the primary lever for
/// reducing mid noise per user's request). Back kept at 0.45 (already
/// sparse, dimming handles the depth push instead).
///   - Back  (0): 0.45 (kept — sparse distant rain)
///   - Mid   (1): 0.62 (reduced — fewer but vivid streaks)
///   - Front (2): 0.85 (restored — sparse crisp glow, matches 5571c0b)
pub(crate) const PARALLAX_DENSITY_MULT: [f32; PARALLAX_LAYERS] = [0.45, 0.62, 0.85];

/// Per-layer glyph simplicity (currently no-op — subsumed by brightness
/// + saturation). Kept as a tuning knob for future use.
pub(crate) const PARALLAX_GLYPH_DIM: [f32; PARALLAX_LAYERS] = [1.0, 1.0, 1.0];

/// Per-layer contrast reduction (depth-of-field perceptual blur).
///
/// Deep Focus preset (comparison round 2 champion): fog identity kept
/// from noir. Back fogged (0.50) for depth atmosphere, front razor
/// sharp (0.0). The contrast between shadowy back and sharp front
/// creates the deep-focus aesthetic — dark entry, crisp resolution.
///   - Back  (0): 0.50 (fog — back dissolves into shadow)
///   - Mid   (1): 0.18 (slight veil)
///   - Front (2): 0.0 (razor sharp — clarity)
pub(crate) const PARALLAX_CONTRAST_REDUCTION: [f32; PARALLAX_LAYERS] = [0.50, 0.18, 0.0];
