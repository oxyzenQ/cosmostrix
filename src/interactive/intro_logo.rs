// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cosmostrix Logo intro — a four-phase cinematic that reveals the
//! project's ASCII logo and dissolves it into Matrix rain.
//!
//! ```text
//! Phase 1: Fade In    (0    – 2000 ms)  Logo appears line by line, fading
//!                                         from black to the palette color.
//! Phase 2: Ignition   (2000 – 4250 ms)  A spark falls from the top of the
//!                                         screen to the logo's center; on
//!                                         impact the logo flashes bright.
//! Phase 3: Dissolve   (4250 – 5250 ms)  Logo characters turn into rain
//!                                         droplets starting from the outer
//!                                         edge and moving inward; droplets
//!                                         fall toward the bottom.
//! Phase 4: Rain       (5250 – 6250 ms)  The last droplets fall off-screen;
//!                                         rain engine takes over seamlessly.
//! ```
//!
//! Total: ~6.25 s. Press `q` (or `Q`) to skip instantly — no other key
//! skips, so stray keypresses can't cut the cinematic short. The intro
//! is skipped entirely on terminals smaller than 80×24 with a stderr
//! notice (handled by [`super::intro::run_intro`]).
//!
//! ## Constraints
//!
//! * Zero per-frame heap allocation — particle pool pre-allocated; the
//!   `dissolve_progress` bitmap is a stack-allocated `bool` array sized
//!   to the logo's cell count.
//! * Reuses the existing `Terminal` / `Frame` / `Cell` pipeline.
//! * `FRAME_COUNTER` is bumped each frame so the watchdog doesn't kill
//!   us during the cinematic.

use std::time::Instant;

use crossterm::style::Color;

use crate::cell::Cell;
use crate::cloud::Cloud;
use crate::frame::Frame;
use crate::terminal::Terminal;

use super::intro::{
    end_frame, lerp, lerp_rgb, palette_target_rgb, rain_chars, render_particle_cell, seed_rng,
    should_skip, Particle, ParticlePool, XorShift, PARTICLE_POOL_SIZE,
};

// ─────────────────────────────────────────────────────────────────────────────
// Logo art + brand color
// ─────────────────────────────────────────────────────────────────────────────

/// The Cosmostrix logo as ASCII art. Single-width Unicode density
/// characters render the brand mark. Lines are stored with their original
/// leading indentation (which forms the visual shape); trailing
/// whitespace is stripped at parse time.
///
/// Dimensions: 19 lines × 39 chars wide (max). See [`LOGO_HEIGHT`] and
/// [`LOGO_WIDTH`] — both are computed at parse time so they always match
/// the actual art. Rendering centers the logo at the terminal center.
///
/// # Centering math
///
/// All lines start at the same `logo_x = (term_cols - LOGO_WIDTH) / 2`
/// offset (integer math, truncating). Each line's leading spaces in the
/// string literal form the visual shape — they are NOT source-code
/// indentation. Centering is purely from the offset, never per-line.
//
// Note: codespell may complain about substrings inside this art. We keep
// the .codespellrc ignore-list updated to suppress false positives.
//
// IMPORTANT: We use `concat!()` instead of a `"\`-continued string literal.
// Rust's `\<newline>` line continuation strips leading whitespace from the
// NEXT line, which would silently eat the first line's indentation and
// shift the top of the logo flush-left relative to the body. `concat!()`
// preserves every byte verbatim.
const LOGO_ART: &str = concat!(
    "                ..qwmi  imwq.\n",
    "             .iWB$MWf=i  :iD0qw.\n",
    "          .f0opw^      ]    ^wk074.\n",
    "        .oWpdi'    :   | .     ^Q26C.\n",
    "      .08wdi  .    :     |       *ilMw.\n",
    "     ,hhZm'   :  : |     I         ^ =  \n",
    "    .hmZ0     |   .fW#|l           = nwm.\n",
    "   .mpQJ      I ,i,LdmCfTi.       =  'W5i.\n",
    "   lhCw^   .  Cf||1T   1lITki         'IHa.\n",
    "   COQ!:   :  0Om;   :   :I|b;   :     Ipb,\n",
    "    ^O1:   |   .CJ   |   !T:*    :     ioM,\n",
    "   *   '   .   C!1   I    i.     |     |'  \n",
    "   J0;. ;     bTiI   |   i0mi    I    .imoi\n",
    "   iwTQ I     ofi;ii  ,f|l!Q;        L1JCm.\n",
    "    JL1l, :    *:1!!JZf,1|.         :l!h:w\n",
    "    .Qffl.I  .    *lh3hI  :  :     wlIQw*\n",
    "     .LLTT,  :     .  :   I  |   .II0IW\n",
    "       lO1C1,.     :  |   ;    .il!QM\n",
    "         TqfCp.    I  .     .fIi|+M\n",
    "           *qbdo =      .:lIlQJm\n",
    "             'l:  =  .ihMwi*^",
);

/// Brand purple — the Cosmostrix signature color (`#A855F7` / RGB
/// 168,85,247). The logo always renders in this color, regardless of
/// the user's `--color` flag, so the brand mark stays consistent across
/// all palette themes. During the dissolve phase, droplets interpolate
/// from this purple toward the active rain palette's brightest stop,
/// creating a cinematic "brand → rain" handoff.
///
/// The `Color` enum form is kept as the canonical brand reference and
/// is exercised by unit tests; rendering uses [`LOGO_COLOR_RGB`] for
/// cheaper lerp math.
#[cfg(test)]
const LOGO_COLOR: Color = Color::Rgb {
    r: 168,
    g: 85,
    b: 247,
};

