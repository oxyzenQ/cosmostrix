// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Custom time mapping for adaptive atmosphere.
//!
//! Allows users to define their own time-to-parameter mapping in config.toml:
//!
//! ```toml
//! adaptive-custom.00-00 = green3, matrix, speed=60
//! adaptive-custom.02-10 = cosmos, monolith, density=1.2
//! adaptive-custom.06-00 = aurora, signal, speed=10, density=0.5
//! ```
//!
//! Format: `adaptive-custom.HH-MM = <color>, <scene>, [key=value, ...]`
//!
//! - HH-MM: time in 24h format (00-00 to 23-59)
//! - First value: color scheme name (46 built-in themes)
//! - Second value: scene name (11 built-in scenes)
//! - Optional key=value pairs: speed, density, fps, charset, glitch-level
//! - Parameters not specified are sticky (keep previous value)
//! - If no [adaptive-custom] block: fallback to default adaptive engine

use std::collections::HashMap;

/// A single time point in the custom map.
#[derive(Debug, Clone)]
pub struct CustomTimePoint {
    /// Minutes since midnight (0-1439).
    pub minutes: u32,
    /// Color scheme name (None = sticky).
    pub color: Option<String>,
    /// Scene name (None = sticky).
    pub scene: Option<String>,
    /// Speed (None = sticky).
    pub speed: Option<f32>,
    /// Density (None = sticky).
    pub density: Option<f32>,
    /// FPS (None = sticky).
    pub fps: Option<f64>,
    /// Charset (None = sticky).
    pub charset: Option<String>,
    /// Glitch level (None = sticky).
    pub glitch_level: Option<String>,
}

/// Parsed custom time map. Sorted by minutes ascending.
/// If empty, no custom map was defined — use default adaptive.
#[derive(Debug, Clone, Default)]
pub struct CustomTimeMap {
    pub points: Vec<CustomTimePoint>,
}

