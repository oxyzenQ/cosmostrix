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
///
/// Bug 1 fix: when the native `notify` watcher (kqueue on BSDs, inotify on
/// Linux, FSEvents on macOS, ReadDirectoryChanges on Windows) fails to
/// initialize OR fails to register the watch path, we fall back to a
/// polling mechanism that checks the file's modification time every
/// 2 seconds. This guarantees live reload works on FreeBSD/GhostBSD and
/// any platform where the native backend is unavailable (containers with
/// restricted syscalls, network filesystems, etc.).
fn watcher_loop(path: PathBuf, tx: Sender<LiveConfigEvent>) {
    const DEBOUNCE_MS: u64 = 200;
    const POLL_INTERVAL_MS: u64 = 2000;

    let (notify_tx, notify_rx) = std::sync::mpsc::channel::<notify::Result<notify::Event>>();

    let mut watcher: Option<RecommendedWatcher> = match RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            let _ = notify_tx.send(res);
        },
        notify::Config::default(),
    ) {
        Ok(w) => Some(w),
        Err(e) => {
            eprintln!(
                "[live-reload] native watcher unavailable: {e} — falling back to polling every {POLL_INTERVAL_MS}ms"
            );
            None
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

    if let Some(ref mut w) = watcher {
        if let Err(e) = w.watch(&watch_dir, RecursiveMode::NonRecursive) {
            eprintln!(
                "[live-reload] native watcher failed to register {}: {e} — falling back to polling every {POLL_INTERVAL_MS}ms",
                watch_dir.display()
            );
            // Drop the broken watcher so the polling fallback runs.
            watcher = None;
        }
    }

    let target_file = Arc::new(path.clone());
    let mut last_event = std::time::Instant::now();

    // Native watcher path: blocks on `notify_rx.iter()`.
    if let Some(_watcher) = watcher {
        // Hold the watcher alive via `_watcher` for the loop's lifetime.
        // The watcher is dropped when this scope exits.
        for event_result in notify_rx.iter() {
            if !handle_notify_event(
                event_result,
                &target_file,
                &path,
                &tx,
                &mut last_event,
                DEBOUNCE_MS,
            ) {
                break;
            }
        }
        return;
    }

    // Polling fallback path: check mtime every POLL_INTERVAL_MS.
    eprintln!(
        "[live-reload] polling {path} every {POLL_INTERVAL_MS}ms",
        path = path.display()
    );
    let mut last_mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
    loop {
        std::thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
        let current_mtime = match std::fs::metadata(&path).and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => continue,
        };
        if Some(current_mtime) == last_mtime {
            continue;
        }
        last_mtime = Some(current_mtime);

        // Same reparse + validate pipeline as the native path.
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let parsed = configfile::parse_config_text(&content);
        if parsed.values.is_empty() && parsed.malformed_lines.is_empty() {
            continue;
        }
        if let Err(msg) = validate_and_send(&parsed, &tx) {
            eprintln!("[live-reload] {msg}");
        }
    }
}

/// Process a single notify event. Returns `false` if the channel is closed
/// and the watcher loop should exit.
fn handle_notify_event(
    event_result: notify::Result<notify::Event>,
    target_file: &Arc<PathBuf>,
    path: &PathBuf,
    tx: &Sender<LiveConfigEvent>,
    last_event: &mut std::time::Instant,
    debounce_ms: u64,
) -> bool {
    match event_result {
        Ok(event) => {
            let touches_target = event.paths.iter().any(|p| p == &**target_file);
            if !touches_target {
                return true;
            }

            let relevant = matches!(
                event.kind,
                EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
            );
            if !relevant {
                return true;
            }

            // Debounce
            let now = std::time::Instant::now();
            if now.duration_since(*last_event) < Duration::from_millis(debounce_ms) {
                return true;
            }
            *last_event = now;

            // Small delay for atomic-save rename completion.
            std::thread::sleep(Duration::from_millis(50));

            // Reparse config using parse_config_text (not load_config_file)
            // so we can check malformed_lines AND unknown_keys.
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => return true,
            };
            let parsed = configfile::parse_config_text(&content);
            if parsed.values.is_empty() && parsed.malformed_lines.is_empty() {
                return true;
            }

            if let Err(msg) = validate_and_send(&parsed, tx) {
                eprintln!("[live-reload] {msg}");
            }
            true
        }
        Err(e) => {
            eprintln!("[live-reload] watch error: {e}");
            true
        }
    }
}

