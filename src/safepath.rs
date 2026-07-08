// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Path security validation for file-reading CLI flags.
//!
//! Prevents cosmostrix from being used as an arbitrary file reader.
//! Only allows reading from safe locations: home directory, current
//! directory, system config, and temp directory.

use std::path::PathBuf;

/// Check if a file path is in a safe location for reading.
///
/// Allowed locations:
/// - Home directory (`~` or `$HOME/...`)
/// - Current directory (relative paths not starting with `/`)
/// - System config directory (`/etc/cosmostrix/...`)
/// - Temp directory (`/tmp/...`) — for testing and scripts
///
/// This prevents cosmostrix from being used to read sensitive files
/// like `/etc/shadow`, `/proc/self/environ`, `/sys/...`, etc.
/// Resolves `~` to `$HOME` before checking.
pub(crate) fn is_safe_path(path: &str) -> bool {
    // Expand ~ to $HOME if present.
    let expanded = if path.starts_with("~/") {
        if let Some(home) = std::env::var("HOME").ok().filter(|h| !h.is_empty()) {
            PathBuf::from(home).join(path.strip_prefix("~/").unwrap())
        } else {
            return false;
        }
    } else if path == "~" {
        if let Some(home) = std::env::var("HOME").ok().filter(|h| !h.is_empty()) {
            PathBuf::from(home)
        } else {
            return false;
        }
    } else {
        PathBuf::from(path)
    };

    let expanded_str = expanded.to_string_lossy();

    // Relative paths (not starting with /) are in the current directory — allowed.
    if !expanded_str.starts_with('/') {
        return true;
    }

    // Absolute paths: only allow specific safe directories.
    if let Some(home) = std::env::var("HOME").ok().filter(|h| !h.is_empty()) {
        let home_prefix = format!("{}/", home);
        if expanded_str == home.as_str() || expanded_str.starts_with(&home_prefix) {
            return true;
        }
    }

    // System config directory.
    if expanded_str.starts_with("/etc/cosmostrix/") {
        return true;
    }

    // Temp directory — allowed for testing and scripts.
    if expanded_str.starts_with("/tmp/") {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_are_safe() {
        assert!(is_safe_path("my-chars.txt"));
        assert!(is_safe_path("./config.toml"));
        assert!(is_safe_path("../shared/chars.txt"));
    }

    #[test]
    fn home_paths_are_safe() {
        std::env::set_var("HOME", "/home/testuser");
        assert!(is_safe_path("~/chars.txt"));
        assert!(is_safe_path("/home/testuser/chars.txt"));
        assert!(is_safe_path(
            "/home/testuser/.config/cosmostrix/config.toml"
        ));
    }

    #[test]
    fn etc_cosmostrix_is_safe() {
        assert!(is_safe_path("/etc/cosmostrix/config.toml"));
        assert!(is_safe_path("/etc/cosmostrix/chars.txt"));
    }

    #[test]
    fn tmp_is_safe() {
        assert!(is_safe_path("/tmp/test-config.toml"));
    }

    #[test]
    fn dangerous_paths_are_rejected() {
        assert!(!is_safe_path("/etc/shadow"));
        assert!(!is_safe_path("/proc/self/environ"));
        assert!(!is_safe_path("/sys/kernel/proc"));
        assert!(!is_safe_path("/root/.bashrc"));
        assert!(!is_safe_path("/var/log/auth.log"));
    }

    #[test]
    fn etc_non_cosmostrix_is_rejected() {
        assert!(!is_safe_path("/etc/passwd"));
        assert!(!is_safe_path("/etc/hostname"));
    }
}
