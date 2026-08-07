// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Ambient phase scheduler — config-driven time-of-day scene/parameter switching.
//!
//! Replaces the archived `adaptive-custom` subsystem (eliminated with the
//! atmosphere engine at commit `07b44b5`). Unlike `adaptive-custom`, this
//! module is **config-only** (no CLI flag) and uses **instant switch** (no
//! smoothstep blend window) — the user explicitly asked for snappy phase
//! boundaries, not the imperceptible 5-minute cross-fade the old engine used.
//!
//! ## Config format
//!
//! ```text
//! ambient.<HH-MM> = <color>, <scene>, [key=value, ...]
//! ```
//!
//! - **`HH-MM`**: 24-hour time, zero-padded (`00-00` to `23-59`). The phase
//!   becomes "active" at this wall-clock minute and stays active until the
//!   next entry's boundary.
//! - **Positional 1** (`color`): built-in scheme name (52 themes) OR a
//!   `colors-custom.<name>` palette name. Optional — if omitted, color is
//!   sticky (keeps previous value).
//! - **Positional 2** (`scene`): built-in scene name (`matrix`, `monolith`,
//!   `signal`, etc.). Optional — if omitted, scene is sticky.
//! - **Optional `key=value` pairs**:
//!   - `speed` — float in `[1.0, 100.0]` (asymmetric vs top-level `speed`
//!     which is integer; float allows future lerp extension).
//!   - `density` — float in `[0.01, 5.0]`.
//!   - `fps` — integer in `[1, 120]`.
//!   - `charset` — built-in charset name OR `charset-custom.<name>`.
//!   - `glitch-level` — one of `none`, `subtle`, `default`, `intense`.
//!
//! ## Sticky semantics
//!
//! Fields not specified in a phase entry keep the previous value (the engine
//! does NOT reset unspecified fields to defaults when transitioning between
//! phases). This matches the archived `adaptive-custom` contract.
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
//! scene/color/charset/speed/density/glitch-level are applied immediately
//! via [`Cloud::apply_ambient_entry`]. The only visual smoothing comes from
//! the existing `transition_chars` and `transition_rain_style` machinery
//! (glyph warm-start, rain-style pool reset) — those are required for
//! correctness (preventing ghosting), not for cinematic blending.

use std::collections::HashMap;

/// Config namespace prefix for ambient phase entries.
pub(crate) const AMBIENT_NAMESPACE: &str = "ambient";

/// Maximum number of distinct phase entries a config may declare.
///
/// Defensive cap — a healthy schedule has 2–6 entries (one per major time
/// block). A config with 256+ entries is almost certainly a script-generated
/// mistake. The cap also bounds the sort cost (O(n log n)) at parse time.
pub(crate) const AMBIENT_MAX_ENTRIES: usize = 256;

