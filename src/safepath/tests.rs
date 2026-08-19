// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! safepath tests, extracted from inline `mod tests { ... }` block.
//!
//! Uses `use super::*;` to access parent's private items unchanged.

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

// --- Allowed: /usr/local/etc/cosmostrix/ (FreeBSD system-wide) ---

#[test]
fn freebsd_usr_local_etc_cosmostrix_is_safe() {
    // FreeBSD uses /usr/local/etc/ for ports/packages (NOT /etc/ which
    // is reserved for the base system). The whitelist must accept this
    // path on all unix targets because the same binary may run on
    // GhostBSD or other FreeBSD descendants. On Linux/macOS the extra
    // prefix is harmless — it just won't match any real path there.
    // This test verifies the whitelist entry exists and matches.
    assert!(is_safe_path("/usr/local/etc/cosmostrix/config.toml"));
    assert!(is_safe_path("/usr/local/etc/cosmostrix/chars.txt"));
    // Subdirectories are also allowed (consistent with /etc/cosmostrix/).
    assert!(is_safe_path(
        "/usr/local/etc/cosmostrix/profiles/nightcore.toml"
    ));
}

// --- Allowed: $PREFIX/etc/cosmostrix/ (Termux system-wide) ---

#[test]
fn termux_prefix_etc_cosmostrix_is_safe() {
    // Termux's $PREFIX is typically /data/data/com.termux/files/usr.
    // The system-wide config lives at $PREFIX/etc/cosmostrix/config.toml.
    // This test sets TERMUX_VERSION + PREFIX to simulate a Termux
    // environment and verifies the whitelist accepts $PREFIX/etc/cosmostrix/.
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::set_var("TERMUX_VERSION", "0.118.0");
    std::env::set_var("PREFIX", "/data/data/com.termux/files/usr");
    let result = is_safe_path("/data/data/com.termux/files/usr/etc/cosmostrix/config.toml");
    // Cleanup env vars (best-effort — don't leak the Termux simulation
    // to other tests). Use remove_var instead of unset because some
    // tests may have set TERMUX_VERSION earlier.
    std::env::remove_var("TERMUX_VERSION");
    std::env::remove_var("PREFIX");
    assert!(
        result,
        "Termux $PREFIX/etc/cosmostrix/ must be whitelisted when TERMUX_VERSION is set"
    );
}

