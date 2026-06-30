// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Lightning atmospheric event — flash-only illumination.
//!
//! Instead of drawing bolt paths, lightning modifies the foreground colors
//! of existing rain cells (and other rendered content), blending them toward
//! white or toward black depending on the lifecycle phase. This creates a
//! full-screen illumination effect that feels like lightning without adding
//! new characters to the frame.
//!
//! ## Bolt Families (v10.0.0 Flash Pivot)
//!
//! Each family maps to a different flash geometry and peak intensity:
//!
//! - 0: FullScreen — entire screen, peak 0.7
//! - 1: CloudLayer — top 35% of screen, peak 0.5
//! - 2: Sweep — left-to-right intensity gradient, peak 0.65
//! - 3: DualPeak — two bright columns with falloff, peak 0.7
//! - 4: Intense — full screen at peak 1.0
//! - 5: Diffuse — full screen at low peak 0.45
//!
//! ## Return Strokes
//!
//! 25% chance of 1-2 secondary flashes after initial bolt, with a 40-80ms
//! dark gap between strokes at 60-80% brightness. During dark gaps, the
//! screen is dimmed slightly to create a visible flicker.

use std::time::{Duration, Instant};

use crossterm::style::Color;

use crate::constants::*;
use crate::frame::Frame;

use super::super::atmospheric_events::{AtmosphericEvent, EventCtx, EventState};

/// Phase within the lightning lifecycle.
#[derive(Clone, Copy, Debug, PartialEq)]
enum LightningPhase {
    /// 0-50ms: bolt appears at peak intensity.
    Strike,
    /// 50-200ms: secondary glow, branch visibility, flash.
    Flash,
    /// 200-700ms: phosphor-powered fade (no per-frame rendering).
    Decay,
    /// Return stroke dark gap (brief pause before secondary flash).
    ReturnStrokeDark,
    /// Return stroke flash (secondary peak at reduced brightness).
    ReturnStrokeFlash,
    /// Event complete.
    Finished,
}

/// Flash illumination geometry type.
#[derive(Clone, Copy, Debug)]
enum FlashType {
    /// Full-screen uniform illumination.
    FullScreen,
    /// Top portion only (cloud layer).
    CloudLayer,
    /// Left-to-right intensity gradient.
    Sweep,
    /// Two bright columns with falloff.
    DualPeak,
    /// Full-screen at high intensity.
    Intense,
    /// Full-screen at low intensity, uniform.
    Diffuse,
}

/// A lightning event that illuminates the scene via color blending.
///
/// No line geometry is stored — only flash parameters and phase state.
/// Rendering reads existing cell foreground colors and blends them
/// toward white (illumination) or black (dark gap).
#[allow(dead_code)]
pub(crate) struct LightningEvent {
    /// Current lifecycle phase.
    phase: LightningPhase,
    /// When the current phase began.
    phase_start: Instant,
    /// When the event was spawned.
    spawn_time: Instant,
    /// Overall intensity multiplier (0.0-2.0, default 1.0).
    intensity: f32,
    /// Terminal dimensions at spawn time.
    cols: u16,
    lines: u16,

    // ── Bolt family (v10.0.0 Phase 2D) ──
    /// Bolt family index (0-5).
    bolt_family: u8,
    /// Target length as fraction of screen height.
    length_pct: f32,
    /// Per-family brightness multiplier.
    family_brightness: f32,

    // ── Return strokes (v10.0.0 Phase 2D) ──
    /// Total return strokes configured.
    return_stroke_count: u8,
    /// Return strokes completed so far.
    return_stroke_done: u8,
    /// True during a return stroke flash.
    #[allow(dead_code)]
    return_stroke_phase: bool,
    /// Dark gap end time (when the return flash should begin).
    return_stroke_dark_until: Option<Instant>,

