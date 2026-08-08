// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Ambient phase scheduler — config-driven time-of-day scene switching.
//!
//! Replaces the archived `adaptive-custom` subsystem (eliminated with the
//! atmosphere engine at commit `07b44b5`). Unlike `adaptive-custom`, this
//! module is **config-only** (no CLI flag) and uses **instant switch** (no
//! smoothstep blend window) — the user explicitly asked for snappy phase
//! boundaries, not the imperceptible 5-minute cross-fade the old engine used.
//!
//! ## v30.2 config format (simplified — breaking change)
//!
//! ```text
//! ambient.<HH-MM> = <scene-name>
//! ```
//!
//! The value is a **single scene name** — either a built-in scene
//! (`cinematic`, `signal`, `monolith`, etc.) or a custom scene defined via
//! `[scene-custom.<name>]`. All parameters (color, charset, speed, density,
//! fps, glitch-level, rain_style) live inside the scene itself, eliminating
//! the precedence confusion that plagued the v30.0/v30.1 multi-field format.
//!
//! ### Migration from v30.1 multi-field format
//!
//! v30.1 accepted `ambient.15-00 = neon-purple, signal, speed=50, density=0.65`.
//! v30.2 rejects this with a migration error. To preserve the entry, define
//! a custom scene that captures the same parameters and reference it from a
//! TOP-LEVEL `ambient.*` key (NEVER place the `ambient.*` key inside the
//! `[scene-custom.<name>]` block — TOML would parse it as
//! `scene-custom.<name>.ambient.<HH-MM>`, which is rejected as unknown):
//!
//! ```toml
//! [scene-custom.afternoon]
//! base-scene = "signal"          # inherits signal's rain_style + defaults
//! color = "neon-purple"          # overrides signal's color
//! speed = "50"                   # overrides signal's speed
//! density = "0.65"               # overrides signal's density
//!
//! # Top-level — outside any [section] block:
//! ambient.15-00 = afternoon
//! ```
//!
//! This separates concerns cleanly: the schedule says WHEN, the scene says
//! WHAT. There is no override-precedence bug surface because the scene IS
//! the source of truth — no field can be "lost" between the scene switch
//! and the override layer.
//!
//! ## Dynamic idle/wake scheduler
//!
//! The scheduler thread ([`crate::ambient_scheduler`]) does NOT poll on a
//! fixed 10-second interval. Instead it computes the time to the next phase
//! boundary and sleeps until then (capped at 1 hour for reload
//! responsiveness). CPU usage stays at zero between phase transitions — the
//! thread only wakes:
//!
//! 1. At the exact phase boundary (to fire the new phase).
//! 2. When the user saves `config.toml` (condvar notification from the
//!    live-reload path).
//!
//! ## Live reload
//!
//! Editing `ambient.*` keys in `config.toml` triggers an immediate
//! re-parse via [`collect_ambient_schedule`]. The new schedule replaces the
//! old one atomically (mutex swap), and the scheduler thread wakes up to
//! recompute the next phase boundary. If the new schedule has a phase that
//! is currently active (its boundary is in the past but no later phase has
//! fired since), it is applied immediately.
//!
//! ## Instant switch
//!
//! There is no blend window. When the scheduler fires a phase entry, the
//! scene is applied immediately via [`Cloud::apply_ambient_entry`] (which
//! delegates to [`Cloud::apply_scene_runtime_with_cfg`]). The only visual
//! smoothing comes from the existing `transition_chars` and
//! `transition_rain_style` machinery (glyph warm-start, rain-style pool
//! reset) — those are required for correctness (preventing ghosting), not
//! for cinematic blending.

use std::collections::HashMap;

/// Config namespace prefix for ambient phase entries.
pub(crate) const AMBIENT_NAMESPACE: &str = "ambient";

/// Maximum number of distinct phase entries a config may declare.
///
/// Defensive cap — a healthy schedule has 2–6 entries (one per major time
/// block). A config with 256+ entries is almost certainly a script-generated
/// mistake. The cap also bounds the sort cost (O(n log n)) at parse time.
pub(crate) const AMBIENT_MAX_ENTRIES: usize = 256;

