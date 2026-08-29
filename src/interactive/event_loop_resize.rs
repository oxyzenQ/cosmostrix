// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Terminal resize handler — extracted from `event_loop.rs` to keep that
//! file under the 800-LOC cap. Pure code motion — no behavior change.

use std::time::Instant;

use crate::cloud::Cloud;
use crate::color_cache::ColorCache;
use crate::app::CloudConfig;
use crate::frame::Frame;
use crate::interactive::hud::HudState;
use crate::terminal::Terminal;

/// Handle a pending terminal resize.
///
/// Updates w/h locals, resets cloud + frame to new dimensions, applies
/// density settings from the live-reloaded config, forces full redraw,
/// refreshes the SGR color cache, and updates the HUD screen size.
#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_resize(
    pending_resize: Option<(u16, u16)>,
    w: &mut u16,
    h: &mut u16,
    cloud: &mut Cloud,
    frame: &mut Frame,
    hud_state: &mut HudState,
    term: &mut Terminal,
    current_cfg: &CloudConfig,
    last_resync_time: &mut Instant,
    screen_size_fixed: bool,
) {
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
        *w = nw;
        *h = nh;
        cloud.reset(nw, nh);
        *frame = Frame::new(nw, nh, cloud.palette.bg);
        // v50.0.0-beta.6: use current_cfg (live-reloaded) instead of
        // cfg (startup) for density settings. If the user live-reloads
        // density_auto or base_density, the resize handler must respect
        // the new values — otherwise a resize after live-reload would
        // use stale startup density.
        if current_cfg.density_auto {
            cloud.set_droplet_density(crate::effective_density(current_cfg.base_density, nw, true));
        }
        cloud.force_draw_everything();
        // H1 (internal independent QA): refresh the SGR color cache after
        // resize — every other palette-affecting path calls set_color_cache,
        // but the resize handler was missing it. Without this, a live-reload
        // palette change coinciding with a resize could produce a 1-frame
        // color flicker from a stale cache.
        term.set_color_cache(ColorCache::new(&cloud.palette));
        *last_resync_time = Instant::now();
        // Update HUD screen size on dynamic resize (fixed mode ignores resize)
        if !screen_size_fixed {
            hud_state.set_screen_size(nw, nh, false);
        }
    }
}
