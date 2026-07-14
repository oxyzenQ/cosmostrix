// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Live config reload — "The Dragon's Awakening".
//!
//! Watches config.toml for changes and sends parsed updates to the render
//! thread via a channel. The render thread applies deltas to the running
//! Cloud via setters, preserving visual state (rain streams don't reset).
//!
//! ## Architecture
//!
//! ```text
//! config.toml ──► notify watcher thread ──► mpsc channel ──► render thread
//!                (parse + validate)         (try_recv/frame)  (apply setters)
//! ```
//!
//! - Watcher thread: blocks on filesystem events, reparses config on change.
//!   On parse error, logs warning and keeps old config (no crash).
//! - Render thread: `try_recv()` each frame (~1ns atomic check on empty channel).
//!   If update pending, applies field deltas via Cloud setters.
//! - Zero parsing on render thread. Zero locks on render thread.
//!
//! ## Applied fields
//!
//! Only fields with Cloud setters are hot-reloadable:
//! density, speed (chars_per_sec), glitch_pct, glitch_times, linger_times,
//! short_pct, die_early_pct, max_droplets_per_column, glitchy, monolith_size.
//!
//! Fields that CANNOT hot-reload (require Cloud rebuild): charset, color
//! scheme, screen size, rain_style, fullwidth. These need a restart.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

