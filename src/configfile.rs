// Copyright (c) 2026 rezky_nightky

//! Simple key=value config file support.
//!
//! Reads `~/.config/cosmostrix/config` (or `$XDG_CONFIG_HOME/cosmostrix/config`).
//! Format: one `key = value` per line, `#` comments, blank lines ignored.
//! Config file values serve as defaults; CLI args always take precedence.

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use crate::constants::{CONFIG_DIR_NAME, CONFIG_FILE_NAME};

/// Load config file and return a HashMap of key → value pairs.
/// Returns empty HashMap if file doesn't exist or can't be read.
/// Warns on stderr for unrecognized keys (likely typos).
#[must_use]
pub fn load_config_file() -> HashMap<String, String> {
    let path = config_file_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };

    /// Known configuration keys (lowercase). Anything else is a likely typo.
    const KNOWN_KEYS: &[&str] = &[
        "color",
        "charset",
        "fps",
        "speed",
        "density",
        "bold",
        "shadingmode",
        "glitchpct",
        "shortpct",
        "rippct",
        "maxdpc",
    ];

    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        // Skip comments and blank lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Parse key = value
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim().to_string();
            if !key.is_empty() && !value.is_empty() {
                if !KNOWN_KEYS.contains(&key.as_str()) {
                    eprintln!(
                        "config: ignoring unknown key '{}' (known: {})",
                        key,
                        KNOWN_KEYS.join(", ")
                    );
                    continue;
                }
                map.insert(key, value);
            }
        }
    }
    map
}

/// Returns the path to the config file.
/// Uses `$XDG_CONFIG_HOME` if set, otherwise `~/.config`.
#[must_use]
fn config_file_path() -> PathBuf {
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
            .join(CONFIG_DIR_NAME)
            .join(CONFIG_FILE_NAME)
    } else if let Ok(home) = env::var("HOME") {
        PathBuf::from(home)
            .join(".config")
            .join(CONFIG_DIR_NAME)
            .join(CONFIG_FILE_NAME)
    } else {
        PathBuf::from(".config")
            .join(CONFIG_DIR_NAME)
            .join(CONFIG_FILE_NAME)
    }
}