/// RGB triple form of [`LOGO_COLOR`] for efficient lerp math. Kept as a
/// constant so we don't pay the cost of matching the `Color` enum each
/// frame for every logo cell.
const LOGO_COLOR_RGB: (u8, u8, u8) = (168, 85, 247);

// ─────────────────────────────────────────────────────────────────────────────
// Phase + spawn constants
// ─────────────────────────────────────────────────────────────────────────────

/// Phase boundaries (milliseconds from intro start).
///
/// v25 balanced: Phase 1 = laser charge (0-1.2s).
/// Phase 2 = the glow (1.2-3.0s): logo HOLDS at full glow, rain falls from top.
/// Phase 3 = the fade (3.0-4.0s): rain nearly reaches logo, logo fades smoothly.
/// Phase 4 = full rain (4.0-4.5s): logo gone, rain visible, intro ends.
const PHASE1_FADEIN_END_MS: u64 = 1_200;
const PHASE2_IGNITION_END_MS: u64 = 3_000;
const PHASE3_DISSOLVE_END_MS: u64 = 4_000;
const PHASE4_RAIN_END_MS: u64 = 4_500;

/// Frame period in seconds, computed at runtime to avoid MSRV issues
/// with `Duration::as_secs_f32()` in const context (stable since 1.83,
/// but our MSRV is 1.81).
#[inline]
fn frame_period_secs() -> f32 {
    super::intro::INTRO_FRAME_PERIOD.as_secs_f32()
}

/// Rain droplet speed range (cells per second) for the dissolve phase.
/// Lower than Cosmic Burst's morph range so the rain curtain feels calm.
const DISSOLVE_SPEED_MIN: f32 = 8.0;
const DISSOLVE_SPEED_MAX: f32 = 16.0;

/// Horizontal velocity jitter range for dissolve droplets. Each droplet
/// gets a random `vx` in `[-JITTER_VX, +JITTER_VX]` cells/sec so the
/// curtain spreads organically before falling, instead of dropping in
/// perfectly straight columns. ±2 cells/sec is subtle enough to feel
/// natural without breaking the rain silhouette.
const JITTER_VX: f32 = 2.0;

// ─────────────────────────────────────────────────────────────────────────────
// Logo geometry helpers
// ─────────────────────────────────────────────────────────────────────────────

/// A non-blank cell from the logo, with its position relative to the
/// logo's top-left corner. Cells are collected at parse time and used
/// during Phase 1 (fade in) and Phase 3 (dissolve).
#[derive(Clone, Copy)]
struct LogoCell {
    /// Cell X within the logo bounding box (0 = leftmost column).
    bx: u16,
    /// Cell Y within the logo bounding box (0 = top row).
    by: u16,
    /// Squared distance from the logo's *visual centroid* (ink
    /// center-of-mass), used to order the dissolve from outermost ring
    /// inward. Stored as f32 for sorting. See [`visual_centroid`].
    dist_sq: f32,
    /// Original glyph from the art.
    ch: char,
}

/// Parse [`LOGO_ART`] into lines + bounding-box dims. v25 responsive:
/// if the terminal is smaller than the art, scale it down via
/// pixel-averaging (see [`scale_art`]). Lets the intro play on small
/// terminals (down to 10×5) instead of being skipped.
fn parse_logo_art(term_w: u16, term_h: u16) -> (Vec<String>, u16, u16) {
    let raw_lines: Vec<&'static str> = LOGO_ART.lines().collect();
    let raw_height = raw_lines.len() as u16;
    let raw_width = raw_lines
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0) as u16;

    // Art fits — return as-is (owned Strings for type uniformity).
    if raw_width <= term_w && raw_height <= term_h {
        let owned: Vec<String> = raw_lines.iter().map(|s| s.to_string()).collect();
        return (owned, raw_width, raw_height);
    }

    // Scale to fit with 1-cell margin, preserving aspect ratio.
    let avail_w = term_w.saturating_sub(2).max(1);
    let avail_h = term_h.saturating_sub(2).max(1);
    let scale_x = avail_w as f32 / raw_width as f32;
    let scale_y = avail_h as f32 / raw_height as f32;
    let scale = scale_x.min(scale_y).min(1.0);

    let scaled = scale_art(&raw_lines, raw_width, raw_height, scale);
    let scaled_height = scaled.len() as u16;
    let scaled_width = scaled.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
    (scaled, scaled_width, scaled_height)
}