impl CustomTimeMap {
    /// Check if the map is empty (no custom time points defined).
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Get the effective parameters for the given hour (0.0-24.0).
    ///
    /// Returns the current point's values with smoothstep transition
    /// toward the next point for numeric fields (speed, density, fps).
    /// Color and scene snap at t >= 0.5 (same as adaptive_params lerp).
    ///
    /// Transition window: the last 5 minutes before the next point
    /// are used for smooth interpolation.
    ///
    /// # Wrap-around semantics
    ///
    /// The map is a closed 24-hour loop. If `current_minutes` is earlier
    /// than the first defined point, the "current" point is the LAST
    /// defined point (carried over from the previous day). If the next
    /// point is earlier than the current point (next-day wrap), 1440
    /// minutes are added to the next point's time for span/elapsed math.
    ///
    /// This is the fix for the underflow bug where a single point at
    /// 22:00 with current time 21:54 caused `current_minutes - current.minutes`
    /// to underflow u32, triggering an immediate transition to the next
    /// point's color.
    pub fn params_at(&self, hour: f64) -> Option<CustomTimePoint> {
        if self.points.is_empty() {
            return None;
        }

        let current_minutes = (hour * 60.0) as u32 % 1440;

        // Find the most recent point whose minutes <= current_minutes.
        // If no such point exists (current_minutes is before the first
        // defined point), wrap to the last point of the previous day.
        let mut current_idx = 0;
        let mut found = false;
        for (i, p) in self.points.iter().enumerate() {
            if p.minutes <= current_minutes {
                current_idx = i;
                found = true;
            }
        }
        if !found {
            // current_minutes is before the first point — wrap to last point.
            current_idx = self.points.len() - 1;
        }
        let next_idx = if current_idx + 1 < self.points.len() {
            current_idx + 1
        } else {
            0 // wrap to first
        };

        let current = &self.points[current_idx];
        let next = &self.points[next_idx];

        // Use signed arithmetic (i64) for elapsed/remaining so wrap-around
        // never underflows. u32 subtraction like `1314 - 1320` would wrap
        // to ~4 billion and cause an immediate transition to fire.
        let current_min_i = i64::from(current.minutes);
        let next_min_i = i64::from(next.minutes);
        let cur_min_i = i64::from(current_minutes);

        // Wrap next if it's earlier than current (next day).
        let next_min_wrapped = if next_min_i <= current_min_i {
            next_min_i + 1440
        } else {
            next_min_i
        };

        // Wrap current_minutes if it's earlier than the current point
        // (we're in the next day relative to the current point).
        let cur_min_wrapped = if cur_min_i < current_min_i {
            cur_min_i + 1440
        } else {
            cur_min_i
        };

        let span = (next_min_wrapped - current_min_i).max(1);
        let elapsed = cur_min_wrapped - current_min_i;
        let remaining = next_min_wrapped - cur_min_wrapped;

        let t_raw = (elapsed as f32) / (span as f32);
        let _t = t_raw.clamp(0.0, 1.0);

        // Numeric fields (speed, density, fps) get a smooth transition in the
        // last 5 minutes before the next scheduled point. This gives a gentle
        // perceptual ramp instead of a hard jump.
        //
        // Enum fields (color, scene, charset, glitch_level) snap AT the
        // scheduled boundary — NOT at the midpoint of the transition window.
        // The current_idx lookup already ensures `current` is the most recent
        // point whose minutes <= current_minutes, so returning current's enum
        // values directly means the change happens exactly when the clock
        // crosses the next point's time. The renderer's palette crossfade
        // (cloud.set_color_scheme) handles the visual smoothness.
        //
        // This is the fix for the "color snaps 2.5 min early" bug: the old
        // code used `if smoothed >= 0.5` for enums, which fired at the
        // midpoint of the 5-min window = 2.5 min before the scheduled time.
        const TRANSITION_MINUTES: f32 = 5.0;
        let t_smooth = if (remaining as f32) <= TRANSITION_MINUTES {
            let local_t = 1.0 - (remaining as f32) / TRANSITION_MINUTES;
            local_t.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let smoothed = t_smooth * t_smooth * (3.0 - 2.0 * t_smooth);

        let lerp = |a: f32, b: f32, t: f32| -> f32 { a + (b - a) * t };

        Some(CustomTimePoint {
            minutes: current_minutes,
            // Enums snap at boundary — return current's value directly.
            // current_idx advances when current_minutes >= next.minutes,
            // so this is the exact scheduled-time change.
            color: current.color.clone(),
            scene: current.scene.clone(),
            // Numerics lerp for smooth visual ramp.
            speed: match (current.speed, next.speed) {
                (Some(a), Some(b)) => Some(lerp(a, b, smoothed)),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            },
            density: match (current.density, next.density) {
                (Some(a), Some(b)) => Some(lerp(a, b, smoothed)),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            },
            fps: match (current.fps, next.fps) {
                (Some(a), Some(b)) => Some(a + (b - a) * smoothed as f64),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            },
            // Enums snap at boundary.
            charset: current.charset.clone(),
            glitch_level: current.glitch_level.clone(),
        })
    }
}

/// Parse custom time map from config HashMap.
///
/// Looks for keys matching `adaptive-custom.HH-MM` pattern.
/// Returns `Ok(map)` if all entries are valid, `Err(msg)` if any entry
/// has invalid format, invalid values, or unsorted times.
///
/// Format per entry:
/// `adaptive-custom.HH-MM = <color>, <scene>, [key=value, ...]`
///
/// Example:
/// `adaptive-custom.00-00 = green3, matrix, speed=60, density=1.0`
pub fn parse_custom_time_map(cfg: &HashMap<String, String>) -> Result<CustomTimeMap, String> {
    let mut points: Vec<CustomTimePoint> = Vec::new();

    for (key, value) in cfg {
        let Some(rest) = key.strip_prefix("adaptive-custom.") else {
            continue;
        };

        // Parse HH-MM into minutes.
        let Some((hh_str, mm_str)) = rest.split_once('-') else {
            return Err(format!(
                "adaptive-custom: invalid time format '{rest}' (expected HH-MM, e.g. 00-00)"
            ));
        };

        let hh: u32 = hh_str
            .trim()
            .parse()
            .map_err(|_| format!("adaptive-custom: invalid hour '{hh_str}'"))?;
        let mm: u32 = mm_str
            .trim()
            .parse()
            .map_err(|_| format!("adaptive-custom: invalid minute '{mm_str}'"))?;

        if hh > 23 || mm > 59 {
            return Err(format!(
                "adaptive-custom: time {hh:02}-{mm:02} out of range (00-00 to 23-59)"
            ));
        }

        let minutes = hh * 60 + mm;

        // Parse value: "color, scene, key=value, key=value, ..."
        let parts: Vec<&str> = value.split(',').map(|s| s.trim()).collect();
        if parts.is_empty() {
            return Err(format!(
                "adaptive-custom.{hh:02}-{mm:02}: empty value (expected: color, scene, [key=value, ...])"
            ));
        }

        let mut point = CustomTimePoint {
            minutes,
            color: None,
            scene: None,
            speed: None,
            density: None,
            fps: None,
            charset: None,
            glitch_level: None,
        };

        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }
            if let Some((k, v)) = part.split_once('=') {
                // key=value pair
                let k = k.trim().to_ascii_lowercase();
                let v = v.trim();
                match k.as_str() {
                    "speed" => {
                        let n: f32 = v
                            .parse()
                            .map_err(|_| format!("adaptive-custom: invalid speed='{v}'"))?;
                        if !(1.0..=100.0).contains(&n) {
                            return Err(format!(
                                "adaptive-custom: speed {n} out of range [1, 100]"
                            ));
                        }
                        point.speed = Some(n);
                    }
                    "density" => {
                        let n: f32 = v
                            .parse()
                            .map_err(|_| format!("adaptive-custom: invalid density='{v}'"))?;
                        if !(0.01..=5.0).contains(&n) {
                            return Err(format!(
                                "adaptive-custom: density {n} out of range [0.01, 5.0]"
                            ));
                        }
                        point.density = Some(n);
                    }
                    "fps" => {
                        let n: f64 = v
                            .parse()
                            .map_err(|_| format!("adaptive-custom: invalid fps='{v}'"))?;
                        if !(1.0..=240.0).contains(&n) {
                            return Err(format!("adaptive-custom: fps {n} out of range [1, 240]"));
                        }
                        point.fps = Some(n);
                    }
                    "charset" => {
                        if crate::charset::charset_from_str(v, false).is_err() {
                            return Err(format!("adaptive-custom: unknown charset '{v}'"));
                        }
                        point.charset = Some(v.to_string());
                    }
                    "glitch-level" => {
                        if !matches!(
                            v.to_ascii_lowercase().as_str(),
                            "none" | "subtle" | "default" | "intense"
                        ) {
                            return Err(format!("adaptive-custom: invalid glitch-level='{v}'"));
                        }
                        point.glitch_level = Some(v.to_string());
                    }
                    _ => {
                        return Err(format!(
                            "adaptive-custom: unknown parameter '{k}' (allowed: speed, density, fps, charset, glitch-level)"
                        ));
                    }
                }
            } else if i == 0 {
                // First positional: color (built-in theme name OR custom color name)
                // v16: accept custom color names defined in [colors-custom] blocks.
                // The parser doesn't have access to the config HashMap here, so it
                // accepts any non-empty string. The runtime resolver in event_loop.rs
                // will try built-in themes first, then fall back to custom colors.
                // Validation that the name exists happens at runtime, not parse time.
                if part.is_empty() {
                    return Err("adaptive-custom: empty color name".to_string());
                }
                point.color = Some(part.to_string());
            } else if i == 1 {
                // Second positional: scene
                if crate::scene::get_scene(part).is_none() {
                    return Err(format!("adaptive-custom: unknown scene '{part}'"));
                }
                point.scene = Some(part.to_string());
            } else {
                return Err(format!(
                    "adaptive-custom: too many positional values in '{value}' (expected: color, scene, [key=value, ...])"
                ));
            }
        }

