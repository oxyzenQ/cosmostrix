// Copyright (c) 2026 rezky_nightky

//! Result-returning argument validation functions.
//!
//! Replaces the old `require_*_range` helpers with proper `Result`-returning
//! functions so that validation errors can be propagated without `process::exit`.

/// Validate that a `f64` value is finite and within `[min, max]`.
pub fn validate_f64_range(name: &str, v: f64, min: f64, max: f64) -> Result<f64, String> {
    if !v.is_finite() {
        return Err(format!(
            "failed to apply {} {} (must be a finite number)",
            name, v
        ));
    }
    if v < min || v > max {
        return Err(format!(
            "failed to apply {} {} (min {} max {})",
            name, v, min, max
        ));
    }
    Ok(v)
}

/// Validate that a `f32` value is finite and within `[min, max]`.
pub fn validate_f32_range(name: &str, v: f32, min: f32, max: f32) -> Result<f32, String> {
    if !v.is_finite() {
        return Err(format!(
            "failed to apply {} {} (must be a finite number)",
            name, v
        ));
    }
    if v < min || v > max {
        return Err(format!(
            "failed to apply {} {} (min {} max {})",
            name, v, min, max
        ));
    }
    Ok(v)
}

/// Validate that a `u8` value is within `[min, max]`.
pub fn validate_u8_range(name: &str, v: u8, min: u8, max: u8) -> Result<u8, String> {
    if v < min || v > max {
        return Err(format!(
            "failed to apply {} {} (min {} max {})",
            name, v, min, max
        ));
    }
    Ok(v)
}

/// Validate that a `u16` value is within `[min, max]`.
pub fn validate_u16_range(name: &str, v: u16, min: u16, max: u16) -> Result<u16, String> {
    if v < min || v > max {
        return Err(format!(
            "failed to apply {} {} (min {} max {})",
            name, v, min, max
        ));
    }
    Ok(v)
}
