// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Path security validation for file-reading CLI flags.
//!
//! **Whitelist-only** approach: only explicitly allowed directories
//! can be read. Everything else is rejected — no blacklist to maintain.
//!
//! ## Allowed locations (cross-platform)
//! - `~/.config/cosmostrix/` — cosmostrix config directory
//! - Current directory (`.` or relative paths)
//! - `/etc/cosmostrix/` — system-wide config (Linux/macOS)
//! - `%APPDATA%\cosmostrix\` — Windows app data
//! - `/tmp/` — temp directory (for testing/scripts)
//!
//! ## Rejected
//! Everything else, including: `~/.ssh/`, `/etc/shadow`, `~/.aws/`,
//! `/proc/`, `/sys/`, `~/.bashrc`, etc. No blacklist needed — if
//! it's not in the whitelist, it's denied.

use std::path::PathBuf;

/// Check if a file path is in a safe location for reading.
///
/// Whitelist-only: returns `true` if the path is inside one of the
/// explicitly allowed directories. Everything else returns `false`.
///
/// Cross-platform:
/// - Linux/macOS: `~/.config/cosmostrix/`, `.`, `/etc/cosmostrix/`, `/tmp/`
/// - Windows: `%APPDATA%\cosmostrix\`, `.`, temp dir
pub(crate) fn is_safe_path(path: &str) -> bool {
    let expanded = expand_tilde(path);
    let expanded_str = expanded.to_string_lossy();

    // --- Security: reject unexpanded ~ paths (HOME not set) ---
    // If ~/... couldn't be expanded (HOME unset), the literal "~/..." is
    // NOT safe — it's a directory traversal attempt or missing env.
    if expanded_str.starts_with("~/") || expanded_str == "~" {
        return false;
    }

    // --- Relative paths: current directory — always allowed ---
    if !expanded_str.starts_with('/') && !expanded_str.contains('\\') {
        return true;
    }

    // --- Whitelist of allowed absolute path prefixes ---
    let mut allowed_prefixes: Vec<String> = Vec::new();

    // 1. ~/.config/cosmostrix/ (Linux/macOS)
    if let Some(home) = std::env::var("HOME").ok().filter(|h| !h.is_empty()) {
        allowed_prefixes.push(format!("{home}/.config/cosmostrix/"));
    }

    // 2. /etc/cosmostrix/ (Linux/macOS system-wide)
    allowed_prefixes.push("/etc/cosmostrix/".to_string());

    // 3. /tmp/ (Linux/macOS temp)
    allowed_prefixes.push("/tmp/".to_string());

    // 4. Windows: %APPDATA%\cosmostrix\
    if let Some(appdata) = std::env::var("APPDATA").ok().filter(|a| !a.is_empty()) {
        allowed_prefixes.push(format!("{appdata}\\cosmostrix\\"));
    }

    // 5. Windows: temp directory
    if let Some(temp) = std::env::var("TEMP").ok().filter(|t| !t.is_empty()) {
        allowed_prefixes.push(format!("{temp}\\"));
    }

    // Check if the expanded path starts with any allowed prefix.
    for prefix in &allowed_prefixes {
        if expanded_str.starts_with(prefix.as_str()) {
            return true;
        }
    }

    false
}