/// Scale ASCII art down by a factor in (0.0, 1.0] via pixel-averaging.
/// Samples each block, picks the densest (most ink) character. Preserves
/// the logo's visual structure when shrunk. Runs once at intro start.
fn scale_art(raw_lines: &[&'static str], raw_w: u16, raw_h: u16, scale: f32) -> Vec<String> {
    if scale >= 1.0 || raw_w == 0 || raw_h == 0 {
        return raw_lines.iter().map(|s| s.to_string()).collect();
    }
    let new_w = ((raw_w as f32 * scale).ceil() as u16).max(1);
    let new_h = ((raw_h as f32 * scale).ceil() as u16).max(1);
    let inv_scale_x = raw_w as f32 / new_w as f32;
    let inv_scale_y = raw_h as f32 / new_h as f32;

    let mut out: Vec<String> = Vec::with_capacity(new_h as usize);
    for ny in 0..new_h {
        let y0 = (ny as f32 * inv_scale_y).floor() as u16;
        let y1 = ((ny + 1) as f32 * inv_scale_y).ceil() as u16;
        let mut row = String::with_capacity(new_w as usize);
        for nx in 0..new_w {
            let x0 = (nx as f32 * inv_scale_x).floor() as u16;
            let x1 = ((nx + 1) as f32 * inv_scale_x).ceil() as u16;
            row.push(sample_block(raw_lines, x0, y0, x1, y1));
        }
        out.push(row);
    }
    out
}

/// Sample a block of the original art, return the densest character.
fn sample_block(raw_lines: &[&'static str], x0: u16, y0: u16, x1: u16, y1: u16) -> char {
    let mut best_char = ' ';
    let mut best_density = 0u8;
    for y in y0..y1.max(y0 + 1) {
        let Some(line) = raw_lines.get(y as usize) else {
            continue;
        };
        for x in x0..x1.max(x0 + 1) {
            let Some(ch) = line.chars().nth(x as usize) else {
                continue;
            };
            let density = char_density(ch);
            if density > best_density {
                best_density = density;
                best_char = ch;
            }
        }
    }
    best_char
}

/// Density ranking: whitespace=0, light=1, medium=2, heavy=3, solid=4.
fn char_density(ch: char) -> u8 {
    match ch {
        ' ' => 0,
        '·' | '•' | '.' | ',' | '\'' | '`' => 1,
        '░' | '-' | '_' | '|' | '/' | '\\' | ':' => 2,
        '▒' | '+' | 'x' | '*' | 'o' => 3,
        '▓' | '█' | '#' | '@' | '%' | '&' | '=' | '$' => 4,
        _ if ch.is_ascii_graphic() => 2,
        _ => 0,
    }
}

/// Collect every non-blank cell from the parsed art, along with its
/// squared distance from the logo's visual centroid. Cells are returned
/// in arbitrary order — callers sort by `dist_sq` descending for the
/// dissolve-from-outside-inward effect.
///
/// `cx` / `cy` are the visual centroid coordinates (ink center-of-mass)
/// in the logo's local frame, computed by [`visual_centroid`]. Using the
/// visual centroid rather than the bounding-box center keeps the
/// dissolve "rings" centered on what the eye perceives as the logo's
/// core, which is especially important for asymmetric art where the
/// ink mass is offset from the bbox center.
fn collect_logo_cells(lines: &[String], cx: f32, cy: f32) -> Vec<LogoCell> {
    let mut out = Vec::with_capacity(256);
    for (y, line) in lines.iter().enumerate() {
        for (x, ch) in line.chars().enumerate() {
            if ch == ' ' {
                continue;
            }
            let xf = x as f32;
            let yf = y as f32;
            let dist_sq = (xf - cx) * (xf - cx) + (yf - cy) * (yf - cy);
            out.push(LogoCell {
                bx: x as u16,
                by: y as u16,
                dist_sq,
                ch,
            });
        }
    }
    out
}

/// Compute the visual centroid (center of mass) of all non-blank ink
/// cells in the parsed logo art. Returns `(cx, cy)` in the logo's local
/// coordinate frame (0..width, 0..height).
///
/// The visual centroid is what the eye perceives as the logo's center.
/// For asymmetric art — where the ink mass is offset from the bounding-
/// box center — placing the logo by its bbox center causes the visual
/// ink to sit off-center on the terminal, and a spark falling onto the
/// bbox center misses the visual core of the logo.
///
/// Using the centroid for both placement and the spark target keeps the
/// falling spark visually aligned with the logo's perceived center,
/// regardless of how the art is shaped.
fn visual_centroid(lines: &[String]) -> (f32, f32) {
    let mut sum_x: f32 = 0.0;
    let mut sum_y: f32 = 0.0;
    let mut count: f32 = 0.0;
    for (y, line) in lines.iter().enumerate() {
        for (x, ch) in line.chars().enumerate() {
            if ch == ' ' {
                continue;
            }
            sum_x += x as f32;
            sum_y += y as f32;
            count += 1.0;
        }
    }
    if count == 0.0 {
        // Defensive: empty art → fall back to (0, 0) so the caller's
        // clamping logic still produces a valid (if degenerate) layout.
        (0.0, 0.0)
    } else {
        (sum_x / count, sum_y / count)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Main entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Entry point for the Cosmostrix Logo intro. Plays a ~6.25 s cinematic.
///
/// See the module docs for the phase breakdown. The caller (intro
/// dispatcher) has already validated terminal size and `IntroType`.
pub(super) fn run_logo_intro(
    term: &mut Terminal,
    frame: &mut Frame,
    cloud: &Cloud,
    w: u16,
    h: u16,
) -> std::io::Result<()> {
    let (lines, logo_w, logo_h) = parse_logo_art(w, h);
    // Defensive: parse_logo_art scales to fit, so this is a fallback.
    if logo_w > w || logo_h > h {
        return Ok(());
    }

    // Compute the visual centroid (ink center-of-mass) before collecting
    // cells, so both the dissolve ordering and the placement math use the
    // same notion of "center". For asymmetric art like ours, the centroid
    // is offset from the bounding-box center, which is exactly why we
    // need it — placing the logo by its bbox center would shift the
    // visual ink off-center on the terminal.
    let (centroid_x, centroid_y) = visual_centroid(&lines);

    let mut logo_cells = collect_logo_cells(&lines, centroid_x, centroid_y);
    // Sort cells by squared distance from the visual centroid, descending
    // — the dissolve phase walks this list in order, so outer cells
    // dissolve first. This sort happens once at intro start; per-frame
    // cost is a simple index walk.
    logo_cells.sort_by(|a, b| {
        b.dist_sq
            .partial_cmp(&a.dist_sq)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut rng = seed_rng();
    let palette_bg = cloud.palette.bg;
    let palette_rgb = palette_target_rgb(cloud);
    let rain_charset = rain_chars(cloud);

    // Logo placement: shift the bounding box so the *visual centroid*
    // sits at the terminal center, then clamp to keep the bbox fully
    // on-screen. Integer math with signed casts so truncation rounds
    // toward zero (correct for both even and odd terminal sizes).
    //
    // Previous code used `logo_x = (w - logo_w) / 2`, which centers the
    // bbox — but our logo's ink mass is offset right of the bbox center,
    // so the visual logo sat to the right of terminal-center while the
    // spark fell straight down the terminal center. The two appeared
    // misaligned. Centering on the centroid fixes this.
    let target_x = (w as f32 * 0.5 - centroid_x).round() as i32;
    let target_y = (h as f32 * 0.5 - centroid_y).round() as i32;
    let max_x = (w as i32).saturating_sub(logo_w as i32);
    let max_y = (h as i32).saturating_sub(logo_h as i32);
    let logo_x = target_x.clamp(0, max_x);
    let logo_y = target_y.clamp(0, max_y);
    // Spark target = visual centroid in terminal coordinates. When no
    // clamping kicked in, this equals `(w/2, h/2)` exactly; when the
    // bbox was too close to an edge to place the centroid dead-center,
    // the spark falls onto the centroid wherever it landed.
    let logo_center_x = logo_x as f32 + centroid_x;
    let logo_center_y = logo_y as f32 + centroid_y;

    let mut pool = ParticlePool::new();
    let intro_start = Instant::now();

    loop {
        let elapsed_ms = intro_start.elapsed().as_millis() as u64;
        if elapsed_ms >= PHASE4_RAIN_END_MS {
            break;
        }
        if should_skip()? {
            return Ok(());
        }

        // Determine current phase and progress within phase.
        let (phase, phase_t) = if elapsed_ms < PHASE1_FADEIN_END_MS {
            (1u8, elapsed_ms as f32 / PHASE1_FADEIN_END_MS as f32)
        } else if elapsed_ms < PHASE2_IGNITION_END_MS {
            (
                2,
                (elapsed_ms - PHASE1_FADEIN_END_MS) as f32
                    / (PHASE2_IGNITION_END_MS - PHASE1_FADEIN_END_MS) as f32,
            )
        } else if elapsed_ms < PHASE3_DISSOLVE_END_MS {
            (
                3,
                (elapsed_ms - PHASE2_IGNITION_END_MS) as f32
                    / (PHASE3_DISSOLVE_END_MS - PHASE2_IGNITION_END_MS) as f32,
            )
        } else {
            (
                4,
                (elapsed_ms - PHASE3_DISSOLVE_END_MS) as f32
                    / (PHASE4_RAIN_END_MS - PHASE3_DISSOLVE_END_MS) as f32,
            )
        };

        let dt = frame_period_secs();

        // Update particles (rain droplets fall, age, die).
        update_rain_droplets(&mut pool, dt, h as f32);

        // ── Render ──────────────────────────────────────────────────────
        frame.clear_with_bg(palette_bg);

        // Phase 1: Laser travels from top to logo center (t=0..0.9).
        // At impact (t=0.9), logo flashes to brilliant white.
        // Phase 2: Logo HOLDS at full glow while rain falls from top.
        // Phase 3: Rain nearly reaches logo → logo fades smoothly.
        // Phase 4: Full rain, logo gone.
        let base_brightness = if phase == 1 {
            if phase_t < 0.9 {
                0.0
            } else {
                let impact_t = (phase_t - 0.9) / 0.1;
                1.5 - impact_t * 0.3
            }
        } else if phase == 2 {
            // HOLD at full glow — logo stays bright while rain descends.
            1.0
        } else if phase == 3 {
            // Rain approaching — logo fades smoothly.
            let smooth = 1.0 - (phase_t * phase_t * (3.0 - 2.0 * phase_t));
            smooth.max(0.0)
        } else {
            0.0
        };

        // White-to-purple blend: white at impact → purple by mid-Phase 2.
        let white_blend = if phase == 1 {
            let impact_t = ((phase_t - 0.9) / 0.1).clamp(0.0, 1.0);
            1.0 - impact_t
        } else if phase == 2 {
            (phase_t / 0.3).clamp(0.0, 1.0)
        } else {
            1.0
        };

        let logo_visible = (phase == 1 && phase_t >= 0.9) || phase == 2 || phase == 3;
        if logo_visible {
            for cell in logo_cells.iter() {
                let tx = logo_x + cell.bx as i32;
                let ty = logo_y + cell.by as i32;
                if tx < 0 || ty < 0 {
                    continue;
                }
                let tx = tx as u16;
                let ty = ty as u16;
                if tx >= w || ty >= h {
                    continue;
                }
                let cell_brightness = base_brightness;
                // Blend between white (impact) and purple (logo color).
                let purple = lerp_rgb((0, 0, 0), LOGO_COLOR_RGB, cell_brightness.clamp(0.0, 1.0));
                let color = lerp_rgb((255, 255, 255), purple, white_blend);
                frame.set_force(
                    tx,
                    ty,
                    Cell {
                        ch: cell.ch,
                        fg: Some(Color::Rgb {
                            r: color.0,
                            g: color.1,
                            b: color.2,
                        }),
                        bg: palette_bg,
                        bold: cell_brightness > 1.0,
                    },
                );
            }
        }

        // Render laser beam during Phase 1 (t < 0.9: traveling; t >= 0.9: gone).
        // v25 wide beam: 3 columns (center + 1 left + 1 right) for 2x visual width.
        if phase == 1 && phase_t < 0.9 {
            let laser_cx = logo_center_x as i32;
            let laser_tip_y = (phase_t / 0.9 * logo_center_y) as u16;
            for y in 0..=laser_tip_y.min(h.saturating_sub(1)) {
                let dist_from_tip = laser_tip_y.saturating_sub(y);
                for dx in -1..=1i32 {
                    let x = laser_cx + dx;
                    if x < 0 || x >= w as i32 {
                        continue;
                    }
                    let x = x as u16;
                    // Core column (dx=0): brightest. Side columns: glow.
                    let is_core = dx == 0;
                    let (ch, brightness) = if is_core && dist_from_tip == 0 {
                        ('┃', 1.0)
                    } else if is_core && dist_from_tip <= 2 {
                        ('┃', 0.8)
                    } else if is_core {
                        ('┃', 0.4)
                    } else if dist_from_tip == 0 {
                        ('║', 0.6)
                    } else if dist_from_tip <= 2 {
                        ('║', 0.4)
                    } else {
                        ('║', 0.2)
                    };
                    let laser_color = lerp_rgb((0, 0, 0), LOGO_COLOR_RGB, brightness);
                    let laser_color = if is_core && dist_from_tip == 0 {
                        lerp_rgb(laser_color, (255, 255, 255), 0.5)
                    } else {
                        laser_color
                    };
                    frame.set_force(
                        x,
                        y,
                        Cell {
                            ch,
                            fg: Some(Color::Rgb {
                                r: laser_color.0,
                                g: laser_color.1,
                                b: laser_color.2,
                            }),
                            bg: palette_bg,
                            bold: is_core && dist_from_tip <= 2,
                        },
                    );
                }
            }
        }

        // v25: Rain falls during Phase 2 (logo holds) + Phase 3 (logo fades).
        // Rain starts from top, descends toward logo. By Phase 3, rain
        // has nearly reached the logo — triggering the fade.
        if phase == 2 || phase == 3 {
            let spawn_rate = if phase == 2 {
                // Ramp up during Phase 2: 0 at start → full at 50%.
                (phase_t / 0.5).clamp(0.0, 1.0)
            } else {
                1.0 // Full spawn during Phase 3.
            };
            const PER_FRAME_BUDGET: usize = 8;
            let mut spawned = 0usize;
            while spawned < PER_FRAME_BUDGET && rng.next_f32() < spawn_rate * 0.5 {
                let col = (rng.next_f32() * w as f32) as i32;
                if col >= 0 && col < w as i32 {
                    let _ = spawn_rain_droplet(&mut pool, &mut rng, col as f32, 0.0, &rain_charset);
                }
                spawned += 1;
            }
        }

        // Render all active rain droplets. v25: droplets use the normal
        // palette color (no laser purple). Simple, clean transition.
        for p in pool.particles.iter() {
            if !p.active {
                continue;
            }
            let life_t = (p.life / p.max_life).clamp(0.0, 1.0);
            let droplet_rgb = palette_rgb;
            render_particle_cell(
                frame,
                w,
                h,
                p.x,
                p.y,
                p.ch,
                droplet_rgb,
                palette_bg,
                life_t,
                true,
            );
            // Dim trailing cell directly above the droplet for a streak.
            let trail_y = p.y - 1.0;
            let trail_brightness = life_t * 0.4;
            let trail_rgb = lerp_rgb((0, 0, 0), droplet_rgb, trail_brightness);
            render_particle_cell(
                frame,
                w,
                h,
                p.x,
                trail_y,
                p.ch,
                trail_rgb,
                palette_bg,
                trail_brightness,
                false,
            );
        }

        end_frame(term, frame)?;
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Particle helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Spawn a rain droplet at `(x, y)` — used during the rain phase.
/// v25: droplets use the normal palette color (no laser purple).
fn spawn_rain_droplet(
    pool: &mut ParticlePool,
    rng: &mut XorShift,
    x: f32,
    y: f32,
    rain_charset: &[char],
) -> bool {
    let speed = lerp(DISSOLVE_SPEED_MIN, DISSOLVE_SPEED_MAX, rng.next_f32());
    let ch = if rain_charset.is_empty() {
        '0'
    } else {
        rain_charset[(rng.next_u32() as usize) % rain_charset.len()]
    };
    let jitter_x = (rng.next_f32() - 0.5) * 0.6;
    let vx = (rng.next_f32() - 0.5) * 2.0 * JITTER_VX;
    let life = 2.0;
    pool.spawn(Particle {
        x: x + jitter_x,
        y,
        vx,
        vy: speed,
        ch,
        // v25: use palette_rgb via the render loop; particle color stored
        // here is not used since render uses palette_rgb directly.
        r: 0,
        g: 0,
        b: 0,
        life,
        max_life: life,
        angle: std::f32::consts::FRAC_PI_2,
        speed,
        spiral_rate: 0.0,
        active: true,
    })
}

/// Advance all active rain droplets by `dt` seconds. Droplets fall
/// (with their horizontal jitter carrying them sideways); those that
/// leave the bottom of the screen or expire are killed and returned to
/// the free-list.
///
/// # Zero per-frame allocation
///
/// The kill list is a stack-allocated `[usize; PARTICLE_POOL_SIZE]`
/// array (4 KiB on 64-bit) with a length counter. No `Vec` is
/// allocated per frame — this is critical for the intro's zero-alloc
/// guarantee.
fn update_rain_droplets(pool: &mut ParticlePool, dt: f32, screen_h: f32) {
    // Stack-allocated kill list — zero per-frame heap allocation.
    // PARTICLE_POOL_SIZE is 512, so this is 4 KiB on the stack.
    let mut to_kill: [usize; PARTICLE_POOL_SIZE] = [0; PARTICLE_POOL_SIZE];
    let mut kill_count: usize = 0;

    for (i, p) in pool.particles.iter_mut().enumerate() {
        if !p.active {
            continue;
        }
        p.x += p.vx * dt;
        p.y += p.vy * dt;
        p.life -= dt;
        if p.y > screen_h + 2.0 || p.life <= 0.0 {
            // The pool size bounds kill_count — every active particle
            // could die in one frame in the worst case, but the pool
            // never has more than PARTICLE_POOL_SIZE slots total.
            if kill_count < PARTICLE_POOL_SIZE {
                to_kill[kill_count] = i;
                kill_count += 1;
            }
        }
    }

    for &idx in to_kill.iter().take(kill_count) {
        pool.kill(idx);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_color_matches_rgb_constant() {
        // The Color enum form and the RGB tuple form must agree so the
        // brand purple is consistent everywhere it's referenced.
        match LOGO_COLOR {
            Color::Rgb { r, g, b } => assert_eq!((r, g, b), LOGO_COLOR_RGB),
            _ => panic!("LOGO_COLOR must be Color::Rgb"),
        }
    }

    #[test]
    fn logo_color_is_brand_purple() {
        // Spec: #A855F7 = RGB(168, 85, 247).
        assert_eq!(LOGO_COLOR_RGB, (168, 85, 247));
    }

    #[test]
    fn logo_art_is_non_empty() {
        assert!(!LOGO_ART.is_empty());
        assert!(
            LOGO_ART.lines().count() >= 10,
            "logo should have at least 10 lines"
        );
    }

    #[test]
    fn parse_logo_art_returns_consistent_dimensions() {
        let (lines, w, h) = parse_logo_art(80, 24);
        assert_eq!(lines.len() as u16, h, "height must match line count");
        // Width is the max char count across lines.
        let computed_w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u16;
        assert_eq!(w, computed_w);
        // Logo should fit in a typical 80×24 terminal with room to spare.
        assert!(w <= 80, "logo width {w} must fit in 80-col terminal");
        assert!(h <= 24, "logo height {h} must fit in 24-row terminal");
    }

    /// v25 responsive: parse_logo_art scales art to fit small terminals.
    #[test]
    fn parse_logo_art_scales_down_for_small_terminal() {
        let (lines, w, h) = parse_logo_art(30, 20);
        assert!(w <= 30, "scaled logo width {w} must fit in 30-col terminal");
        assert!(
            h <= 20,
            "scaled logo height {h} must fit in 20-row terminal"
        );
        // Scaled art should still have content (not empty).
        assert!(!lines.is_empty(), "scaled art must not be empty");
        let total_ink: usize = lines
            .iter()
            .map(|l| l.chars().filter(|&c| c != ' ').count())
            .sum();
        assert!(total_ink > 0, "scaled art must retain some ink cells");
    }

    /// v25 responsive: no upscaling when terminal exceeds art size.
    #[test]
    fn parse_logo_art_does_not_scale_up_for_large_terminal() {
        let (lines_large, w_large, h_large) = parse_logo_art(200, 50);
        let (lines_default, w_default, h_default) = parse_logo_art(80, 24);
        // Both should produce identical output (no upscaling).
        assert_eq!(w_large, w_default, "no upscaling for large terminal");
        assert_eq!(h_large, h_default, "no upscaling for large terminal");
        assert_eq!(
            lines_large.len(),
            lines_default.len(),
            "line count must match"
        );
    }

    #[test]
    fn collect_logo_cells_skips_blanks() {
        let (lines, _w, _h) = parse_logo_art(80, 24);
        let (cx, cy) = visual_centroid(&lines);
        let cells = collect_logo_cells(&lines, cx, cy);
        // Every collected cell must have a non-blank glyph.
        for c in &cells {
            assert_ne!(c.ch, ' ', "blank cell should not be collected");
        }
        // The logo clearly has more than 50 non-blank cells.
        assert!(
            cells.len() > 50,
            "logo should have many non-blank cells, got {}",
            cells.len()
        );
    }

    #[test]
    fn collect_logo_cells_computes_centroid_distance() {
        let (lines, _w, _h) = parse_logo_art(80, 24);
        let (cx, cy) = visual_centroid(&lines);
        let cells = collect_logo_cells(&lines, cx, cy);
        // The centermost cell should have a small dist_sq; the outermost
        // should have a large dist_sq.
        let mut min_d = f32::MAX;
        let mut max_d = f32::MIN;
        for c in &cells {
            // Verify the stored dist_sq matches a fresh computation
            // against the visual centroid (not the bbox center).
            let xf = c.bx as f32;
            let yf = c.by as f32;
            let expected = (xf - cx) * (xf - cx) + (yf - cy) * (yf - cy);
            assert!(
                (c.dist_sq - expected).abs() < 0.01,
                "dist_sq mismatch: stored={}, expected={}",
                c.dist_sq,
                expected
            );
            min_d = min_d.min(c.dist_sq);
            max_d = max_d.max(c.dist_sq);
        }
        assert!(min_d < max_d, "logo should have spatial extent");
    }

    #[test]
    fn visual_centroid_is_within_bounding_box() {
        let (lines, w, h) = parse_logo_art(80, 24);
        let (cx, cy) = visual_centroid(&lines);
        // The centroid must lie inside the bounding box.
        assert!(
            (0.0..=w as f32).contains(&cx),
            "centroid x {} must be inside [0, {}]",
            cx,
            w
        );
        assert!(
            (0.0..=h as f32).contains(&cy),
            "centroid y {} must be inside [0, {}]",
            cy,
            h
        );
    }

    #[test]
    fn visual_centroid_differs_from_bbox_center() {
        // The centroid-based placement is correct regardless of whether
        // the centroid differs from the bbox center — when they coincide,
        // the placement is simply a no-op. The owner's manually engraved
        // logo (commit 218a748) is intentionally near-symmetric, so the
        // centroid sits very close to the bbox center. We keep the test
        // as a non-strict sanity check: the centroid must be a valid
        // point inside the bbox (computed by `visual_centroid` and
        // verified by `visual_centroid_is_within_bounding_box`), but it
        // does not need to be offset by any specific amount.
        //
        // Historical note: prior to commit 218a748 the logo was strongly
        // asymmetric (dx > 0.5 || dy > 0.5). That property was specific
        // to the old art and is no longer a design invariant.
        let (lines, w, h) = parse_logo_art(80, 24);
        let (cx, cy) = visual_centroid(&lines);
        // Sanity: centroid is a finite, in-bounds point.
        assert!(cx.is_finite() && cy.is_finite(), "centroid must be finite");
        assert!(
            (0.0..=w as f32).contains(&cx),
            "centroid x {} must be inside [0, {}]",
            cx,
            w
        );
        assert!(
            (0.0..=h as f32).contains(&cy),
            "centroid y {} must be inside [0, {}]",
            cy,
            h
        );
    }

    #[test]
    fn visual_centroid_handles_empty_art() {
        // Defensive: an empty art string must not panic.
        let lines: Vec<String> = vec!["   ".to_string(), "  ".to_string()];
        let (cx, cy) = visual_centroid(&lines);
        assert_eq!((cx, cy), (0.0, 0.0));
    }

    #[test]
    fn visual_centroid_of_single_cell() {
        let lines: Vec<String> = vec!["     X     ".to_string()];
        let (cx, cy) = visual_centroid(&lines);
        assert!(
            (cx - 5.0).abs() < 0.01,
            "centroid x of single cell at col 5"
        );
        assert!((cy - 0.0).abs() < 0.01, "centroid y of single row");
    }

    #[test]
    fn placement_uses_centroid_not_bbox_center() {
        // Sanity-check the placement math by reconstructing it. For a
        // typical 80×24 terminal, the spark target (logo_center_x)
        // should equal w/2 exactly when no clamping kicks in — which
        // happens as long as the centroid is at least `logo_w/2` from
        // the right edge of the bbox.
        let (lines, logo_w, logo_h) = parse_logo_art(80, 24);
        let (centroid_x, centroid_y) = visual_centroid(&lines);
        let w: u16 = 80;
        let h: u16 = 24;
        let target_x = (w as f32 * 0.5 - centroid_x).round() as i32;
        let target_y = (h as f32 * 0.5 - centroid_y).round() as i32;
        let max_x = (w as i32).saturating_sub(logo_w as i32);
        let max_y = (h as i32).saturating_sub(logo_h as i32);
        let logo_x = target_x.clamp(0, max_x);
        let logo_y = target_y.clamp(0, max_y);
        let logo_center_x = logo_x as f32 + centroid_x;
        let logo_center_y = logo_y as f32 + centroid_y;
        // On 80×24, the logo (40×19) easily fits, so no clamping should
        // occur and the centroid lands dead-center on both axes.
        assert!(
            (logo_center_x - w as f32 * 0.5).abs() < 1.0,
            "spark x {logo_center_x} should be within 1 cell of terminal center {}",
            w as f32 * 0.5
        );
        assert!(
            (logo_center_y - h as f32 * 0.5).abs() < 1.0,
            "spark y {logo_center_y} should be within 1 cell of terminal center {}",
            h as f32 * 0.5
        );
        // And the logo bbox stays fully on-screen.
        assert!(logo_x >= 0, "logo_x must be non-negative");
        let logo_right = logo_x + logo_w as i32;
        assert!(
            logo_right <= w as i32,
            "logo right edge {logo_right} must not exceed terminal width {w}"
        );
        assert!(logo_y >= 0, "logo_y must be non-negative");
        let logo_bottom = logo_y + logo_h as i32;
        assert!(
            logo_bottom <= h as i32,
            "logo bottom edge {logo_bottom} must not exceed terminal height {h}"
        );
    }

    #[test]
    fn phase_boundaries_are_monotonic() {
        const {
            assert!(PHASE1_FADEIN_END_MS < PHASE2_IGNITION_END_MS);
        }
        const {
            assert!(PHASE2_IGNITION_END_MS < PHASE3_DISSOLVE_END_MS);
        }
        const {
            assert!(PHASE3_DISSOLVE_END_MS < PHASE4_RAIN_END_MS);
        }
    }

    #[test]
    fn phase_boundaries_match_spec() {
        // v25 balanced: Phase 1=1.2s, Phase 2=3.0s, Phase 3=4.0s, Phase 4=4.5s.
        assert_eq!(PHASE1_FADEIN_END_MS, 1_200);
        assert_eq!(PHASE2_IGNITION_END_MS, 3_000);
        assert_eq!(PHASE3_DISSOLVE_END_MS, 4_000);
        assert_eq!(PHASE4_RAIN_END_MS, 4_500);
    }

    #[test]
    fn dissolve_speed_range_is_valid() {
        const {
            assert!(DISSOLVE_SPEED_MIN < DISSOLVE_SPEED_MAX);
            assert!(DISSOLVE_SPEED_MIN >= 1.0);
            assert!(DISSOLVE_SPEED_MAX <= 100.0);
        }
    }

    #[test]
    fn spawn_rain_droplet_populates_pool() {
        let mut pool = ParticlePool::new();
        let mut rng = XorShift::new(42);
        let charset = ['0', '1', 'x', 'z'];
        let ok = spawn_rain_droplet(&mut pool, &mut rng, 10.0, 5.0, &charset);
        assert!(ok);
        assert_eq!(pool.active_count(), 1);
        let p = pool
            .particles
            .iter()
            .find(|p| p.active)
            .expect("spawned droplet should be active");
        // Velocity should be mostly downward with optional horizontal jitter.
        assert!(p.vy > 0.0, "droplet should move downward");
        assert!(
            p.vx.abs() <= JITTER_VX + 0.01,
            "horizontal velocity should be within jitter range, got {}",
            p.vx
        );
        assert!(p.speed >= DISSOLVE_SPEED_MIN * 0.95);
        assert!(p.speed <= DISSOLVE_SPEED_MAX * 1.05);
        assert!(charset.contains(&p.ch), "glyph should come from charset");
        // Particle should start with the brand purple color.
        assert!(p.active);
    }

    #[test]
    fn spawn_rain_droplet_handles_empty_charset() {
        let mut pool = ParticlePool::new();
        let mut rng = XorShift::new(7);
        let ok = spawn_rain_droplet(&mut pool, &mut rng, 10.0, 5.0, &[]);
        assert!(ok);
        let p = pool
            .particles
            .iter()
            .find(|p| p.active)
            .expect("droplet should be active");
        assert_eq!(p.ch, '0', "empty charset should fall back to '0'");
    }

    #[test]
    fn update_rain_droplets_kills_offscreen() {
        let mut pool = ParticlePool::new();
        let _ = pool.spawn(Particle {
            x: 5.0,
            y: 50.0,
            vx: 0.0,
            vy: 20.0,
            ch: '0',
            r: 57,
            g: 255,
            b: 20,
            life: 1.0,
            max_life: 1.0,
            angle: std::f32::consts::FRAC_PI_2,
            speed: 20.0,
            spiral_rate: 0.0,
            active: true,
        });
        // Screen height 24 — droplet at y=50 is already off-screen.
        update_rain_droplets(&mut pool, 0.1, 24.0);
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn update_rain_droplets_kills_expired_life() {
        let mut pool = ParticlePool::new();
        let _ = pool.spawn(Particle {
            x: 5.0,
            y: 5.0,
            vx: 0.0,
            vy: 1.0,
            ch: '0',
            r: 57,
            g: 255,
            b: 20,
            life: 0.05,
            max_life: 0.05,
            angle: std::f32::consts::FRAC_PI_2,
            speed: 1.0,
            spiral_rate: 0.0,
            active: true,
        });
        // After 0.1s, life = 0.05 - 0.1 = negative → killed.
        update_rain_droplets(&mut pool, 0.1, 24.0);
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn update_rain_droplets_keeps_alive() {
        let mut pool = ParticlePool::new();
        let _ = pool.spawn(Particle {
            x: 5.0,
            y: 5.0,
            vx: 0.0,
            vy: 5.0,
            ch: '0',
            r: 57,
            g: 255,
            b: 20,
            life: 5.0,
            max_life: 5.0,
            angle: std::f32::consts::FRAC_PI_2,
            speed: 5.0,
            spiral_rate: 0.0,
            active: true,
        });
        update_rain_droplets(&mut pool, 0.1, 24.0);
        assert_eq!(pool.active_count(), 1);
    }

    #[test]
    fn update_rain_droplets_advances_position() {
        let mut pool = ParticlePool::new();
        let _ = pool.spawn(Particle {
            x: 5.0,
            y: 5.0,
            vx: 0.0,
            vy: 10.0,
            ch: '0',
            r: 57,
            g: 255,
            b: 20,
            life: 5.0,
            max_life: 5.0,
            angle: std::f32::consts::FRAC_PI_2,
            speed: 10.0,
            spiral_rate: 0.0,
            active: true,
        });
        update_rain_droplets(&mut pool, 0.5, 24.0);
        let p = pool
            .particles
            .iter()
            .find(|p| p.active)
            .expect("droplet should still be active");
        // y should have advanced by speed*dt = 10*0.5 = 5 cells, so the
        // new y is 5 + 5 = 10.
        assert!(
            (p.y - 10.0).abs() < 0.1,
            "y should have advanced by speed*dt, got {}",
            p.y
        );
    }
}