    // ── Flash geometry (v10.0.0 Flash Pivot) ──
    /// Type of flash illumination pattern.
    flash_type: FlashType,
    /// Flash region: (col_start, col_end, line_start, line_end).
    flash_region: (u16, u16, u16, u16),
    /// Peak intensity for this flash (0.0-1.0).
    peak_intensity: f32,
    /// Whether this is a near-strike (camera shake proximity).
    is_near_strike: bool,

    /// Last palette color, captured at spawn for phosphor seeding.
    last_palette_color: Option<Color>,
}

impl LightningEvent {
    /// Create a new lightning event with flash parameters.
    /// `bolt_family`: 0-5 family index
    /// `length_pct`: target length as fraction of screen height
    /// `return_strokes`: 0-2 return strokes after the initial bolt
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cols: u16,
        lines: u16,
        intensity: f32,
        palette_color: Option<Color>,
        bolt_family: u8,
        length_pct: f32,
        return_strokes: u8,
    ) -> Self {
        let now = Instant::now();

        // Determine per-family brightness
        let family_brightness = match bolt_family {
            0 => 0.85, // Long: normal
            1 => 0.75, // Short: dimmer (stopped mid-screen)
            2 => 0.95, // Diagonal: slightly bright
            3 => 1.0,  // Fork: normal, branches carry the drama
            4 => 1.25, // Massive: bright (min 1.2 per spec)
            _ => 0.7,  // Sheet: dimmer individual channels (the quantity creates impact)
        };

        // Map bolt family to flash configuration with RANDOM column bands.
        // Distant lightning illuminates a PORTION of the sky, not the entire screen.
        // Only family 4 (Intense/Massive, 8% chance) gets full-screen.
        let band_width = (cols as f32 * (0.25 + rand::random::<f32>() * 0.35)) as u16; // 25-60% of width
        let band_start = (rand::random::<f32>() * (cols.saturating_sub(band_width)) as f32) as u16;
        let band_end = (band_start + band_width).min(cols);

        let (flash_type, flash_region, mut peak_intensity) = match bolt_family {
            0 => (
                FlashType::FullScreen,
                (band_start, band_end, 0, lines),
                0.20,
            ),
            1 => (
                FlashType::CloudLayer,
                (band_start, band_end, 0, lines * 35 / 100),
                0.15,
            ),
            2 => (
                FlashType::Sweep,
                (
                    band_start,
                    (band_start + band_width * 2).min(cols),
                    0,
                    lines,
                ),
                0.18,
            ),
            3 => (FlashType::DualPeak, (band_start, band_end, 0, lines), 0.22),
            4 => (FlashType::Intense, (0, cols, 0, lines), 0.55),
            _ => (FlashType::Diffuse, (band_start, band_end, 0, lines), 0.12),
        };

        // 1% near-strike chance: override with full-screen, low-intensity flash
        let is_near_strike = {
            use rand::distr::Distribution;
            let uniform = rand::distr::Uniform::new(0.0f32, 1.0f32).expect("[0,1) always valid");
            let mut rng = rand::rng();
            uniform.sample(&mut rng) < 0.01
        };
        if is_near_strike {
            let (ft, fr, _) = match bolt_family {
                0 => (FlashType::FullScreen, (0, cols, 0, lines), 0.7),
                _ => (FlashType::FullScreen, (0, cols, 0, lines), 0.7),
            };
            // Use the flash_type and region but override peak
            peak_intensity = 0.12;
            Self {
                phase: LightningPhase::Strike,
                phase_start: now,
                spawn_time: now,
                intensity: intensity.clamp(0.1, 2.0),
                cols,
                lines,
                bolt_family,
                length_pct: length_pct.clamp(0.1, 1.0),
                family_brightness,
                return_stroke_count: return_strokes.min(2),
                return_stroke_done: 0,
                return_stroke_phase: false,
                return_stroke_dark_until: None,
                flash_type: ft,
                flash_region: fr,
                peak_intensity,
                is_near_strike: true,
                last_palette_color: palette_color,
            }
        } else {
            Self {
                phase: LightningPhase::Strike,
                phase_start: now,
                spawn_time: now,
                intensity: intensity.clamp(0.1, 2.0),
                cols,
                lines,
                bolt_family,
                length_pct: length_pct.clamp(0.1, 1.0),
                family_brightness,
                return_stroke_count: return_strokes.min(2),
                return_stroke_done: 0,
                return_stroke_phase: false,
                return_stroke_dark_until: None,
                flash_type,
                flash_region,
                peak_intensity,
                is_near_strike: false,
                last_palette_color: palette_color,
            }
        }
    }

    /// Compute progress [0.0, 1.0] within the current phase based on elapsed time.
    fn calc_phase_progress(&self, now: Instant) -> f32 {
        let elapsed = now
            .saturating_duration_since(self.phase_start)
            .as_secs_f32();
        let duration = match self.phase {
            LightningPhase::Strike => {
                (LIGHTNING_ACTIVE_MS as f32 * LIGHTNING_STRIKE_FRACTION) / 1000.0
            }
            LightningPhase::Flash => {
                (LIGHTNING_ACTIVE_MS as f32 * (1.0 - LIGHTNING_STRIKE_FRACTION)) / 1000.0
            }
            LightningPhase::ReturnStrokeDark => 0.06,
            LightningPhase::ReturnStrokeFlash => 0.06,
            LightningPhase::Decay | LightningPhase::Finished => 0.0,
        };
        if duration <= 0.0 {
            return 1.0;
        }
        (elapsed / duration).clamp(0.0, 1.0)
    }

    /// Illuminate a rectangular region of the frame by blending existing cell
    /// foreground colors toward white (positive intensity) or black (negative).
    #[allow(clippy::too_many_arguments)]
    fn illuminate(
        &self,
        frame: &mut Frame,
        ctx: &EventCtx,
        c0: u16,
        c1: u16,
        l0: u16,
        l1: u16,
        intensity: f32,
    ) {
        if intensity.abs() < 0.01 {
            return;
        }
        let max_line = l1.min(self.lines).min(ctx.lines);
        let max_col = c1.min(self.cols).min(ctx.cols);

        for line in l0..max_line {
            for col in c0..max_col {
                // Skip message box
                if let Some((mx, my, mw, mh)) = ctx.message_bounds {
                    if col >= mx && col < mx + mw && line >= my && line < my + mh {
                        continue;
                    }
                }

                let Some(idx) = frame.index(col, line) else {
                    continue;
                };
                let cell = frame.cell_at_index(idx);

                // Per-column modulation for Sweep/DualPeak flash types
                let col_intensity = match self.flash_type {
                    FlashType::Sweep => {
                        let progress = (col - c0) as f32 / (c1 - c0).max(1) as f32;
                        intensity * (0.2 + 0.8 * progress)
                    }
                    FlashType::DualPeak => {
                        let mid = (c0 + c1) as f32 / 2.0;
                        let third = (c1 - c0) as f32 / 6.0;
                        let p1 = mid - (c1 - c0) as f32 / 4.0;
                        let p2 = mid + (c1 - c0) as f32 / 4.0;
                        let d1 = (col as f32 - p1).abs().min(third);
                        let d2 = (col as f32 - p2).abs().min(third);
                        intensity * (1.0 - d1.min(d2) / third * 0.6)
                    }
                    _ => intensity,
                };

                if col_intensity.abs() < 0.01 {
                    continue;
                }

                // Edge falloff: soft-edged glow instead of hard rectangular block
                let band_mid = (c0 + c1) as f32 / 2.0;
                let band_half = (c1 - c0) as f32 / 2.0;
                let dist_from_center =
                    ((col as f32 - band_mid).abs() / band_half.max(1.0)).min(1.0);
                let edge_falloff = 1.0 - dist_from_center.powf(1.5) * 0.7;
                let col_intensity = col_intensity * edge_falloff;

                let fg = cell.fg;
                let (r, g, b) = match fg {
                    Some(Color::Rgb { r, g, b }) => (r as f32, g as f32, b as f32),
                    Some(Color::AnsiValue(v)) => {
                        let v = v as f32 / 255.0 * 255.0;
                        (v, v, v)
                    }
                    _ => continue,
                };

                if col_intensity > 0.0 {
                    let blend = (col_intensity * 0.45).clamp(0.0, 0.70);
                    let nr = (r + (255.0 - r) * blend) as u8;
                    let ng = (g + (255.0 - g) * blend) as u8;
                    let nb = (b + (255.0 - b) * blend) as u8;
                    let mut new_cell = cell;
                    new_cell.fg = Some(Color::Rgb {
                        r: nr,
                        g: ng,
                        b: nb,
                    });
                    frame.set_force(col, line, new_cell);
                } else {
                    let dim = (-col_intensity).clamp(0.0, 0.5);
                    let nr = (r * (1.0 - dim)) as u8;
                    let ng = (g * (1.0 - dim)) as u8;
                    let nb = (b * (1.0 - dim)) as u8;
                    let mut new_cell = cell;
                    new_cell.fg = Some(Color::Rgb {
                        r: nr,
                        g: ng,
                        b: nb,
                    });
                    frame.set_force(col, line, new_cell);
                }
            }
        }
    }
}

