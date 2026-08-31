// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Message overlay drawing — extracted from `cloud/mod.rs` to keep
//! that file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns `Cloud::draw_message()` — the cinematic message overlay
//! renderer with progressive text reveal, clockwise border gradient
//! (BC-01..05 chroma dragon), F2 splash crown sparks, and Z-5 zero-alloc
//! scratch buffers.
//!
//! v51 msg-fill-style: the text reveal is style-driven (see
//! `msg_fill_style/` — one file per style, dispatch in
//! `msg_fill_style/mod.rs`). Seven styles are selectable via
//! `-mfs`/`--msg-fill-style` or the `msg-fill-style` config key:
//! typewriter (default, bit-identical to pre-v51), fade, words, slide,
//! pulse, instant, engrave. All timing constants, per-cell reveal
//! math, and the engrave spark sidecar live in the style files; this
//! renderer only consumes the dispatch API. The engrave style's
//! spark pass is implemented in `msg_fill_style/engrave.rs` (the
//! only stateful member of the family) and is invoked at the end of
//! this method.
//!
//! Implemented as a separate `impl Cloud` block (Rust allows multiple
//! impl blocks across files for the same type). The method stays
//! private (`fn`, not `pub`) — called only from `rain.rs::rain_at()`
//! via `self.draw_message(frame, now)`.

use std::time::Instant;

use crossterm::style::Color;

use crate::cell::Cell;
use crate::cloud::border::is_border_char;
use crate::frame::Frame;
use crate::msg_fill_style::{self as mfs, MsgFillStyle};
use crate::runtime::{BoldMode, ColorMode};

use super::palette_blend::interpolate_palette_color;
use super::BorderPulse;

