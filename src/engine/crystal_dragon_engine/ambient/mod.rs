// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Ambient phase scheduler — config-driven time-of-day scene switching.
//!
//! Replaces the archived `adaptive-custom` subsystem (eliminated with the
//! atmosphere engine at commit `07b44b5`). Unlike `adaptive-custom`, this
//! module is config-only (no CLI flag) and uses instant switch (no
//! smoothstep blend window) — the user explicitly asked for snappy phase
//! boundaries, not the imperceptible 5-minute cross-fade the old engine used.
//!
//! ## config format (simplified — breaking change)
//!
//! ```text
//! ambient.<HH-MM> = <scene-name>
//! ```
//!
//! The value is a single scene name — either a built-in scene
//! (`cinematic`, `signal`, `monolith`, etc.) or a custom scene defined via
//! `[scene-custom.<name>]`. All parameters (color, charset, speed, density,
//! fps, glitch-level, rain_style) live inside the scene itself, eliminating
//! the precedence confusion that plagued the multi-field format.
//!
//! ### Migration from multi-field format
//!
//! accepted `ambient.15-00 = neon-purple, signal, speed=50, density=0.65`.
//! rejects this with a migration error. To preserve the entry, define
//! a custom scene that captures the same parameters and reference it from a
//! TOP-LEVEL `ambient.` key (NEVER place the `ambient.` key inside the
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
/// simplified from a 7-field struct (color/scene/speed/density/fps/
/// charset/glitch_level) to just `scene`. All parameters now live inside the
/// referenced scene (built-in or `[scene-custom.<name>]`). This eliminates
/// the override-precedence bugs that plagued the old (e.g. `speed=50`
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

    /// Find the current phase — the latest entry whose `minutes_of_day()`
    /// is `<= now_min`. Returns `None` if no entry has fired yet today
    /// (i.e. `now_min` is earlier than the first entry's boundary, which
    /// only happens at startup before the first phase of the day).
    ///
    /// Wrap-around: if `now_min` is 0:30 and the earliest entry is 6:00,
    /// the "current" phase is the last entry of the previous day (it has
    /// been active since its boundary fired yesterday). This means a single
    /// entry schedule is active ALL DAY (it wraps from yesterday before its
    /// boundary and becomes today's phase at/after its boundary). This is
    /// correct by design — ambient is a 24-hour repeating schedule, not a
    /// one-shot timer. To have a scene activate only after a specific time,
    /// use at least two entries (e.g. `03-16 = cinematic` then
    /// `03-17 = hacker-mode`).
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

    /// Find the next phase — the earliest entry whose `minutes_of_day()`
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
/// format: `<scene-name>` — a single token, no commas, no `key=value`
/// pairs. The value must be a built-in scene name OR a custom scene name
/// defined via `[scene-custom.<name>]`.
///
/// # Errors
///
/// Returns `Err` with a migration message if the value contains `,` or `=`,
/// indicating the user is still using the multi-field format. The
/// migration message shows exactly how to convert the entry to the new
/// format using a `[scene-custom.<name>]` block with `base-scene`.
///
/// Returns `Err` if the value is empty or whitespace-only.
pub(crate) fn parse_ambient_value(value: &str) -> Result<AmbientEntry, String> {
    let scene = value.trim().to_string();
    if scene.is_empty() {
        return Err("ambient: empty scene name".to_string());
    }
    // detect legacy multi-field format and surface a migration
    // message. The user almost certainly has a config like
    // `ambient.15-00 = neon-purple, signal, speed=50, density=0.65` and
    // needs to convert it to a custom scene block.
    if scene.contains(',') || scene.contains('=') {
        return Err(format!(
            "ambient: legacy multi-field format no longer supported (got '{value}').\n\
             \n\
              simplified ambient entries to a single scene name. To preserve\n\
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
/// validation rules:
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
            // almost certainly still using the multi-field format.
            // The parse_ambient_value error already covers this case with
            // a full migration message, but we re-surface a shorter hint
            // here in case the value slipped through (e.g. quoted CSV).
            let hint = if scene.contains(',') || scene.contains('=') {
                " —  requires a single scene name; see migration guide in --testconf output above"
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
/// Used by event-gated callers only (ambient startup resolve, auto-snapback
/// phase pick) — each fires at most once per cycle, so a dedicated call is
/// fine there. The scheduler THREAD uses [`AmbientClockSnapshot`] instead:
/// it needs minute + second + yday from ONE clock read per wake.
///
/// Delegates to `crate::posix_time::local_tm()` — see that module for the
/// consolidated POSIX FFI path.
#[must_use]
pub(crate) fn current_minute_of_day() -> u32 {
    crate::posix_time::local_tm()
        .map(|tm| tm.minute_of_day())
        .unwrap_or(0)
}

/// One wall-clock snapshot for the ambient scheduler loop: minute-of-day,
/// second-of-minute, and day-of-year from a single `local_tm()` call.
///
/// NIGHT-hunter-12 (ambient scheduler thread cadence audit): the loop
/// previously read the clock as three separate calls per idle wake —
/// `current_minute_of_day()` + `current_second_of_minute()` +
/// `current_yday()` — and four on a fire wake (`current_yday()` ran twice:
/// once in the Delivered arm, once unconditionally for the day-boundary
/// check). Each call re-ran `libc::time` + `libc::localtime_r` (FFI +
/// timezone conversion). Worse than the redundancy, the minute and second
/// came from two different clock samples: a wake landing on a minute
/// boundary could read now_min = 11:59 (sample 1) and now_sec = 0 (sample
/// 2 at 12:00:00), so `seconds_to_next_phase` computed the sleep from an
/// instant that never existed and the phase boundary fired up to a minute
/// late. One struct, one syscall, one consistent instant.
///
/// The fallback on clock failure mirrors the pre-hunter-12 helpers'
/// `unwrap_or(0)` semantics: all fields zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AmbientClockSnapshot {
    /// Minute of day, 0..=1439.
    pub(crate) minute_of_day: u32,
    /// Second within the minute, 0..=59 (a leap-second 60 from
    /// `localtime_r` clamps to 59, matching the scheduler's previous
    /// `now_sec.min(59)` handling).
    pub(crate) second_of_minute: u32,
    /// Day of year, 0..=365 (366 only on Dec 31 of a leap year).
    pub(crate) yday: i32,
}

impl AmbientClockSnapshot {
    /// Capture the wall clock now — one `local_tm()` FFI round-trip.
    #[must_use]
    pub(crate) fn now() -> Self {
        match crate::posix_time::local_tm() {
            Some(tm) => Self {
                minute_of_day: tm.minute_of_day(),
                second_of_minute: tm.second.clamp(0, 59) as u32,
                yday: tm.yday,
            },
            None => Self {
                minute_of_day: 0,
                second_of_minute: 0,
                yday: 0,
            },
        }
    }
}

// NIGHT-hunter-12: `current_second_of_minute()` and `current_yday()` were
// removed — the scheduler thread now takes one `AmbientClockSnapshot` per
// wake instead of three-to-four separate `local_tm()` reads, and no other
// caller needed the single fields.

/// masterclass: compute the current ambient phase and apply it to the
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
/// hotfix: the `cfg` parameter MUST be the live config HashMap (loaded
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
    // stabilization: warn if the cfg map is empty. Custom-scene
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
    // stabilization: verify the apply actually changed cloud state.
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
#[path = "../../../../test/engine/crystal_dragon_engine/ambient/tests.rs"]
mod tests;
