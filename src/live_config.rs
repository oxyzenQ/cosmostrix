// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Live config reload — "The Dragon's true Awakening".
//!
//! Watches config.toml for changes, validates strictly, and sends the
//! validated config HashMap to the render thread for a full Cloud rebuild.
//!
//! ## Architecture
//!
//! ```text
//! config.toml → notify watcher thread → mpsc channel → render thread
//!               (parse + validate)      (try_recv/frame)  (rebuild Cloud)
//! ```
//!
//! - Watcher thread: blocks on filesystem events, reparses config on change.
//!   Validates EVERY field strictly — any invalid value rejects the entire
//!   config (no partial apply). On error, logs to stderr and keeps old config.
//! - Render thread: `try_recv()` each frame (~1ns on empty channel).
//!   If update pending, rebuilds CloudConfig from base + new config values,
//!   then rebuilds Cloud (full create_cloud + reset). Visual state resets
//!   (rain streams restart) but color/charset/scene changes take effect.
//!
//! ## Strict validation
//!
//! Uses the same `validate_field_value` rules as `--testconf`. If ANY field
//! has an invalid value (e.g. `speed = 100000`), the entire config is
//! rejected with a clear error message. No silent fallback.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU8;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use notify::{event::EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::configfile;

/// Global exit code set by live-reload when invalid config is detected.
/// 0 = no error (default), 2 = live-reload validation failure.
/// Main.rs checks this after run_interactive() returns and exits accordingly.
pub static LIVE_RELOAD_EXIT_CODE: AtomicU8 = AtomicU8::new(0);

/// Global error message captured during live-reload failure.
/// Printed to stderr AFTER terminal restoration (in main.rs) so the user
/// can actually see it — printing during alternate-screen mode swallows
/// the output.
pub static LIVE_RELOAD_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// Live config event sent from watcher to render thread.
/// Ok = valid config, rebuild Cloud. Err = invalid, exit cosmostrix.
pub type LiveConfigEvent = Result<HashMap<String, String>, String>;

/// Spawn a config file watcher on a background thread.
///
/// Returns a `Receiver<HashMap<String, String>>` that the render thread polls
/// with `try_recv()` each frame. The watcher validates config strictly before
/// sending — invalid configs are rejected with a stderr error message.
///
/// If the config file doesn't exist or can't be watched, returns `None`.
pub fn spawn_watcher(config_path: PathBuf) -> Option<Receiver<LiveConfigEvent>> {
    if !config_path.exists() {
        return None;
    }

    let (tx, rx) = mpsc::channel::<LiveConfigEvent>();
    let path = config_path.clone();

    std::thread::Builder::new()
        .name("cosmostrix-config-watcher".to_string())
        .spawn(move || {
            watcher_loop(path, tx);
        })
        .ok()?;

    Some(rx)
}

/// Main watcher loop — blocks on filesystem events, reparses on change.
fn watcher_loop(path: PathBuf, tx: Sender<LiveConfigEvent>) {
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
            eprintln!("[live-reload] failed to create watcher: {e}");
            return;
        }
    };

    // Watch the parent directory to catch atomic-save renames.
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
        eprintln!("[live-reload] failed to watch {}: {e}", watch_dir.display());
        return;
    }

    let target_file = Arc::new(path.clone());
    let mut last_event = std::time::Instant::now();

    for event_result in notify_rx.iter() {
        match event_result {
            Ok(event) => {
                let touches_target = event.paths.iter().any(|p| p == &*target_file);
                if !touches_target {
                    continue;
                }

                let relevant = matches!(
                    event.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
                );
                if !relevant {
                    continue;
                }

                // Debounce
                let now = std::time::Instant::now();
                if now.duration_since(last_event) < Duration::from_millis(DEBOUNCE_MS) {
                    continue;
                }
                last_event = now;

                // Small delay for atomic-save rename completion.
                std::thread::sleep(Duration::from_millis(50));

                // Reparse config using parse_config_text (not load_config_file)
                // so we can check malformed_lines AND unknown_keys.
                let content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let parsed = configfile::parse_config_text(&content);
                if parsed.values.is_empty() && parsed.malformed_lines.is_empty() {
                    continue;
                }

                // Check malformed lines first — these are syntax errors.
                if !parsed.malformed_lines.is_empty() {
                    let lines: Vec<&str> = parsed
                        .malformed_lines
                        .iter()
                        .take(3)
                        .map(String::as_str)
                        .collect();
                    let msg = format!(
                        "malformed line(s): '{}' (expected 'key = value' syntax)",
                        lines.join(", ")
                    );
                    let _ = tx.send(Err(msg));
                    continue;
                }

                // Check unknown keys.
                if !parsed.unknown_keys.is_empty() {
                    let keys: Vec<&str> = parsed
                        .unknown_keys
                        .iter()
                        .take(3)
                        .map(String::as_str)
                        .collect();
                    let msg = format!(
                        "unknown key(s): '{}' (run 'cosmostrix --testconf' for known keys)",
                        keys.join(", ")
                    );
                    let _ = tx.send(Err(msg));
                    continue;
                }

                let cfg = &parsed.values;

                // Strict validation: reject entire config if ANY field is invalid.
                match crate::testconf::validate_config_strictly(cfg) {
                    Ok(()) => {
                        if tx.send(Ok(cfg.clone())).is_err() {
                            break;
                        }
                    }
                    Err(msg) => {
                        let _ = tx.send(Err(msg));
                    }
                }
            }
            Err(e) => {
                eprintln!("[live-reload] watch error: {e}");
            }
        }
    }
}

