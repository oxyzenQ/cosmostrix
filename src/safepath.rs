// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Path security validation for file-reading CLI flags.
//!
//! Prevents cosmostrix from being used as an arbitrary file reader.
//!
//! ## Allowed locations
//! - Home directory (`~` or `$HOME/...`) — **except** dangerous
//!   subdirectories (`.ssh/`, `.gnupg/`, `.aws/`, etc.)
//! - Current directory (relative paths not starting with `/`)
//! - Cosmostrix config directory (`~/.config/cosmostrix/...`)
//! - System config directory (`/etc/cosmostrix/...`)
//! - Temp directory (`/tmp/...`) — for testing and scripts
//!
//! ## Rejected
//! - `/etc/shadow`, `/etc/passwd`, `/proc/*`, `/sys/*`
//! - `~/.ssh/`, `~/.gnupg/`, `~/.aws/`, `~/.docker/`, `~/.kube/`
//! - `~/.bash_history`, `~/.bashrc`, `~/.profile`, `~/.netrc`
//! - `/root/*`, `/var/log/*`
//! - Any absolute path outside the allowed directories above

use std::path::PathBuf;

/// Dangerous subdirectories within the home directory that must
/// never be readable via `--config` or `--charset-file`, even though
/// they are technically under `~`.
///
/// These contain private keys, credentials, command history, and
/// other secrets that could be exfiltrated if cosmostrix were used
/// as an arbitrary file reader.
const DANGEROUS_HOME_SUBDIRS: &[&str] = &[
    ".ssh",
    ".gnupg",
    ".aws",
    ".docker",
    ".kube",
    ".config/keychain",
    ".password-store",
    ".cache",
    ".local/share/keyrings",
];

/// Dangerous dot-files in the home directory root that contain
/// credentials or shell history.
const DANGEROUS_HOME_FILES: &[&str] = &[
    ".bashrc",
    ".bash_history",
    ".bash_profile",
    ".zshrc",
    ".zsh_history",
    ".profile",
    ".netrc",
    ".env",
    ".gitconfig",
    ".npmrc",
    ".pypirc",
];

/// Check if a file path is in a safe location for reading.
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

    // Get HOME for home-directory checks.
    let home = std::env::var("HOME").ok().filter(|h| !h.is_empty());

    // --- Home directory: allowed, but check for dangerous subdirs/files ---
    if let Some(ref home) = home {
        let home_prefix = format!("{}/", home);
        if expanded_str == home.as_str() || expanded_str.starts_with(&home_prefix) {
            // Path is inside home. Check if it's in a dangerous location.
            let relative = &expanded_str[home.len()..]; // e.g. "/.ssh/id_rsa"
            let relative = relative.strip_prefix('/').unwrap_or(relative);

            // Check dangerous subdirectories: .ssh/, .gnupg/, .aws/, etc.
            for dir in DANGEROUS_HOME_SUBDIRS {
                let dir_prefix = format!("{dir}/");
                if relative.starts_with(&dir_prefix) || relative == *dir {
                    return false;
                }
            }

            // Check dangerous dot-files in home root: .bashrc, .netrc, etc.
            // Only match exact filenames at the root of home (no subdirectory).
            if !relative.contains('/') {
                for file in DANGEROUS_HOME_FILES {
                    if relative == *file {
                        return false;
                    }
                }
            }

            // Safe location within home directory.
            return true;
        }
    }

    // --- System config directory: /etc/cosmostrix/ only ---
    if expanded_str.starts_with("/etc/cosmostrix/") {
        return true;
    }

    // --- Temp directory: /tmp/ ---
    if expanded_str.starts_with("/tmp/") {
        return true;
    }

    // Everything else is rejected.
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_test_home() {
        std::env::set_var("HOME", "/home/testuser");
    }

    // --- Allowed paths ---

    #[test]
    fn relative_paths_are_safe() {
        assert!(is_safe_path("my-chars.txt"));
        assert!(is_safe_path("./config.toml"));
        assert!(is_safe_path("../shared/chars.txt"));
    }

    #[test]
    fn home_paths_are_safe() {
        set_test_home();
        assert!(is_safe_path("~/chars.txt"));
        assert!(is_safe_path("/home/testuser/chars.txt"));
        assert!(is_safe_path(
            "/home/testuser/.config/cosmostrix/config.toml"
        ));
        assert!(is_safe_path("~/Documents/my-charset.txt"));
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

    // --- Dangerous system paths rejected ---

    #[test]
    fn system_secrets_rejected() {
        assert!(!is_safe_path("/etc/shadow"));
        assert!(!is_safe_path("/etc/passwd"));
        assert!(!is_safe_path("/etc/hostname"));
        assert!(!is_safe_path("/proc/self/environ"));
        assert!(!is_safe_path("/sys/kernel/proc"));
        assert!(!is_safe_path("/root/.bashrc"));
        assert!(!is_safe_path("/var/log/auth.log"));
    }

    // --- Dangerous home paths rejected ---

    #[test]
    fn ssh_dir_rejected() {
        set_test_home();
        assert!(!is_safe_path("~/.ssh/id_rsa"));
        assert!(!is_safe_path("~/.ssh/config"));
        assert!(!is_safe_path("/home/testuser/.ssh/authorized_keys"));
        assert!(!is_safe_path("~/.ssh/"));
    }

    #[test]
    fn gnupg_dir_rejected() {
        set_test_home();
        assert!(!is_safe_path("~/.gnupg/secring.gpg"));
        assert!(!is_safe_path("~/.gnupg/"));
    }

    #[test]
    fn cloud_credential_dirs_rejected() {
        set_test_home();
        assert!(!is_safe_path("~/.aws/credentials"));
        assert!(!is_safe_path("~/.docker/config.json"));
        assert!(!is_safe_path("~/.kube/config"));
    }

    #[test]
    fn shell_config_and_history_rejected() {
        set_test_home();
        assert!(!is_safe_path("~/.bashrc"));
        assert!(!is_safe_path("~/.bash_history"));
        assert!(!is_safe_path("~/.zshrc"));
        assert!(!is_safe_path("~/.profile"));
        assert!(!is_safe_path("~/.netrc"));
        assert!(!is_safe_path("~/.env"));
        assert!(!is_safe_path("~/.gitconfig"));
    }

    #[test]
    fn password_store_rejected() {
        set_test_home();
        assert!(!is_safe_path("~/.password-store/email.gpg"));
    }

    #[test]
    fn cache_and_keyring_rejected() {
        set_test_home();
        assert!(!is_safe_path("~/.cache/credentials"));
        assert!(!is_safe_path("~/.local/share/keyrings/login.keyring"));
    }

    // --- Edge cases ---

    #[test]
    fn home_subdir_with_same_prefix_as_dangerous_is_safe() {
        set_test_home();
        // .sshbook/ should NOT be blocked just because it starts with .ssh
        assert!(is_safe_path("~/.sshbook/config"));
        // .aws_backup/ should NOT be blocked
        assert!(is_safe_path("~/.aws_backup/credentials"));
    }

    #[test]
    fn home_root_itself_is_safe() {
        set_test_home();
        assert!(is_safe_path("~"));
        assert!(is_safe_path("/home/testuser"));
    }
}
