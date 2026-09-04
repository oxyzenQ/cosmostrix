// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Path security validation for file-reading CLI flags.
//!
//! Strict whitelist-only approach: only explicitly allowed directories
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
//! FreeBSD:
//! - `~/.config/cosmostrix/` — user config (XDG)
//! - `/usr/local/etc/cosmostrix/` — system-wide config (FreeBSD uses
//!   `/usr/local/etc/` for ports/packages, NOT `/etc/` which is reserved
//!   for the base system)
//!
//! Windows:
//! - `%APPDATA%\cosmostrix\` — user config (Roaming)
//! - `%ProgramData%\cosmostrix\` — system-wide config
//!
//! Android (via Termux):
//! - `~/.config/cosmostrix/` — user config (Termux HOME; XDG_CONFIG_HOME
//!   is deliberately ignored on Termux because it may point to $PREFIX/etc)
//! - `$PREFIX/etc/cosmostrix/` — system-wide config (typically
//!   `/data/data/com.termux/files/usr/etc/cosmostrix/`)
//! - `/sdcard/cosmostrix/` — external storage (writable without root,
//!   accessible from other Android apps)
//!
//! Termux detection is RUNTIME (via `TERMUX_VERSION` / `PREFIX` env vars),
//! NOT compile-time — Termux installs regular Linux ARM binaries, so
//! `#[cfg(target_os = "android")]` would never match a Termux build.
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

/// Expand Windows-style environment variables like `%APPDATA%`, `%USERPROFILE%`,
/// `%ProgramData%` in a path string. On non-Windows, returns the path unchanged.
///
/// This is needed because when a user passes `%APPDATA%\cosmostrix\config.toml`
/// on the command line, the shell does NOT expand `%VAR%` (unlike Unix `$VAR`).
/// Without this expansion, `is_safe_path` would see the literal `%APPDATA%` prefix
/// and reject the path since it doesn't match the resolved `C:\Users\...\AppData\Roaming\cosmostrix\`.
#[cfg(windows)]
fn expand_windows_env_vars(path: &str) -> String {
    let mut result = path.to_string();
    // Common Windows env vars that users reference in paths.
    // std::env::var uses the OS native lookup (case-insensitive on Windows).
    for (var, val) in [
        ("APPDATA", std::env::var("APPDATA")),
        ("USERPROFILE", std::env::var("USERPROFILE")),
        ("ProgramData", std::env::var("ProgramData")),
        ("LOCALAPPDATA", std::env::var("LOCALAPPDATA")),
        ("HOME", std::env::var("HOME")),
    ] {
        if let Ok(v) = val {
            if !v.is_empty() {
                let pattern_upper = format!("%{var}%");
                let var_lower = var.to_lowercase();
                let pattern_lower = format!("%{var_lower}%");
                // Replace all occurrences (case-insensitive on Windows).
                // Windows env vars are case-insensitive (%APPDATA% == %appdata%),
                // but Rust string replacement is case-sensitive, so we must
                // try both the original and lowercased form.
                result = result.replace(&pattern_upper, &v);
                if pattern_lower != pattern_upper {
                    result = result.replace(&pattern_lower, &v);
                }
            }
        }
    }
    result
}

#[cfg(not(windows))]
#[inline]
fn expand_windows_env_vars(path: &str) -> String {
    path.to_string()
}

fn push_normalized_allowed_prefix(allowed_prefixes: &mut Vec<String>, raw_prefix: String) {
    let trimmed = raw_prefix.trim_end_matches(['/', '\\']);
    if trimmed.is_empty() {
        return;
    }
    let normalized = normalize_path_segments(trimmed).unwrap_or_else(|| trimmed.replace('\\', "/"));
    allowed_prefixes.push(format!("{normalized}/"));
}

