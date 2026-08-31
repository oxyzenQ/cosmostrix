// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Droplet draw method — extracted from `droplet/mod.rs` to keep that
//! source file under the 800-LOC hard cap.
//!
//! Owns `Droplet::draw()` — the per-cell rendering pipeline that
//! composes the visual effects stack (tail cleanup, edge fade, transition
//! energy, head bloom, head selfbloom, fractional head brightness, mouse
//! flash/glow, glitch, fog, vignette, brightness multiplier, saturation
//! multiplier) and writes the final cell to the frame.

use crossterm::style::Color;

use crate::cloud::{CharLoc, DrawCtx};
use crate::constants::{
    EDGE_FADE_BOLD_THRESHOLD, FOG_MIN_FACTOR, FOG_ROWS, FRACTIONAL_BLOOM_AMP, HEAD_BLOOM_CELLS,
    HEAD_BLOOM_INTENSITY, HEAD_BLOOM_SIGMA, HEAD_SELFBLOOM_BASE, HEAD_SHIMMER_PERIOD_SECS,
    MOUSE_FLASH_INTENSITY, MOUSE_FLASH_RING_WIDTH, MOUSE_FLASH_SECONDARY_FRAC,
    MOUSE_GLOW_INTENSITY, MOUSE_GLOW_RADIUS_COLS, MOUSE_GLOW_RADIUS_LINES,
    PARALLAX_BRIGHTNESS_MULT, PARALLAX_CONTRAST_REDUCTION, PARALLAX_GLYPH_DIM,
    PARALLAX_HEAD_BLOOM_MULT, PARALLAX_HEAD_SELFBLOOM_MULT, PARALLAX_SATURATION_MULT,
    RAIN_SHADOW_LAYER_MULT, TRANSITION_ENERGY_DURATION_SECS, TRANSITION_ENERGY_SATURATION_BOOST,
    TRANSITION_HEAD_GLOW_BOOST, VIGNETTE_LAYER_MULT,
};
use crate::frame::Frame;
use crate::palette;

use std::time::Instant;

