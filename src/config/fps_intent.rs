// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Explicit fps user-intent tracker (v80.0.0-beta.2).
//!
//! Extracted from `config/mod.rs` to keep that file under the 800-LOC
//! hard cap (see `src/RULES_LOC.md`). Pure code motion + module docs.
//!
//! Owner bug (2026-09-02): an explicit `fps = 60` from config.toml or a
//! `[scene-custom.<name>]` block was silently stomped by the dynamic
//! default (144 on high-perf terminals) because main.rs's heuristic
//! (`fps_user_set = args.fps != 60.0`) cannot distinguish "user wrote 60"
//! from "clap default is still 60". The HIGH_PERF_DEFAULT_FPS doc
//! promises "the user's explicit value always wins over this default" —
//! the promise was broken for the exact-value collision case.
//!
//! This tracker records the LAYER that last set an explicit fps value
//! during apply_config_and_runtime_defaults (config file key or
//! scene-custom block field). main.rs consults it after config apply, so
//! the dynamic default only applies when NEITHER the CLI NOR any config
//! layer expressed fps intent. Built-in scene defaults (including the
//! default cinematic template) deliberately do NOT record intent — they
//! are authored templates, not user choices, and the dynamic
//! terminal-aware default keeps refining them (unchanged pre-beta.2
//! behavior).
//!
//! Startup-only: written once during config apply (single-threaded),
//! read once in main.rs. The Mutex is uncontended; a plain static would
//! be race-prone under `cargo test --threads > 1`.

/// Records that the fps value was explicitly set by a config layer.
/// `source` is a short label for verbose output ("config" or
/// "scene-custom"). Last writer wins — later layers overwrite earlier
/// ones, mirroring the actual application order.
pub(crate) fn record_fps_explicit(source: &'static str) {
    FPS_EXPLICIT_SOURCE
        .lock()
        .map(|mut guard| *guard = Some(source))
        .ok();
}

/// The config layer that last recorded explicit fps intent, if any.
/// None = fps was never set outside the CLI (clap default still stands).
#[must_use]
pub(crate) fn fps_explicit_source() -> Option<&'static str> {
    FPS_EXPLICIT_SOURCE.lock().ok().and_then(|guard| *guard)
}

static FPS_EXPLICIT_SOURCE: std::sync::Mutex<Option<&'static str>> = std::sync::Mutex::new(None);
