// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! CLI value types + the explicit-bool value_parser for the Args
//! derive (config/mod.rs).
//!
//! NIGHT-perf-2 LOC refactor (the fps_intent.rs precedent): this block
//! lived in config/mod.rs, which sat at exactly the 800-LOC cap — the
//! new --bench-cosmetics flag needed headroom. Pure code motion: every
//! item is re-exported from config/mod.rs so all existing
//! `crate::config::X` paths resolve unchanged.
//!
//! Contents:
//! - `parse_true_false` — the v50-beta.3 explicit-bool value_parser.
//! - `ColorBg` / `GlitchLevel` — clap ValueEnum presets.
//! - `U16Range` — the "NUM1,NUM2" range parser.

use std::str::FromStr;

/// v50-beta.3: clap value_parser for boolean CLI flags that MUST receive
/// an explicit `true`/`false` value (no bare-flag toggle). This prevents
/// the silent-ignore class of bugs where a user types `--crystal-dragon`
/// expecting an error or a toggle, but clap quietly sets the bool to true.
///
/// Accepted values (case-insensitive): `true`, `false`, `1`, `0`, `yes`,
/// `no`, `on`, `off`. Any other input → clap error.
///
/// Used by: `--crystal-dragon`, `--power-dragon`, `--msg-mode`.
pub(crate) fn parse_true_false(input: &str) -> Result<bool, String> {
    match input.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        other => {
            // v80.0.0-alpha.2 (owner typo `--crystal-dragon 10`): hint the
            // -secs twin when a NUMBER lands on a bool flag.
            let hint = if other.parse::<f64>().is_ok() {
                " — numeric values are not booleans; for seconds use --crystal-dragon-secs (e.g. --crystal-dragon-secs 10)"
            } else {
                ""
            };
            Err(format!(
                "invalid boolean value '{other}' (expected: true|false|1|0|yes|no|on|off){hint}"
            ))
        }
    }
}

/// Test-only accessor for the `parse_true_false` value_parser (pub(crate) wrapper; tests cannot reach the private fn directly).
#[cfg(test)]
pub(crate) fn test_parse_true_false(input: &str) -> Result<bool, String> {
    parse_true_false(input)
}

// Enums

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorBg {
    #[value(name = "black")]
    Black,
    // Both "default-background" (kebab-case, canonical CLI form) and
    // "default_background" (snake_case) are accepted by config.toml
    // parsing (configfile.rs, config_apply.rs, profile.rs, live_config.rs,
    // testconf.rs) via explicit match arms. The CLI exposes only the
    // canonical kebab-case name to avoid duplicate entries in error output.
    #[value(name = "default-background")]
    DefaultBackground,
}

/// Glitch intensity presets. Provides a grouped interface over individual
/// glitch tuning parameters (glitchpct, glitch-ms, shortpct, rippct).
#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlitchLevel {
    #[value(name = "none")]
    None,
    #[value(name = "subtle")]
    Subtle,
    #[value(name = "default")]
    Default,
    #[value(name = "intense")]
    Intense,
}

// U16Range

#[derive(Clone, Copy, Debug)]
pub struct U16Range {
    pub low: u16,
    pub high: u16,
}

impl FromStr for U16Range {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (a, b) = s
            .split_once(',')
            .ok_or_else(|| "expected: NUM1,NUM2".to_string())?;
        let low: u16 = a
            .trim()
            .parse()
            .map_err(|_| "invalid low value".to_string())?;
        let high: u16 = b
            .trim()
            .parse()
            .map_err(|_| "invalid high value".to_string())?;
        if low == 0 || high == 0 || low > high {
            return Err("range must be >0 and low <= high (min allowed value is 1)".to_string());
        }
        Ok(Self { low, high })
    }
}