use notify::{event::EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::configfile;
use crate::runtime::MonolithSize;

/// Snapshot of config fields that can be hot-reloaded.
///
/// Sent from watcher thread to render thread via mpsc channel. The render
/// thread applies each `Some` field to Cloud via setters. `None` means
/// "field not in config, don't change".
#[derive(Debug, Clone, Default)]
pub struct ConfigUpdate {
    pub density: Option<f32>,
    pub speed: Option<f32>,
    pub fps: Option<f64>,
    pub glitch_pct: Option<f32>,
    pub glitch_low_ms: Option<u16>,
    pub glitch_high_ms: Option<u16>,
    pub linger_low_ms: Option<u16>,
    pub linger_high_ms: Option<u16>,
    pub short_pct: Option<f32>,
    pub die_early_pct: Option<f32>,
    pub max_dpc: Option<u8>,
    pub glitchy: Option<bool>,
    pub monolith_size: Option<MonolithSize>,
    pub auto_color_drift: Option<bool>,
}

/// Parse a flat config HashMap into a ConfigUpdate snapshot.
///
/// Reads only hot-reloadable fields. Unknown/invalid values are silently
/// skipped (the watcher logs parse warnings via load_config_file already).
fn parse_update(cfg: &std::collections::HashMap<String, String>) -> ConfigUpdate {
    let mut update = ConfigUpdate::default();

    if let Some(v) = cfg.get("density") {
        if let Ok(n) = v.parse::<f32>() {
            update.density = Some(n.clamp(0.01, 5.0));
        }
    }
    if let Some(v) = cfg.get("speed") {
        if let Ok(n) = v.parse::<f32>() {
            update.speed = Some(n.clamp(1.0, 100.0));
        }
    }
    if let Some(v) = cfg.get("fps") {
        if let Ok(n) = v.parse::<f64>() {
            update.fps = Some(n.clamp(1.0, 240.0));
        }
    }
    if let Some(v) = cfg.get("glitchpct") {
        if let Ok(n) = v.parse::<f32>() {
            update.glitch_pct = Some(n.clamp(0.0, 100.0) / 100.0);
        }
    }
    if let Some(v) = cfg.get("glitchms") {
        if let Some((lo, hi)) = parse_range(v) {
            update.glitch_low_ms = Some(lo);
            update.glitch_high_ms = Some(hi);
        }
    }
    if let Some(v) = cfg.get("lingerms") {
        if let Some((lo, hi)) = parse_range(v) {
            update.linger_low_ms = Some(lo);
            update.linger_high_ms = Some(hi);
        }
    }
    if let Some(v) = cfg.get("shortpct") {
        if let Ok(n) = v.parse::<f32>() {
            update.short_pct = Some(n.clamp(0.0, 100.0) / 100.0);
        }
    }
    if let Some(v) = cfg.get("rippct") {
        if let Ok(n) = v.parse::<f32>() {
            update.die_early_pct = Some(n.clamp(0.0, 100.0) / 100.0);
        }
    }
    if let Some(v) = cfg.get("maxdpc") {
        if let Ok(n) = v.parse::<u8>() {
            update.max_dpc = Some(n.clamp(1, 3));
        }
    }
    if let Some(v) = cfg.get("glitch-level") {
        update.glitchy = Some(!matches!(v.trim().to_ascii_lowercase().as_str(), "none"));
    }
    if let Some(v) = cfg.get("monolith-size") {
        update.monolith_size = match v.trim().to_ascii_lowercase().as_str() {
            "small" => Some(MonolithSize::Small),
            "large" => Some(MonolithSize::Large),
            "normal" => Some(MonolithSize::Normal),
            _ => None,
        };
    }
    if let Some(v) = cfg.get("auto-color-drift") {
        update.auto_color_drift = Some(v.trim() == "true");
    }

    update
}

/// Parse "LOW,HIGH" range string (e.g. "200,300" for glitchms).
fn parse_range(s: &str) -> Option<(u16, u16)> {
    let (lo, hi) = s.split_once(',')?;
    let lo: u16 = lo.trim().parse().ok()?;
    let hi: u16 = hi.trim().parse().ok()?;
    Some((lo.min(hi), lo.max(hi)))
}

/// Spawn a config file watcher on a background thread.
///
/// Returns a `Receiver<ConfigUpdate>` that the render thread polls with
/// `try_recv()` each frame. The watcher thread blocks on filesystem events
/// and exits when the sender is dropped (i.e. when the receiver is dropped
/// at program exit).
///
/// If the config file doesn't exist or can't be watched, returns `None`
/// and no watcher is spawned (graceful degradation).
pub fn spawn_watcher(config_path: PathBuf) -> Option<Receiver<ConfigUpdate>> {
    if !config_path.exists() {
        return None;
    }

    let (tx, rx) = mpsc::channel::<ConfigUpdate>();
    let path = config_path.clone();

    // Spawn watcher thread. It owns the RecommendedWatcher and blocks on
    // events. When the main thread drops `rx`, the channel closes and the
    // watcher's send() will fail, causing the thread to exit naturally.
    std::thread::Builder::new()
        .name("cosmostrix-config-watcher".to_string())
        .spawn(move || {
            watcher_loop(path, tx);
        })
        .ok()?;

    Some(rx)
}

/// Main watcher loop — blocks on filesystem events, reparses on change.
fn watcher_loop(path: PathBuf, tx: Sender<ConfigUpdate>) {
    // Debounce: coalesce rapid successive events (editors often write twice).
    const DEBOUNCE_MS: u64 = 200;

    let (notify_tx, notify_rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();

    let mut watcher: RecommendedWatcher = match RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            let _ = notify_tx.send(res);
        },
        notify::Config::default(),
    ) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("live-config: failed to create watcher: {e}");
            return;
        }
    };

    // Watch the parent directory to catch atomic-save renames (editors that
    // write to a temp file then rename over the original).
    let watch_dir = path
        .parent()
        .map(|p| {
            if p.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                p.to_path_buf()
            }
        })
        .unwrap_or_else(|| PathBuf::from("."));

    if let Err(e) = watcher.watch(&watch_dir, RecursiveMode::NonRecursive) {
        eprintln!("live-config: failed to watch {}: {e}", watch_dir.display());
        return;
    }

    let target_file = Arc::new(path.clone());
    let mut last_event = std::time::Instant::now();

    for event_result in notify_rx.iter() {
        if tx.send(ConfigUpdate::default()).is_err() {
            // Receiver dropped — program is exiting.
            break;
        }

        match event_result {
            Ok(event) => {
                // Only process events that touch our config file.
                let touches_target = event.paths.iter().any(|p| p == &*target_file);
                if !touches_target {
                    continue;
                }

                // Only react to write/modify/create/rename events.
                let relevant = matches!(
                    event.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                );
                if !relevant {
                    continue;
                }

                // Debounce: ignore events within DEBOUNCE_MS of the last one.
                let now = std::time::Instant::now();
                if now.duration_since(last_event) < Duration::from_millis(DEBOUNCE_MS) {
                    continue;
                }
                last_event = now;

                // Small delay to let the editor finish writing (atomic save
                // may rename after the event fires).
                std::thread::sleep(Duration::from_millis(50));

                // Reparse config.
                let cfg = configfile::load_config_file(Some(&path));
                if cfg.is_empty() {
                    // Config might be momentarily empty during atomic save.
                    // Skip this event — the next one will have content.
                    continue;
                }

                let update = parse_update(&cfg);
                if tx.send(update).is_err() {
                    break;
                }
            }
            Err(e) => {
                eprintln!("live-config: watch error: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_update_extracts_density() {
        let mut cfg = std::collections::HashMap::new();
        cfg.insert("density".to_string(), "0.9".to_string());
        let update = parse_update(&cfg);
        assert_eq!(update.density, Some(0.9));
    }

    #[test]
    fn parse_update_extracts_speed() {
        let mut cfg = std::collections::HashMap::new();
        cfg.insert("speed".to_string(), "25".to_string());
        let update = parse_update(&cfg);
        assert_eq!(update.speed, Some(25.0));
    }

    #[test]
    fn parse_update_clamps_out_of_range() {
        let mut cfg = std::collections::HashMap::new();
        cfg.insert("density".to_string(), "99.0".to_string());
        let update = parse_update(&cfg);
        assert_eq!(update.density, Some(5.0)); // clamped
    }

    #[test]
    fn parse_update_handles_invalid_values() {
        let mut cfg = std::collections::HashMap::new();
        cfg.insert("density".to_string(), "not_a_number".to_string());
        let update = parse_update(&cfg);
        assert_eq!(update.density, None); // invalid -> None
    }

    #[test]
    fn parse_update_extracts_glitch_range() {
        let mut cfg = std::collections::HashMap::new();
        cfg.insert("glitchms".to_string(), "200,300".to_string());
        let update = parse_update(&cfg);
        assert_eq!(update.glitch_low_ms, Some(200));
        assert_eq!(update.glitch_high_ms, Some(300));
    }

    #[test]
    fn parse_update_extracts_monolith_size() {
        let mut cfg = std::collections::HashMap::new();
        cfg.insert("monolith-size".to_string(), "large".to_string());
        let update = parse_update(&cfg);
        assert_eq!(update.monolith_size, Some(MonolithSize::Large));
    }

    #[test]
    fn parse_update_extracts_glitchy_from_level() {
        let mut cfg = std::collections::HashMap::new();
        cfg.insert("glitch-level".to_string(), "none".to_string());
        let update = parse_update(&cfg);
        assert_eq!(update.glitchy, Some(false));

        cfg.insert("glitch-level".to_string(), "intense".to_string());
        let update = parse_update(&cfg);
        assert_eq!(update.glitchy, Some(true));
    }

    #[test]
    fn parse_update_empty_config_returns_default() {
        let cfg = std::collections::HashMap::new();
        let update = parse_update(&cfg);
        assert!(update.density.is_none());
        assert!(update.speed.is_none());
    }

    #[test]
    fn parse_range_handles_whitespace() {
        assert_eq!(parse_range(" 200 , 300 "), Some((200, 300)));
        assert_eq!(parse_range("300,200"), Some((200, 300))); // ordered
    }

    #[test]
    fn parse_range_rejects_invalid() {
        assert_eq!(parse_range("abc"), None);
        assert_eq!(parse_range("200"), None); // no comma
    }
}
