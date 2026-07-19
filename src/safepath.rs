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
//!
//! ## Path traversal hardening (v16 audit)
//!
//! `..` segments are lexically normalized before prefix matching. This
//! prevents attacks like `--config /etc/cosmostrix/../../../tmp/leak.toml`
//! which would otherwise pass the literal-prefix check but resolve to a
//! file outside the whitelist after the OS follows the `..` components.
//! After normalization, the path is checked again — if it escapes the
//! whitelist prefix, it is rejected.

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

    // --- v16 audit: reject `..` path traversal ---
    // Lexically normalize the path so `..` and `.` segments are resolved
    // without touching the filesystem. If normalization changes the path
    // (i.e., there were any `..` to resolve) OR the normalized form no
    // longer starts with one of the whitelisted prefixes, reject.
    //
    // This blocks attacks like:
    //   /etc/cosmostrix/../../../tmp/leak.toml
    //   /etc/cosmostrix/../passwd.toml
    //   ~/.config/cosmostrix/../../etc/shadow
    //
    // Without this check, the literal prefix match below would pass and
    // std::fs::read_to_string would follow the `..` to read an arbitrary
    // file outside the whitelist.
    let expanded_str_owned = expanded_str.into_owned();
    let normalized = normalize_path_segments(&expanded_str_owned);
    if normalized.is_none() {
        // Path tried to escape above the root via excessive `..` segments.
        return false;
    }
    // Use the normalized form for prefix matching — if normalization didn't
    // change anything, `normalized` is identical to the input.
    let check_str: &str = normalized.as_deref().unwrap_or(&expanded_str_owned);

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

    // Check if the normalized path starts with any allowed prefix.
    for prefix in &allowed_prefixes {
        if check_str.starts_with(prefix.as_str()) {
            return true;
        }
    }

    false
}