/// Expand `~` to `$HOME` if present. Returns the path as-is if no tilde.
fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Some(home) = std::env::var("HOME").ok().filter(|h| !h.is_empty()) {
            return PathBuf::from(home).join(path.strip_prefix("~/").unwrap());
        }
    }
    if path == "~" {
        if let Some(home) = std::env::var("HOME").ok().filter(|h| !h.is_empty()) {
            return PathBuf::from(home);
        }
    }
    PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Mutex to serialize tests that mutate HOME env var.
    /// Without this, parallel tests race on the global env state.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_test_home<F: FnOnce()>(home: &str, f: F) {
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let old_home = std::env::var("HOME").ok();
        std::env::set_var("HOME", home);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        // Restore old HOME
        match old_home {
            Some(h) => std::env::set_var("HOME", h),
            None => std::env::remove_var("HOME"),
        }
        if let Err(e) = result {
            std::panic::resume_unwind(e);
        }
    }

    // --- Allowed: relative paths (current directory) ---

    #[test]
    fn relative_paths_are_safe() {
        assert!(is_safe_path("my-chars.txt"));
        assert!(is_safe_path("./config.toml"));
        assert!(is_safe_path("../shared/chars.txt"));
    }

    // --- Allowed: ~/.config/cosmostrix/ ---

    #[test]
    fn cosmostrix_config_dir_is_safe() {
        with_test_home("/home/testuser", || {
            assert!(is_safe_path("~/.config/cosmostrix/config.toml"));
            assert!(is_safe_path(
                "/home/testuser/.config/cosmostrix/my-chars.txt"
            ));
            assert!(is_safe_path("~/.config/cosmostrix/profiles/nightcore.toml"));
        });
    }

    // --- Allowed: /etc/cosmostrix/ ---

    #[test]
    fn etc_cosmostrix_is_safe() {
        assert!(is_safe_path("/etc/cosmostrix/config.toml"));
        assert!(is_safe_path("/etc/cosmostrix/chars.txt"));
    }

    // --- Allowed: /tmp/ ---

    #[test]
    fn tmp_is_safe() {
        assert!(is_safe_path("/tmp/test-config.toml"));
        assert!(is_safe_path("/tmp/cosmostrix-chars.txt"));
    }

    // --- Rejected: everything else ---

    #[test]
    fn home_root_rejected() {
        with_test_home("/home/testuser", || {
            assert!(!is_safe_path("~"));
            assert!(!is_safe_path("/home/testuser"));
            assert!(!is_safe_path("/home/testuser/chars.txt"));
            assert!(!is_safe_path("~/Documents/chars.txt"));
        });
    }

    #[test]
    fn ssh_dir_rejected() {
        with_test_home("/home/testuser", || {
            assert!(!is_safe_path("~/.ssh/id_rsa"));
            assert!(!is_safe_path("/home/testuser/.ssh/config"));
        });
    }

    #[test]
    fn aws_creds_rejected() {
        with_test_home("/home/testuser", || {
            assert!(!is_safe_path("~/.aws/credentials"));
        });
    }

    #[test]
    fn system_secrets_rejected() {
        assert!(!is_safe_path("/etc/shadow"));
        assert!(!is_safe_path("/etc/passwd"));
        assert!(!is_safe_path("/proc/self/environ"));
        assert!(!is_safe_path("/sys/kernel/proc"));
        assert!(!is_safe_path("/root/.bashrc"));
        assert!(!is_safe_path("/var/log/auth.log"));
    }

    #[test]
    fn shell_config_rejected() {
        with_test_home("/home/testuser", || {
            assert!(!is_safe_path("~/.bashrc"));
            assert!(!is_safe_path("~/.bash_history"));
            assert!(!is_safe_path("~/.netrc"));
            assert!(!is_safe_path("~/.env"));
        });
    }

    #[test]
    fn arbitrary_paths_rejected() {
        assert!(!is_safe_path("/opt/data/config.toml"));
        assert!(!is_safe_path("/usr/share/chars.txt"));
        assert!(!is_safe_path("/home/other-user/file.txt"));
    }

    #[test]
    fn etc_non_cosmostrix_rejected() {
        assert!(!is_safe_path("/etc/passwd"));
        assert!(!is_safe_path("/etc/nginx/nginx.conf"));
    }

    // --- Security: unexpanded ~ when HOME is unset ---

    #[test]
    fn unexpanded_tilde_rejected_when_home_unset() {
        with_test_home("", || {
            std::env::remove_var("HOME");
            // When HOME is unset, ~/... cannot expand. The literal "~/..."
            // must NOT be treated as a relative safe path.
            assert!(!is_safe_path("~/.ssh/id_rsa"), "unexpanded ~/ must be rejected");
            assert!(!is_safe_path("~/.aws/credentials"), "unexpanded ~/ must be rejected");
            assert!(!is_safe_path("~/.bashrc"), "unexpanded ~/ must be rejected");
            assert!(!is_safe_path("~"), "unexpanded ~ must be rejected");
        });
    }
}