#[test]
fn termux_prefix_etc_cosmostrix_rejected_when_prefix_unset() {
    // When PREFIX env var is unset (non-Termux environment), the
    // $PREFIX/etc/cosmostrix/ whitelist entry is NOT added — so a
    // literal path that looks like a Termux path must be rejected.
    // This verifies the whitelist entry is conditional on PREFIX being set.
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    std::env::remove_var("TERMUX_VERSION");
    std::env::remove_var("PREFIX");
    assert!(
        !is_safe_path("/data/data/com.termux/files/usr/etc/cosmostrix/config.toml"),
        "Termux system-wide path must be rejected when PREFIX env var is unset"
    );
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

// --- v14.0.0: /usr/ paths rejected (with FreeBSD exception) ---

#[test]
fn usr_paths_rejected_v14() {
    // v14.0.0 strict policy: /usr/ paths are rejected.
    // EXCEPTION: /usr/local/etc/cosmostrix/ is ALLOWED on
    // FreeBSD because the ports/packages convention puts system-wide
    // config there, NOT in /etc/ (which is reserved for the base
    // system). See `freebsd_usr_local_etc_cosmostrix_is_safe` for
    // the positive test. All other /usr/ paths are still rejected.
    assert!(!is_safe_path("/usr/share/cosmostrix/config.toml"));
    assert!(!is_safe_path("/usr/local/share/cosmostrix/config.toml"));
    assert!(!is_safe_path("/usr/local/cosmostrix/config.toml"));
    // /usr/local/etc/cosmostrix/ IS allowed (FreeBSD convention) —
    // see `freebsd_usr_local_etc_cosmostrix_is_safe`.
    // But /usr/local/etc/ outside the cosmostrix subdir is rejected:
    assert!(!is_safe_path("/usr/local/etc/cosmostrix-other/config.toml"));
    assert!(!is_safe_path("/usr/local/etc/foo.toml"));
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

// --- Windows drive-letter path normalization ---
// These tests exercise normalize_path_segments for C:\... paths.
// They run on ALL platforms because the normalizer is cross-platform.

#[test]
fn normalize_drive_letter_path() {
    // C:\Users\test\config.toml → C:/Users/test/config.toml
    assert_eq!(
        normalize_path_segments(r"C:\Users\test\config.toml").as_deref(),
        Some("C:/Users/test/config.toml")
    );
}

#[test]
fn normalize_drive_letter_with_dot_dot() {
    // C:\Users\test\..\other\file.toml → C:/Users/other/file.toml
    assert_eq!(
        normalize_path_segments(r"C:\Users\test\..\other\file.toml").as_deref(),
        Some("C:/Users/other/file.toml")
    );
}

#[test]
fn normalize_drive_letter_escape_above_root_rejected() {
    // C:\..\..\etc\shadow — `..` above drive root is an escape attempt.
    assert_eq!(normalize_path_segments(r"C:\..\..\etc\shadow"), None);
}

#[test]
fn normalize_drive_letter_forward_slash() {
    // C:/Users/test/config.toml (forward slashes) — should also work.
    assert_eq!(
        normalize_path_segments("C:/Users/test/config.toml").as_deref(),
        Some("C:/Users/test/config.toml")
    );
}

// --- UNC path normalization ---

#[test]
fn normalize_unc_path() {
    // \\server\share\cosmostrix\config.toml → //server/share/cosmostrix/config.toml
    assert_eq!(
        normalize_path_segments(r"\\server\share\cosmostrix\config.toml").as_deref(),
        Some("//server/share/cosmostrix/config.toml")
    );
}

#[test]
fn normalize_unc_path_with_dot_dot() {
    // \\server\share\cosmostrix\..\other.toml → //server/share/other.toml
    assert_eq!(
        normalize_path_segments(r"\\server\share\cosmostrix\..\other.toml").as_deref(),
        Some("//server/share/other.toml")
    );
}

#[test]
fn normalize_unc_path_escape_above_share_rejected() {
    // \\server\share\..\..\etc\shadow — `..` above \\server\share is escape.
    assert_eq!(
        normalize_path_segments(r"\\server\share\..\..\etc\shadow"),
        None
    );
}

#[test]
fn normalize_unc_path_forward_slash() {
    // //server/share/cosmostrix/config.toml (forward slashes)
    assert_eq!(
        normalize_path_segments("//server/share/cosmostrix/config.toml").as_deref(),
        Some("//server/share/cosmostrix/config.toml")
    );
}

// --- Windows-specific integration tests ---
// These only run on Windows where APPDATA etc. are available.

#[cfg(windows)]
mod windows_tests {
    use super::*;

    /// Helper to set an env var and restore it on drop.
    struct EnvGuard {
        key: String,
        old_val: Option<String>,
    }
    impl EnvGuard {
        fn set(key: &str, val: &str) -> Self {
            let old_val = std::env::var(key).ok();
            std::env::set_var(key, val);
            Self {
                key: key.to_string(),
                old_val,
            }
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.old_val {
                Some(v) => std::env::set_var(&self.key, v),
                None => std::env::remove_var(&self.key),
            }
        }
    }

    #[test]
    fn appdata_path_is_safe() {
        // A path inside %APPDATA%\cosmostrix\ should be accepted.
        if let Ok(appdata) = std::env::var("APPDATA") {
            let path = format!(r"{}\cosmostrix\config.toml", appdata);
            assert!(
                is_safe_path(&path),
                "APPDATA cosmostrix path should be safe: {path}"
            );
        }
    }

    #[test]
    fn appdata_path_with_forward_slash_is_safe() {
        // Same path with / separators should also be accepted.
        if let Ok(appdata) = std::env::var("APPDATA") {
            let path = format!("{}/cosmostrix/config.toml", appdata.replace('\\', "/"));
            assert!(
                is_safe_path(&path),
                "APPDATA cosmostrix path with / separators should be safe: {path}"
            );
        }
    }

    #[test]
    fn appdata_path_case_insensitive() {
        // Path casing differs from APPDATA env var — should still match.
        if let Ok(appdata) = std::env::var("APPDATA") {
            let path_lower = appdata.to_lowercase();
            let path = format!(r"{}\cosmostrix\config.toml", path_lower);
            assert!(
                is_safe_path(&path),
                "APPDATA path with different casing should be safe (case-insensitive): {path}"
            );
        }
    }

    #[test]
    fn programdata_path_is_safe() {
        // A path inside %ProgramData%\cosmostrix\ should be accepted.
        if let Ok(progdata) = std::env::var("ProgramData") {
            let path = format!(r"{}\cosmostrix\config.toml", progdata);
            assert!(
                is_safe_path(&path),
                "ProgramData cosmostrix path should be safe: {path}"
            );
        }
    }

    #[test]
    fn percent_appdata_expansion() {
        // %APPDATA%\cosmostrix\config.toml should be expanded and accepted.
        let path = r"%APPDATA%\cosmostrix\config.toml";
        assert!(
            is_safe_path(path),
            "%APPDATA% should be expanded and path should be safe"
        );
    }

    #[test]
    fn percent_appdata_lowercase_expansion() {
        // %appdata%\cosmostrix\config.toml (lowercase) should also work.
        let path = r"%appdata%\cosmostrix\config.toml";
        assert!(
            is_safe_path(path),
            "%appdata% (lowercase) should be expanded and path should be safe"
        );
    }

    #[test]
    fn windows_rejects_system32() {
        // C:\Windows\System32\config.toml should be rejected.
        assert!(!is_safe_path(r"C:\Windows\System32\config.toml"));
    }

    #[test]
    fn windows_rejects_c_root() {
        // C:\config.toml should be rejected (not inside whitelist).
        assert!(!is_safe_path(r"C:\config.toml"));
    }

    #[test]
    fn userprofile_tilde_expansion() {
        // ~/... should expand via USERPROFILE when HOME is not set.
        let _guard_home = EnvGuard::set("HOME", "");
        std::env::remove_var("HOME");
        // On Windows, USERPROFILE should allow tilde expansion.
        // We can't fully test this without a real USERPROFILE value,
        // but we verify the function doesn't panic.
        let _ = is_safe_path("~/.config/cosmostrix/config.toml");
    }
}