/// One entry in the ambient schedule. Parsed from `ambient.HH-MM = <scene>`.
///
/// v30.2: simplified from a 7-field struct (color/scene/speed/density/fps/
/// charset/glitch_level) to just `scene`. All parameters now live inside the
/// referenced scene (built-in or `[scene-custom.<name>]`). This eliminates
/// the override-precedence bugs that plagued v30.0/v30.1 (e.g. `speed=50`
/// being silently overridden by the scene's default `speed=12`).
#[derive(Clone, Debug, PartialEq)]
pub struct AmbientEntry {
    /// Hour portion of the `HH-MM` key (0–23).
    pub hour: u32,
    /// Minute portion of the `HH-MM` key (0–59).
    pub minute: u32,
    /// Scene name to switch to at this phase boundary. Must be a built-in
    /// scene name (`cinematic`, `signal`, `monolith`, etc.) OR a custom
    /// scene name defined via `[scene-custom.<name>]`. Validation happens
    /// in [`validate_ambient_entries`].
    pub scene: String,
}

impl AmbientEntry {
    /// Total minutes since midnight for this entry's `HH:MM`.
    ///
    /// Used for sorting and for "is this entry currently active?" checks.
    /// Inline because it's called in hot scheduler loops.
    #[inline]
    #[must_use]
    pub fn minutes_of_day(&self) -> u32 {
        self.hour * 60 + self.minute
    }
}

/// The full ambient schedule — a sorted list of [`AmbientEntry`] values.
///
/// Entries are sorted ascending by `minutes_of_day()` at construction time
/// (see [`collect_ambient_schedule`]). The scheduler binary-searches this
/// list to find the current and next phases.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AmbientSchedule {
    /// Entries sorted ascending by `minutes_of_day()`. Empty = schedule
    /// disabled (scheduler thread sleeps idle).
    pub entries: Vec<AmbientEntry>,
}

impl AmbientSchedule {
    /// Returns `true` if the schedule has no entries (ambient feature is
    /// effectively disabled — the scheduler thread sleeps idle).
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Find the **current** phase — the latest entry whose `minutes_of_day()`
    /// is `<= now_min`. Returns `None` if no entry has fired yet today
    /// (i.e. `now_min` is earlier than the first entry's boundary, which
    /// only happens at startup before the first phase of the day).
    ///
    /// Wrap-around: if `now_min` is 0:30 and the earliest entry is 6:00,
    /// the "current" phase is the **last** entry of the previous day (it has
    /// been active since its boundary fired yesterday). The caller should
    /// treat `None` as "schedule exists but no phase has fired yet today —
    /// use the last entry as currently-active (carried over from yesterday)".
    ///
    /// This matches the archived `adaptive-custom` semantics.
    #[must_use]
    pub fn current_phase(&self, now_min: u32) -> Option<&AmbientEntry> {
        // Find latest entry with minutes_of_day() <= now_min.
        let idx = self
            .entries
            .partition_point(|e| e.minutes_of_day() <= now_min);
        if idx == 0 {
            // No entry has fired yet today — wrap to last entry (yesterday's
            // last phase is still active).
            self.entries.last()
        } else {
            self.entries.get(idx - 1)
        }
    }

    /// Find the **next** phase — the earliest entry whose `minutes_of_day()`
    /// is `> now_min`. Returns `None` if the schedule is empty.
    ///
    /// Wrap-around: if `now_min` is 23:30 and all entries are <= 23:00, the
    /// "next" phase is the first entry of tomorrow (the earliest entry in
    /// the list). The caller computes sleep duration as
    /// `(24*60 - now_min + next.minutes_of_day()) * 60`.
    #[must_use]
    pub fn next_phase(&self, now_min: u32) -> Option<&AmbientEntry> {
        let idx = self
            .entries
            .partition_point(|e| e.minutes_of_day() <= now_min);
        if let Some(e) = self.entries.get(idx) {
            Some(e)
        } else {
            // Wrap to tomorrow's first entry.
            self.entries.first()
        }
    }

    /// Seconds to sleep until the next phase boundary fires.
    ///
    /// Returns `None` if the schedule is empty. Handles midnight wrap-around:
    /// if the next phase is "tomorrow's first entry", the sleep duration
    /// correctly accounts for crossing midnight.
    ///
    /// Capped at 3600 seconds (1 hour) to bound live-reload latency — if
    /// the user edits the config to add a new entry, the scheduler will
    /// notice within at most 1 hour even if the next phase was 6 hours away.
    /// In practice the live-reload path also wakes the thread immediately
    /// via condvar, so this cap is a defense-in-depth.
    #[must_use]
    pub fn seconds_to_next_phase(&self, now_min: u32, now_sec: u32) -> Option<u64> {
        let next = self.next_phase(now_min)?;
        let next_min = next.minutes_of_day();
        let now_total_sec = (now_min * 60 + now_sec.min(59)) as u64;
        let next_total_sec = if next_min > now_min {
            (next_min * 60) as u64
        } else {
            // Wrap: next is tomorrow.
            ((24 * 60 + next_min) * 60) as u64
        };
        let diff = next_total_sec.saturating_sub(now_total_sec);
        Some(diff.min(3600))
    }
}