/// One entry in the ambient schedule. Parsed from `ambient.HH-MM = ...`.
///
/// All optional fields implement **sticky semantics**: when `None`, the
/// previous phase's value is retained (the engine does NOT reset to default).
/// `hour` and `minute` are always present (they are the key, not the value).
#[derive(Clone, Debug, PartialEq)]
pub struct AmbientEntry {
    /// Hour portion of the `HH-MM` key (0–23).
    pub hour: u32,
    /// Minute portion of the `HH-MM` key (0–59).
    pub minute: u32,
    /// Positional 1: color scheme name (built-in OR `colors-custom.<name>`).
    pub color: Option<String>,
    /// Positional 2: scene name (`matrix`, `monolith`, `signal`, …).
    pub scene: Option<String>,
    /// Optional `speed=N` (float 1.0–100.0).
    pub speed: Option<f32>,
    /// Optional `density=N` (float 0.01–5.0).
    pub density: Option<f32>,
    /// Optional `fps=N` (integer 1–120).
    pub fps: Option<u32>,
    /// Optional `charset=<name>` (built-in OR `charset-custom.<name>`).
    pub charset: Option<String>,
    /// Optional `glitch-level=<none|subtle|default|intense>`.
    pub glitch_level: Option<String>,
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
/// Format: `<color>, <scene>, [key=value, ...]`
///
/// - Positional 1 (`color`): optional — if the first non-`key=value` token
///   is not a recognized color scheme name, it is treated as missing and
///   the parser tries to interpret it as the scene. This is forgiving: the
///   user may write `ambient.12-00 = monolith, speed=15` (color omitted).
/// - Positional 2 (`scene`): optional — same forgiving logic.
/// - `key=value` tokens: `speed`, `density`, `fps`, `charset`, `glitch-level`.
///   Unknown keys are returned as `Err`.
///
/// Returns `AmbientEntry` with `hour`/`minute` unset (the caller fills them
/// in from the config key). Validation of value ranges happens here; unknown
/// color/scene/charset names are NOT validated here (they are validated at
/// apply time and by `--testconf`).
pub(crate) fn parse_ambient_value(value: &str) -> Result<AmbientEntry, String> {
    let mut entry = AmbientEntry {
        hour: 0,
        minute: 0,
        color: None,
        scene: None,
        speed: None,
        density: None,
        fps: None,
        charset: None,
        glitch_level: None,
    };

    let mut positionals: Vec<String> = Vec::new();

    for raw_token in value.split(',') {
        let token = raw_token.trim();
        if token.is_empty() {
            continue;
        }
        if let Some((k, v)) = token.split_once('=') {
            let k = k.trim().to_ascii_lowercase();
            let v = v.trim();
            if v.is_empty() {
                return Err(format!("ambient: empty value for '{k}' in '{value}'"));
            }
            match k.as_str() {
                "speed" => {
                    let n: f32 = v
                        .parse()
                        .map_err(|_| format!("ambient: speed='{v}' is not a number"))?;
                    if !(1.0..=100.0).contains(&n) {
                        return Err(format!("ambient: speed={n} out of range [1.0, 100.0]"));
                    }
                    entry.speed = Some(n);
                }
                "density" => {
                    let n: f32 = v
                        .parse()
                        .map_err(|_| format!("ambient: density='{v}' is not a number"))?;
                    if !(0.01..=5.0).contains(&n) {
                        return Err(format!("ambient: density={n} out of range [0.01, 5.0]"));
                    }
                    entry.density = Some(n);
                }
                "fps" => {
                    let n: u32 = v
                        .parse()
                        .map_err(|_| format!("ambient: fps='{v}' is not an integer"))?;
                    if !(1..=120).contains(&n) {
                        return Err(format!("ambient: fps={n} out of range [1, 120]"));
                    }
                    entry.fps = Some(n);
                }
                "charset" => {
                    entry.charset = Some(v.to_string());
                }
                "glitch-level" => {
                    let normalized = v.to_ascii_lowercase();
                    if !matches!(
                        normalized.as_str(),
                        "none" | "subtle" | "default" | "intense"
                    ) {
                        return Err(format!(
                            "ambient: glitch-level='{v}' not in [none, subtle, default, intense]"
                        ));
                    }
                    entry.glitch_level = Some(normalized);
                }
                _ => {
                    return Err(format!(
                        "ambient: unknown key '{k}' in '{value}' (allowed: speed, density, fps, charset, glitch-level)"
                    ));
                }
            }
        } else {
            positionals.push(token.to_string());
        }
    }

    // Assign positionals: 1st = color, 2nd = scene (per archived spec).
    //
    // Forgiving logic for the ambiguous case where the first positional is
    // a valid scene name but NOT a valid color name — the user almost
    // certainly meant "scene only, color omitted" (e.g. `ambient.12-00 =
    // monolith, speed=15`). Without this, `monolith` would be treated as a
    // color and fail validation, forcing the user to write
    // `scene=monolith, speed=15` instead.
    //
    // When the first positional is BOTH a valid color AND a valid scene
    // (e.g. `cosmos`), we default to color (per archived spec: 1st
    // positional is color). The user can disambiguate by providing a 2nd
    // positional scene, or by using `key=value` syntax.
    if let Some(first) = positionals.first() {
        let is_color = crate::cli::parse_color_scheme(first).is_ok();
        let is_scene = crate::scene::get_scene(first).is_some();
        match (is_color, is_scene) {
            (false, true) => {
                // Scene only — color omitted.
                entry.scene = Some(first.clone());
            }
            _ => {
                // Color (or ambiguous, or neither — validation catches
                // neither later). 2nd positional becomes scene.
                entry.color = Some(first.clone());
                if let Some(second) = positionals.get(1) {
                    entry.scene = Some(second.clone());
                }
            }
        }
    }

    Ok(entry)
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
/// Validation rules:
/// - Value must parse (no unknown `key=value` keys, no out-of-range numbers).
/// - `color` (if present) must be a recognized built-in scheme OR a
///   `colors-custom.<name>` block that exists in the config.
/// - `scene` (if present) must be a recognized built-in scene.
/// - `charset` (if present) must be a recognized built-in charset OR a
///   `charset-custom.<name>` block that exists in the config.
/// - `glitch-level` must be one of `none`, `subtle`, `default`, `intense`.
pub(crate) fn validate_ambient_entries(cfg: &HashMap<String, String>) -> Result<(), String> {
    // Sort keys for deterministic error ordering (BTreeMap iteration).
    let mut keys: Vec<&String> = cfg.keys().filter(|k| k.starts_with("ambient.")).collect();
    keys.sort();

    for key in keys {
        let value = &cfg[key];
        let rest = key.strip_prefix("ambient.").unwrap_or("");
        if !is_valid_hh_mm(rest) {
            return Err(format!(
                "ambient: invalid time key '{key}' (expected HH-MM, e.g. 'ambient.12-00')"
            ));
        }
        let entry = parse_ambient_value(value).map_err(|e| format!("{key}: {e}"))?;

        // Validate color (if present) — built-in OR colors-custom.<name>.
        if let Some(color) = &entry.color {
            if crate::cli::parse_color_scheme(color).is_err()
                && !crate::colors_custom::is_colors_custom_name(cfg, color)
            {
                return Err(format!(
                    "{key}: unknown color '{color}' (not a built-in scheme and no [colors-custom.{color}] block)"
                ));
            }
        }

        // Validate scene (if present) — built-in only.
        if let Some(scene) = &entry.scene {
            if crate::scene::get_scene(scene).is_none() {
                return Err(format!(
                    "{key}: unknown scene '{scene}' (see --list-scenes)"
                ));
            }
        }

        // Validate charset (if present) — built-in OR charset-custom.<name>.
        if let Some(charset) = &entry.charset {
            if crate::charset::charset_from_str(charset, false).is_err()
                && crate::charset_custom::load_custom_charset_if_matches(cfg, charset).is_none()
            {
                return Err(format!(
                    "{key}: unknown charset '{charset}' (not a built-in charset and no [charset-custom.{charset}] block)"
                ));
            }
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

    // ── parse_ambient_value ──

    #[test]
    fn parses_color_scene_positional() {
        let e = parse_ambient_value("cosmos, monolith").unwrap();
        assert_eq!(e.color.as_deref(), Some("cosmos"));
        assert_eq!(e.scene.as_deref(), Some("monolith"));
        assert!(e.speed.is_none());
        assert!(e.density.is_none());
    }

    #[test]
    fn parses_color_scene_and_kv_pairs() {
        let e = parse_ambient_value("cosmos, monolith, speed=15, density=1.2").unwrap();
        assert_eq!(e.color.as_deref(), Some("cosmos"));
        assert_eq!(e.scene.as_deref(), Some("monolith"));
        assert_eq!(e.speed, Some(15.0));
        assert_eq!(e.density, Some(1.2));
    }

    #[test]
    fn forgives_omitted_color_when_first_positional_is_scene() {
        // User wrote "monolith, speed=15" (color omitted).
        let e = parse_ambient_value("monolith, speed=15").unwrap();
        assert!(e.color.is_none());
        assert_eq!(e.scene.as_deref(), Some("monolith"));
        assert_eq!(e.speed, Some(15.0));
    }

    #[test]
    fn parses_kv_only_no_positionals() {
        let e = parse_ambient_value("speed=15, density=1.2").unwrap();
        assert!(e.color.is_none());
        assert!(e.scene.is_none());
        assert_eq!(e.speed, Some(15.0));
        assert_eq!(e.density, Some(1.2));
    }

    #[test]
    fn handles_extra_whitespace() {
        let e = parse_ambient_value("  cosmos ,  monolith ,  speed = 15  ").unwrap();
        assert_eq!(e.color.as_deref(), Some("cosmos"));
        assert_eq!(e.scene.as_deref(), Some("monolith"));
        assert_eq!(e.speed, Some(15.0));
    }

    #[test]
    fn rejects_unknown_kv_key() {
        let err = parse_ambient_value("cosmos, monolith, brightness=0.5").unwrap_err();
        assert!(err.contains("unknown key 'brightness'"));
    }

    #[test]
    fn rejects_speed_out_of_range() {
        assert!(parse_ambient_value("cosmos, speed=0.5").is_err());
        assert!(parse_ambient_value("cosmos, speed=150").is_err());
        assert!(parse_ambient_value("cosmos, speed=1.0").is_ok());
        assert!(parse_ambient_value("cosmos, speed=100.0").is_ok());
    }

    #[test]
    fn rejects_density_out_of_range() {
        assert!(parse_ambient_value("cosmos, density=0.0").is_err());
        assert!(parse_ambient_value("cosmos, density=5.5").is_err());
        assert!(parse_ambient_value("cosmos, density=0.01").is_ok());
        assert!(parse_ambient_value("cosmos, density=5.0").is_ok());
    }

    #[test]
    fn rejects_fps_out_of_range() {
        assert!(parse_ambient_value("cosmos, fps=0").is_err());
        assert!(parse_ambient_value("cosmos, fps=121").is_err());
        assert!(parse_ambient_value("cosmos, fps=1").is_ok());
        assert!(parse_ambient_value("cosmos, fps=120").is_ok());
    }

    #[test]
    fn rejects_invalid_glitch_level() {
        assert!(parse_ambient_value("cosmos, glitch-level=ultra").is_err());
        assert!(parse_ambient_value("cosmos, glitch-level=high").is_err());
        assert!(parse_ambient_value("cosmos, glitch-level=medium").is_err());
        assert!(parse_ambient_value("cosmos, glitch-level=low").is_err());
        assert!(parse_ambient_value("cosmos, glitch-level=none").is_ok());
        assert!(parse_ambient_value("cosmos, glitch-level=subtle").is_ok());
        assert!(parse_ambient_value("cosmos, glitch-level=default").is_ok());
        assert!(parse_ambient_value("cosmos, glitch-level=intense").is_ok());
    }

    #[test]
    fn rejects_empty_kv_value() {
        let err = parse_ambient_value("cosmos, speed=").unwrap_err();
        assert!(err.contains("empty value for 'speed'"));
    }

    #[test]
    fn rejects_non_numeric_speed() {
        assert!(parse_ambient_value("cosmos, speed=fast").is_err());
    }

    // ── AmbientSchedule::current_phase ──

    #[test]
    fn current_phase_finds_latest_before_now() {
        let s = AmbientSchedule {
            entries: vec![
                AmbientEntry {
                    hour: 0,
                    minute: 0,
                    color: Some("cosmos".into()),
                    scene: Some("monolith".into()),
                    speed: None,
                    density: None,
                    fps: None,
                    charset: None,
                    glitch_level: None,
                },
                AmbientEntry {
                    hour: 12,
                    minute: 0,
                    color: Some("aurora".into()),
                    scene: Some("matrix".into()),
                    speed: None,
                    density: None,
                    fps: None,
                    charset: None,
                    glitch_level: None,
                },
            ],
        };
        // 12:30 → current is 12:00
        assert_eq!(s.current_phase(12 * 60 + 30).unwrap().hour, 12);
        // 11:59 → current is 00:00 (12:00 not yet fired)
        assert_eq!(s.current_phase(11 * 60 + 59).unwrap().hour, 0);
        // 13:00 next day → wraps to 12:00 (last entry of today)
        // Actually 13:00 = 780 min, last entry is 12:00=720, so 720<=780 → current = 12:00
        assert_eq!(s.current_phase(13 * 60).unwrap().hour, 12);
    }

    #[test]
    fn current_phase_wraps_to_last_entry_before_first() {
        // 2 entries: 06:00, 18:00. now=03:00 → no entry has fired today,
        // wrap to last entry (18:00 from yesterday).
        let s = AmbientSchedule {
            entries: vec![
                AmbientEntry {
                    hour: 6,
                    minute: 0,
                    color: None,
                    scene: None,
                    speed: None,
                    density: None,
                    fps: None,
                    charset: None,
                    glitch_level: None,
                },
                AmbientEntry {
                    hour: 18,
                    minute: 0,
                    color: None,
                    scene: None,
                    speed: None,
                    density: None,
                    fps: None,
                    charset: None,
                    glitch_level: None,
                },
            ],
        };
        let cur = s.current_phase(3 * 60).unwrap();
        assert_eq!(cur.hour, 18);
    }

    #[test]
    fn current_phase_empty_schedule_returns_none() {
        let s = AmbientSchedule::default();
        assert!(s.current_phase(0).is_none());
    }

    // ── AmbientSchedule::next_phase ──

    #[test]
    fn next_phase_finds_earliest_after_now() {
        let s = AmbientSchedule {
            entries: vec![
                AmbientEntry {
                    hour: 0,
                    minute: 0,
                    color: None,
                    scene: None,
                    speed: None,
                    density: None,
                    fps: None,
                    charset: None,
                    glitch_level: None,
                },
                AmbientEntry {
                    hour: 12,
                    minute: 0,
                    color: None,
                    scene: None,
                    speed: None,
                    density: None,
                    fps: None,
                    charset: None,
                    glitch_level: None,
                },
            ],
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

    // ── AmbientSchedule::seconds_to_next_phase ──

    #[test]
    fn seconds_to_next_phase_normal_case() {
        let s = AmbientSchedule {
            entries: vec![AmbientEntry {
                hour: 12,
                minute: 0,
                color: None,
                scene: None,
                speed: None,
                density: None,
                fps: None,
                charset: None,
                glitch_level: None,
            }],
        };
        // now = 11:00:00 (660 min, 0 sec). next = 12:00:00 (720 min). diff = 60*60 = 3600 sec.
        assert_eq!(s.seconds_to_next_phase(660, 0), Some(3600));
        // now = 11:59:30 (719 min, 30 sec). next = 12:00:00. diff = 30 sec.
        assert_eq!(s.seconds_to_next_phase(719, 30), Some(30));
    }

    #[test]
    fn seconds_to_next_phase_wraps_midnight() {
        let s = AmbientSchedule {
            entries: vec![AmbientEntry {
                hour: 6,
                minute: 0,
                color: None,
                scene: None,
                speed: None,
                density: None,
                fps: None,
                charset: None,
                glitch_level: None,
            }],
        };
        // now = 23:00:00 (1380 min). next = 06:00:00 tomorrow (360 min).
        // diff = (24*60 - 1380 + 360) * 60 = (1440 - 1380 + 360) * 60 = 420 * 60 = 25200 sec.
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
        cfg.insert("ambient.18-00".into(), "neon, monolith".into());
        cfg.insert("ambient.06-00".into(), "aurora, matrix".into());
        cfg.insert("ambient.12-00".into(), "cosmos, monolith".into());
        let s = collect_ambient_schedule(&cfg);
        assert_eq!(s.entries.len(), 3);
        assert_eq!(s.entries[0].hour, 6);
        assert_eq!(s.entries[1].hour, 12);
        assert_eq!(s.entries[2].hour, 18);
    }

    #[test]
    fn collect_skips_malformed_entries() {
        let mut cfg = HashMap::new();
        cfg.insert("ambient.12-00".into(), "cosmos, monolith".into());
        cfg.insert("ambient.18-00".into(), "cosmos, bogus_key=1".into()); // unknown key
        let s = collect_ambient_schedule(&cfg);
        // Only the valid entry survives.
        assert_eq!(s.entries.len(), 1);
        assert_eq!(s.entries[0].hour, 12);
    }

    #[test]
    fn collect_returns_empty_when_no_ambient_keys() {
        let mut cfg = HashMap::new();
        cfg.insert("color".into(), "cosmos".into());
        cfg.insert("scene".into(), "monolith".into());
        let s = collect_ambient_schedule(&cfg);
        assert!(s.is_empty());
    }

    // ── validate_ambient_entries ──

    #[test]
    fn validate_accepts_valid_entries() {
        let mut cfg = HashMap::new();
        cfg.insert("ambient.00-00".into(), "cosmos, monolith".into());
        cfg.insert("ambient.12-00".into(), "aurora, matrix, speed=15".into());
        assert!(validate_ambient_entries(&cfg).is_ok());
    }

    #[test]
    fn validate_rejects_unknown_color() {
        let mut cfg = HashMap::new();
        cfg.insert("ambient.00-00".into(), "nonexistent-color, monolith".into());
        let err = validate_ambient_entries(&cfg).unwrap_err();
        assert!(err.contains("unknown color"));
    }

    #[test]
    fn validate_rejects_unknown_scene() {
        let mut cfg = HashMap::new();
        // Use a name that's neither a color nor a scene — `nonexistent-scene`
        // would be treated as color (parse_color_scheme fails), then 2nd
        // positional `matrix` would be scene. To test the scene rejection
        // path, we put a known color first and an unknown scene second.
        cfg.insert("ambient.00-00".into(), "cosmos, nonexistent-scene".into());
        let err = validate_ambient_entries(&cfg).unwrap_err();
        assert!(err.contains("unknown scene"), "got: {err}");
    }

    #[test]
    fn validate_rejects_unknown_charset() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "ambient.00-00".into(),
            "cosmos, monolith, charset=nonexistent".into(),
        );
        let err = validate_ambient_entries(&cfg).unwrap_err();
        assert!(err.contains("unknown charset"));
    }

    #[test]
    fn validate_rejects_invalid_value() {
        let mut cfg = HashMap::new();
        cfg.insert("ambient.00-00".into(), "cosmos, monolith, speed=999".into());
        assert!(validate_ambient_entries(&cfg).is_err());
    }

    #[test]
    fn validate_accepts_empty_schedule() {
        let cfg = HashMap::new();
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