/// Check if a file path is in a safe location for reading.
///
/// Strict whitelist-only: returns `true` if the path is inside one of the
/// explicitly allowed cosmostrix directories. Everything else returns `false`.
///
/// Cross-platform allowed locations:
/// - Linux: `~/.config/cosmostrix/`, `/etc/cosmostrix/`
/// - macOS: `~/.config/cosmostrix/`, `~/Library/Application Support/cosmostrix/`, `/etc/cosmostrix/`
/// - FreeBSD: `~/.config/cosmostrix/`, `/usr/local/etc/cosmostrix/`
/// - Windows: `%APPDATA%\cosmostrix\`, `%ProgramData%\cosmostrix\`
/// - Android (Termux, runtime-detected): `~/.config/cosmostrix/`,
///   `$PREFIX/etc/cosmostrix/`, `/sdcard/cosmostrix/`
pub(crate) fn is_safe_path(path: &str) -> bool {
    // On Windows, expand %VAR% environment variables first.
    // The shell does NOT expand %APPDATA% etc., so we must do it here.
    let path = expand_windows_env_vars(path);

    let expanded = expand_tilde(&path);
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
    //
    // Absolute path detection:
    //   Unix: starts with /
    //   Windows: starts with a drive letter (C:\) or a UNC path (\\)
    let is_absolute = expanded_str.starts_with('/')
        || (expanded_str.len() >= 3
            && expanded_str.as_bytes()[0].is_ascii_alphabetic()
            && expanded_str.as_bytes()[1] == b':')
        || expanded_str.starts_with("\\\\");
    if !is_absolute {
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
        push_normalized_allowed_prefix(
            &mut allowed_prefixes,
            format!("{home}/.config/cosmostrix/"),
        );
        // macOS native: ~/Library/Application Support/cosmostrix/
        #[cfg(target_os = "macos")]
        push_normalized_allowed_prefix(
            &mut allowed_prefixes,
            format!("{home}/Library/Application Support/cosmostrix/"),
        );
    }

    // Linux/macOS/Android: /etc/cosmostrix/ (system-wide)
    #[cfg(unix)]
    push_normalized_allowed_prefix(&mut allowed_prefixes, "/etc/cosmostrix/".to_string());

    // FreeBSD: /usr/local/etc/cosmostrix/ (system-wide).
    //
    // FreeBSD uses /usr/local/etc/ for ports/packages (everything installed
    // via `pkg` or the ports tree goes there), NOT /etc/ (which is reserved
    // for the base system). Without this entry, a FreeBSD user following
    // the platform convention would have their config silently rejected
    // by the whitelist — the README and config template document this path,
    // so the whitelist must accept it.
    //
    // Compiled on all unix targets because FreeBSD binaries are built with
    // `target_os = "freebsd"` but the same binary may run on GhostBSD or
    // other FreeBSD descendants. The path is harmless on Linux/macOS
    // (just an extra prefix that won't match any real path there).
    #[cfg(unix)]
    push_normalized_allowed_prefix(
        &mut allowed_prefixes,
        "/usr/local/etc/cosmostrix/".to_string(),
    );

    // Android (Termux): /sdcard/cosmostrix/ (external storage).
    //
    // v25: MUST use runtime env-var detection, NOT `#[cfg(target_os =
    // "android")]`. Termux installs regular Linux ARM binaries (compiled
    // with `target_os = "linux"`), NOT Android NDK binaries. The previous
    // `#[cfg(target_os = "android")]` gate compiled the whitelist entry
    // out of every Termux build, so `/sdcard/cosmostrix/` was NEVER
    // actually allowed in Termux — the README and docs claimed Android
    // support, but the implementation silently rejected it.
    //
    // Termux detection is centralized in `configfile::is_termux_environment()`
    // — the canonical runtime check (`TERMUX_VERSION` env var or `PREFIX`
    // containing "com.termux"). verbose.rs calls the same function; the
    // single source of truth prevents drift if the detection heuristic ever
    // needs to change.
    let is_termux = crate::configfile::is_termux_environment();
    if is_termux {
        push_normalized_allowed_prefix(&mut allowed_prefixes, "/sdcard/cosmostrix/".to_string());

        // Termux system-wide: $PREFIX/etc/cosmostrix/ (typically
        // /data/data/com.termux/files/usr/etc/cosmostrix/).
        //
        // This is the Termux-equivalent of /etc/cosmostrix/ on Linux —
        // system-wide config that survives `$HOME` resets. The config
        // template and README document this path, so the whitelist must
        // accept it. $PREFIX is always set on Termux (it's the prefix
        // where the Termux package was installed, e.g.
        // /data/data/com.termux/files/usr).
        if let Some(prefix) = std::env::var("PREFIX").ok().filter(|p| !p.is_empty()) {
            push_normalized_allowed_prefix(
                &mut allowed_prefixes,
                format!("{prefix}/etc/cosmostrix/"),
            );
        }
    }

    // Windows: %APPDATA%\cosmostrix\ (user)
    #[cfg(windows)]
    if let Some(appdata) = std::env::var("APPDATA").ok().filter(|a| !a.is_empty()) {
        push_normalized_allowed_prefix(&mut allowed_prefixes, format!("{appdata}\\cosmostrix\\"));
    }

    // Windows: %ProgramData%\cosmostrix\ (system-wide)
    #[cfg(windows)]
    if let Some(progdata) = std::env::var("ProgramData").ok().filter(|p| !p.is_empty()) {
        push_normalized_allowed_prefix(&mut allowed_prefixes, format!("{progdata}\\cosmostrix\\"));
    }

    // Test-only override: allow COSMOSTRIX_TEST_CONFIG_DIR for test configs.
    // This env var is ONLY respected in test builds (#[cfg(test)] ensures the
    // block is compiled out of release binaries). Test helpers set it to a
    // temp directory so they can write config files without polluting
    // ~/.config/cosmostrix/.
    #[cfg(test)]
    if let Ok(test_dir) = std::env::var("COSMOSTRIX_TEST_CONFIG_DIR") {
        push_normalized_allowed_prefix(&mut allowed_prefixes, test_dir);
    }

    // Check if the normalized path starts with any allowed prefix.
    // On Windows, path comparison must be case-insensitive (NTFS is
    // case-insensitive by default, and env vars like APPDATA may return
    // a different casing than what the user typed on the command line).
    for prefix in &allowed_prefixes {
        #[cfg(windows)]
        {
            if check_str.to_lowercase().starts_with(&prefix.to_lowercase()) {
                return true;
            }
        }
        #[cfg(not(windows))]
        {
            if check_str.starts_with(prefix.as_str()) {
                return true;
            }
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
/// `\` are treated as separators). UNC paths (`\\server\share\...`) and
/// drive-letter paths (`C:\...`) preserve their authority/drive prefix.
fn normalize_path_segments(path: &str) -> Option<String> {
    // --- UNC path handling: \\server\share\... → //server/share/... ---
    // UNC paths start with exactly two separators. The \\server\share prefix
    // is an authority that must be preserved as a unit — it cannot be
    // traversed with `..` (you can't `..` above \\server\share).
    if path.starts_with("\\\\") || path.starts_with("//") {
        // Find the third separator after \\server\share\
        let rest = &path[2..];
        let mut sep_count = 0;
        let mut after_share = 0;
        for (i, &b) in rest.as_bytes().iter().enumerate() {
            if b == b'/' || b == b'\\' {
                sep_count += 1;
                if sep_count == 2 {
                    after_share = i + 1;
                    break;
                }
            }
        }
        if sep_count < 2 {
            // Malformed UNC: \\server without \share — treat the whole thing
            // as a single authority unit. No further normalization needed.
            return Some(path.replace('\\', "/"));
        }
        let authority = &path[..2 + after_share - 1]; // \\server\share → //server/share
        let authority_normalized = authority.replace('\\', "/");
        let remaining = &path[2 + after_share..];
        // Normalize the remaining path segments after the UNC authority.
        // `is_absolute = true` so that `..` above the share root is rejected
        // as an escape attempt (you cannot `..` above \\server\share). The
        // leading `/` produced by the inner normalizer is stripped because
        // the authority already contributes its own trailing separator.
        if remaining.is_empty() {
            return Some(format!("{authority_normalized}/"));
        }
        return match normalize_path_segments_inner(remaining, true) {
            Some(norm) => {
                let stripped = norm.strip_prefix('/').unwrap_or(&norm);
                Some(format!("{authority_normalized}/{stripped}"))
            }
            None => None,
        };
    }

    // --- Drive-letter path handling: C:\... → C:/... ---
    // Detect C: or c: at the start. The drive letter is a root that `..`
    // cannot escape above.
    if path.len() >= 2 && path.as_bytes()[0].is_ascii_alphabetic() && path.as_bytes()[1] == b':' {
        let drive = &path[..2];
        let rest = if path.len() > 2 && (path.as_bytes()[2] == b'/' || path.as_bytes()[2] == b'\\')
        {
            &path[3..]
        } else {
            &path[2..]
        };
        let drive_normalized = drive.replace('\\', "/");
        if rest.is_empty() {
            return Some(format!("{drive_normalized}/"));
        }
        // `is_absolute = true` so that `..` above the drive root is rejected
        // as an escape attempt (you cannot `..` above C:\). The leading `/`
        // produced by the inner normalizer is stripped because the drive
        // prefix already contributes its own trailing separator.
        return match normalize_path_segments_inner(rest, true) {
            Some(norm) => {
                let stripped = norm.strip_prefix('/').unwrap_or(&norm);
                Some(format!("{drive_normalized}/{stripped}"))
            }
            None => None,
        };
    }

    // --- Unix-style absolute or relative path ---
    let is_absolute = path.starts_with('/');
    normalize_path_segments_inner(path, is_absolute)
}

/// Inner normalization: resolve `.` and `..` segments without touching the
/// filesystem. Returns `None` if `..` would escape above the root.
fn normalize_path_segments_inner(path: &str, is_absolute: bool) -> Option<String> {
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
                    if is_absolute {
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
    if is_absolute {
        Some(format!("/{joined}"))
    } else {
        Some(joined)
    }
}

/// Expand `~` to `$HOME` if present. Returns the path as-is if no tilde.
///
/// On Windows, `HOME` is typically not set. Falls back to `USERPROFILE`
/// (which Windows always sets: `C:\Users\<name>`). This matches
/// `configfile::default_config_file_path()`'s fallback chain.
///
/// S2 (internal independent QA): `~user` (POSIX per-user tilde expansion)
/// is NOT supported — only `~/path` and bare `~`. This is a documented
/// limitation; full POSIX expansion would require `getpwnam(3)` which
/// adds a libc dependency for a rarely-used feature. Users who need
/// another user's config should use an absolute path.
fn expand_tilde(path: &str) -> PathBuf {
    // S1 (internal independent QA): use `if let Some(rest)` instead of
    // `.unwrap()` to make the invariant explicit. The outer `if path.starts_with("~/")`
    // guarantees the strip succeeds, but the unwrap was fragile if a future
    // refactor changes the outer condition without updating the inner call.
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var("HOME").ok().filter(|h| !h.is_empty()) {
            return PathBuf::from(home).join(rest);
        }
        #[cfg(windows)]
        if let Some(userprofile) = std::env::var("USERPROFILE").ok().filter(|h| !h.is_empty()) {
            return PathBuf::from(userprofile).join(rest);
        }
    }
    if path == "~" {
        if let Some(home) = std::env::var("HOME").ok().filter(|h| !h.is_empty()) {
            return PathBuf::from(home);
        }
        #[cfg(windows)]
        if let Some(userprofile) = std::env::var("USERPROFILE").ok().filter(|h| !h.is_empty()) {
            return PathBuf::from(userprofile);
        }
    }
    PathBuf::from(path)
}

/// Validate a `--config <path>` argument: must be inside the strict
/// whitelist AND have a `.toml` extension. Returns `Ok(resolved_path)` if
/// valid (with Windows `%VAR%` env vars expanded), or
/// `Err(formatted_error_message)` if rejected.
///
/// The returned resolved path MUST be used for all subsequent file I/O
/// (reading, writing, existence checks). Using the original `path_str`
/// would fail on Windows because `%APPDATA%` is a shell convention, not
/// an OS-level feature — `std::fs::read_to_string("%APPDATA%\\...")`
/// creates a literal `%APPDATA%` directory instead of resolving it.
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
pub(crate) fn validate_config_path(path_str: &str, verbose: bool) -> Result<String, String> {
    // Expand Windows env vars before validation so %APPDATA% paths work.
    let resolved = expand_windows_env_vars(path_str);
    let safe = is_safe_path(&resolved);
    if verbose {
        crate::output::eprintln_verbose_raw(&format!(
            "config path: {path_str} (resolved: {resolved}, safe: {safe})"
        ));
    }
    if !safe {
        return Err(format!(
            "error: --config '{path_str}' is outside allowed directories\n  \
             Allowed: ~/.config/cosmostrix/ (Linux, macOS, FreeBSD, Android Termux);\n  \
             /etc/cosmostrix/ (Linux, macOS);\n  \
             /usr/local/etc/cosmostrix/ (FreeBSD — ports/packages convention);\n  \
             $PREFIX/etc/cosmostrix/ (Android Termux — system-wide);\n  \
             /sdcard/cosmostrix/ (Android Termux — external storage);\n  \
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
    Ok(resolved)
}

#[cfg(test)]
#[path = "../../test/safepath/tests.rs"]
mod tests;