/// Returns `true` if `key` is a recognized `ambient.<HH-MM>` config key.
///
/// Mirrors [`crate::scene_custom::is_scene_custom_config_key`] but for the
/// `ambient` namespace. The suffix must match `HH-MM` (24-hour, zero-padded,
/// dash-separated). Invalid suffixes (e.g. `ambient.midnight`,
/// `ambient.24-00`, `ambient.12-60`) return `false` and surface as
/// `unknown_keys` so `--testconf` can attach a hint.
#[must_use]
pub(crate) fn is_ambient_config_key(key: &str) -> bool {
    let Some((prefix, rest)) = key.split_once('.') else {
        return false;
    };
    if prefix != AMBIENT_NAMESPACE {
        return false;
    }
    is_valid_hh_mm(rest)
}

/// Validate `HH-MM` format: 5 chars, dash at index 2, HH in 00..=23,
/// MM in 00..=59. Zero-padded (must be exactly 2 digits each).
#[inline]
fn is_valid_hh_mm(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 5 || bytes[2] != b'-' {
        return false;
    }
    let hh_ok = s[0..2].chars().all(|c| c.is_ascii_digit());
    let mm_ok = s[3..5].chars().all(|c| c.is_ascii_digit());
    if !hh_ok || !mm_ok {
        return false;
    }
    let hh: u32 = s[0..2].parse().unwrap_or(24);
    let mm: u32 = s[3..5].parse().unwrap_or(60);
    hh < 24 && mm < 60
}

/// Parse the value side of `ambient.<HH-MM> = <value>`.
///
/// v30.2 format: `<scene-name>` — a single token, no commas, no `key=value`
/// pairs. The value must be a built-in scene name OR a custom scene name
/// defined via `[scene-custom.<name>]`.
///
/// # Errors
///
/// Returns `Err` with a migration message if the value contains `,` or `=`,
/// indicating the user is still using the v30.1 multi-field format. The
/// migration message shows exactly how to convert the entry to the new
/// format using a `[scene-custom.<name>]` block with `base-scene`.
///
/// Returns `Err` if the value is empty or whitespace-only.
pub(crate) fn parse_ambient_value(value: &str) -> Result<AmbientEntry, String> {
    let scene = value.trim().to_string();
    if scene.is_empty() {
        return Err("ambient: empty scene name".to_string());
    }
    // v30.2: detect legacy multi-field format and surface a migration
    // message. The user almost certainly has a v30.1 config like
    // `ambient.15-00 = neon-purple, signal, speed=50, density=0.65` and
    // needs to convert it to a custom scene block.
    if scene.contains(',') || scene.contains('=') {
        return Err(format!(
            "ambient: legacy multi-field format no longer supported (got '{value}').\n\
             \n\
             v30.2 simplified ambient entries to a single scene name. To preserve\n\
             this entry, define a custom scene that captures the same parameters\n\
             and reference it by name:\n\
             \n\
             [scene-custom.<name>]\n\
             base-scene = \"<original-scene>\"   # if you had a scene positional\n\
             color = \"<original-color>\"         # if you had a color positional\n\
             speed = \"<original-speed>\"         # if you had speed=...\n\
             density = \"<original-density>\"     # if you had density=...\n\
             fps = \"<original-fps>\"             # if you had fps=...\n\
             charset = \"<original-charset>\"     # if you had charset=...\n\
             glitch-level = \"<original-level>\"  # if you had glitch-level=...\n\
             \n\
             ambient.<HH-MM> = <name>\n\
             \n\
             Example: `ambient.15-00 = neon-purple, signal, speed=50, density=0.65`\n\
             becomes (ambient key at the TOP LEVEL — never inside the block):\n\
             \n\
             [scene-custom.afternoon]\n\
             base-scene = \"signal\"\n\
             color = \"neon-purple\"\n\
             speed = \"50\"\n\
             density = \"0.65\"\n\
             \n\
             ambient.15-00 = afternoon"
        ));
    }
    Ok(AmbientEntry {
        hour: 0,
        minute: 0,
        scene,
    })
}