/// Lexically normalize a Unix-style path by resolving `.` and `..` segments
/// without touching the filesystem. Returns `Some(normalized)` on success,
/// or `None` if `..` would escape above the root (a clear traversal attack).
///
/// Examples:
///   `/etc/cosmostrix/../passwd.toml`   → `/etc/passwd.toml`
///   `/etc/cosmostrix/./leak.toml`      → `/etc/cosmostrix/leak.toml`
///   `/etc/cosmostrix/../../etc/shadow` → `/etc/shadow`
///   `/../../etc/shadow`                → `None` (escapes above root)
///
/// Windows-style backslash paths are normalized the same way (both `/` and
/// `\` are treated as separators).
fn normalize_path_segments(path: &str) -> Option<String> {
    // Split on both `/` and `\` (Windows compat). Empty segments from
    // leading `/` or doubled separators are filtered out.
    let segments: Vec<&str> = path.split(['/', '\\']).filter(|s| !s.is_empty()).collect();

    let mut out: Vec<&str> = Vec::with_capacity(segments.len());
    for seg in segments {
        match seg {
            "." => {
                // Skip — `.` is the current directory.
            }
            ".." => {
                // Pop the last segment if any. If there is nothing to pop
                // and the path is absolute, this is an escape attempt —
                // return None so the caller rejects the path.
                if out.pop().is_none() {
                    // For absolute paths, `..` at the root means escape.
                    if path.starts_with('/') || path.contains('\\') {
                        return None;
                    }
                    // For relative paths, preserve the `..` (let the
                    // relative-path rejection above handle it).
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }

    // Reconstruct with `/` separator. Preserve leading `/` for absolute paths.
    let joined = out.join("/");
    if path.starts_with('/') {
        Some(format!("/{joined}"))
    } else {
        Some(joined)
    }
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

/// Validate a `--config <path>` argument: must be inside the strict
/// whitelist AND have a `.toml` extension. Returns `Ok(())` if valid,
/// or `Err(formatted_error_message)` if rejected.
///
/// This centralizes the security check so every code path that reads a
/// config file (`apply_config_and_runtime_defaults`, `testconf::run`,
/// `--show-scene`, `--colors-custom`, `--scene-custom`) applies the same
/// validation consistently. Previously, `--testconf` and `--show-scene`
/// bypassed `is_safe_path` entirely, allowing them to read arbitrary
/// files (e.g. `cosmostrix --testconf --config /etc/passwd` would parse
/// `/etc/passwd` as TOML and leak its content via malformed-line errors).
///
/// # Arguments
/// * `path_str` — The raw path string from `--config <path>`.
/// * `verbose` — If true, emit a verbose log line showing the safety check
///   result. Matches the behavior of the previous inline check in
///   `apply_config_and_runtime_defaults`.
pub(crate) fn validate_config_path(path_str: &str, verbose: bool) -> Result<(), String> {
    let safe = is_safe_path(path_str);
    if verbose {
        crate::output::eprintln_verbose_raw(&format!("config path: {path_str} (safe: {safe})"));
    }
    if !safe {
        return Err(format!(
            "error: --config '{path_str}' is outside allowed directories\n  \
             Allowed: ~/.config/cosmostrix/, /etc/cosmostrix/ (Linux/macOS);\n  \
             %APPDATA%\\cosmostrix\\, %ProgramData%\\cosmostrix\\ (Windows)"
        ));
    }
    // Strict: only .toml files allowed. Prevents reading arbitrary
    // file types (.c, .txt, .py, .sh, etc.) via --config.
    if !path_str.ends_with(".toml") {
        return Err(format!(
            "error: --config '{path_str}' must have a .toml extension\n  \
             Only TOML config files are accepted."
        ));
    }
    Ok(())
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

    // --- v16 audit: path traversal via `..` must be rejected ---

    #[test]
    fn etc_cosmostrix_traversal_to_passwd_rejected() {
        // Even though the literal string starts with /etc/cosmostrix/, the
        // `..` segments resolve to /etc/passwd.toml which is outside the
        // whitelist. Must be rejected.
        assert!(!is_safe_path("/etc/cosmostrix/../passwd.toml"));
        assert!(!is_safe_path("/etc/cosmostrix/../../etc/shadow"));
    }

    #[test]
    fn etc_cosmostrix_traversal_to_tmp_rejected() {
        // /etc/cosmostrix/../../../tmp/leak.toml — bypasses the /tmp/
        // rejection via path traversal. Must be rejected.
        assert!(!is_safe_path("/etc/cosmostrix/../../../tmp/leak.toml"));
        assert!(!is_safe_path("/etc/cosmostrix/../../../../tmp/any.toml"));
    }

    #[test]
    fn user_config_traversal_to_shadow_rejected() {
        // ~/.config/cosmostrix/../../etc/shadow — escapes via `..` to /etc/.
        with_test_home("/home/testuser", || {
            assert!(!is_safe_path("~/.config/cosmostrix/../../etc/shadow"));
            assert!(!is_safe_path(
                "/home/testuser/.config/cosmostrix/../../../etc/shadow"
            ));
        });
    }

    #[test]
    fn user_config_traversal_to_local_rejected() {
        // ~/.config/cosmostrix/../../.local/leak.toml — escapes to ~/.local/.
        with_test_home("/home/testuser", || {
            assert!(!is_safe_path("~/.config/cosmostrix/../../.local/leak.toml"));
        });
    }

    #[test]
    fn dot_segments_resolved_correctly() {
        // Single `.` segments are no-ops — the path stays inside the whitelist.
        with_test_home("/home/testuser", || {
            assert!(is_safe_path("~/.config/cosmostrix/./config.toml"));
            assert!(is_safe_path(
                "/home/testuser/.config/cosmostrix/./sub/file.toml"
            ));
        });
    }

    #[test]
    fn trailing_dot_dot_inside_whitelist_rejected_when_escape() {
        // /etc/cosmostrix/sub/../leak.toml — `..` stays inside the whitelist
        // (resolves to /etc/cosmostrix/leak.toml), so this is safe.
        assert!(is_safe_path("/etc/cosmostrix/sub/../leak.toml"));
        // /etc/cosmostrix/sub/../../leak.toml — `..` escapes to /etc/, unsafe.
        assert!(!is_safe_path("/etc/cosmostrix/sub/../../leak.toml"));
    }

    #[test]
    fn escape_above_root_rejected() {
        // Path that tries to go above the filesystem root via excessive `..`.
        // normalize_path_segments returns None, is_safe_path returns false.
        assert!(!is_safe_path("/../../../../etc/shadow"));
        assert!(!is_safe_path("/.."));
        assert!(!is_safe_path("/../etc/passwd"));
    }

    #[test]
    fn normalize_path_segments_unit_tests() {
        // Direct unit tests for the lexical normalizer.
        assert_eq!(
            normalize_path_segments("/etc/cosmostrix/../passwd.toml").as_deref(),
            Some("/etc/passwd.toml")
        );
        assert_eq!(
            normalize_path_segments("/etc/cosmostrix/./leak.toml").as_deref(),
            Some("/etc/cosmostrix/leak.toml")
        );
        assert_eq!(
            normalize_path_segments("/etc/cosmostrix/../../etc/shadow").as_deref(),
            Some("/etc/shadow")
        );
        // Escape above root — None.
        assert_eq!(normalize_path_segments("/../../../../etc/shadow"), None);
        assert_eq!(normalize_path_segments("/.."), None);
        // No `..` or `.` — unchanged.
        assert_eq!(
            normalize_path_segments("/etc/cosmostrix/config.toml").as_deref(),
            Some("/etc/cosmostrix/config.toml")
        );
        // Double slashes are collapsed.
        assert_eq!(
            normalize_path_segments("/etc//cosmostrix/config.toml").as_deref(),
            Some("/etc/cosmostrix/config.toml")
        );
    }
}
