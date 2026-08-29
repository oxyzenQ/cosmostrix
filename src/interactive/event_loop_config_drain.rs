// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Live config event draining — extracted from `event_loop.rs` to keep
//! that file under the 800-LOC cap. Pure code motion — no behavior change.

use std::collections::HashMap;
use std::sync::mpsc;

use crate::cloud::Cloud;

/// Drain pending config events from the watcher channel.
///
/// Non-blocking check for config events. On Ok(cfg_map): stores in
/// pending_config for rebuild next frame. On Err(msg): sets
/// LIVE_RELOAD_EXIT_CODE=2 + LIVE_RELOAD_ERROR, stops rain (caller
/// breaks the loop).
///
/// Returns `false` when a validation error was received (caller should
/// break the rain loop). Returns `true` to continue.
pub(crate) fn drain_config_events(
    config_rx: &Option<mpsc::Receiver<Result<HashMap<String, String>, String>>>,
    pending_config: &mut Option<HashMap<String, String>>,
    cloud: &mut Cloud,
) -> bool {
    // Live config reload: non-blocking check for config events.
    if let Some(ref rx) = config_rx {
        while let Ok(event) = rx.try_recv() {
            crate::lr_trace!("render thread received config event from watcher channel");
            match event {
                Ok(cfg) => {
                    crate::lr_trace!(
                        "render thread: pending config set ({} keys) — will rebuild next frame",
                        cfg.len()
                    );
                    *pending_config = Some(cfg);
                }
                Err(msg) => {
                    // config validation errors cause immediate exit.
                    crate::lr_trace!(
                        "render thread: config validation error — setting exit code + breaking rain loop"
                    );
                    if let Ok(mut guard) = crate::live_config::LIVE_RELOAD_ERROR.lock() {
                        *guard = Some(msg);
                    }
                    crate::live_config::LIVE_RELOAD_EXIT_CODE
                        .store(2, std::sync::atomic::Ordering::Release);
                    cloud.raining = false;
                    break;
                }
            }
        }
    }
    true
}