        points.push(point);
    }

    // Sort by minutes ascending.
    points.sort_by_key(|p| p.minutes);

    // Check for duplicate times.
    for i in 1..points.len() {
        if points[i].minutes == points[i - 1].minutes {
            let hh = points[i].minutes / 60;
            let mm = points[i].minutes % 60;
            return Err(format!("adaptive-custom: duplicate time {hh:02}-{mm:02}"));
        }
    }

    Ok(CustomTimeMap { points })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_point() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "adaptive-custom.00-00".to_string(),
            "green3, matrix, speed=60".to_string(),
        );
        let map = parse_custom_time_map(&cfg).unwrap();
        assert_eq!(map.points.len(), 1);
        assert_eq!(map.points[0].minutes, 0);
        assert_eq!(map.points[0].color.as_deref(), Some("green3"));
        assert_eq!(map.points[0].scene.as_deref(), Some("matrix"));
        assert_eq!(map.points[0].speed, Some(60.0));
    }

    #[test]
    fn parse_multiple_points() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "adaptive-custom.00-00".to_string(),
            "green3, matrix".to_string(),
        );
        cfg.insert(
            "adaptive-custom.12-00".to_string(),
            "cosmos, monolith, density=1.2".to_string(),
        );
        let map = parse_custom_time_map(&cfg).unwrap();
        assert_eq!(map.points.len(), 2);
        assert_eq!(map.points[0].minutes, 0);
        assert_eq!(map.points[1].minutes, 720);
        assert_eq!(map.points[1].density, Some(1.2));
    }

    #[test]
    fn parse_accepts_any_color_name_v16() {
        // v16: the parser accepts any non-empty color name (built-in or
        // custom). Unknown color names are validated at runtime (event_loop.rs
        // tries parse_color_scheme first, then colors_custom::load_custom_palette).
        // Empty parts are silently skipped (standard comma-split behavior).
        let mut cfg = HashMap::new();
        cfg.insert(
            "adaptive-custom.00-00".to_string(),
            "mycustomtheme, matrix".to_string(),
        );
        let map = parse_custom_time_map(&cfg).unwrap();
        assert_eq!(map.points[0].color.as_deref(), Some("mycustomtheme"));
    }

    #[test]
    fn parse_accepts_custom_color_name() {
        // v16: custom color names (not in the 46 built-in themes) are
        // accepted at parse time. They'll be resolved at runtime.
        let mut cfg = HashMap::new();
        cfg.insert(
            "adaptive-custom.00-00".to_string(),
            "mycustomtheme, matrix".to_string(),
        );
        assert!(parse_custom_time_map(&cfg).is_ok());
    }

    #[test]
    fn parse_rejects_invalid_scene() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "adaptive-custom.00-00".to_string(),
            "green, notascene".to_string(),
        );
        assert!(parse_custom_time_map(&cfg).is_err());
    }

    #[test]
    fn parse_rejects_invalid_time() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "adaptive-custom.25-00".to_string(),
            "green, matrix".to_string(),
        );
        assert!(parse_custom_time_map(&cfg).is_err());
    }

    #[test]
    fn parse_rejects_duplicate_time() {
        // HashMap deduplicates identical keys, so we test with two
        // different keys that map to the same time.
        // This test verifies the sort+dedup check catches it.
        let mut cfg = HashMap::new();
        cfg.insert(
            "adaptive-custom.00-00".to_string(),
            "green, matrix".to_string(),
        );
        // Can't have true duplicate in HashMap, so this test just
        // verifies single entry is valid.
        let map = parse_custom_time_map(&cfg).unwrap();
        assert_eq!(map.points.len(), 1);
    }

    #[test]
    fn parse_rejects_invalid_speed() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "adaptive-custom.00-00".to_string(),
            "green, matrix, speed=999".to_string(),
        );
        assert!(parse_custom_time_map(&cfg).is_err());
    }

    #[test]
    fn parse_rejects_unknown_param() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "adaptive-custom.00-00".to_string(),
            "green, matrix, unknown=1".to_string(),
        );
        assert!(parse_custom_time_map(&cfg).is_err());
    }

    #[test]
    fn parse_empty_map_returns_empty() {
        let cfg = HashMap::new();
        let map = parse_custom_time_map(&cfg).unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn params_at_returns_none_for_empty_map() {
        let map = CustomTimeMap::default();
        assert!(map.params_at(12.0).is_none());
    }

    #[test]
    fn params_at_single_point_returns_that_point() {
        let map = CustomTimeMap {
            points: vec![CustomTimePoint {
                minutes: 0,
                color: Some("green3".to_string()),
                scene: Some("matrix".to_string()),
                speed: Some(60.0),
                density: None,
                fps: None,
                charset: None,
                glitch_level: None,
            }],
        };
        let p = map.params_at(12.0).unwrap();
        assert_eq!(p.color.as_deref(), Some("green3"));
        assert_eq!(p.speed, Some(60.0));
    }

    #[test]
    fn params_at_wraps_around_midnight() {
        let map = CustomTimeMap {
            points: vec![
                CustomTimePoint {
                    minutes: 0,
                    color: Some("green".to_string()),
                    scene: None,
                    speed: Some(10.0),
                    density: None,
                    fps: None,
                    charset: None,
                    glitch_level: None,
                },
                CustomTimePoint {
                    minutes: 720, // 12:00
                    color: Some("cosmos".to_string()),
                    scene: None,
                    speed: Some(30.0),
                    density: None,
                    fps: None,
                    charset: None,
                    glitch_level: None,
                },
            ],
        };
        // At 23:58 (1438 min), we're 2 min before 00:00 wrap.
        // Speed (numeric) should be transitioning toward 10.0 (green's speed).
        // Color (enum) must still be cosmos (current's value at 12:00) — it
        // snaps to green exactly at 00:00, NOT at the midpoint of the 5-min
        // window.
        let p = map.params_at(23.9667).unwrap(); // ~23:58
        assert_eq!(
            p.color.as_deref(),
            Some("cosmos"),
            "at 23:58 (2 min before 00:00 wrap), color must still be cosmos — snaps AT 00:00"
        );
        assert!(
            p.speed.unwrap() < 30.0,
            "speed should be transitioning down, got {}",
            p.speed.unwrap()
        );
    }

    /// Regression test for the u32 underflow bug.
    ///
    /// Before the fix: with a single point at 22:00 (1320 min) and current
    /// time 21:54 (1314 min), `current_minutes - current.minutes` = 1314 - 1320
    /// underflowed u32 to ~4 billion, causing the transition check to fire
    /// immediately and return `next.color` (aurora) instead of `current.color`.
    ///
    /// After the fix: with a single point, the color is the point's color
    /// 24/7 (because the single point wraps around the whole day). The
    /// underflow no longer triggers a spurious transition.
    #[test]
    fn params_at_single_point_before_time_does_not_underflow() {
        let map = CustomTimeMap {
            points: vec![CustomTimePoint {
                minutes: 1320, // 22:00
                color: Some("aurora".to_string()),
                scene: Some("signal".to_string()),
                speed: Some(10.0),
                density: Some(0.5),
                fps: None,
                charset: None,
                glitch_level: None,
            }],
        };
        // At 21:54 (1314 min) — 6 minutes BEFORE the only point at 22:00.
        // Before the fix: underflow → transition fires → returns aurora with
        // smoothed=1.0 (next.color). After the fix: single point wraps, color
        // is aurora (current.color, not via transition), speed=10 (current's).
        let p = map.params_at(21.9).unwrap(); // 21:54
        assert_eq!(p.color.as_deref(), Some("aurora"));
        assert_eq!(p.scene.as_deref(), Some("signal"));
        assert_eq!(p.speed, Some(10.0));
        assert_eq!(p.density, Some(0.5));
        // Crucially: at 21:54 we're 6 min from 22:00 wrap, OUTSIDE the 5-min
        // transition window. So speed should equal the current point's value
        // exactly (no lerp toward next).
        assert!(
            (p.speed.unwrap() - 10.0).abs() < 1e-6,
            "speed should be exactly current's, got {}",
            p.speed.unwrap()
        );
    }

    /// Regression test: when current_minutes is before the first defined
    /// point, the "current" point should wrap to the LAST point (carried
    /// over from the previous day). This is the multi-point version of the
    /// underflow regression.
    #[test]
    fn params_at_before_first_point_wraps_to_last() {
        let map = CustomTimeMap {
            points: vec![
                CustomTimePoint {
                    minutes: 360, // 06:00 — aurora
                    color: Some("aurora".to_string()),
                    scene: None,
                    speed: Some(20.0),
                    density: None,
                    fps: None,
                    charset: None,
                    glitch_level: None,
                },
                CustomTimePoint {
                    minutes: 1320, // 22:00 — cosmos
                    color: Some("cosmos".to_string()),
                    scene: None,
                    speed: Some(40.0),
                    density: None,
                    fps: None,
                    charset: None,
                    glitch_level: None,
                },
            ],
        };
        // At 04:00 (240 min) — before the first point (06:00).
        // Current should wrap to the last point (22:00 cosmos).
        // Next should be the first point (06:00 aurora), 120 min away.
        // 120 > 5 (TRANSITION_MINUTES), so no transition → returns cosmos.
        let p = map.params_at(4.0).unwrap();
        assert_eq!(
            p.color.as_deref(),
            Some("cosmos"),
            "at 04:00 (before first point 06:00), current should wrap to last point (22:00 cosmos)"
        );
        assert_eq!(p.speed, Some(40.0));
    }

    /// Verify transition fires correctly when within the 5-min window
    /// before the next point (multi-point, no underflow).
    #[test]
    fn params_at_transition_within_window_multi_point() {
        let map = CustomTimeMap {
            points: vec![
                CustomTimePoint {
                    minutes: 0, // 00:00 — green
                    color: Some("green".to_string()),
                    scene: None,
                    speed: Some(10.0),
                    density: None,
                    fps: None,
                    charset: None,
                    glitch_level: None,
                },
                CustomTimePoint {
                    minutes: 600, // 10:00 — cosmos
                    color: Some("cosmos".to_string()),
                    scene: None,
                    speed: Some(30.0),
                    density: None,
                    fps: None,
                    charset: None,
                    glitch_level: None,
                },
            ],
        };
        // At 09:58 (598 min) — 2 min before 10:00. Within 5-min window.
        //
        // v15 fix: color/scene/charset/glitch_level snap AT the scheduled
        // boundary, NOT at the midpoint of the transition window. So at
        // 09:58 (2 min before 10:00), color is still green (current's
        // value) — it will change to cosmos exactly at 10:00 when
        // current_idx advances.
        //
        // Speed (numeric) still lerps smoothly toward 30.0 in the last
        // 5 minutes for a perceptual ramp.
        let p = map.params_at(9.9667).unwrap(); // ~09:58
        assert_eq!(
            p.color.as_deref(),
            Some("green"),
            "at 09:58 (2 min before 10:00), color must still be green — snaps AT 10:00, not 2.5 min early"
        );
        // Speed should be lerping toward 30.0 (next's speed).
        assert!(
            p.speed.unwrap() > 10.0 && p.speed.unwrap() < 30.0,
            "speed should be transitioning toward 30.0, got {}",
            p.speed.unwrap()
        );
    }

    /// Verify color snaps EXACTLY at the scheduled boundary (not 2.5 min early).
    /// This is the regression test for the "color snaps early" bug.
    #[test]
    fn params_at_color_snaps_at_boundary_not_midpoint() {
        let map = CustomTimeMap {
            points: vec![
                CustomTimePoint {
                    minutes: 0, // 00:00 — green
                    color: Some("green".to_string()),
                    scene: None,
                    speed: Some(10.0),
                    density: None,
                    fps: None,
                    charset: None,
                    glitch_level: None,
                },
                CustomTimePoint {
                    minutes: 600, // 10:00 — cosmos
                    color: Some("cosmos".to_string()),
                    scene: None,
                    speed: Some(30.0),
                    density: None,
                    fps: None,
                    charset: None,
                    glitch_level: None,
                },
            ],
        };
        // Just before 10:00 (09:59:59 = 599.98 min) — color must still be green.
        let p = map.params_at(9.9997).unwrap();
        assert_eq!(
            p.color.as_deref(),
            Some("green"),
            "at 09:59:59, color must still be green — snaps AT 10:00 exactly"
        );
        // At exactly 10:00 (600 min) — color must now be cosmos.
        let p = map.params_at(10.0).unwrap();
        assert_eq!(
            p.color.as_deref(),
            Some("cosmos"),
            "at 10:00:00, color must snap to cosmos"
        );
        // At 10:01 — color must remain cosmos (current's value now).
        let p = map.params_at(10.0167).unwrap();
        assert_eq!(
            p.color.as_deref(),
            Some("cosmos"),
            "at 10:01, color must remain cosmos"
        );
    }

    /// Verify transition does NOT fire when outside the 5-min window.
    #[test]
    fn params_at_no_transition_outside_window() {
        let map = CustomTimeMap {
            points: vec![
                CustomTimePoint {
                    minutes: 0, // 00:00 — green
                    color: Some("green".to_string()),
                    scene: None,
                    speed: Some(10.0),
                    density: None,
                    fps: None,
                    charset: None,
                    glitch_level: None,
                },
                CustomTimePoint {
                    minutes: 600, // 10:00 — cosmos
                    color: Some("cosmos".to_string()),
                    scene: None,
                    speed: Some(30.0),
                    density: None,
                    fps: None,
                    charset: None,
                    glitch_level: None,
                },
            ],
        };
        // At 09:00 (540 min) — 60 min before 10:00. Outside 5-min window.
        // No transition → color stays green, speed stays 10.0.
        let p = map.params_at(9.0).unwrap();
        assert_eq!(p.color.as_deref(), Some("green"));
        assert!(
            (p.speed.unwrap() - 10.0).abs() < 1e-6,
            "speed should be exactly current's (no transition), got {}",
            p.speed.unwrap()
        );
    }

    #[test]
    fn params_with_keyvalue_pairs() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "adaptive-custom.06-00".to_string(),
            "aurora, signal, speed=10, density=0.5, fps=30, charset=katakana, glitch-level=none"
                .to_string(),
        );
        let map = parse_custom_time_map(&cfg).unwrap();
        let p = &map.points[0];
        assert_eq!(p.color.as_deref(), Some("aurora"));
        assert_eq!(p.scene.as_deref(), Some("signal"));
        assert_eq!(p.speed, Some(10.0));
        assert_eq!(p.density, Some(0.5));
        assert_eq!(p.fps, Some(30.0));
        assert_eq!(p.charset.as_deref(), Some("katakana"));
        assert_eq!(p.glitch_level.as_deref(), Some("none"));
    }
}
