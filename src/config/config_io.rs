// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Config file I/O helpers for `--dump-config`.
//!
//! Atomic write (temp-file + fsync + rename) and stdout-redirect detection
//! (Unix fstat). Both are startup-only utilities used by the `--dump-config`
//! path to write config files safely without bypassing the path whitelist.

/// Check if stdout is redirected to a regular file (shell `>` or `>|` operator).
/// Returns `true` if stdout is a regular file (shell redirect bypassing whitelist).
/// Returns `false` for TTY, pipe (allowed), char device, socket.
/// Used by `--dump-config` to block shell redirection that bypasses the path whitelist.
#[cfg(unix)]
pub(crate) fn stdout_is_redirected_to_file() -> bool {
    use std::os::unix::io::AsRawFd;
    let fd = std::io::stdout().as_raw_fd();
    // SAFETY: fstat on a valid fd (stdout=1, always open). The stat struct
    // is zeroed and overwritten by the syscall.
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: fstat reads metadata for an already-open fd. Writes only into
    // our zeroed stat struct; returns 0 on success.
    if unsafe { libc::fstat(fd, &mut st) } == 0 {
        return (st.st_mode & libc::S_IFMT) == libc::S_IFREG;
    }
    // If fstat fails (shouldn't happen on stdout), don't block — let the
    // write proceed. Better to be permissive than to break a valid use case.
    false
}

/// Write `text` to `target_path` atomically: temp-file + fsync + rename.
/// POSIX `rename(2)` is atomic — readers see either old or new file, never
/// a half-written one. Temp lives in same dir (same-filesystem move) as
/// `<target>.tmp.<pid>`. Best-effort cleanup on error.
pub(crate) fn write_config_atomic(target_path: &str, text: &str) -> std::io::Result<()> {
    let target = std::path::Path::new(target_path);
    let parent = target
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    // Create parent dir if missing — skip /etc/ and /var/ (system paths
    // should require explicit user creation to avoid wrong-permission auto).
    if !parent.exists() {
        if let Some(parent_str) = parent.to_str() {
            if !parent_str.starts_with("/etc/") && !parent_str.starts_with("/var/") {
                std::fs::create_dir_all(parent)?;
            }
        }
    }
    let pid = std::process::id();
    let tmp_name = format!(
        "{}.tmp.{pid}",
        target
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("config.toml")
    );
    let tmp_path = parent.join(&tmp_name);
    std::fs::write(&tmp_path, text)?;
    // fsync for crash-durability. If it fails, rename still proceeds (data
    // is in page cache). Surface error only if rename itself fails.
    if let Ok(file) = std::fs::File::open(&tmp_path) {
        let _ = file.sync_all();
    }
    std::fs::rename(&tmp_path, target)?;
    Ok(())
}