impl super::Cloud {
    pub(crate) fn draw_message(&mut self, frame: &mut Frame, now: Instant) {
        let bg = self.palette.bg;
        // BC-01..05 (border chroma dragon): per-cell gradient sweeping the
        // active palette's chroma colors clockwise around the message box.
        // `palette.colors` IS the chroma gradient output (OKLab polar
        // interpolation applied at build time). On 'c'/'C' the gradient pops
        // to the new palette instantly (UI overlay semantics, no wave).
        let palette_colors = &self.palette.colors;
        let palette_n = palette_colors.len();
        let content_fg = if self.color_mode == ColorMode::Mono {
            None
        } else {
            palette_colors.last().copied()
        };

        // Count total text (content) chars and border chars.
        let total_text: usize = self
            .message
            .iter()
            .filter(|mc| !is_border_char(mc.val))
            .count();
        let total_border: usize = self
            .message
            .iter()
            .filter(|mc| is_border_char(mc.val) && mc.val != ' ')
            .count();

        // v30 Hinnant: hoist start.elapsed() above the per-cell loop below
        // (was 1 syscall per revealed content cell, 50-200×/frame).
        let message_elapsed_ms: Option<usize> = self
            .message_start_time
            .map(|start| start.elapsed().as_millis() as usize);

        // v51 msg-fill-style: per-style reveal plan. The six stateless
        // styles derive everything from elapsed time; engrave keeps the
        // same stateless reveal math but adds a bounded spark sidecar
        // (msg_fill_style/engrave.rs). Typewriter keeps the exact pre-v51
        // semantics:
        //   reveal_count = (elapsed / 80).max(1).min(total_text)
        //   per-char 100 ms fade-in from 30% brightness.
        let style = self.msg_fill_style;
        let block_alpha = mfs::fade_block_alpha(message_elapsed_ms);
        // Index-paced styles (typewriter/pulse/slide/engrave) budget
        // cells by their per-char constant; word/block styles
        // (words/fade/instant) report everything revealed — their
        // reveal math decides per-cell and never reads the budget.
        // The dispatch lives in msg_fill_style/mod.rs (one arm per
        // style file), so the renderer carries no style match here.
        let reveal_count = mfs::index_reveal_count(style, message_elapsed_ms, total_text);
        // Total word count from the hoisted ordinals (words style only;
        // 0 when no message cells exist).
        let total_words = self
            .message_word_ordinals
            .iter()
            .copied()
            .max()
            .unwrap_or(0) as usize;

        // v25 progressive border: border cells revealed clockwise,
        // lagging behind text reveal (cinematic effect).
        // v51: per-style — typewriter-paced styles keep the t^1.5 lag,
        // fade ramps the border together with the block alpha, instant
        // draws the border on an independent 1 s timeline.
        let text_progress = mfs::text_progress(
            style,
            reveal_count,
            total_text,
            total_words,
            message_elapsed_ms,
        );
        // Border progress = text_progress ^ 1.5 (ease-out) for paced
        // styles; see msg_fill_style::border_progress for the rest.
        let border_progress = mfs::border_progress(style, text_progress, message_elapsed_ms);
        let border_show = (border_progress * total_border as f32).floor() as usize;

        // BN-01/02 (Dragon Hunt v3): use hoisted `border_order` (rebuilt in
        // reset_message) instead of recomputing per frame. Replaces the
        // O((W+H)×N) `build_border_order` call + the per-frame HashSet
        // allocation with a Vec<bool> bit-set lookup.
        let border_order = &self.border_order;
        let mut visible_border: Vec<bool> = vec![false; self.message.len()];
        for &idx in border_order.iter().take(border_show) {
            if idx < visible_border.len() {
                visible_border[idx] = true;
            }
        }

        // BC-02 (border chroma gradient): precompute per-cell gradient color
        // for visible border cells. Maps clockwise border position i to
        // gradient t = i/total_border, then interpolates between adjacent
        // palette stops (instead of picking the discrete rounded index).
        //
        // v50 (2026-08-17) smooth gradient fix: the previous implementation
        // rounded `t * palette_last` to the nearest integer palette index,
        // which produced discrete palette-stop blocks with visible "gaps"
        // between transitions (e.g. white→red jumped from a white block to
        // a red block with no in-between). The owner explicitly flagged:
        // "border message color was already using chroma dragon but owner
        // still sees gap color between transition like white+red should be
        // smooth gradient like rain color using chroma dragon white+semi
        // red+red". The new implementation linearly interpolates between
        // palette[pos] and palette[pos+1] using the fractional remainder
        // `frac` (linear sRGB blend via `chroma::legacy::blend_toward_rgb`),
        // so adjacent border cells get smoothly-varying interpolated colors
        // — matching the rain color's per-cell chroma dragon sweep.
        //
        // On 'c'/'C' keypress the gradient pops to the new palette
        // instantly (UI overlay semantics, no wave); the interpolation is
        // recomputed every frame from the current palette so palette
        // changes reflect on the very next draw.
        // Z-5: use hoisted `border_gradient_scratch` buffer (Cloud field)
        // instead of allocating a new Vec every frame. Pattern matches
        // crt_vignette_candidates (T1.1-real) + border_cross_candidates (B-1).
        // clear() preserves the allocation, so after the first frame
        // this is zero-alloc. Resize to message.len() (no-op if same size).
        self.border_gradient_scratch.clear();
        self.border_gradient_scratch
            .resize(self.message.len(), None);
        let border_gradient = &mut self.border_gradient_scratch;
        if palette_n > 0 && self.color_mode != ColorMode::Mono {
            // BD-02 (Border Dragon) - LTS Stable: corner-aware gradient system.
            //
            // ## Design Invariants (LTS guarantees)
            // 1. Bottom corners (╰╯) ALWAYS use bright anchor → visual anchoring
            // 2. Top corners (╭╮) follow natural gradient → chroma dragon flow
            // 3. Triangle wave ensures no sharp color gaps on left/right borders
            // 4. All t-values clamped to [0.0, 1.0] → safe interpolation
            //
            // ## Owner Requirements (preserved across versions)
            // - "Bottom corners white head must perfectly enter round corners"
            // - "Top-left should be dark per chroma dragon gradient"
            // - No lone bright heads at top corners
            //
            // ## Performance Notes
            // - HashSet pre-allocated for exactly 2 bottom corners (no rehash)
            // - Single pass over border_order for corner detection
            // - O(border_count) time, O(2) space for corner set

            const BOTTOM_CORNER_BRIGHTNESS: f32 = 0.8; // LTS: named constant
            const EXPECTED_BOTTOM_CORNERS: usize = 2; // ╰ + ╯

            let total_border_f = total_border.max(1) as f32;

            // Pre-allocate HashSet with known capacity (LTS optimization)
            // Z-5: use hoisted `bottom_corner_scratch` buffer (Cloud field)
            // instead of allocating a new HashSet every frame.
            self.bottom_corner_scratch.clear();
            let bottom_corner_indices = &mut self.bottom_corner_scratch;

            // Detect bottom corners from border order (defensive iteration)
            if !border_order.is_empty() {
                for &idx in border_order.iter() {
                    // Bounds check: guard against stale indices
                    if idx < self.message.len() {
                        let mc = &self.message[idx];
                        // Only bottom corners get special treatment
                        if matches!(mc.val, '╰' | '╯') {
                            bottom_corner_indices.insert(idx);
                        }
                    }
                    // Early exit: found both bottom corners
                    if bottom_corner_indices.len() >= EXPECTED_BOTTOM_CORNERS {
                        break;
                    }
                }
            }

            // Apply gradient with corner-aware brightness (LTS stable)
            for (i, &idx) in border_order.iter().take(border_show).enumerate() {
                // Defensive bounds check (LTS requirement)
                if idx >= border_gradient.len() {
                    continue;
                }

                // Compute parametric position t ∈ [0.0, 1.0]
                // BD-02 rule: bottom corners override to bright anchor
                let use_t = if bottom_corner_indices.contains(&idx) {
                    BOTTOM_CORNER_BRIGHTNESS // Bright anchor for bottom corners
                } else {
                    // Triangle wave: dark→bright→dark around perimeter
                    // Prevents left/right border color dominance (v50.0.0-alpha.7 fix)
                    let t_raw = i as f32 / total_border_f;
                    // Clamp to [0.0, 1.0] for numerical safety (LTS defensive)
                    let t_clamped = t_raw.clamp(0.0, 1.0);
                    if t_clamped <= 0.5 {
                        t_clamped * 2.0 // Rising edge: dark → bright
                    } else {
                        2.0 - t_clamped * 2.0 // Falling edge: bright → dark
                    }
                };

                // Safe interpolation (returns None only on empty palette, guarded above)
                border_gradient[idx] = interpolate_palette_color(palette_colors, use_t);
            }
        }

        // RAIN_BORDER_TOUCH_GLOW (Option C+D): compute per-cell pulse
        // factors (for border-cell blending) and per-column halo factors
        // (for the halo row above the top border). Both decay via a
        // smoothstep envelope peaking at touch and decaying to 0 over
        // their respective lifetimes.
        //
        // For per-cell: take the max envelope across all active pulses
        // pointing at that msg_idx (multiple droplets can touch the same
        // cell in the same column within the lifetime — strongest wins).
        // For per-column: same logic across all active halo entries
        // pointing at that col.
        //
        // The newest palette's head_rgb (cached in BorderPulse at touch
        // time) is the source color, so the glow dynamically follows the
        // palette at the moment of touch. The `now` parameter is threaded
        // from `rain_at` so tests can advance time and verify decay.
        let pulse_lifetime_ms = crate::chroma_dragon_engine::tuning::BORDER_TOUCH_PULSE_LIFETIME_MS;
        let pulse_max = crate::chroma_dragon_engine::tuning::BORDER_TOUCH_PULSE_MAX;
        let halo_lifetime_ms = crate::chroma_dragon_engine::tuning::BORDER_TOUCH_HALO_LIFETIME_MS;
        let halo_max = crate::chroma_dragon_engine::tuning::BORDER_TOUCH_HALO_MAX;

        let mut pulse_factor: Vec<f32> = vec![0.0; self.message.len()];
        let mut pulse_color: Vec<(u8, u8, u8)> = vec![(0, 0, 0); self.message.len()];
        let mut halo_factor: Vec<f32> = vec![0.0; self.cols as usize];
        let mut halo_color: Vec<(u8, u8, u8)> = vec![(0, 0, 0); self.cols as usize];

        // Drain-and-rebuild: keep only pulses with at least one active
        // envelope (pulse OR halo). The kept entries go back into
        // self.border_pulses for the next frame's decay continuation.
        let mut alive_pulses: Vec<BorderPulse> = Vec::with_capacity(self.border_pulses.len());
        for p in self.border_pulses.drain(..) {
            let elapsed_ms = now.saturating_duration_since(p.birth).as_millis() as u32;

            // Pulse envelope (Option C): smoothstep decay from peak.
            let pf = if elapsed_ms < pulse_lifetime_ms {
                let t = elapsed_ms as f32 / pulse_lifetime_ms as f32;
                let u = 1.0 - t; // u=1 at touch, u=0 at end
                let envelope = u * u * (3.0 - 2.0 * u);
                envelope * pulse_max
            } else {
                0.0
            };
            if pf > 0.0 && pf > pulse_factor[p.msg_idx] {
                pulse_factor[p.msg_idx] = pf;
                pulse_color[p.msg_idx] = p.head_rgb;
            }

            // Halo envelope (Option D): shorter lifetime, lower max.
            let hf = if elapsed_ms < halo_lifetime_ms {
                let t = elapsed_ms as f32 / halo_lifetime_ms as f32;
                let u = 1.0 - t;
                let envelope = u * u * (3.0 - 2.0 * u);
                envelope * halo_max
            } else {
                0.0
            };
            let col_idx = p.col as usize;
            if hf > 0.0 && col_idx < halo_factor.len() && hf > halo_factor[col_idx] {
                halo_factor[col_idx] = hf;
                halo_color[col_idx] = p.head_rgb;
            }

            // Keep the pulse alive if either envelope is still active.
            if pf > 0.0 || hf > 0.0 {
                alive_pulses.push(p);
            }
        }
        self.border_pulses = alive_pulses;

        let mut content_idx = 0usize;
        // v51 msg-fill-style: track the most recently revealed content
        // cell ("the head") for the stateful sidecars (engrave spark
        // pass, scorch smoke pass). Both styles share the same 80 ms/char
        // pacing, so the head index is style-independently
        // `reveal_count - 1`. The position is captured during the main
        // loop below and handed to the pass afterwards. `None` when the
        // style has no sidecar, the head is off-screen, or there is no
        // content at all (border-only overlay).
        let head_idx = if style == MsgFillStyle::Engrave
            || style == MsgFillStyle::Scorch
            || style == MsgFillStyle::Pulse
        {
            reveal_count.saturating_sub(1)
        } else {
            usize::MAX
        };
        let mut head_pos: Option<(u16, u16)> = None;
        // v51 msg-fill-style (slide): phase-1 cells are drawn one row
        // below their final position, AFTER the main loop — the row below
        // is itself a message cell (padding / border / next content line)
        // that would otherwise overwrite the sliding glyph in the same
        // frame. SmallVec-free: expected size is bounded by the number of
        // concurrently sliding chars (SLIDE_TRAVEL_MS / SLIDE_CHAR_MS
        // stagger window), typically < 5. The tuple carries (col, line,
        // glyph, factor, tint) — the tint is the scorch extension so a
        // future slide + scorch combo would tint the mid-slide glyph.
        #[allow(clippy::type_complexity)]
        let mut slide_cells: Vec<(u16, u16, char, f32, Option<(u8, u8, u8, f32)>)> = Vec::new();
        for (idx, mc) in self.message.iter().enumerate() {
            let is_content = !is_border_char(mc.val);
            let is_visible_border = mc.val != ' ' && visible_border[idx];

            let (ch, cell_fg) = if is_content {
                // v51: every content cell advances the reading-order
                // index (was: only when revealed). Per-style visibility
                // and brightness come from the stateless reveal solver.
                let idx0 = content_idx;
                if idx0 == head_idx {
                    head_pos = Some((mc.col, mc.line));
                }
                content_idx += 1;
                let word_ord = self.message_word_ordinals.get(idx).copied().unwrap_or(0);
                let reveal = mfs::content_reveal(
                    style,
                    idx0,
                    word_ord,
                    message_elapsed_ms,
                    reveal_count,
                    block_alpha,
                );
                // v51 msg-fill-style (glitch): the per-cell reveal may
                // carry a substitute glyph (wrong-glyph during the
                // settle window). Every stateless style leaves this
                // `None`, so they remain bit-identical to the pre-glitch
                // renderer. Slide also passes the glyph through the
                // deferred second pass so a future slide + glyph-
                // override combo would Just Work.
                let glyph = reveal.glyph_override.unwrap_or(mc.val);
                // v51 msg-fill-style (scorch): the per-cell reveal may
                // carry a tint (ember blend during the cool window).
                // Every stateless style leaves this `None`, so they
                // remain bit-identical to the pre-scorch renderer.
                // The tint is applied AFTER the brightness factor: the
                // scaled palette color is linearly blended toward the
                // tint RGB by the blend factor (via
                // `chroma_dragon_engine::palette::blend_toward_bg_rgb`).
                let tint = reveal.tint;
                let cell_fg_tinted = |fg: Option<Color>| -> Option<Color> {
                    if let Some((tr, tg, tb, blend)) = tint {
                        if let Some(base) = fg {
                            if let Some((br, bgc, bb)) = crate::palette::decode_color(base) {
                                let (nr, ng, nb) =
                                    crate::chroma_dragon_engine::palette::blend_toward_bg_rgb(
                                        br, bgc, bb, tr, tg, tb, blend,
                                    );
                                return Some(Color::Rgb {
                                    r: nr,
                                    g: ng,
                                    b: nb,
                                });
                            }
                        }
                    }
                    fg
                };
                if reveal.visible && reveal.slide_rows != 0 {
                    // Slide phase 1: glyph is still offset from the final
                    // position (positive = N rows below, slide style rises
                    // from below; negative = N rows above, cascade style
                    // drops from above) — blank the final cell now, defer
                    // the moving glyph. `saturating_add_signed` handles
                    // both directions and clamps to [0, lines-1].
                    slide_cells.push((
                        mc.col,
                        mc.line.saturating_add_signed(reveal.slide_rows),
                        glyph,
                        reveal.factor,
                        tint,
                    ));
                    (' ', None)
                } else if reveal.visible {
                    (
                        glyph,
                        cell_fg_tinted(scale_msg_content_fg(
                            &self.color_pipeline,
                            content_fg,
                            reveal.factor,
                        )),
                    )
                } else {
                    (' ', None)
                }
            } else if is_visible_border {
                // BC-02: border cell uses the per-cell gradient color
                // (chroma dragon gradient sweeping clockwise around the box).
                // Falls back to content_fg (head color) if palette has no
                // colors (Mono mode) or the gradient wasn't populated.
                //
                // RAIN_BORDER_TOUCH_GLOW (Option C): if this border cell has
                // an active touch pulse (a rain droplet's head crossed this
                // exact cell within the last BORDER_TOUCH_PULSE_LIFETIME_MS),
                // blend the gradient color toward the touching droplet's
                // head_rgb by the pulse envelope. Owner insight: dynamic
                // color from the droplet, not static white. LTS invariant
                // for top corners is RELAXED for transient touch events.
                //
                // v51 msg-fill-style (fade): border brightness follows the
                // block alpha so the border fades in together with the
                // text instead of popping in at full color.
                let base = if style == MsgFillStyle::Fade && block_alpha < 1.0 {
                    scale_msg_content_fg(
                        &self.color_pipeline,
                        border_gradient[idx].or(content_fg),
                        block_alpha,
                    )
                } else {
                    border_gradient[idx].or(content_fg)
                };
                let pf = pulse_factor[idx];
                let fg = if pf > 0.0 {
                    if let Some(base_color) = base {
                        let (br, bgc, bb) =
                            crate::palette::decode_color(base_color).unwrap_or((0, 0, 0));
                        let (hr, hg, hb) = pulse_color[idx];
                        let (nr, ng, nb) =
                            crate::chroma_dragon_engine::palette::blend_toward_bg_rgb(
                                br, bgc, bb, hr, hg, hb, pf,
                            );
                        Some(Color::Rgb {
                            r: nr,
                            g: ng,
                            b: nb,
                        })
                    } else {
                        base
                    }
                } else {
                    base
                };
                (mc.val, fg)
            } else {
                (' ', None)
            };

            frame.set_force(
                mc.col,
                mc.line,
                Cell {
                    ch,
                    fg: cell_fg,
                    bg,
                    bold: ch != ' ' && self.bold_mode != BoldMode::Off,
                },
            );
        }

        // v51 msg-fill-style (slide): deferred second pass — draw the
        // mid-slide glyphs one row below their final position. Bounds
        // guard: the row below must exist inside the terminal (the
        // message box has pad_y=1 (+1 border row), so this only fails
        // on degenerate 1-row terminals). The slide_cells tuple now
        // carries the per-cell tint (scorch extension) so a future
        // slide + scorch combo would tint the mid-slide glyph too.
        for (col, line, ch, factor, tint) in slide_cells {
            if line >= self.lines {
                continue;
            }
            let mut fg = scale_msg_content_fg(&self.color_pipeline, content_fg, factor);
            if let Some((tr, tg, tb, blend)) = tint {
                if let Some(base) = fg {
                    if let Some((br, bgc, bb)) = crate::palette::decode_color(base) {
                        let (nr, ng, nb) =
                            crate::chroma_dragon_engine::palette::blend_toward_bg_rgb(
                                br, bgc, bb, tr, tg, tb, blend,
                            );
                        fg = Some(Color::Rgb {
                            r: nr,
                            g: ng,
                            b: nb,
                        });
                    }
                }
            }
            frame.set_force(
                col,
                line,
                Cell {
                    ch,
                    fg,
                    bg,
                    bold: self.bold_mode != BoldMode::Off,
                },
            );
        }

        // RAIN_BORDER_TOUCH_GLOW (Option D, halo above border): render a
        // single-row halo above the top border, modulated per-column by the
        // active touch pulses. The halo uses the same head_rgb as the
        // touching droplet, blended at a lower max factor (0.3) so it does
        // not compete with the message text. The glyph is '·' (middle dot,
        // U+00B7) — subtle but visible on a dark background.
        //
        // Skipped when message_top_line == 0 (overlay at the very top of
        // the terminal — no row above the border to draw on).
        if self.message_top_line != u16::MAX && self.message_top_line > 0 {
            let halo_line = self.message_top_line - 1;
            // Decode bg once for all halo cells (Option<Color> → Option<(u8,u8,u8)>).
            let bg_rgb = bg.and_then(crate::palette::decode_color);
            for col in 0..self.cols as usize {
                let hf = halo_factor[col];
                if hf <= 0.0 {
                    continue;
                }
                // Blend bg toward head_rgb by hf — at hf=0, halo is bg
                // (invisible against the background); at hf=HALO_MAX, halo
                // is 30% toward the head color (subtle splash).
                let (hr, hg, hb) = halo_color[col];
                let halo_rgb = if let Some((br, bgc, bb)) = bg_rgb {
                    crate::chroma_dragon_engine::palette::blend_toward_bg_rgb(
                        br, bgc, bb, hr, hg, hb, hf,
                    )
                } else {
                    // No bg (Color::Reset): use the head color directly,
                    // scaled by the halo factor for a dim glow.
                    crate::chroma_dragon_engine::legacy::scale_rgb(hr, hg, hb, hf)
                };
                frame.set_force(
                    col as u16,
                    halo_line,
                    Cell {
                        ch: '·',
                        fg: Some(Color::Rgb {
                            r: halo_rgb.0,
                            g: halo_rgb.1,
                            b: halo_rgb.2,
                        }),
                        bg,
                        bold: false,
                    },
                );
            }
        }

        // v51 msg-fill-style (engrave): spark pass, LAST so sparks render
        // on top of the overlay text (the engraving head throws debris
        // across the freshly burned-in chars). Runs only for engrave;
        // the pass itself early-outs in O(1) when the pool is idle.
        if style == MsgFillStyle::Engrave {
            self.engrave_spark_pass(frame, now, head_pos, head_idx, message_elapsed_ms);
        }

        // v51 msg-fill-style (scorch): smoke pass, LAST so smoke
        // renders on top of the overlay text (the scorching head
        // throws slow upward gray puffs above the freshly burned-in
        // chars). Runs only for scorch; the pass itself early-outs
        // in O(1) when the pool is idle. Same dedicated-pool pattern
        // as engrave (see `msg_fill_style/scorch.rs` for why the
        // shared quantum pool cannot be reused).
        if style == MsgFillStyle::Scorch {
            self.scorch_smoke_pass(frame, now, head_pos, head_idx, message_elapsed_ms);
        }

        // v51 msg-fill-style (hologram): scanline pass, LAST so the
        // scanline renders on top of the overlay text — a single
        // horizontal CRT-style sweep down the box over 600 ms, then
        // gone. Stateless: pure function of elapsed_ms, no pool, no
        // per-frame bookkeeping (see `msg_fill_style/hologram.rs`).
        // The pass early-returns when the sweep has completed or
        // there is no animation timeline (bench/edge paths).
        if style == MsgFillStyle::Hologram {
            self.hologram_scanline_pass(frame, message_elapsed_ms);
        }

        // v51 msg-fill-style (pulse): scanner cursor pass, LAST so
        // the cursor glyph renders ON TOP of the overlay text — a
        // visible `▌` (U+258C LEFT ONE QUARTER BLOCK) painted at the
        // most recently revealed content cell (the "scanner head").
        // Stateless: pure function of head_pos + elapsed_ms, no pool,
        // no per-frame bookkeeping (see `msg_fill_style/pulse.rs`).
        // The pass early-returns when there is no head, no timeline,
        // or effects are disabled (--no-effects, PERF-4).
        if style == MsgFillStyle::Pulse {
            self.pulse_cursor_pass(frame, head_pos, message_elapsed_ms);
        }
    }
}