/// Validate parsed config strictly, then send Ok(cfg) or Err(msg) to the
/// render thread. Returns Err(msg) if validation failed (caller logs it).
fn validate_and_send(
    parsed: &configfile::ParsedConfig,
    tx: &Sender<LiveConfigEvent>,
) -> Result<(), String> {
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
        let _ = tx.send(Err(msg.clone()));
        return Err(msg);
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
        let _ = tx.send(Err(msg.clone()));
        return Err(msg);
    }

    let cfg = &parsed.values;

    // Strict validation: reject entire config if ANY field is invalid.
    match crate::testconf::validate_config_strictly(cfg) {
        Ok(()) => {
            if tx.send(Ok(cfg.clone())).is_err() {
                return Ok(()); // channel closed — caller will exit
            }
            Ok(())
        }
        Err(msg) => {
            let _ = tx.send(Err(msg.clone()));
            Err(msg)
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
    // Bug 3 fix: snapshot the CLI-explicit tracker from the base config.
    // Each config-derived field below consults this tracker before
    // applying — CLI-explicit fields are preserved across live reload,
    // enforcing the CLI > config.toml > scene priority contract.
    let cli = new.cli_explicit.clone();

    // Color scheme — skip if CLI explicitly set --color
    if !cli.color {
        if let Some(v) = cfg.get("color") {
            if let Ok(scheme) = crate::cli::parse_color_scheme(v) {
                new.color_scheme = scheme;
            }
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

    // Charset (requires rebuilding chars vector) — skip if CLI --charset
    if !cli.charset {
        if let Some(v) = cfg.get("charset") {
            if let Ok(charset) = crate::charset::charset_from_str(v, false) {
                new.charset_preset = v.clone();
                new.chars = crate::charset::build_chars(charset, &new.user_ranges, new.def_ascii);
            }
        }
    }

    // Scene (affects rain_style) — skip if CLI --scene was explicit
    if !cli.scene {
        if let Some(v) = cfg.get("scene") {
            if let Some(scene_info) = crate::scene::get_scene(v) {
                new.rain_style = scene_info.config.rain_style;
                // Apply scene color/charset if set — but still respect
                // CLI-explicit color/charset (don't let scene override CLI).
                if let Some(color) = scene_info.config.color {
                    if !cli.color {
                        if let Ok(scheme) = crate::cli::parse_color_scheme(color) {
                            new.color_scheme = scheme;
                        }
                    }
                }
                if let Some(charset_name) = scene_info.config.charset {
                    if !cli.charset {
                        if let Ok(charset) = crate::charset::charset_from_str(charset_name, false) {
                            new.charset_preset = charset_name.to_string();
                            new.chars = crate::charset::build_chars(
                                charset,
                                &new.user_ranges,
                                new.def_ascii,
                            );
                        }
                    }
                }
                if let Some(speed) = scene_info.config.speed {
                    if !cli.speed {
                        new.speed = speed;
                    }
                }
                if let Some(density) = scene_info.config.density {
                    if !cli.density {
                        new.density = density;
                        new.base_density = density;
                    }
                }
            }
        }
    }

    // Speed — skip if CLI --speed was explicit
    if !cli.speed {
        if let Some(v) = cfg.get("speed") {
            if let Ok(n) = crate::validation::parse_canonical_speed("speed", v) {
                new.speed = n;
            }
        }
    }

    // Density — skip if CLI --density was explicit
    if !cli.density {
        if let Some(v) = cfg.get("density") {
            if let Ok(n) = crate::validation::parse_canonical_f32_range("density", v, 0.01, 5.0) {
                new.density = n;
                new.base_density = n;
            }
        }
    }

    // FPS — skip if CLI --fps was explicit
    if !cli.fps {
        if let Some(v) = cfg.get("fps") {
            if let Ok(n) = crate::validation::parse_canonical_f64_range("fps", v, 1.0, 240.0) {
                new.target_fps = n;
            }
        }
    }

    // Glitch level — skip if CLI --glitch-level was explicit
    if !cli.glitch_level {
        if let Some(v) = cfg.get("glitch-level") {
            new.noglitch = v.trim().eq_ignore_ascii_case("none");
        }
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

    // v17 mastery: scene-custom selector key REMOVED from config.toml.
    // Density map is now loaded only via --scene-custom CLI flag in main.rs.
    // Live reload no longer reads the 'scene-custom' config key.

    // v20: scene-custom live reload. If a custom scene is active (set via
    // --scene-custom <name>), re-apply its fields from the new config so
    // editing [scene-custom.<name>] takes effect immediately. Color/charset
    // changes are applied to the CloudConfig; density-map changes will be
    // picked up by main.rs's monolith_density_map re-resolution on the next
    // Cloud rebuild (the watcher path in event_loop.rs already calls
    // create_cloud + cloud.reset()).
    if let Some(ref custom_name) = base.scene_custom_name {
        apply_scene_custom_to_cloud_config(&mut new, cfg, custom_name);
    }

    new
}

/// Apply a `[scene-custom.<name>]` block from config to a CloudConfig in
/// place. Used by live reload so edits to a custom scene take effect
/// without restarting cosmostrix.
///
/// Fields override the CloudConfig directly. Missing fields are left
/// unchanged (the CloudConfig retains whatever it had before the reload).
/// This mirrors the startup-time behavior in `apply_profile_overrides`
/// but operates on CloudConfig instead of Args.
///
/// v20.1: `base-scene` and `preset` are no longer recognized fields, so
/// they never reach this function — `is_scene_custom_config_key` rejects
/// them and `collect_custom_scenes` skips them. They will instead show up
/// as unknown keys via `--testconf`.
fn apply_scene_custom_to_cloud_config(
    new: &mut crate::app::CloudConfig,
    cfg: &HashMap<String, String>,
    name: &str,
) {
    use clap::ValueEnum;

    let normalized = name.trim().to_ascii_lowercase();
    let prefix = format!("scene-custom.{normalized}.");
    let mut touched_any = false;

    for (key, value) in cfg {
        let Some(field) = key.strip_prefix(&prefix) else {
            continue;
        };
        // v20.1: base-scene and preset are no longer recognized; they are
        // filtered upstream by is_scene_custom_config_key. Unknown fields
        // are ignored here.
        touched_any = true;
        match field {
            "color" => {
                if let Ok(scheme) = crate::cli::parse_color_scheme(value) {
                    new.color_scheme = scheme;
                }
            }
            "charset" => {
                if let Ok(charset) = crate::charset::charset_from_str(value, false) {
                    new.charset_preset = value.clone();
                    new.chars =
                        crate::charset::build_chars(charset, &new.user_ranges, new.def_ascii);
                }
            }
            "fps" => {
                if let Ok(n) =
                    crate::validation::parse_canonical_f64_range("fps", value, 1.0, 240.0)
                {
                    new.target_fps = n;
                }
            }
            "speed" => {
                if let Ok(n) = crate::validation::parse_canonical_speed("speed", value) {
                    new.speed = n;
                }
            }
            "density" => {
                if let Ok(n) =
                    crate::validation::parse_canonical_f32_range("density", value, 0.01, 5.0)
                {
                    new.density = n;
                    new.base_density = n;
                }
            }
            "glitch-level" => {
                new.noglitch = value.trim().eq_ignore_ascii_case("none");
            }
            "monolith-size" => {
                if let Ok(size) = crate::runtime::MonolithSize::from_str(value, true) {
                    new.monolith_size = size;
                }
            }
            "density-map" => {
                // Re-parse density map from new config. parse_density_map
                // leaks the Vec so it lives for the process lifetime — this
                // is intentional (bounded by config size, a few KB total).
                if let Some(map) = crate::scene_custom::parse_density_map(value) {
                    new.monolith_density_map = Some(map);
                }
            }
            // color-bg / atmosphere-mode / atmosphere-regime are not yet
            // applied to CloudConfig at runtime; they remain as-is from the
            // startup-time resolution. Future work can wire them in.
            "color-bg" | "atmosphere-mode" | "atmosphere-regime" => {}
            _ => {}
        }
    }

    if touched_any {
        // The scene_name stays as the custom scene name (already set at
        // startup). No need to re-assign — custom scenes are first-class
        // citizens and their identity doesn't change on live reload.
        eprintln!("[live-reload] scene-custom '{normalized}': re-applied fields from config");
    }
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
        cfg.insert(
            "scene-custom.test.base-scene".to_string(),
            "monolith".to_string(),
        );
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

    // ── v20: scene-custom live reload tests ──

    /// Build a minimal CloudConfig for testing rebuild_cloud_config.
    fn minimal_cloud_config() -> crate::app::CloudConfig {
        use crate::atmosphere_apply::{AtmosphereApplicationMode, AtmosphereRuntimeModulation};
        use crate::rain_style::RainStyle;
        use crate::runtime::{BoldMode, ColorMode, ColorScheme, MonolithSize, ShadingMode};

        crate::app::CloudConfig {
            color_mode: ColorMode::TrueColor,
            fullwidth: false,
            shading_mode: ShadingMode::Random,
            bold_mode: BoldMode::Random,
            async_mode: true,
            default_bg: true,
            color_scheme: ColorScheme::NeonPurple,
            custom_palette: None,
            custom_palette_name: None,
            rain_style: RainStyle::Glyph,
            noglitch: false,
            glitch_pct: 10.0,
            glitch_low: 300,
            glitch_high: 400,
            linger_low: 400,
            linger_high: 600,
            short_pct: 50.0,
            die_early_pct: 33.0,
            max_dpc: 5,
            density: 0.75,
            speed: 9.0,
            monolith_size: MonolithSize::Normal,
            chars: vec!['0', '1'],
            message: None,
            message_border: false,
            target_fps: 60.0,
            duration: None,
            duration_s: None,
            bench_frames: None,
            benchmark: false,
            bench_duration: None,
            screen_size: None,
            color_tune: crate::color_tune::ColorTune::IDENTITY,
            json: false,
            save_baseline: None,
            compare_baseline: None,
            bench_io: false,
            bench_all: false,
            verbose: false,
            density_auto: true,
            base_density: 0.75,
            perf_stats: false,
            screensaver: false,
            intro: crate::config::IntroType::None,
            mouse: false,
            charset_preset: "binary".to_string(),
            user_ranges: vec![],
            def_ascii: false,
            auto_color_drift: false,
            atmosphere_modulation: AtmosphereRuntimeModulation::identity(),
            atmosphere_mode: AtmosphereApplicationMode::Disabled,
            monolith_density_map: None,
            config_path_for_watcher: None,
            scene_name: "test-scene".to_string(),
            scene_custom_name: Some("test-scene".to_string()),
            cli_explicit: crate::app::CliExplicit::default(),
        }
    }

    #[test]
    fn rebuild_applies_scene_custom_color_change() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "scene-custom.test-scene.color".to_string(),
            "green".to_string(),
        );
        let base = minimal_cloud_config();
        let new = rebuild_cloud_config(&base, &cfg);
        assert_eq!(
            new.color_scheme,
            crate::runtime::ColorScheme::Green,
            "live reload must apply scene-custom color change"
        );
        // scene_name must remain the custom scene name (not be overwritten).
        assert_eq!(new.scene_name, "test-scene");
    }

    #[test]
    fn rebuild_applies_scene_custom_speed_and_density_changes() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "scene-custom.test-scene.speed".to_string(),
            "24".to_string(),
        );
        cfg.insert(
            "scene-custom.test-scene.density".to_string(),
            "0.50".to_string(),
        );
        let base = minimal_cloud_config();
        let new = rebuild_cloud_config(&base, &cfg);
        assert_eq!(new.speed, 24.0);
        assert!((new.density - 0.50).abs() < f32::EPSILON);
        assert!((new.base_density - 0.50).abs() < f32::EPSILON);
    }

    #[test]
    fn rebuild_applies_scene_custom_density_map_change() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "scene-custom.test-scene.density-map".to_string(),
            "1.0,0.5,0.0,0.8".to_string(),
        );
        let base = minimal_cloud_config();
        let new = rebuild_cloud_config(&base, &cfg);
        let map = new
            .monolith_density_map
            .expect("density-map must be parsed and applied");
        assert_eq!(map.len(), 4);
        assert_eq!(map[0], 1.0);
        assert_eq!(map[2], 0.0);
    }

    #[test]
    fn rebuild_ignores_scene_custom_base_scene_unknown_field() {
        // v20.1: `base-scene` is no longer a recognized field. The
        // apply_scene_custom_to_cloud_config helper iterates raw cfg keys
        // (including unknown ones) but only acts on known fields, so the
        // color_scheme should remain the base's NeonPurple.
        let mut cfg = HashMap::new();
        cfg.insert(
            "scene-custom.test-scene.base-scene".to_string(),
            "storm".to_string(),
        );
        let mut base = minimal_cloud_config();
        base.scene_custom_name = Some("test-scene".to_string());
        let new = rebuild_cloud_config(&base, &cfg);
        assert_eq!(
            new.color_scheme,
            crate::runtime::ColorScheme::NeonPurple,
            "base-scene must have no effect"
        );
    }

    #[test]
    fn rebuild_without_scene_custom_name_does_not_apply_custom_fields() {
        // When scene_custom_name is None (no --scene-custom active), the
        // scene-custom.* keys in config must NOT be applied — they belong
        // to a different scene and could clobber the active one.
        let mut cfg = HashMap::new();
        cfg.insert(
            "scene-custom.other-scene.color".to_string(),
            "green".to_string(),
        );
        let mut base = minimal_cloud_config();
        base.scene_custom_name = None;
        let new = rebuild_cloud_config(&base, &cfg);
        assert_eq!(
            new.color_scheme,
            crate::runtime::ColorScheme::NeonPurple,
            "scene-custom fields must not apply when no custom scene is active"
        );
    }

    /// Bug 3 test: CLI-explicit color must NOT be overridden by config.toml
    /// during live reload. The priority contract is CLI > config.toml > scene.
    /// Without the `cli_explicit` tracker, `rebuild_cloud_config` would
    /// blindly apply `color = "snow"` from config, clobbering the user's
    /// `-c green` CLI flag.
    #[test]
    fn rebuild_preserves_cli_explicit_color_over_config() {
        let mut cfg = HashMap::new();
        cfg.insert("color".to_string(), "snow".to_string());
        let mut base = minimal_cloud_config();
        // Simulate the user running `cosmostrix -c green`: the CLI flag
        // is recorded as explicit, and the color_scheme is set to Green.
        base.cli_explicit.color = true;
        base.color_scheme = crate::runtime::ColorScheme::Green;
        let new = rebuild_cloud_config(&base, &cfg);
        assert_eq!(
            new.color_scheme,
            crate::runtime::ColorScheme::Green,
            "CLI --color green must NOT be overridden by config.toml color=snow"
        );
    }

    /// Bug 3 test: when CLI did NOT explicitly set a field, config.toml
    /// overrides scene defaults. This is the normal live-reload path.
    #[test]
    fn rebuild_applies_config_color_when_cli_not_explicit() {
        let mut cfg = HashMap::new();
        cfg.insert("color".to_string(), "snow".to_string());
        let base = minimal_cloud_config();
        // base.cli_explicit.color is false (default) — no CLI override.
        let new = rebuild_cloud_config(&base, &cfg);
        assert_eq!(
            new.color_scheme,
            crate::runtime::ColorScheme::Snow,
            "config.toml color=snow must apply when CLI did not set --color"
        );
    }

    /// Bug 3 test: CLI-explicit speed must NOT be overridden by scene's
    /// speed default during live reload (e.g., user runs `cosmostrix -s 25`,
    /// config.toml has `scene = "matrix"` which sets speed=18 — CLI wins).
    #[test]
    fn rebuild_preserves_cli_explicit_speed_over_scene() {
        let mut cfg = HashMap::new();
        cfg.insert("scene".to_string(), "matrix".to_string());
        let mut base = minimal_cloud_config();
        base.cli_explicit.speed = true;
        base.cli_explicit.scene = false;
        base.speed = 25.0;
        let new = rebuild_cloud_config(&base, &cfg);
        assert_eq!(
            new.speed, 25.0,
            "CLI --speed 25 must NOT be overridden by scene=matrix speed=18"
        );
    }
}
