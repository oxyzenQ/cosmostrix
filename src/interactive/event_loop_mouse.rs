// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Non-key runtime event handling: mouse, paste, focus.
//!
//! Extracted from `event_loop.rs` (NIGHT-improve-8) to keep that file
//! under the 800-LOC cap (repo convention: split, don't exempt).
//!
//! Anti-copy policy (owner request: disable copy/paste, including via
//! shift+click and any other method):
//! - mouse capture is held for the entire session, so plain drag-select
//!   is consumed by the app and never reaches the terminal's selection
//!   engine;
//! - modified mouse events (shift+click and every other modifier
//!   combination on Down, plus the Drag/Up/Moved continuation of that
//!   gesture) are the terminal's local selection bypass — most terminals
//!   never forward them to the application, and those that do get zero
//!   visual acknowledgment (hover glow frozen, no click wave, no idle
//!   click wake) plus a full-frame redraw per event that erases freshly
//!   painted native selection highlights where the terminal clears
//!   selection state on grid updates, and keeps the grid churning under
//!   the whole gesture so position-anchored selection copies capture
//!   moving rain glyphs instead of the highlighted text;
//! - pasted content is structurally discarded (the payload is never
//!   read) — the arm only feeds the paste burst guard and the renderer
//!   wake, keeping phosphor decay state coherent.

use std::time::Instant;

use crossterm::event::{Event, MouseEventKind};

use super::activity::register_activity;
use super::event_loop_ctx::LoopCtx;
use super::input::is_selection_bypass_event;

/// Handle one non-key runtime event (mouse / paste / focus gained).
///
/// Returns the immediate wake time when the renderer should be woken
/// ahead of its scheduled frame; `None` leaves the loop cadence
/// unchanged. Every event is fully consumed — mouse events block
/// drag-select by never reaching the terminal's local selection engine.
pub(super) fn handle_runtime_event(
    event: Event,
    ctx: &mut LoopCtx,
    is_idle: bool,
) -> Option<Instant> {
    match event {
        Event::Paste(_) => {
            let activity_time = Instant::now();
            ctx.paste_guard.note_bracketed_paste(activity_time);
            let _ = register_activity(
                &mut ctx.power_manager,
                &mut ctx.last_resync_time,
                activity_time,
                is_idle,
                false,
            );
            ctx.cloud.force_draw_everything();
            Some(activity_time)
        }
        Event::Mouse(m) => {
            // Mouse events always captured (blocks drag-select). No force_draw
            // on MOVE (old: bright-color flash). CLICK wakes renderer
            // on idle→active (old: click effect vanished at 30 FPS idle cadence).
            let activity_time = Instant::now();
            let is_click = matches!(m.kind, MouseEventKind::Down(_));
            let was_idle = is_idle;
            let _ = register_activity(
                &mut ctx.power_manager,
                &mut ctx.last_resync_time,
                activity_time,
                was_idle,
                false,
            );
            // Hover/click visual effects are ALWAYS ON (--mouse deleted).
            // BUT: when paused OR decelerating, skip click wave
            // effects to prevent queued flash waves from
            // accumulating and causing "stuck particles" on
            // resume (owner-reported bug: rapid pause/unpause
            // cycles left effects hanging).
            //
            // Must check `is_paused_or_decelerating()` (not just
            // `pause`) because the deceleration phase is also a
            // pause-related state where click effects should be
            // suppressed.
            //
            // NIGHT-improve-8 follow-up: the bypass check runs BEFORE
            // the hover-position update so a modified gesture gets
            // zero visual acknowledgment — the hover glow must never
            // track a selection attempt, including its drag phase
            // (the pre-change order let the glow follow shift+drags).
            if is_selection_bypass_event(&m) {
                // Modified mouse events (shift+click and any other
                // modifier combination, on Down / Drag / Up / Moved)
                // are the terminal's native selection-bypass path.
                // Where the terminal forwards such events, the WHOLE
                // gesture — not just the anchor Down — gets zero
                // visual acknowledgment: hover position untouched
                // (glow frozen), no click wave, no click-wave wake.
                // The full-frame redraw per event erases the freshly
                // painted native selection highlight in terminals
                // that clear selection state when the grid content
                // underneath updates, and keeps the grid churning
                // under the extending selection so position-anchored
                // copies (xterm-style: copy reads CURRENT cell
                // content) capture moving rain glyphs, not the text
                // the user highlighted. Modified scroll stays on the
                // plain path: the wheel is not a selection primitive.
                ctx.cloud.force_draw_everything();
                Some(activity_time)
            } else {
                // Plain path: mouse position is still tracked (hover
                // glow) and the event is still consumed (blocks
                // drag-select).
                ctx.cloud.set_mouse_position(m.column, m.row);
                if is_click && !ctx.cloud.is_paused_or_decelerating() {
                    ctx.cloud.set_mouse_click(m.column, m.row);
                    // Wake renderer immediately on idle→active click.
                    if was_idle {
                        ctx.cloud.force_draw_everything();
                        Some(activity_time)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        }
        Event::FocusGained => {
            let activity_time = Instant::now();
            if register_activity(
                &mut ctx.power_manager,
                &mut ctx.last_resync_time,
                activity_time,
                is_idle,
                true,
            ) {
                ctx.cloud.force_draw_everything();
                Some(activity_time)
            } else {
                None
            }
        }
        _ => None,
    }
}