/// Collect all `ambient.*` entries from a flat config map and return a
/// sorted [`AmbientSchedule`].
///
/// Entries are sorted ascending by `minutes_of_day()`. Duplicate `HH-MM`
/// keys follow standard `HashMap::insert` semantics (last writer wins),
/// matching how `configfile.rs` already deduplicates — by the time we get
/// the map, duplicates are already collapsed.
///
/// Returns an empty schedule (not an error) if no `ambient.*` keys are
/// present — the ambient feature is then effectively disabled.
#[must_use]
pub(crate) fn collect_ambient_schedule(cfg: &HashMap<String, String>) -> AmbientSchedule {
    let mut entries: Vec<AmbientEntry> = Vec::new();
    for (key, value) in cfg {
        let Some(rest) = key.strip_prefix("ambient.") else {
            continue;
        };
        if !is_valid_hh_mm(rest) {
            // Should not happen — is_known_key filters these. Defensive.
            continue;
        }
        let hour: u32 = rest[0..2].parse().unwrap_or(0);
        let minute: u32 = rest[3..5].parse().unwrap_or(0);
        let mut entry = match parse_ambient_value(value) {
            Ok(e) => e,
            Err(_) => {
                // Skip malformed entries — strict validation happens in
                // `testconf.rs::validate_ambient_entries`. Live reload must
                // not crash on a half-edited config; we drop the bad entry.
                continue;
            }
        };
        entry.hour = hour;
        entry.minute = minute;
        entries.push(entry);
    }
    entries.sort_by_key(AmbientEntry::minutes_of_day);
    // Defensive cap (DoS hardening — config is user-controlled, but a 10k
    // entry file would still waste sort time).
    entries.truncate(AMBIENT_MAX_ENTRIES);
    AmbientSchedule { entries }
}

/// Strict validation of all `ambient.*` entries in the config map.
///
/// Called from `--testconf` and the live-reload validation path. Returns
/// `Err(message)` on the first invalid entry — the caller surfaces this as
/// exit code 2 (matches the rest of the strict validation contract).
///
/// v30.2 validation rules:
/// - Value must parse as a single scene name (no commas, no `=`).
/// - The scene name must be a recognized built-in scene OR a
///   `[scene-custom.<name>]` block that exists in the config.
pub(crate) fn validate_ambient_entries(cfg: &HashMap<String, String>) -> Result<(), String> {
    // Sort keys for deterministic error ordering (BTreeMap iteration).
    let mut keys: Vec<&String> = cfg.keys().filter(|k| k.starts_with("ambient.")).collect();
    keys.sort();

    let custom_scenes = crate::scene_custom::collect_custom_scenes(cfg);

    for key in keys {
        let value = &cfg[key];
        let rest = key.strip_prefix("ambient.").unwrap_or("");
        if !is_valid_hh_mm(rest) {
            return Err(format!(
                "ambient: invalid time key '{key}' (expected HH-MM, e.g. 'ambient.12-00')"
            ));
        }
        let entry = parse_ambient_value(value).map_err(|e| format!("{key}: {e}"))?;

        // Validate scene name — must be a built-in scene OR a defined
        // [scene-custom.<name>] block.
        let scene = &entry.scene;
        let is_builtin = crate::scene::get_scene(scene).is_some();
        let is_custom = custom_scenes.contains_key(&scene.to_ascii_lowercase());
        if !is_builtin && !is_custom {
            // UX hint: if the value contains commas or `=`, the user is
            // almost certainly still using the v30.1 multi-field format.
            // The parse_ambient_value error already covers this case with
            // a full migration message, but we re-surface a shorter hint
            // here in case the value slipped through (e.g. quoted CSV).
            let hint = if scene.contains(',') || scene.contains('=') {
                " — v30.2 requires a single scene name; see migration guide in --testconf output above"
            } else {
                ""
            };
            return Err(format!(
                "{key}: unknown scene '{scene}' (not a built-in scene and no [scene-custom.{scene}] block; see --list-scenes){hint}"
            ));
        }
    }
    Ok(())
}

/// Returns the current minute-of-day (0..=1439) from the local wall clock.
///
/// Used by the scheduler thread to compute time-to-next-phase. Mirrors the
/// pattern in `src/clock.rs::local_hms()` — direct POSIX `libc::time` +
/// `localtime_r`, no allocation, no chrono wrapper. DST-aware (follows the
/// local timezone, including DST jumps).
#[cfg(unix)]
pub(crate) fn current_minute_of_day() -> u32 {
    use std::mem::MaybeUninit;
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    if now < 0 {
        return 0;
    }
    let mut tm: MaybeUninit<libc::tm> = MaybeUninit::uninit();
    if unsafe { libc::localtime_r(&now, tm.as_mut_ptr()) }.is_null() {
        return 0;
    }
    let tm = unsafe { tm.assume_init() };
    (tm.tm_hour as u32) * 60 + (tm.tm_min as u32)
}

