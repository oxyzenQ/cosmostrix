// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Result-returning argument validation functions.
//!
//! Replaces the old `require_*_range` helpers with proper `Result`-returning
//! functions so that validation errors can be propagated without `process::exit`.

use crate::constants::{SPEED_MAX, SPEED_MIN};

/// Validate that a `f64` value is finite and within `[min, max]`.
pub fn validate_f64_range(name: &str, v: f64, min: f64, max: f64) -> Result<f64, String> {
    if !v.is_finite() {
        return Err(format!(
            "error: invalid value for {name}: {v}\nexpected a finite number"
        ));
    }
    if v < min || v > max {
        return Err(range_error(name, v, min, max));
    }
    Ok(v)
}

/// Validate user-facing rain speed.
pub fn validate_speed(v: f32) -> Result<f32, String> {
    validate_f32_range("--speed", v, SPEED_MIN, SPEED_MAX)
}

fn format_number<T: std::fmt::Display>(value: T) -> String {
    format!("{value}")
}

fn format_range(min: impl std::fmt::Display, max: impl std::fmt::Display) -> String {
    format!("{}..={}", format_number(min), format_number(max))
}

pub fn range_error(
    name: &str,
    value: impl std::fmt::Display,
    min: impl std::fmt::Display,
    max: impl std::fmt::Display,
) -> String {
    format!(
        "error: invalid value for {name}: {value}\nallowed range: {}",
        format_range(min, max)
    )
}

/// Validate that a `f32` value is finite and within `[min, max]`.
pub fn validate_f32_range(name: &str, v: f32, min: f32, max: f32) -> Result<f32, String> {
    if !v.is_finite() {
        return Err(format!(
            "error: invalid value for {name}: {v}\nexpected a finite number"
        ));
    }
    if v < min || v > max {
        return Err(range_error(name, v, min, max));
    }
    Ok(v)
}

/// Validate that a `u8` value is within `[min, max]`.
pub fn validate_u8_range(name: &str, v: u8, min: u8, max: u8) -> Result<u8, String> {
    if v < min || v > max {
        return Err(range_error(name, v, min, max));
    }
    Ok(v)
}

/// Validate that a `u16` value is within `[min, max]`.
pub fn validate_u16_range(name: &str, v: u16, min: u16, max: u16) -> Result<u16, String> {
    if v < min || v > max {
        return Err(range_error(name, v, min, max));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_accepts_safe_range_edges() {
        assert_eq!(validate_speed(SPEED_MIN).unwrap(), SPEED_MIN);
        assert_eq!(validate_speed(SPEED_MAX).unwrap(), SPEED_MAX);
    }

    #[test]
    fn speed_rejects_unsafe_values_with_human_error() {
        for value in [0.0, 0.5, 100.1, 1000.0, 100_000.0] {
            let err = validate_speed(value).expect_err("speed should be rejected");
            assert!(err.contains(&format!("error: invalid value for --speed: {value}")));
            assert!(err.contains("allowed range: 1..=100"));
            assert!(!err.contains("Custom {"));
            assert!(!err.contains("0.001"));
            assert!(!err.contains("min 0.001 max 1000"));
        }
    }
}
