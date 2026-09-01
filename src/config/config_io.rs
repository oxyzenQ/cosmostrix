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

/// Read a config-sized text file with a hard size cap
/// (`CONFIG_FILE_MAX_BYTES`, 1 MiB).
///
/// S-master-3-v2 hardening: every config read path funnels through this
/// helper so a runaway or maliciously large file in a whitelisted
/// config directory cannot trigger an unbounded `read_to_string` (OOM
/// DoS vector — the ambient ground-truth check re-reads the file every
/// 5 s, so a multi-GB file would thrash the process repeatedly). An
/// oversized file returns `InvalidData` and callers treat it exactly
/// like an unreadable file: startup falls back to defaults, the
/// live-reload watcher skips the reparse, the ambient check skips.
///
/// The cap is enforced through `Read::take` (bounded syscalls), not a
/// metadata-then-read check — a file growing concurrently past the cap
/// is still bounded, with no TOCTOU window.
pub(crate) fn read_config_capped(path: &std::path::Path) -> std::io::Result<String> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let mut buf = String::new();
    // take() bounds the read even if the file grows while we read it.
    // Reading MAX+1 bytes lets us distinguish "at the cap" (ok) from
    // "past the cap" (reject) without a second stat call.
    file.take(crate::constants::CONFIG_FILE_MAX_BYTES + 1)
        .read_to_string(&mut buf)?;
    if buf.len() as u64 > crate::constants::CONFIG_FILE_MAX_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "config file exceeds the {} byte cap — treating as unreadable \
                 (defaults apply)",
                crate::constants::CONFIG_FILE_MAX_BYTES
            ),
        ));
    }
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::read_config_capped;
    use crate::constants::CONFIG_FILE_MAX_BYTES;
    use std::io::Write;

    fn tmp_file(name: &str, size: usize) -> std::path::PathBuf {
        let path =
            std::env::temp_dir().join(format!("cosmostrix-cap-{name}-{}", std::process::id()));
        let mut f = std::fs::File::create(&path).unwrap();
        // Fill with valid UTF-8 so read_to_string cannot fail on encoding.
        let chunk = "a".repeat(4096);
        let mut written = 0usize;
        while written < size {
            let n = chunk.len().min(size - written);
            f.write_all(&chunk.as_bytes()[..n]).unwrap();
            written += n;
        }
        path
    }

    #[test]
    fn capped_read_accepts_normal_config() {
        let p = tmp_file("ok", 2048);
        assert_eq!(read_config_capped(&p).unwrap().len(), 2048);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn capped_read_accepts_file_at_exact_cap() {
        let p = tmp_file("edge", CONFIG_FILE_MAX_BYTES as usize);
        assert_eq!(
            read_config_capped(&p).unwrap().len() as u64,
            CONFIG_FILE_MAX_BYTES
        );
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn capped_read_rejects_file_past_cap() {
        let p = tmp_file("over", CONFIG_FILE_MAX_BYTES as usize + 1);
        let err = read_config_capped(&p).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn capped_read_missing_file_is_not_found() {
        let err = read_config_capped(std::path::Path::new("/nonexistent/cosmostrix/config.toml"))
            .unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }
}