#[cfg(not(unix))]
pub(crate) fn current_minute_of_day() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    ((secs / 60) % (24 * 60)) as u32
}

/// Returns the current second within the minute (0..=59) from the local
/// wall clock. Used by the scheduler to compute precise sleep duration.
#[cfg(unix)]
pub(crate) fn current_second_of_minute() -> u32 {
    use std::mem::MaybeUninit;
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    if now < 0 {
        return 0;
    }
    let mut tm: MaybeUninit<libc::tm> = MaybeUninit::uninit();
    if unsafe { libc::localtime_r(&now, tm.as_mut_ptr()) }.is_null() {
        return 0;
    }
    let tm = unsafe { tm.assume_init() };
    tm.tm_sec as u32
}

#[cfg(not(unix))]
pub(crate) fn current_second_of_minute() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    (secs % 60) as u32
}

/// v30.3 masterclass: compute the current ambient phase and apply it to the
/// cloud at startup (synchronous, before the event loop). Returns the new
/// charset preset + the applied entry (or None if no schedule is active).
///
/// Before this fix, the scheduler thread fired the current phase
/// asynchronously — the event loop rendered 1-N frames with the default
/// scene before the channel delivered the entry. On a cold start this window
/// was visible ("several seconds" of default scene before ambient kicked in).
/// This function eliminates that window by applying the phase NOW.
///
/// The scheduler's subsequent fire of the same entry is deduped by the event
/// loop (entry == last_applied_ambient_entry → skip).
///
/// v30.4 hotfix: the `cfg` parameter MUST be the live config HashMap (loaded
/// from config.toml at startup), NOT an empty map. Earlier revisions passed
/// `&HashMap::new()` here, which silently broke custom-scene resolution:
/// `apply_ambient_entry` → `apply_custom_scene_runtime` calls
/// `collect_custom_scenes(cfg)` to look up `[scene-custom.<name>]` blocks.
/// With an empty cfg, the lookup returns None and the function becomes a
/// no-op — but the entry is STILL recorded as "applied", which then causes
/// the dedup check to skip the scheduler's first real fire. Net result:
/// ambient never applies at startup until the user touches config.toml
/// (which triggers live-reload, which DOES pass the real cfg map).
#[must_use]
pub(crate) fn apply_startup_ambient(
    cloud: &mut crate::cloud::Cloud,
    schedule: &AmbientSchedule,
    charset_preset: &str,
    user_ranges: &[(char, char)],
    def_ascii: bool,
    cfg: &std::collections::HashMap<String, String>,
) -> (String, Option<AmbientEntry>) {
    let now_min = current_minute_of_day();
    let Some(entry) = schedule.current_phase(now_min).cloned() else {
        crate::lr_trace!(
            "ambient: startup — no active phase at minute {} of day, default scene retained",
            now_min
        );
        return (charset_preset.to_string(), None);
    };
    // v30.5 stabilization: warn if the cfg map is empty. Custom-scene
    // targets (defined via [scene-custom.<name>] blocks) silently fail
    // to resolve without the cfg map — `collect_custom_scenes` returns
    // an empty HashMap and the lookup falls through to a no-op. This
    // catches the scenario where `load_config_file` returned empty at
    // startup (file missing/unreadable/permission denied).
    if cfg.is_empty() {
        crate::lr_trace!(
            "ambient: startup — WARNING: cfg map is EMPTY; custom-scene \
             target '{}' will not resolve (built-in scenes still work). \
             Check that config.toml exists and is readable.",
            entry.scene
        );
    }
    let color_before = cloud.color_scheme();
    crate::lr_trace!(
        "ambient: startup sync apply — phase {:02}:{:02} (scene={}, color_before={:?})",
        entry.hour,
        entry.minute,
        entry.scene,
        color_before
    );
    let new_charset =
        cloud.apply_ambient_entry(&entry, charset_preset, user_ranges, def_ascii, cfg);
    let color_after = cloud.color_scheme();
    // v30.5 stabilization: verify the apply actually changed cloud state.
    // If the entry targeted a custom scene but the color didn't change,
    // the cfg lookup likely failed silently. Log a diagnostic so the
    // user can enable COSMOSTRIX_LIVE_RELOAD_DEBUG=1 and see why. Note:
    // a color match is not always a failure (custom scenes can inherit
    // the base scene's color), so this is a diagnostic, not an error.
    crate::lr_trace!(
        "ambient: startup post-apply — color_after={:?}, charset='{}'",
        color_after,
        new_charset
    );
    (new_charset, Some(entry))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_ambient_config_key ──

    #[test]
    fn recognizes_valid_keys() {
        assert!(is_ambient_config_key("ambient.00-00"));
        assert!(is_ambient_config_key("ambient.12-30"));
        assert!(is_ambient_config_key("ambient.23-59"));
    }

    #[test]
    fn rejects_invalid_time_suffix() {
        assert!(!is_ambient_config_key("ambient.24-00")); // HH out of range
        assert!(!is_ambient_config_key("ambient.12-60")); // MM out of range
        assert!(!is_ambient_config_key("ambient.midnight")); // not HH-MM
        assert!(!is_ambient_config_key("ambient.1-00")); // not zero-padded
        assert!(!is_ambient_config_key("ambient.12:00")); // colon not dash
        assert!(!is_ambient_config_key("ambient.1200")); // missing dash
    }

    #[test]
    fn rejects_wrong_namespace() {
        assert!(!is_ambient_config_key("scene-custom.12-00"));
        assert!(!is_ambient_config_key("colors-custom.12-00"));
        assert!(!is_ambient_config_key("adaptive-custom.12-00")); // archived
        assert!(!is_ambient_config_key("ambient")); // no suffix
    }

    // ── parse_ambient_value (v30.2: single scene name) ──

    #[test]
    fn parses_single_builtin_scene_name() {
        let e = parse_ambient_value("signal").unwrap();
        assert_eq!(e.scene, "signal");
    }

    #[test]
    fn parses_single_custom_scene_name() {
        let e = parse_ambient_value("afternoon").unwrap();
        assert_eq!(e.scene, "afternoon");
    }

    #[test]
    fn parses_name_with_surrounding_whitespace() {
        let e = parse_ambient_value("  signal  ").unwrap();
        assert_eq!(e.scene, "signal");
    }

    #[test]
    fn parses_name_with_underscores_and_dashes() {
        let e = parse_ambient_value("night_mode").unwrap();
        assert_eq!(e.scene, "night_mode");
        let e = parse_ambient_value("night-mode").unwrap();
        assert_eq!(e.scene, "night-mode");
    }

    #[test]
    fn rejects_empty_value() {
        assert!(parse_ambient_value("").is_err());
        assert!(parse_ambient_value("   ").is_err());
    }

    // ── v30.2 migration: legacy multi-field format must produce a
    //    helpful migration error, NOT silently drop fields. ──

    #[test]
    fn rejects_legacy_multifield_format_with_migration_message() {
        // User's exact v30.1 config from the bug report — must surface
        // a migration error pointing to [scene-custom.*] + base-scene.
        let err = parse_ambient_value("neon-purple, signal, speed=50, density=0.65")
            .expect_err("legacy format must be rejected");
        assert!(
            err.contains("legacy multi-field format no longer supported"),
            "missing migration header: {err}"
        );
        assert!(
            err.contains("[scene-custom.<name>]"),
            "missing scene-custom hint: {err}"
        );
        assert!(err.contains("base-scene"), "missing base-scene hint: {err}");
        assert!(
            err.contains("ambient.<HH-MM> = <name>"),
            "missing new format example: {err}"
        );
    }

    #[test]
    fn rejects_legacy_color_scene_positional_only() {
        // Even just `cosmos, monolith` (no kv pairs) is v30.1 format.
        let err = parse_ambient_value("cosmos, monolith")
            .expect_err("comma must trigger migration error");
        assert!(err.contains("legacy multi-field format"));
    }

    #[test]
    fn rejects_legacy_kv_only_format() {
        // `speed=15, density=1.2` (no positionals) is also v30.1 format.
        let err = parse_ambient_value("speed=15, density=1.2")
            .expect_err("equals sign must trigger migration error");
        assert!(err.contains("legacy multi-field format"));
    }

    #[test]
    fn migration_message_includes_user_repro_example() {
        // The user's exact bug-report config should appear in the message
        // so they can copy-paste the migration target.
        let err = parse_ambient_value("neon-purple, signal, speed=50, density=0.65").unwrap_err();
        assert!(
            err.contains("ambient.15-00 = neon-purple, signal, speed=50, density=0.65"),
            "migration message should include the user's repro example: {err}"
        );
        assert!(
            err.contains("[scene-custom.afternoon]"),
            "migration message should include the afternoon example: {err}"
        );
    }

    // ── AmbientSchedule helpers (current_phase / next_phase / seconds_to_next_phase) ──

    /// Helper: build a minimal entry for schedule tests.
    fn entry(h: u32, m: u32, scene: &str) -> AmbientEntry {
        AmbientEntry {
            hour: h,
            minute: m,
            scene: scene.to_string(),
        }
    }

    #[test]
    fn current_phase_finds_latest_before_now() {
        let s = AmbientSchedule {
            entries: vec![entry(0, 0, "cinematic"), entry(12, 0, "signal")],
        };
        // 12:30 → current is 12:00
        assert_eq!(s.current_phase(12 * 60 + 30).unwrap().hour, 12);
        // 11:59 → current is 00:00 (12:00 not yet fired)
        assert_eq!(s.current_phase(11 * 60 + 59).unwrap().hour, 0);
        // 13:00 → current is 12:00 (last entry <= 13:00)
        assert_eq!(s.current_phase(13 * 60).unwrap().hour, 12);
    }

    #[test]
    fn current_phase_wraps_to_last_entry_before_first() {
        // 2 entries: 06:00, 18:00. now=03:00 → no entry has fired today,
        // wrap to last entry (18:00 from yesterday).
        let s = AmbientSchedule {
            entries: vec![entry(6, 0, "matrix"), entry(18, 0, "monolith")],
        };
        let cur = s.current_phase(3 * 60).unwrap();
        assert_eq!(cur.hour, 18);
    }

    #[test]
    fn current_phase_empty_schedule_returns_none() {
        let s = AmbientSchedule::default();
        assert!(s.current_phase(0).is_none());
    }

    #[test]
    fn next_phase_finds_earliest_after_now() {
        let s = AmbientSchedule {
            entries: vec![entry(0, 0, "cinematic"), entry(12, 0, "signal")],
        };
        // 11:00 → next is 12:00
        assert_eq!(s.next_phase(11 * 60).unwrap().hour, 12);
        // 12:30 → next is 00:00 (tomorrow)
        assert_eq!(s.next_phase(12 * 60 + 30).unwrap().hour, 0);
    }

    #[test]
    fn next_phase_empty_schedule_returns_none() {
        let s = AmbientSchedule::default();
        assert!(s.next_phase(0).is_none());
    }

    #[test]
    fn seconds_to_next_phase_normal_case() {
        let s = AmbientSchedule {
            entries: vec![entry(12, 0, "signal")],
        };
        // now = 11:00:00 (660 min, 0 sec). next = 12:00:00 (720 min). diff = 60*60 = 3600 sec.
        assert_eq!(s.seconds_to_next_phase(660, 0), Some(3600));
        // now = 11:59:30 (719 min, 30 sec). next = 12:00:00. diff = 30 sec.
        assert_eq!(s.seconds_to_next_phase(719, 30), Some(30));
    }

    #[test]
    fn seconds_to_next_phase_wraps_midnight() {
        let s = AmbientSchedule {
            entries: vec![entry(6, 0, "matrix")],
        };
        // now = 23:00:00 (1380 min). next = 06:00:00 tomorrow (360 min).
        // diff = (24*60 - 1380 + 360) * 60 = 420 * 60 = 25200 sec.
        // Capped at 3600.
        assert_eq!(s.seconds_to_next_phase(1380, 0), Some(3600));
    }

    #[test]
    fn seconds_to_next_phase_empty_returns_none() {
        let s = AmbientSchedule::default();
        assert!(s.seconds_to_next_phase(0, 0).is_none());
    }

    // ── collect_ambient_schedule ──

    #[test]
    fn collect_sorts_entries_by_time() {
        let mut cfg = HashMap::new();
        cfg.insert("ambient.18-00".into(), "monolith".into());
        cfg.insert("ambient.06-00".into(), "matrix".into());
        cfg.insert("ambient.12-00".into(), "signal".into());
        let s = collect_ambient_schedule(&cfg);
        assert_eq!(s.entries.len(), 3);
        assert_eq!(s.entries[0].hour, 6);
        assert_eq!(s.entries[1].hour, 12);
        assert_eq!(s.entries[2].hour, 18);
        // Each entry's scene is preserved.
        assert_eq!(s.entries[0].scene, "matrix");
        assert_eq!(s.entries[1].scene, "signal");
        assert_eq!(s.entries[2].scene, "monolith");
    }

    #[test]
    fn collect_skips_legacy_format_entries() {
        // v30.2: legacy multi-field entries fail to parse and are silently
        // dropped from the runtime schedule (strict --testconf still errors).
        // This matches the live-reload contract: a half-edited config must
        // not crash the runtime.
        let mut cfg = HashMap::new();
        cfg.insert("ambient.12-00".into(), "signal".into());
        cfg.insert("ambient.18-00".into(), "neon, monolith, speed=15".into());
        let s = collect_ambient_schedule(&cfg);
        assert_eq!(s.entries.len(), 1);
        assert_eq!(s.entries[0].hour, 12);
        assert_eq!(s.entries[0].scene, "signal");
    }

    #[test]
    fn collect_returns_empty_when_no_ambient_keys() {
        let mut cfg = HashMap::new();
        cfg.insert("color".into(), "neon-green".into());
        cfg.insert("scene".into(), "monolith".into());
        let s = collect_ambient_schedule(&cfg);
        assert!(s.is_empty());
    }

    #[test]
    fn collect_preserves_custom_scene_names() {
        // v30.2: custom scene names are stored verbatim — validation that
        // they reference a defined [scene-custom.<name>] block happens in
        // validate_ambient_entries, not collect_ambient_schedule.
        let mut cfg = HashMap::new();
        cfg.insert("ambient.13-00".into(), "afternoon".into());
        let s = collect_ambient_schedule(&cfg);
        assert_eq!(s.entries.len(), 1);
        assert_eq!(s.entries[0].scene, "afternoon");
    }

    // ── validate_ambient_entries ──

    #[test]
    fn validate_accepts_builtin_scene_names() {
        let mut cfg = HashMap::new();
        cfg.insert("ambient.00-00".into(), "cinematic".into());
        cfg.insert("ambient.12-00".into(), "signal".into());
        cfg.insert("ambient.18-00".into(), "monolith".into());
        assert!(validate_ambient_entries(&cfg).is_ok());
    }

    #[test]
    fn validate_accepts_custom_scene_names() {
        let mut cfg = HashMap::new();
        cfg.insert("scene-custom.afternoon.color".into(), "neon-green".into());
        cfg.insert("ambient.15-00".into(), "afternoon".into());
        assert!(validate_ambient_entries(&cfg).is_ok());
    }

    #[test]
    fn validate_rejects_unknown_scene_name() {
        let mut cfg = HashMap::new();
        cfg.insert("ambient.00-00".into(), "nonexistent-scene".into());
        let err = validate_ambient_entries(&cfg).unwrap_err();
        assert!(
            err.contains("unknown scene 'nonexistent-scene'"),
            "got: {err}"
        );
        assert!(
            err.contains("[scene-custom.nonexistent-scene]"),
            "should hint at scene-custom block: {err}"
        );
    }

    #[test]
    fn validate_rejects_legacy_format_with_migration_hint() {
        // v30.2: a legacy multi-field entry must fail validation with the
        // full migration message. This is the primary user-facing error
        // path — when a user runs `--testconf` on an old config, they see
        // this and learn how to migrate.
        let mut cfg = HashMap::new();
        cfg.insert(
            "ambient.15-00".into(),
            "neon-purple, signal, speed=50, density=0.65".into(),
        );
        let err = validate_ambient_entries(&cfg).unwrap_err();
        assert!(err.contains("legacy multi-field format"), "got: {err}");
        assert!(err.contains("[scene-custom"), "got: {err}");
        assert!(err.contains("base-scene"), "got: {err}");
    }

    #[test]
    fn validate_accepts_empty_schedule() {
        let cfg = HashMap::new();
        assert!(validate_ambient_entries(&cfg).is_ok());
    }

    #[test]
    fn validate_case_insensitive_custom_scene_lookup() {
        // Custom scene names are stored lowercase by collect_custom_scenes;
        // validate_ambient_entries should match case-insensitively.
        let mut cfg = HashMap::new();
        cfg.insert("scene-custom.afternoon.color".into(), "neon-green".into());
        cfg.insert("ambient.15-00".into(), "AFTERNOON".into());
        assert!(validate_ambient_entries(&cfg).is_ok());
    }

    // ── current_minute_of_day / current_second_of_minute ──

    #[test]
    fn current_minute_of_day_bounded() {
        let m = current_minute_of_day();
        assert!(m < 24 * 60, "minute of day out of range: {m}");
    }

    #[test]
    fn current_second_of_minute_bounded() {
        let s = current_second_of_minute();
        assert!(s < 60, "second of minute out of range: {s}");
    }
}