/// Rebuild a CloudConfig from a base template + new config values.
///
/// Takes the original CloudConfig (built from CLI + initial config) and
/// overrides config-derived fields with values from the new config HashMap.
/// CLI-only fields (screen_size, color_tune, message, etc.) are preserved
/// from the base.
///
/// For live reload, config values override CLI defaults (the user is
/// actively editing config.toml and expects those values to take effect).
#[must_use]
pub fn rebuild_cloud_config(
    base: &crate::app::CloudConfig,
    cfg: &HashMap<String, String>,
) -> crate::app::CloudConfig {
    let mut new = base.clone();

    // Color scheme
    if let Some(v) = cfg.get("color") {
        if let Ok(scheme) = crate::cli::parse_color_scheme(v) {
            new.color_scheme = scheme;
        }
    }

    // v16: Custom color palette live reload.
    // If a custom palette was active at startup (via --colors-custom),
    // reload its definition from the new config so editing color values
    // takes effect immediately.
    if let Some(ref name) = new.custom_palette_name {
        match crate::colors_custom::load_custom_palette(cfg, name) {
            Ok(palette) => new.custom_palette = Some(palette),
            Err(_) => { /* leave existing palette — don't break live reload */ }
        }
    }

    // Charset (requires rebuilding chars vector)
    if let Some(v) = cfg.get("charset") {
        if let Ok(charset) = crate::charset::charset_from_str(v, false) {
            new.charset_preset = v.clone();
            new.chars = crate::charset::build_chars(charset, &new.user_ranges, new.def_ascii);
        }
    }

    // Scene (affects rain_style)
    if let Some(v) = cfg.get("scene") {
        if let Some(scene_info) = crate::scene::get_scene(v) {
            new.rain_style = scene_info.config.rain_style;
            // Apply scene color/charset if set
            if let Some(color) = scene_info.config.color {
                if let Ok(scheme) = crate::cli::parse_color_scheme(color) {
                    new.color_scheme = scheme;
                }
            }
            if let Some(charset_name) = scene_info.config.charset {
                if let Ok(charset) = crate::charset::charset_from_str(charset_name, false) {
                    new.charset_preset = charset_name.to_string();
                    new.chars =
                        crate::charset::build_chars(charset, &new.user_ranges, new.def_ascii);
                }
            }
            if let Some(speed) = scene_info.config.speed {
                new.speed = speed;
            }
            if let Some(density) = scene_info.config.density {
                new.density = density;
                new.base_density = density;
            }
        }
    }

    // Speed
    if let Some(v) = cfg.get("speed") {
        if let Ok(n) = crate::validation::parse_canonical_speed("speed", v) {
            new.speed = n;
        }
    }

    // Density
    if let Some(v) = cfg.get("density") {
        if let Ok(n) = crate::validation::parse_canonical_f32_range("density", v, 0.01, 5.0) {
            new.density = n;
            new.base_density = n;
        }
    }

    // FPS
    if let Some(v) = cfg.get("fps") {
        if let Ok(n) = crate::validation::parse_canonical_f64_range("fps", v, 1.0, 240.0) {
            new.target_fps = n;
        }
    }

    // Glitch level
    if let Some(v) = cfg.get("glitch-level") {
        new.noglitch = v.trim().eq_ignore_ascii_case("none");
    }

    // v17 mastery: legacy advanced config keys (glitchpct, shortpct, rippct,
    // maxdpc) REMOVED from live reload. These are now fully controlled by
    // --glitch-level. The old keys are silently ignored on live reload.

    // Glitch times (ms range)
    if let Some(v) = cfg.get("glitchms") {
        if let Some((lo, hi)) = parse_range(v) {
            new.glitch_low = lo;
            new.glitch_high = hi;
        }
    }

    // Linger times
    if let Some(v) = cfg.get("lingerms") {
        if let Some((lo, hi)) = parse_range(v) {
            new.linger_low = lo;
            new.linger_high = hi;
        }
    }

    // Monolith size
    if let Some(v) = cfg.get("monolith-size") {
        use clap::ValueEnum;
        if let Ok(size) = crate::runtime::MonolithSize::from_str(v, true) {
            new.monolith_size = size;
        }
    }

    // Auto color drift
    if let Some(v) = cfg.get("auto-color-drift") {
        new.auto_color_drift = v.trim() == "true";
    }

    // Monolith density map (from scene-custom blocks)
    if let Some(v) = cfg.get("scene-custom") {
        let scenes = crate::scene_custom::collect_custom_scenes(cfg);
        if let Some(scene) = scenes.get(v.trim().to_ascii_lowercase().as_str()) {
            if let Some(csv) = scene.density_map.as_deref() {
                new.monolith_density_map = crate::scene_custom::parse_density_map(csv);
            }
        }
    }

    new
}