impl AtmosphericEvent for LightningEvent {
    fn state(&self) -> EventState {
        match self.phase {
            LightningPhase::Strike
            | LightningPhase::Flash
            | LightningPhase::ReturnStrokeDark
            | LightningPhase::ReturnStrokeFlash => EventState::Active,
            LightningPhase::Decay => EventState::Decay,
            LightningPhase::Finished => EventState::Finished,
        }
    }

    fn is_finished(&self) -> bool {
        self.phase == LightningPhase::Finished
    }

    fn phase_durations_ms(&self) -> (u64, u64) {
        // Return strokes extend the active duration
        let extra_active = self.return_stroke_count as u64 * 120; // ~120ms per return stroke
        (LIGHTNING_ACTIVE_MS + extra_active, LIGHTNING_DECAY_MS)
    }

    fn memory_footprint(&self) -> usize {
        std::mem::size_of::<Self>()
    }

    fn update(&mut self, now: Instant) {
        let total_active_ms = LIGHTNING_ACTIVE_MS as u128;
        let total_decay_ms = LIGHTNING_DECAY_MS as u128;
        let elapsed = now.saturating_duration_since(self.spawn_time).as_millis();

        // Check return stroke transitions
        if self.return_stroke_count > 0 && self.return_stroke_done < self.return_stroke_count {
            match self.phase {
                LightningPhase::Strike | LightningPhase::Flash => {
                    // After active phase ends, transition to return stroke dark gap
                    if elapsed >= total_active_ms {
                        // Dark gap duration (40-80ms)
                        let dark_gap_ms = 40u128 + (self.return_stroke_done as u128 * 20);
                        self.phase = LightningPhase::ReturnStrokeDark;
                        self.phase_start = now;
                        self.return_stroke_dark_until =
                            Some(now + Duration::from_millis(dark_gap_ms as u64));
                        return;
                    }
                }
                LightningPhase::ReturnStrokeDark => {
                    if let Some(dark_until) = self.return_stroke_dark_until {
                        if now >= dark_until {
                            self.phase = LightningPhase::ReturnStrokeFlash;
                            self.phase_start = now;
                            self.return_stroke_done += 1;
                            self.return_stroke_dark_until = None;
                            return;
                        }
                    } else {
                        // Fallback: if dark_until was none, advance after some time
                        let dark_elapsed =
                            now.saturating_duration_since(self.phase_start).as_millis();
                        if dark_elapsed > 60 {
                            self.phase = LightningPhase::ReturnStrokeFlash;
                            self.phase_start = now;
                            self.return_stroke_done += 1;
                            return;
                        }
                    }
                    return; // Don't apply normal phase transitions during dark gap
                }
                LightningPhase::ReturnStrokeFlash => {
                    let flash_elapsed = now.saturating_duration_since(self.phase_start).as_millis();
                    let return_flash_ms = 60u128;
                    if flash_elapsed >= return_flash_ms {
                        if self.return_stroke_done < self.return_stroke_count {
                            // Another dark gap before next return stroke
                            let dark_gap_ms = 40u128 + (self.return_stroke_done as u128 * 20);
                            self.phase = LightningPhase::ReturnStrokeDark;
                            self.phase_start = now;
                            self.return_stroke_dark_until =
                                Some(now + Duration::from_millis(dark_gap_ms as u64));
                            return;
                        } else {
                            // All return strokes done, go to decay
                            self.phase = LightningPhase::Decay;
                            return;
                        }
                    }
                    return;
                }
                _ => {}
            }
        }

        // Normal phase transitions (no return strokes or after all return strokes done)
        self.phase = if elapsed < (total_active_ms * LIGHTNING_STRIKE_FRACTION as u128 / 100) {
            LightningPhase::Strike
        } else if elapsed < total_active_ms {
            LightningPhase::Flash
        } else if elapsed < total_active_ms + total_decay_ms {
            LightningPhase::Decay
        } else {
            LightningPhase::Finished
        };
    }