impl super::Droplet {
    pub(crate) fn draw(
        &mut self,
        ctx: &DrawCtx<'_>,
        frame: &mut Frame,
        now: Instant,
        draw_everything: bool,
    ) {
        let bg = ctx.bg;

        let mut start_line = 0u16;
        if let Some(tp) = self.tail_put_line {
            let blank = crate::terminal::blank_cell(bg);
            // Use frame.set (equality-checked) instead of frame.set_force.
            // When multiple droplets share a column (storm scene with high
            // density), their tail cleanup ranges can overlap — the second
            // clear of an already-blank cell short-circuits instead of
            // dirty-marking redundantly. Also skips cells already cleared
            // by phosphor_decay_pass or monolith clear_cell earlier in the
            // frame. Saves ~10-30% of dirty marks in dense scenes.
            for line in self.tail_cur_line..=tp {
                frame.set(self.bound_col, line, blank);
            }
            self.tail_cur_line = tp;
            start_line = tp.saturating_add(1);
        }

        // PERF: head_brightness() depends only on `self` and `now`, NOT on
        // `line`. Previously it was called once per line inside the loop
        // via is_head_bright() and again at line 440 via head_brightness() —
        // 2× redundant Instant::elapsed() + exp() per line per droplet.
        // Hoist both computations out of the loop.
        let head_bright = self.head_brightness(now);
        let is_head_bright_cached = head_bright > 0.3;

        // F6: hoist loop-invariant transition energy + F7: fractional_progress.
        //
        // (chroma audit, A14): the hoist now stores the raw blend
        // factor (f32) instead of a pre-multiplied i32 weight, so the loop
        // body can route through chroma::palette::blend_toward_white_rgb /
        // chroma::legacy::blend_toward_white. Both helpers perform the
        // `(factor * 256.0) as i32` cast internally and use the identical
        // equation `r + (255 - r) * wf / 256` -- bit-identical to the
        // previous inline form.
        //
        // Factor range: t = 1.0 - age/duration (in [0, 1]) multiplied by
        // TRANSITION_ENERGY_SATURATION_BOOST = 0.25, so the blend factor
        // is in [0, 0.25] -- well within the chroma helper's [0, 1] clamp.
        let is_new_generation = self.palette_slot == ctx.active_palette_slot && ctx.transitioning;
        let transition_wf: Option<f32> = if is_new_generation {
            self.last_time.and_then(|birth| {
                let age = now.saturating_duration_since(birth).as_secs_f32();
                if age < TRANSITION_ENERGY_DURATION_SECS {
                    let t = 1.0 - (age / TRANSITION_ENERGY_DURATION_SECS);
                    Some(t * TRANSITION_ENERGY_SATURATION_BOOST)
                } else {
                    None
                }
            })
        } else {
            None
        };
        let frac_progress = self.fractional_progress();

        for line in start_line..=self.head_put_line {
            if line >= ctx.lines {
                break;
            }

            let is_glitched = ctx.is_glitched(line, self.bound_col);
            // Head glyph shimmer: periodically cycle the head character to create
            // subtle "churn" that makes active cells feel alive without flicker.
            // The shimmer uses a time-based offset into the char_pool, so the
            // character changes smoothly at HEAD_SHIMMER_PERIOD_SECS intervals.
            let is_head = line == self.head_put_line && is_head_bright_cached;
            let val = if is_head && self.is_head_crawling {
                let birth = self.birth_time.unwrap_or(now);
                let age = now.saturating_duration_since(birth).as_secs_f32();
                let shimmer_idx = (age / HEAD_SHIMMER_PERIOD_SECS) as u16;
                ctx.get_char(
                    line,
                    self.bound_col,
                    self.char_pool_idx.wrapping_add(shimmer_idx),
                )
            } else {
                ctx.get_char(line, self.bound_col, self.char_pool_idx)
            };

            let mut loc = CharLoc::Middle;
            // Front-layer dynamic tail: for layer 2 droplets with tail_cells > 1,
            // assign the first `tail_cells` cells of the visible trail to
            // CharLoc::TailN(i), mapping them to palette tail color stops
            // (0=darkest/furthest, up to FRONT_LAYER_MAX_TAIL_STOPS-1). This
            // restores visible multi-cell tails that were missing — previously
            // front-layer droplets showed only head+body with no tail.
            //
            // Mid/back layers (tail_cells == 1) retain the existing single-cell
            // CharLoc::Tail assignment to preserve the 3-2-2 distribution.
            let visible_start = self.tail_put_line.map_or(0, |tp| tp.saturating_add(1));
            if line < self.head_put_line && line >= visible_start {
                let dist_from_tail = line.saturating_sub(visible_start);
                if self.tail_cells > 1 && dist_from_tail < self.tail_cells as u16 {
                    loc = CharLoc::TailN {
                        seg: dist_from_tail as u8,
                        total: self.tail_cells,
                    };
                } else if self.tail_put_line.is_some() && dist_from_tail == 0 {
                    loc = CharLoc::Tail;
                }
            }
            if is_head {
                loc = CharLoc::Head;
            }

            if matches!(loc, CharLoc::Middle)
                && line < self.head_cur_line
                && !is_glitched
                && line != self.end_line
                && !ctx.shading_distance
                && !ctx.transitioning
                && !ctx.charset_transitioning()
                && !draw_everything
            {
                continue;
            }

            let (fg, bold) = ctx.get_attr(
                self.palette_slot,
                line,
                self.bound_col,
                val,
                loc,
                self.head_put_line,
                self.length,
            );

            // head_bright was hoisted out of the loop above — reuse cached value.

            // Apply visual effects to foreground color
            let edge_fade = ctx.edge_fade(line);

            let fg = fg.and_then(|c| {
                // Decode color to RGB once; chain all effects on raw tuples.
                let (mut r, mut g, mut b) = palette::decode_color(c)?;

                // F6: transition energy uses hoisted transition_wf.
                //
                // (chroma audit, A14): route the white-blend through
                // chroma::palette::blend_toward_white_rgb when the chroma
                // pipeline is active, fall back to chroma::legacy::blend_toward_white
                // otherwise. Both helpers use the same equation
                // `r + (255 - r) * wf / 256` -- bit-identical to the previous
                // inline form. The hoist now stores the raw f32 factor; the
                // `(factor * 256.0) as i32` cast happens inside the helper.
                if let Some(factor) = transition_wf {
                    let (nr, ng, nb) = if ctx.color_pipeline.is_chroma() {
                        crate::chroma_dragon_engine::palette::blend_toward_white_rgb(
                            r, g, b, factor,
                        )
                    } else {
                        crate::chroma_dragon_engine::legacy::blend_toward_white(r, g, b, factor)
                    };
                    r = nr;
                    g = ng;
                    b = nb;
                }

                // Head bloom: exponential gaussian falloff for natural glow.
                if matches!(loc, CharLoc::Middle) {
                    let dist_from_head = self.head_put_line.saturating_sub(line);
                    if dist_from_head > 0 && dist_from_head < HEAD_BLOOM_CELLS {
                        let d = dist_from_head as f32;
                        let gaussian = (-d * d / (2.0 * HEAD_BLOOM_SIGMA * HEAD_BLOOM_SIGMA)).exp();
                        let bloom = if is_new_generation {
                            HEAD_BLOOM_INTENSITY + TRANSITION_HEAD_GLOW_BOOST
                        } else {
                            HEAD_BLOOM_INTENSITY
                        };
                        // Depth-of-field: scale head bloom by layer so back-layer
                        // heads don't out-bloom front-layer bodies. Without this,
                        // a short back-layer droplet (head + 1 body cell) shows as
                        // a bright bloom spot against the dark background.
                        let layer_bloom = PARALLAX_HEAD_BLOOM_MULT[self.layer as usize];
                        let frac_bloom = 1.0 + frac_progress * FRACTIONAL_BLOOM_AMP;
                        // (chroma audit, A15): route head-bloom white-blend
                        // through chroma::palette::blend_toward_white_rgb when
                        // active, chroma::legacy::blend_toward_white otherwise.
                        // Same equation both paths: `r + (255 - r) * wf / 256`.
                        // Factor range: gaussian * bloom * frac_bloom * layer_bloom
                        // peaks at ~0.858 (front layer, transition boost) -- well
                        // within the chroma helper's [0, 1] clamp.
                        let factor = gaussian * bloom * frac_bloom * layer_bloom;
                        let (nr, ng, nb) = if ctx.color_pipeline.is_chroma() {
                            crate::chroma_dragon_engine::palette::blend_toward_white_rgb(
                                r, g, b, factor,
                            )
                        } else {
                            crate::chroma_dragon_engine::legacy::blend_toward_white(r, g, b, factor)
                        };
                        r = nr;
                        g = ng;
                        b = nb;
                    }
                }

                // Parallax layer brightness + glyph dim: combine into one multiply.
                //
                // Bug fix: the gate was `if combined_layer < 1.0` which
                // silently skipped boosts > 1.0 — front-layer brightness 1.05
                // was a complete no-op. Changed to `!= 1.0` so both dimming
                // (< 1.0) and boosting (> 1.0) apply. The integer pipeline
                // already handles > 1.0 correctly (fi > 256 scales r upward).
                let layer_brightness = PARALLAX_BRIGHTNESS_MULT[self.layer as usize];
                let glyph_dim = PARALLAX_GLYPH_DIM[self.layer as usize];
                let combined_layer = layer_brightness * glyph_dim;
                if combined_layer != 1.0 {
                    // (A16): chroma-routed scale. Unclamped helper
                    // because front layer combined_layer = 1.10 (BOOST > 1.0);
                    // clamped apply_brightness_rgb would regress fix.
                    let (nr, ng, nb) = if ctx.color_pipeline.is_chroma() {
                        crate::chroma_dragon_engine::palette::apply_brightness_rgb_unclamped(
                            r,
                            g,
                            b,
                            combined_layer,
                        )
                    } else {
                        crate::chroma_dragon_engine::legacy::scale_rgb(r, g, b, combined_layer)
                    };
                    r = nr;
                    g = ng;
                    b = nb;
                }

                // Depth-of-field saturation: blend toward luminance (gray) by
                // `1.0 - saturation_mult`. Back layers lose vividness → "haze".
                // .0 fix: gate is `!= 1.0` (not `< 1.0`) so front-layer
                // oversaturation (sat > 1.0) also applies. Luminance uses
                // Rec. 601 (0.299R + 0.587G + 0.114B) integer math.
                let saturation_mult = PARALLAX_SATURATION_MULT[self.layer as usize];
                if saturation_mult != 1.0 {
                    let lum = (r as u32 * 77 + g as u32 * 150 + b as u32 * 29 + 128) >> 8;
                    let lum = lum.min(255) as u8;
                    // (A11): chroma-routed. `1.0 - sat` can be NEGATIVE
                    // (front layer oversaturates), so chroma path uses
                    // `blend_toward_bg_rgb_unclamped` (legacy is already unclamped).
                    // Equation: out = c - (c - lum) * (1 - sat) = lerp(c, lum, 1-sat).
                    let factor = 1.0 - saturation_mult;
                    let (nr, ng, nb) = if ctx.color_pipeline.is_chroma() {
                        crate::chroma_dragon_engine::palette::blend_toward_bg_rgb_unclamped(
                            r, g, b, lum, lum, lum, factor,
                        )
                    } else {
                        crate::chroma_dragon_engine::legacy::blend_toward_rgb(
                            r, g, b, lum, lum, lum, factor,
                        )
                    };
                    r = nr;
                    g = ng;
                    b = nb;
                }

                // Depth-of-field: reduce fg-bg contrast for background layer.
                // Blends the foreground color toward black (background) by
                // PARALLAX_CONTRAST_REDUCTION[layer]. This creates a "foggy"
                // perceptual blur — the terminal equivalent of depth-of-field.
                // Only layer 0 (background) is affected; layers 1-2 stay sharp.
                //
                // (chroma audit, A12): route through chroma engine when
                // active, fall back to chroma::legacy::scale_rgb otherwise.
                // The brightness-scale equation \`((c * fi + 128) >> 8).clamp(0,255)\`
                // is bit-identical between the two paths; the difference is
                // auditability (single source of truth in chroma::palette).
                //
                // Factor safety: PARALLAX_CONTRAST_REDUCTION = [0.50, 0.18, 0.0]
                // and the block is gated on \`contrast_reduction > 0.0\`, so the
                // active layers (0, 1) always produce factor = 1.0 - cr in
                // [0.50, 0.82] -- well within the chroma helper's [0, 1] clamp.
                let contrast_reduction = PARALLAX_CONTRAST_REDUCTION[self.layer as usize];
                if contrast_reduction > 0.0 {
                    let factor = 1.0 - contrast_reduction;
                    // (Color-#5): apply_brightness_rgb_unclamped returns tuple directly,
                    // avoiding the Color wrap + decode_color round-trip. Bit-identical for
                    // factor ∈ [0,1] (call-site guard ensures this).
                    let (nr, ng, nb) = if ctx.color_pipeline.is_chroma() {
                        crate::chroma_dragon_engine::palette::apply_brightness_rgb_unclamped(
                            r, g, b, factor,
                        )
                    } else {
                        crate::chroma_dragon_engine::legacy::scale_rgb(r, g, b, factor)
                    };
                    r = nr;
                    g = ng;
                    b = nb;
                }

                // Depth fog: dim top and bottom rows.
                // v50 (alpha.2): FOG_MIN_FACTOR=1.0 disables this. Const-gate
                // mirrors the GLOW_ENABLED pattern below — LLVM folds the dead
                // branch away, but the explicit gate makes intent clear and
                // avoids the runtime branch + factor computation when disabled.
                const FOG_ENABLED: bool = FOG_MIN_FACTOR < 1.0;
                if FOG_ENABLED {
                    let fog_factor = if line < FOG_ROWS {
                        FOG_MIN_FACTOR + (1.0 - FOG_MIN_FACTOR) * (line as f32 / FOG_ROWS as f32)
                    } else {
                        let bottom_dist = ctx.lines.saturating_sub(line).saturating_sub(1);
                        if bottom_dist < FOG_ROWS {
                            FOG_MIN_FACTOR
                                + (1.0 - FOG_MIN_FACTOR) * (bottom_dist as f32 / FOG_ROWS as f32)
                        } else {
                            1.0
                        }
                    };
                    if fog_factor < 1.0 {
                        // (chroma audit, A13): route depth-fog brightness
                        // scale through chroma engine when active, fall back to
                        // chroma::legacy::scale_rgb otherwise. Same equation both
                        // paths: \`((c * fi + 128) >> 8).clamp(0, 255)\` where
                        // fi = (fog_factor * 256) as i32.
                        let (nr, ng, nb) = if ctx.color_pipeline.is_chroma() {
                            // (Color-#5): tuple-returning variant avoids Color wrap + decode_color round-trip.
                            crate::chroma_dragon_engine::palette::apply_brightness_rgb_unclamped(
                                r, g, b, fog_factor,
                            )
                        } else {
                            crate::chroma_dragon_engine::legacy::scale_rgb(r, g, b, fog_factor)
                        };
                        r = nr;
                        g = ng;
                        b = nb;
                    }
                } // end if FOG_ENABLED

                // Cursor glow: cells near mouse cursor get brighter (elliptical falloff).
                // v30 optimize: const-gate — MOUSE_GLOW_INTENSITY is 0.0 in production,
                // so LLVM folds this to dead code. `mouse_col != u16::MAX` stays as a
                // runtime guard for the day glow is re-enabled. See docs/archive/research/MOUSE_EFFECTS_AUDIT.md.
                const GLOW_ENABLED: bool = MOUSE_GLOW_INTENSITY > 0.0;
                if GLOW_ENABLED && ctx.mouse_col != u16::MAX {
                    let col_dist = if self.bound_col > ctx.mouse_col {
                        (self.bound_col - ctx.mouse_col) as f32
                    } else {
                        (ctx.mouse_col - self.bound_col) as f32
                    };
                    let line_dist = if line > ctx.mouse_line {
                        (line - ctx.mouse_line) as f32
                    } else {
                        (ctx.mouse_line - line) as f32
                    };
                    let norm_col = col_dist / MOUSE_GLOW_RADIUS_COLS;
                    let norm_line = line_dist / MOUSE_GLOW_RADIUS_LINES;
                    let dist_sq = norm_col * norm_col + norm_line * norm_line;
                    if dist_sq < 1.0 {
                        let glow = (1.0 - dist_sq) * MOUSE_GLOW_INTENSITY;
                        // (A17): chroma-routed white-blend (dead code in prod).
                        let (nr, ng, nb) = if ctx.color_pipeline.is_chroma() {
                            crate::chroma_dragon_engine::palette::blend_toward_white_rgb(
                                r, g, b, glow,
                            )
                        } else {
                            crate::chroma_dragon_engine::legacy::blend_toward_white(r, g, b, glow)
                        };
                        r = nr;
                        g = ng;
                        b = nb;
                    }
                }

                // Click flash: expanding glow wave from click point (v17 mastery).
                // v30 fix: iterate ALL active flash waves (was single slot).
                //
                // Each wave is a dual-ring water-drop ripple. A primary bright
                // ring expands outward at MOUSE_FLASH_SPEED cells/s, followed
                // by a secondary dimmer ring at half speed — creating a layered
                // "stone in water" cinematic ripple that propagates to the
                // screen edge.
                //
                // The fade uses a quadratic curve (fade^1.5) for natural energy
                // dissipation — the wave starts strong and decays gradually like
                // a real water ripple, not a linear cutoff.
                //
                // With the v30 bounded pool, multiple clicks within 1.8s each
                // emit their own independent wave. Their per-cell factor
                // contributions sum additively (capped at 1.0 by the clamp on
                // each channel below). Visual result: overlapping rings blend
                // into a richer interference pattern instead of one cancelling
                // the other.
                for w in ctx.flash_waves {
                    let col_dist = if self.bound_col > w.col {
                        (self.bound_col - w.col) as f32
                    } else {
                        (w.col - self.bound_col) as f32
                    };
                    let line_dist = if line > w.line {
                        (line - w.line) as f32
                    } else {
                        (w.line - line) as f32
                    };
                    // v30 optimize (docs/archive/research/MOUSE_EFFECTS_AUDIT.md Quick Win #3):
                    // squared-distance early-out before sqrt. Cells outside
                    // the wave's bounding circle skip the sqrt + ring math
                    // entirely. Skips ~75% of sqrts for typical wave coverage.
                    let dist_sq = col_dist * col_dist + line_dist * line_dist;
                    if dist_sq > w.max_reach_sq {
                        continue;
                    }
                    let euclidean = dist_sq.sqrt();
                    // v30 optimize (Quick Win #2): fade, primary_radius,
                    // secondary_radius are precomputed in FlashWaveCtx.
                    let fade = w.fade;

                    // Primary ring: fast, bright, full intensity.
                    let primary_dist = (euclidean - w.primary_radius).abs();
                    let mut factor = 0.0;
                    if primary_dist < MOUSE_FLASH_RING_WIDTH {
                        // Sharp leading edge, soft trailing tail (squared falloff).
                        let t = 1.0 - primary_dist / MOUSE_FLASH_RING_WIDTH;
                        let t_smooth = t * t;
                        factor = t_smooth * MOUSE_FLASH_INTENSITY * fade;
                    }

                    // Secondary ring: slower, dimmer, layered echo.
                    let secondary_dist = (euclidean - w.secondary_radius).abs();
                    if secondary_dist < MOUSE_FLASH_RING_WIDTH {
                        let t = 1.0 - secondary_dist / MOUSE_FLASH_RING_WIDTH;
                        let t_smooth = t * t;
                        factor +=
                            t_smooth * MOUSE_FLASH_INTENSITY * MOUSE_FLASH_SECONDARY_FRAC * fade;
                    }

                    if factor > 0.0 {
                        // v50 chromatic shockwave + audit C-1: blend toward
                        // the active palette's HEAD color (precomputed in
                        // FlashWaveCtx.head_rgb, once per wave in rain.rs).
                        let (hr, hg, hb) = w.head_rgb;
                        let (nr, ng, nb) = if ctx.color_pipeline.is_chroma() {
                            crate::chroma_dragon_engine::palette::blend_toward_bg_rgb(
                                r, g, b, hr, hg, hb, factor,
                            )
                        } else {
                            crate::chroma_dragon_engine::legacy::blend_toward_rgb(
                                r, g, b, hr, hg, hb, factor,
                            )
                        };
                        r = nr;
                        g = ng;
                        b = nb;
                    }
                }

                // Head brightness modulation
                // (chroma audit, A3): route through chroma engine
                // when active, fall back to chroma::legacy::scale_rgb
                // otherwise. Both paths use the same `((c*fi+128)>>8)`
                // equation -- the difference is auditability.
                if matches!(loc, CharLoc::Head) && head_bright < 1.0 {
                    let factor = 0.7 + 0.3 * head_bright;
                    // (Color-#5): apply_brightness_rgb_unclamped returns tuple directly.
                    let (nr, ng, nb) = if ctx.color_pipeline.is_chroma() {
                        crate::chroma_dragon_engine::palette::apply_brightness_rgb_unclamped(
                            r, g, b, factor,
                        )
                    } else {
                        crate::chroma_dragon_engine::legacy::scale_rgb(r, g, b, factor)
                    };
                    r = nr;
                    g = ng;
                    b = nb;
                }

                // Head self-bloom: per-layer scaled head color boost.
                // Cinematic head pop — head is OBVIOUSLY brighter than body.
                // Was 12% (subtle), raised to ~23% for film-quality head glow.
                //
                // Cinematic final polish: scale HEAD_BOOST by per-layer multiplier
                // so back-layer heads don't get re-brightened after dimming.
                // Without this, the layer brightness dimming was undone by the
                // boost, popping the head back up — visible as a "white dot".
                //
                // Bug fix: the original code used `as i32` on the
                // f32 multiplier, which truncated 0.30→0, 0.65→0, 0.78→0,
                // 1.0→1. Combined with integer division `(60 * 0_or_1) / 256`,
                // the result was `wf = 0` for ALL layers — the self-bloom was
                // a complete no-op since the constant was introduced. Switched
                // to f32 math so fractional multipliers actually apply. With
                // PARALLAX_HEAD_SELFBLOOM_MULT[0] = 0.38, back-layer heads now
                // get a real ~9% boost; front at 1.15 gets ~27%.
                if matches!(loc, CharLoc::Head) {
                    // v25 "glow with color" calibration: instead of blending
                    // toward pure white (255,255,255), boost the head's own
                    // color channels. This makes the head glow brighter in its
                    // theme hue (green → brighter green, not white) at high
                    // speed. The boost factor is scaled by layer via
                    // PARALLAX_HEAD_SELFBLOOM_MULT.
                    //
                    // (chroma audit, A4): boost routes through chroma
                    // engine when active, legacy boost_rgb otherwise. Both
                    // paths use the same `(c as f32 * (1.0 + factor)).round()
                    // .clamp(0,255)` equation -- bit-identical output. The
                    // audit proposed a future perceptual OKLab L lift variant
                    // for the chroma path, but that is a separate behavior
                    // change requiring owner approval.
                    const HEAD_BOOST: f32 = HEAD_SELFBLOOM_BASE; // ~0.234 — centralized
                    let layer_selfbloom = PARALLAX_HEAD_SELFBLOOM_MULT[self.layer as usize];
                    let wf = HEAD_BOOST * layer_selfbloom;
                    let (nr, ng, nb) = if ctx.color_pipeline.is_chroma() {
                        crate::chroma_dragon_engine::palette::boost_rgb(r, g, b, wf)
                    } else {
                        crate::chroma_dragon_engine::legacy::boost_rgb(r, g, b, wf)
                    };
                    r = nr;
                    g = ng;
                    b = nb;
                }

                // Rain shadow: quadratic fade-out across bottom 15% of screen.
                // Applied BEFORE the edge_fade (which is a sharper lip) and
                // BEFORE the vignette (which is a radial effect). The shadow
                // is the broadest, softest bottom dim — gives the frame
                // perceptual depth ("rain dissipating into shadow at ground")
                // rather than "rain hitting a wall". Applied here in the
                // droplet color pipeline so phosphor captures the already-
                // dimmed color (afterglow fades in sync with shadow).
                //
                // Front-layer exclusion: RAIN_SHADOW_LAYER_MULT[2] = 0.0 means
                // front-layer neon is NOT dimmed by the shadow — it stays at
                // full fidelity across the entire screen height. Mid/back
                // layers (mult=1.0) get the full shadow for depth.
                let shadow_raw = crate::brightness_factors::rain_shadow_factor(line, ctx.lines);
                let shadow = 1.0 - (1.0 - shadow_raw) * RAIN_SHADOW_LAYER_MULT[self.layer as usize];
                // (chroma audit, A5): rain shadow brightness scale
                // routes through chroma engine when active, legacy
                // scale_rgb otherwise. Same equation both paths.
                if shadow < 1.0 {
                    // (Color-#5): apply_brightness_rgb_unclamped returns tuple directly.
                    let (nr, ng, nb) = if ctx.color_pipeline.is_chroma() {
                        crate::chroma_dragon_engine::palette::apply_brightness_rgb_unclamped(
                            r, g, b, shadow,
                        )
                    } else {
                        crate::chroma_dragon_engine::legacy::scale_rgb(r, g, b, shadow)
                    };
                    r = nr;
                    g = ng;
                    b = nb;
                }

                // (chroma audit, A6): edge fade brightness scale
                // routes through chroma engine when active, legacy
                // scale_rgb otherwise. The original PERF(v10) note about
                // avoiding decode_color + apply_brightness_rgb still
                // holds -- we keep (r,g,b) tuple form, only branching on
                // pipeline to choose the helper. The chroma path's
                // apply_brightness_rgb is #[inline] and compiles to the
                // same `((c*fi+128)>>8).clamp(0,255)` machine code, so the
                // migration is a pure auditability refactor with zero
                // hot-path cost.
                if edge_fade < 1.0 {
                    let (nr, ng, nb) = if ctx.color_pipeline.is_chroma() {
                        // (Color-#5): tuple-returning variant avoids Color wrap + decode_color round-trip.
                        crate::chroma_dragon_engine::palette::apply_brightness_rgb_unclamped(
                            r, g, b, edge_fade,
                        )
                    } else {
                        crate::chroma_dragon_engine::legacy::scale_rgb(r, g, b, edge_fade)
                    };
                    r = nr;
                    g = ng;
                    b = nb;
                }

                // Cinematic radial vignette — applied LAST, AFTER all other
                // effects (including edge_fade). This is the photographic
                // "lens darkening" that frames the image: corners dimmed
                // smoothly toward 70% of their post-effects brightness,
                // drawing the eye to the focused center.
                //
                // Pre-baked LUT lookup (perf): the vignette factor for every
                // (col, line) pair is computed once on terminal resize and stored
                // in a flat 2D array. The per-cell cost is now a single indexed
                // f32 read instead of 2 subtractions + 2 multiplications + 1 sqrt
                // + 1 smoothstep. Saves ~5µs/frame at 500 visible cells.
                //
                // Front-layer exclusion: VIGNETTE_LAYER_MULT[2] = 0.0 means
                // front-layer neon is NOT dimmed by the vignette — it stays at
                // full fidelity even at screen corners. Mid/back layers
                // (mult=1.0) get the full vignette for depth.
                let vignette_raw = if ctx.vignette_lut.is_empty() {
                    // LUT not sized for this viewport (tests, or pre-resize
                    // frame). Fall back to the live computation.
                    crate::brightness_factors::vignette_factor(
                        self.bound_col,
                        line,
                        ctx.cols,
                        ctx.lines,
                    )
                } else {
                    // Direct index — bounds are guaranteed by construction:
                    // line < ctx.lines (loop invariant) and bound_col < cols
                    // (validated in Cloud::spawn). The LUT is sized to
                    // lines*vignette_lut_cols at resize time. Debug_assert
                    // keeps the safety contract visible without paying the
                    // Option unwrap cost in release (Cosmic Dragon egg pattern).
                    let idx = (line as usize) * (ctx.vignette_lut_cols as usize)
                        + (self.bound_col as usize);
                    debug_assert!(
                        idx < ctx.vignette_lut.len(),
                        "vignette LUT index {idx} out of bounds (line={line}, col={}, lut_cols={}, lut_len={})",
                        self.bound_col,
                        ctx.vignette_lut_cols,
                        ctx.vignette_lut.len()
                    );
                    ctx.vignette_lut[idx]
                };
                let vignette =
                    1.0 - (1.0 - vignette_raw) * VIGNETTE_LAYER_MULT[self.layer as usize];
                // (chroma audit, A7): radial vignette brightness
                // scale routes through chroma engine when active, legacy
                // scale_rgb otherwise. Same equation both paths.
                if vignette < 1.0 {
                    let (nr, ng, nb) = if ctx.color_pipeline.is_chroma() {
                        // (Color-#5): tuple-returning variant avoids Color wrap + decode_color round-trip.
                        crate::chroma_dragon_engine::palette::apply_brightness_rgb_unclamped(
                            r, g, b, vignette,
                        )
                    } else {
                        crate::chroma_dragon_engine::legacy::scale_rgb(r, g, b, vignette)
                    };
                    r = nr;
                    g = ng;
                    b = nb;
                }
                Some(Color::Rgb { r, g, b })
            });
            // Suppress bold at viewport edges to prevent harsh bright spots
            // right at the border where the fade should create smooth dimming.
            let bold = bold && edge_fade >= EDGE_FADE_BOLD_THRESHOLD;

            frame.set_force(
                self.bound_col,
                line,
                crate::cell::Cell {
                    ch: val,
                    fg,
                    bg,
                    bold,
                },
            );
        }

        self.head_cur_line = self.head_put_line;
    }
}
