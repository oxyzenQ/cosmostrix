// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Terminal resize handler — extracted from `event_loop.rs` to keep that
//! file under the 800-LOC cap. Pure code motion — no behavior change.
//!
//! NIGHT-hunter-21: takes the loop context. The old 10-parameter
//! signature carried the `w`/`h` `u16` pair and the
//! `current_cfg`/`startup`-derived `screen_size_fixed` flag as separate
//! params; they are `ctx` fields / derived reads now.

use std::time::Instant;

use super::event_loop_ctx::LoopCtx;
use crate::color_cache::ColorCache;
use crate::frame::Frame;

/// Handle a pending terminal resize.
///
/// Updates `ctx.w`/`ctx.h`, resets cloud + frame to new dimensions,
/// applies density settings from the live-reloaded config, forces full
/// redraw, refreshes the SGR color cache, and updates the HUD screen
/// size.
pub(crate) fn handle_resize(ctx: &mut LoopCtx, pending_resize: Option<(u16, u16)>) {
    if let Some((nw, nh)) = pending_resize {
        // v50.0.0-beta.6 CRITICAL FIX: update the local w/h variables
        // alongside cloud + frame. Previously only cloud.reset() and
        // Frame::new() were called with the new dimensions, but the
        // local `w` and `h` variables stayed at the pre-resize values.
        // When a live-reload triggered the rebuild path (line 342-399),
        // it used the STALE w/h — reverting the screen to the pre-resize
        // size (e.g. 150x32 after the user had gone fullscreen to 212x64).
        // This was a FATAL visual bug for LTS release. Now w/h are kept
        // in sync with the actual terminal dimensions at all times.
        ctx.w = nw;
        ctx.h = nh;
        ctx.cloud.reset(nw, nh);
        ctx.frame = Frame::new(nw, nh, ctx.cloud.palette.bg);
        // v50.0.0-beta.6: use current_cfg (live-reloaded) instead of
        // cfg (startup) for density settings. If the user live-reloads
        // density_auto or base_density, the resize handler must respect
        // the new values — otherwise a resize after live-reload would
        // use stale startup density.
        if ctx.config.current.density_auto {
            ctx.cloud.set_droplet_density(crate::effective_density(
                ctx.config.current.base_density,
                nw,
                true,
            ));
        }
        ctx.cloud.force_draw_everything();
        // H1 (internal independent QA): refresh the SGR color cache after
        // resize — every other palette-affecting path calls set_color_cache,
        // but the resize handler was missing it. Without this, a live-reload
        // palette change coinciding with a resize could produce a 1-frame
        // color flicker from a stale cache.
        ctx.term
            .set_color_cache(ColorCache::new(&ctx.cloud.palette));
        ctx.last_resync_time = Instant::now();
        // Update HUD screen size on dynamic resize (fixed mode ignores resize)
        if ctx.config.startup.screen_size.is_none() {
            ctx.hud_state.set_screen_size(nw, nh, false);
        }
    }
}