/// Parse "LOW,HIGH" range string.
fn parse_range(s: &str) -> Option<(u16, u16)> {
    let (lo, hi) = s.split_once(',')?;
    let lo: u16 = lo.trim().parse().ok()?;
    let hi: u16 = hi.trim().parse().ok()?;
    Some((lo.min(hi), lo.max(hi)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_invalid_speed() {
        let mut cfg = HashMap::new();
        cfg.insert("speed".to_string(), "100000".to_string());
        let result = crate::testconf::validate_config_strictly(&cfg);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("speed"));
    }

    #[test]
    fn validate_rejects_invalid_density() {
        let mut cfg = HashMap::new();
        cfg.insert("density".to_string(), "99.0".to_string());
        let result = crate::testconf::validate_config_strictly(&cfg);
        assert!(result.is_err());
    }

    #[test]
    fn validate_accepts_valid_config() {
        let mut cfg = HashMap::new();
        cfg.insert("speed".to_string(), "30".to_string());
        cfg.insert("density".to_string(), "0.85".to_string());
        cfg.insert("fps".to_string(), "60".to_string());
        let result = crate::testconf::validate_config_strictly(&cfg);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_skips_block_keys() {
        let mut cfg = HashMap::new();
        cfg.insert("scene-custom.test.base".to_string(), "monolith".to_string());
        cfg.insert("speed".to_string(), "30".to_string());
        let result = crate::testconf::validate_config_strictly(&cfg);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_rejects_invalid_charset() {
        let mut cfg = HashMap::new();
        cfg.insert("charset".to_string(), "hackeres".to_string());
        let result = crate::testconf::validate_config_strictly(&cfg);
        assert!(result.is_err());
    }

    #[test]
    fn validate_rejects_invalid_atmosphere_regime() {
        let mut cfg = HashMap::new();
        cfg.insert("atmosphere-regime".to_string(), "adaptivee".to_string());
        let result = crate::testconf::validate_config_strictly(&cfg);
        assert!(result.is_err());
    }

    #[test]
    fn parse_range_handles_whitespace() {
        assert_eq!(parse_range(" 200 , 300 "), Some((200, 300)));
        assert_eq!(parse_range("300,200"), Some((200, 300)));
    }

    #[test]
    fn parse_range_rejects_invalid() {
        assert_eq!(parse_range("abc"), None);
        assert_eq!(parse_range("200"), None);
    }
}
