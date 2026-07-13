// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Path security validation for file-reading CLI flags.
//!
//! **Strict whitelist-only** approach: only explicitly allowed directories
//! can be read. Everything else is rejected — no blacklist to maintain.
//!
//! ## Allowed locations
//!
//! Linux:
//! - `~/.config/cosmostrix/` — user config (XDG)
//! - `/etc/cosmostrix/` — system-wide config
//!
//! macOS:
//! - `~/.config/cosmostrix/` — user config (XDG compat)
//! - `~/Library/Application Support/cosmostrix/` — user config (macOS native)
//! - `/etc/cosmostrix/` — system-wide config
//!
//! Windows:
//! - `%APPDATA%\cosmostrix\` — user config (Roaming)
//! - `%ProgramData%\cosmostrix\` — system-wide config
//!
//! ## Rejected (v14.0.0 strict policy)
//!
//! Everything else, including: `.` / current directory / relative paths
//! (was allowed pre-v14), `/tmp/` (was allowed pre-v14), `~/.local/config/`,
//! `/usr/`, `/opt/`, `/var/`, `~/.ssh/`, `/etc/shadow`, `~/.aws/`, `/proc/`,
//! `/sys/`, `~/.bashrc`. No blacklist needed — if it's not in the whitelist,
//! it's denied.

use std::path::PathBuf;

/// Check if a file path is in a safe location for reading.
///
/// Strict whitelist-only: returns `true` if the path is inside one of the
/// explicitly allowed cosmostrix directories. Everything else returns `false`.
///
/// Cross-platform allowed locations:
/// - Linux: `~/.config/cosmostrix/`, `/etc/cosmostrix/`
/// - macOS: `~/.config/cosmostrix/`, `~/Library/Application Support/cosmostrix/`, `/etc/cosmostrix/`
/// - Windows: `%APPDATA%\cosmostrix\`, `%ProgramData%\cosmostrix\`
pub(crate) fn is_safe_path(path: &str) -> bool {
    let expanded = expand_tilde(path);
    let expanded_str = expanded.to_string_lossy();

    // --- Security: reject unexpanded ~ paths (HOME not set) ---
    // If ~/... couldn't be expanded (HOME unset), the literal "~/..." is
    // NOT safe — it's a directory traversal attempt or missing env.
    if expanded_str.starts_with("~/") || expanded_str == "~" {
        return false;
    }

    // --- v14.0.0: reject relative paths (current directory no longer allowed) ---
    // Pre-v14, relative paths like "./config.toml" were allowed. This was a
    // security risk (symlink attacks, shared working directories). Now only
    // absolute paths inside the whitelisted cosmostrix directories are allowed.
    if !expanded_str.starts_with('/') && !expanded_str.contains('\\') {
        return false;
    }

    // --- Whitelist of allowed absolute path prefixes ---
    let mut allowed_prefixes: Vec<String> = Vec::new();

    // Linux/macOS: ~/.config/cosmostrix/
    if let Some(home) = std::env::var("HOME").ok().filter(|h| !h.is_empty()) {
        allowed_prefixes.push(format!("{home}/.config/cosmostrix/"));
        // macOS native: ~/Library/Application Support/cosmostrix/
        #[cfg(target_os = "macos")]
        allowed_prefixes.push(format!("{home}/Library/Application Support/cosmostrix/"));
    }

    // Linux/macOS: /etc/cosmostrix/ (system-wide)
    #[cfg(unix)]
    allowed_prefixes.push("/etc/cosmostrix/".to_string());

    // Windows: %APPDATA%\cosmostrix\ (user)
    #[cfg(windows)]
    if let Some(appdata) = std::env::var("APPDATA").ok().filter(|a| !a.is_empty()) {
        allowed_prefixes.push(format!("{appdata}\\cosmostrix\\"));
    }

    // Windows: %ProgramData%\cosmostrix\ (system-wide)
    #[cfg(windows)]
    if let Some(progdata) = std::env::var("ProgramData").ok().filter(|p| !p.is_empty()) {
        allowed_prefixes.push(format!("{progdata}\\cosmostrix\\"));
    }

    // Test-only override: allow COSMOSTRIX_TEST_CONFIG_DIR for test configs.
    // This env var is ONLY respected in test builds (#[cfg(test)] ensures the
    // block is compiled out of release binaries). Test helpers set it to a
    // temp directory so they can write config files without polluting
    // ~/.config/cosmostrix/.
    #[cfg(test)]
    if let Ok(test_dir) = std::env::var("COSMOSTRIX_TEST_CONFIG_DIR") {
        let trimmed = test_dir.trim_end_matches('/');
        if !trimmed.is_empty() {
            allowed_prefixes.push(format!("{trimmed}/"));
        }
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

    // --- v14.0.0: relative paths now REJECTED (was allowed pre-v14) ---

    #[test]
    fn relative_paths_are_rejected_v14() {
        // v14.0.0 strict policy: current directory is no longer allowed.
        // Only absolute paths inside whitelisted cosmostrix directories pass.
        assert!(!is_safe_path("my-chars.txt"));
        assert!(!is_safe_path("./config.toml"));
        assert!(!is_safe_path("../shared/chars.txt"));
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

    // --- v14.0.0: /tmp/ now REJECTED (was allowed pre-v14) ---

    #[test]
    fn tmp_is_rejected_v14() {
        // v14.0.0 strict policy: /tmp/ no longer in whitelist.
        // NOTE: Other parallel tests set COSMOSTRIX_TEST_CONFIG_DIR=/tmp,
        // so we must explicitly clear it here to verify production behavior.
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        std::env::remove_var("COSMOSTRIX_TEST_CONFIG_DIR");
        assert!(!is_safe_path("/tmp/test-config.toml"));
        assert!(!is_safe_path("/tmp/cosmostrix-chars.txt"));
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

    // --- v14.0.0: ~/.local/ paths rejected ---

    #[test]
    fn local_config_rejected_v14() {
        with_test_home("/home/testuser", || {
            assert!(!is_safe_path("~/.local/config/cosmostrix/config.toml"));
            assert!(!is_safe_path("~/.local/share/cosmostrix/config.toml"));
        });
    }

    // --- v14.0.0: /usr/ paths rejected ---

    #[test]
    fn usr_paths_rejected_v14() {
        assert!(!is_safe_path("/usr/share/cosmostrix/config.toml"));
        assert!(!is_safe_path("/usr/local/etc/cosmostrix/config.toml"));
    }

    // --- Security: unexpanded ~ when HOME is unset ---

    #[test]
    fn unexpanded_tilde_rejected_when_home_unset() {
        with_test_home("", || {
            std::env::remove_var("HOME");
            // When HOME is unset, ~/... cannot expand. The literal "~/..."
            // must NOT be treated as a relative safe path.
            assert!(
                !is_safe_path("~/.ssh/id_rsa"),
                "unexpanded ~/ must be rejected"
            );
            assert!(
                !is_safe_path("~/.aws/credentials"),
                "unexpanded ~/ must be rejected"
            );
            assert!(!is_safe_path("~/.bashrc"), "unexpanded ~/ must be rejected");
            assert!(!is_safe_path("~"), "unexpanded ~ must be rejected");
        });
    }
}