/// v51 msg-fill-style: apply a brightness factor to the message
/// content fg color. Shared by the main content pass, the slide
/// second pass, and the fade-style border scaling so every style
/// goes through ONE pipeline.
///
/// Free function (not a method) so it can be called while the Z-5
/// `border_gradient_scratch` mutable borrow is live — takes the
/// pipeline by shared reference instead of borrowing all of `self`.
///
/// Factor semantics: 1.0 = settled (returns the base color as-is,
/// zero-cost fast path), < 1.0 = dim (fade-in), > 1.0 = boosted
/// (pulse scanner head — routed through `apply_brightness_rgb_unclamped`,
/// the same boost path droplet's CellShader uses for parallax scaling;
/// per-channel clamp at 255).
/// A23: chroma first, `legacy::scale_rgb` fallback.
fn scale_msg_content_fg(
    pipeline: &crate::runtime::ColorPipeline,
    base: Option<Color>,
    factor: f32,
) -> Option<Color> {
    let Some(base_fg) = base else {
        return base;
    };
    if (factor - 1.0).abs() < 1e-6 {
        return Some(base_fg);
    }
    match crate::palette::decode_color(base_fg) {
        Some((r, g, b)) => Some(if pipeline.is_chroma() {
            if factor > 1.0 {
                // Boost (pulse scanner head): the clamped variant would
                // silently cap the factor at 1.0 and kill the effect.
                let (r, g, b) = crate::palette::apply_brightness_rgb_unclamped(r, g, b, factor);
                Color::Rgb { r, g, b }
            } else {
                crate::palette::apply_brightness_rgb(r, g, b, factor)
            }
        } else {
            let (nr, ng, nb) = crate::chroma_dragon_engine::legacy::scale_rgb(r, g, b, factor);
            Color::Rgb {
                r: nr,
                g: ng,
                b: nb,
            }
        }),
        None => Some(base_fg),
    }
}
