// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Border-touch detection + F2 splash crown spark — extracted from
//! `cloud/rain.rs` to keep that file under the 800-LOC hard cap
//! (see `src/RULES_LOC.md`).
//!
//! Owns `Cloud::detect_border_touch()` — the RAIN_BORDER_TOUCH_GLOW
//! helper that detects when a droplet's head crosses into the message
//! overlay border + spawns the F2 splash crown spark on non-corner
//! touches. LTS-bounded pulse pool (dedup by msg_idx).
//!
//! Implemented as a separate `impl Cloud` block (Rust allows multiple
//! impl blocks across files for the same type).

use std::time::Instant;

use super::state::BorderPulse;

impl super::Cloud {
    /// RAIN_BORDER_TOUCH_GLOW helper (Option C+D, owner-approved 2026-08-26).
    ///
    /// Called from the droplet advance loop (both sim-cap and bench paths)
    /// with the pre-advance (`prev_hp`) and post-advance (`hp`) `head_put_line`
    /// values for the droplet at column `col`. On transition
    /// `prev_hp < top && hp >= top` for a column inside the overlay's
    /// horizontal span `[left, right)`, push a `BorderPulse` with the
    /// current palette's `head_rgb` (dynamic color, not static white —
    /// matches the FlashWaveCtx pattern at line 681-689).
    ///
    /// The cached geometry (`message_top_line`, `message_left_col`,
    /// `message_right_col`) is set in `reset_message`. When no bordered
    /// overlay is active (`-m` without border, or no message at all),
    /// `message_top_line == u16::MAX` and the early-return fires immediately
    /// — zero cost on the hot path.
    ///
    /// `MsgChr` lookup is a linear scan over `self.message` filtered to
    /// `line == top && col == col`. `self.message` is laid out row-major
    /// (see `reset_message`), so the matching cell is found in O(box_w)
    /// — typically ≤ 100 entries, well under 1 µs even on a 200-col
    /// terminal.
    ///
    /// ## LTS bounds (2026-08-26 polish)
    ///
    /// The pulse pool is bounded by deduplication by `msg_idx`: when a
    /// new touch lands on a cell that already has an alive pulse, the
    /// existing pulse is refreshed (`birth = now`, `head_rgb =
    /// current`) instead of pushing a new entry. This guarantees
    /// `self.border_pulses.len() <= self.message.len()` at all times —
    /// the upper bound is the number of distinct border cells in the
    /// overlay (typically 50–100), regardless of how many droplets hit
    /// the same column within the lifetime window. The refresh also
    /// makes the glow more dynamic: each re-touch picks up the
    /// palette's current `head_rgb` (e.g. mid-transition between two
    /// palettes, the glow re-snapshots to the newest stop).
    ///
    /// ## Panic safety
    ///
    /// `palette.colors.last()` returns `Option<&Color>`. The `.copied()`
    /// lift to `Option<Color>` followed by `.and_then(decode_color)`
    /// and `.unwrap_or((255, 255, 255))` guarantees no panic even when
    /// the palette is empty (Mono mode, or a misconfigured `rain = []`
    /// config). This is the LTS hardening called out by the post-impl
    /// audit; do NOT "simplify" to `.last().unwrap()` or `.expect()`.
    #[inline]
    pub(crate) fn detect_border_touch(&mut self, col: u16, prev_hp: u16, hp: u16, now: Instant) {
        // PERF-4: --no-effects gate. The border-touch glow (halo above
        // the message box) and the F2 splash crown spark are both
        // cosmetic effects. spawn_border_spark is already gated, but
        // the border_pulses push (the glow itself) was not — this gate
        // closes that leak. Early-return here means no new pulses are
        // pushed; existing pulses fade out on their own expiry tick
        // (draw_message prunes expired pulses every frame).
        if !self.effects_enabled {
            return;
        }
        let top = self.message_top_line;
        if top == u16::MAX {
            return;
        }
        // Transition: head was strictly above the top border last frame,
        // and is at or below it this frame. Use `>=` on the post-advance
        // side to catch multi-line advances (high-speed droplets may skip
        // the exact top line in a single frame).
        if prev_hp >= top || hp < top {
            return;
        }
        if col < self.message_left_col || col >= self.message_right_col {
            return;
        }

        // Snapshot the current palette's head color BEFORE the immutable
        // borrow on self.message (so the two immutable borrows don't
        // overlap with the later mutable push on self.border_pulses).
        //
        // LTS: this `.last().copied().and_then(...).unwrap_or(...)` chain
        // is panic-safe against an empty palette — see the docstring above.
        let head_rgb = self
            .palette
            .colors
            .last()
            .copied()
            .and_then(crate::palette::decode_color)
            .unwrap_or((255, 255, 255));

        let msg_idx = self
            .message
            .iter()
            .position(|mc| mc.line == top && mc.col == col);

        if let Some(idx) = msg_idx {
            // F2 Splash Crown: check corner-skip BEFORE mutable borrows.
            // LTS invariant: no lone bright heads at top corners.
            let is_corner = matches!(self.message[idx].val, '╭' | '╮' | '╰' | '╯');
            // LTS dedup: if a pulse for this msg_idx is still alive,
            // refresh it in place (re-arm birth + re-snapshot head_rgb)
            // instead of pushing a duplicate. Bounds the pool to
            // `self.message.len()` regardless of touch density.
            if let Some(existing) = self.border_pulses.iter_mut().find(|p| p.msg_idx == idx) {
                existing.birth = now;
                existing.head_rgb = head_rgb;
            } else {
                self.border_pulses.push(BorderPulse {
                    msg_idx: idx,
                    col,
                    head_rgb,
                    birth: now,
                });
            }
            // F2 Splash Crown spark: spawn 6-particle upward splash on
            // non-corner border touches. See
            // docs/research/RAIN_BORDER_TOUCH_SPARK_RESEARCH.md §3.2.
            if !is_corner {
                self.spawn_border_spark(col, top, head_rgb);
            }
        }
    }
}