    fn render(&self, ctx: &EventCtx, frame: &mut Frame) {
        let (c0, c1, l0, l1) = self.flash_region;
        let phase_progress = self.calc_phase_progress(ctx.now);

        match self.phase {
            LightningPhase::Strike => {
                let intensity = self.peak_intensity
                    * self.family_brightness
                    * (1.0 - (-phase_progress * 6.0).exp()); // exponential rise
                self.illuminate(frame, ctx, c0, c1, l0, l1, intensity);
            }
            LightningPhase::Flash => {
                let fade = phase_progress * 0.3;
                let intensity = self.peak_intensity * self.family_brightness * (1.0 - fade);
                self.illuminate(frame, ctx, c0, c1, l0, l1, intensity);
            }
            LightningPhase::ReturnStrokeDark => {
                self.illuminate(frame, ctx, c0, c1, l0, l1, -0.25);
            }
            LightningPhase::ReturnStrokeFlash => {
                let intensity = self.peak_intensity * self.family_brightness * 0.6;
                self.illuminate(frame, ctx, c0, c1, l0, l1, intensity);
            }
            LightningPhase::Decay | LightningPhase::Finished => {}
        }
    }

    fn pulse_factor(&self, now: Instant) -> f32 {
        if !matches!(
            self.phase,
            LightningPhase::Strike | LightningPhase::ReturnStrokeFlash
        ) {
            return 0.0;
        }
        let strike_ms = (LIGHTNING_ACTIVE_MS as f32) * LIGHTNING_STRIKE_FRACTION;
        let elapsed = now.saturating_duration_since(self.phase_start).as_millis() as f32;
        let progress = (elapsed / strike_ms.max(1.0)).min(1.0);
        // Per-family pulse strength
        let family_boost = match self.bolt_family {
            5 => 1.3,  // Heavy: strongest pulse
            4 => 1.15, // Ribbon: strong pulse
            _ => 1.0,
        };
        family_boost * (-progress * 4.0).exp()
    }

    fn seed_phosphor(
        &self,
        _phosphor: &mut [u8],
        _phosphor_base_fg: &mut [Option<Color>],
        _phosphor_base_ch: &mut [char],
        _cols: u16,
        _lines: u16,
    ) {
        // Flash-only illumination leaves no line geometry to seed.
        // Phosphor afterglow is handled naturally by the existing phosphor
        // system from the rain cells that were illuminated.
    }
}
