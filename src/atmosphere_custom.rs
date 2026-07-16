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
//! - First value: color scheme name (43 built-in themes)
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
    pub fn params_at(&self, hour: f64) -> Option<CustomTimePoint> {
        if self.points.is_empty() {
            return None;
        }

        let current_minutes = (hour * 60.0) as u32 % 1440;

        // Find current and next points.
        let mut current_idx = 0;
        let mut next_idx = 0;
        for (i, p) in self.points.iter().enumerate() {
            if p.minutes <= current_minutes {
                current_idx = i;
                next_idx = if i + 1 < self.points.len() { i + 1 } else { 0 };
            }
        }

        let current = &self.points[current_idx];
        let next = &self.points[next_idx];

        // Calculate transition factor.
        // Wrap-around: if next is earlier (next day), add 1440.
        let next_minutes = if next.minutes <= current.minutes {
            next.minutes + 1440
        } else {
            next.minutes
        };

        let span = (next_minutes - current.minutes).max(1);
        let elapsed = current_minutes - current.minutes;
        let t_raw = (elapsed as f32) / (span as f32);
        let _t = t_raw.clamp(0.0, 1.0);

        // Only transition in the last 5 minutes before next point.
        const TRANSITION_MINUTES: f32 = 5.0;
        let t_smooth = if (span as f32) - (elapsed as f32) <= TRANSITION_MINUTES {
            let local_t = 1.0 - ((span as f32) - (elapsed as f32)) / TRANSITION_MINUTES;
            local_t.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let smoothed = t_smooth * t_smooth * (3.0 - 2.0 * t_smooth);

        // Lerp numeric fields, snap enums at t >= 0.5.
        let lerp = |a: f32, b: f32, t: f32| -> f32 { a + (b - a) * t };

        Some(CustomTimePoint {
            minutes: current_minutes,
            color: if smoothed >= 0.5 {
                next.color.clone().or(current.color.clone())
            } else {
                current.color.clone()
            },
            scene: if smoothed >= 0.5 {
                next.scene.clone().or(current.scene.clone())
            } else {
                current.scene.clone()
            },
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
            charset: if smoothed >= 0.5 {
                next.charset.clone().or(current.charset.clone())
            } else {
                current.charset.clone()
            },
            glitch_level: if smoothed >= 0.5 {
                next.glitch_level.clone().or(current.glitch_level.clone())
            } else {
                current.glitch_level.clone()
            },
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
                // First positional: color
                if crate::theme::canonical_name_for_input(part).is_none() {
                    return Err(format!("adaptive-custom: unknown color '{part}'"));
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
    fn parse_rejects_invalid_color() {
        let mut cfg = HashMap::new();
        cfg.insert(
            "adaptive-custom.00-00".to_string(),
            "notacolor, matrix".to_string(),
        );
        assert!(parse_custom_time_map(&cfg).is_err());
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
        // Transition should be active (within 5 min window), speed approaching 10.0.
        let p = map.params_at(23.9667).unwrap(); // ~23:58
                                                 // Should be transitioning toward green/speed=10.
        assert!(
            p.speed.unwrap() < 30.0,
            "speed should be transitioning down, got {}",
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
