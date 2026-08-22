// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Ambient scheduler diagnostics — counters and summary for exit logging.
//!
//! Extracted from interactive/mod.rs to reduce LOC burden and separate
//! concerns. These track the ambient apply path, snapback behavior,
//! schedule reloads, and consistency fixes.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

static AMBIENT_SNAPBACK_COUNT: AtomicU64 = AtomicU64::new(0);
static AMBIENT_RX_COUNT: AtomicU64 = AtomicU64::new(0);
static AMBIENT_REAPPLY_COUNT: AtomicU64 = AtomicU64::new(0);
static AMBIENT_STARTUP_COUNT: AtomicU64 = AtomicU64::new(0);
static LAST_SCENE_CHANGE: Mutex<Option<String>> = Mutex::new(None);
// Snapback guard state diagnostics — capture why snapback fired
// despite user disabling ambient.
static AMBIENT_SNAPBACK_GUARD_SKED_LEN: AtomicU64 = AtomicU64::new(999);
static AMBIENT_SNAPBACK_GUARD_LAST_APPLIED: AtomicU64 = AtomicU64::new(999);
static AMBIENT_SCHEDULE_RELOAD_COUNT: AtomicU64 = AtomicU64::new(0);
static AMBIENT_SCHEDULE_EMPTY_COUNT: AtomicU64 = AtomicU64::new(0);
// Post-rebuild consistency fix + permanent snapback kill diagnostics.
static AMBIENT_CONFIG_REBUILD_COUNT: AtomicU64 = AtomicU64::new(0);
static AMBIENT_CONSISTENCY_FIX_COUNT: AtomicU64 = AtomicU64::new(0);
static AMBIENT_SNAPBACK_KILLED: AtomicU64 = AtomicU64::new(0);

pub(crate) fn ambient_diag_snapback() {
    AMBIENT_SNAPBACK_COUNT.fetch_add(1, Ordering::Relaxed);
}
pub(crate) fn ambient_diag_rx() {
    AMBIENT_RX_COUNT.fetch_add(1, Ordering::Relaxed);
}
pub(crate) fn ambient_diag_reapply() {
    AMBIENT_REAPPLY_COUNT.fetch_add(1, Ordering::Relaxed);
}
pub(crate) fn ambient_diag_startup() {
    AMBIENT_STARTUP_COUNT.fetch_add(1, Ordering::Relaxed);
}
pub(crate) fn ambient_diag_scene_change(source: &str) {
    if let Ok(mut g) = LAST_SCENE_CHANGE.lock() {
        *g = Some(source.to_string());
    }
}
/// Record snapback guard state at call site.
pub(crate) fn ambient_diag_snapback_guard(sked_len: u64, last_applied_is_some: bool) {
    AMBIENT_SNAPBACK_GUARD_SKED_LEN.store(sked_len, Ordering::Relaxed);
    AMBIENT_SNAPBACK_GUARD_LAST_APPLIED
        .store(if last_applied_is_some { 1 } else { 0 }, Ordering::Relaxed);
}
pub(crate) fn ambient_diag_schedule_reload() {
    AMBIENT_SCHEDULE_RELOAD_COUNT.fetch_add(1, Ordering::Relaxed);
}
pub(crate) fn ambient_diag_schedule_empty() {
    AMBIENT_SCHEDULE_EMPTY_COUNT.fetch_add(1, Ordering::Relaxed);
}
pub(crate) fn ambient_diag_config_rebuild() {
    AMBIENT_CONFIG_REBUILD_COUNT.fetch_add(1, Ordering::Relaxed);
}
pub(crate) fn ambient_diag_consistency_fix() {
    AMBIENT_CONSISTENCY_FIX_COUNT.fetch_add(1, Ordering::Relaxed);
}
pub(crate) fn ambient_diag_snapback_killed() {
    AMBIENT_SNAPBACK_KILLED.store(1, Ordering::Relaxed);
}
pub(crate) fn ambient_diag_summary() -> String {
    let snap = AMBIENT_SNAPBACK_COUNT.load(Ordering::Relaxed);
    let rx = AMBIENT_RX_COUNT.load(Ordering::Relaxed);
    let reapply = AMBIENT_REAPPLY_COUNT.load(Ordering::Relaxed);
    let startup = AMBIENT_STARTUP_COUNT.load(Ordering::Relaxed);
    let guard_sked_len = AMBIENT_SNAPBACK_GUARD_SKED_LEN.load(Ordering::Relaxed);
    let guard_last = AMBIENT_SNAPBACK_GUARD_LAST_APPLIED.load(Ordering::Relaxed);
    let reloads = AMBIENT_SCHEDULE_RELOAD_COUNT.load(Ordering::Relaxed);
    let empties = AMBIENT_SCHEDULE_EMPTY_COUNT.load(Ordering::Relaxed);
    let cfg_rebuilds = AMBIENT_CONFIG_REBUILD_COUNT.load(Ordering::Relaxed);
    let consistency_fixes = AMBIENT_CONSISTENCY_FIX_COUNT.load(Ordering::Relaxed);
    let snapback_killed = AMBIENT_SNAPBACK_KILLED.load(Ordering::Relaxed);
    let last = LAST_SCENE_CHANGE
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_else(|| "none".to_string());
    format!(
        "ambient_diag: startup={} rx={} reapply={} snapback={} cfg_rebuilds={} sked_reloads={} sked_empties={} consistency_fixes={} snapback_killed={} snapback_guard_sked_len={} snapback_guard_last_applied={} last_scene_change={}",
        startup, rx, reapply, snap, cfg_rebuilds, reloads, empties, consistency_fixes, snapback_killed, guard_sked_len, guard_last, last
    )
}
