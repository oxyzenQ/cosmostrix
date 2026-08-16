// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Platform abstraction layer.
//!
//! Centralizes cross-platform type aliases and helper functions to reduce
//! `#[cfg]` gate proliferation across the codebase. Each item here replaces
//! multiple per-file `#[cfg(unix)]` / `#[cfg(not(unix))]` / `#[cfg(windows)]`
//! blocks with a single platform-aware definition.
//!
//! ## Design principles
//!
//! 1. **Type aliases over traits** — We use type aliases (`TermReinit`)
//!    rather than trait objects or enums. This keeps zero
//!    overhead on all platforms and avoids vtable indirection. The cfg gate
//!    lives in one place (this file) instead of dozens.
//!
//! 2. **Stub types are `()`** — On platforms where a concept doesn't exist
//!    (e.g. SIGTSTP on Windows), the alias resolves to `()` so callers can
//!    pass a no-op value without any runtime cost.
//!
//! 3. **Helper functions over macros** — Repeated platform-specific logic
//!    (e.g. reading `/sys` files) is centralized in helper functions here
//!    rather than duplicated with `#[cfg]` blocks in each consumer.

// ── Signal / terminal reinit type aliases ─────────────────────────────────

/// Type alias for the "terminal reinit needed" flag used after SIGCONT.
///
/// - **Unix**: `Arc<AtomicBool>` — shared between the SIGTSTP/SIGCONT signal
///   handler thread and the event loop. The handler sets it to `true` on
///   SIGCONT; the event loop swaps it to `false` and reinitializes the
///   terminal.
/// - **Non-Unix (Windows)**: `()` — SIGTSTP/SIGCONT don't exist; ConPTY
///   handles terminal state automatically.
///
/// ### Impact
/// This single alias eliminates ~35 `#[cfg(unix)]` parameter annotations
/// across `interactive/tests.rs`, `interactive/input.rs`, and
/// `interactive/event_loop.rs`.
#[cfg(unix)]
pub(crate) type TermReinit = std::sync::Arc<std::sync::atomic::AtomicBool>;

#[cfg(not(unix))]
pub(crate) type TermReinit = ();

/// Construct a default `TermReinit` value.
///
/// On Unix, returns a new `Arc<AtomicBool>` initialized to `false`.
/// On non-Unix, returns `()`.
/// Only used in test code (interactive/tests.rs).
#[cfg(test)]
#[cfg(unix)]
#[inline]
pub(crate) fn default_term_reinit() -> TermReinit {
    std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false))
}

#[cfg(test)]
#[cfg(not(unix))]
#[inline]
#[must_use]
pub(crate) fn default_term_reinit() -> TermReinit {}

/// Swap the `term_reinit` flag and return the old value.
///
/// - **Unix**: calls `AtomicBool::swap(false, AcqRel)` on the inner `Arc<AtomicBool>`.
/// - **Non-Unix**: always returns `false` (no SIGTSTP/SIGCONT).
///
/// This eliminates the need for `#[cfg(unix)]` at every call site in the
/// event loop. The compiler inlines the `false` return on non-Unix.
#[cfg(unix)]
#[inline]
pub(crate) fn swap_term_reinit(reinit: &TermReinit) -> bool {
    use std::sync::atomic::Ordering;
    reinit.swap(false, Ordering::AcqRel)
}

#[cfg(not(unix))]
#[inline]
#[must_use]
pub(crate) fn swap_term_reinit(_reinit: &TermReinit) -> bool {
    false
}

// ── Linux /sys and /proc helpers ─────────────────────────────────────────

/// Read a single line from a Linux `/sys` or `/proc` file.
///
/// Returns `None` if the file doesn't exist or can't be read.
/// On non-Linux, always returns `None`.
///
/// This replaces the duplicated `linux_sys_read()` in `envstat.rs`
/// and similar patterns in `diagnostics.rs`, `thermal_sampler.rs`, etc.
#[cfg(target_os = "linux")]
#[must_use]
pub(crate) fn sysfs_read_line(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

#[cfg(not(target_os = "linux"))]
#[must_use]
#[allow(dead_code)]
pub(crate) fn sysfs_read_line(_path: &str) -> Option<String> {
    None
}

/// Read a single line from `/sys/devices/system/cpu/cpu0/cpufreq/<key>`.
///
/// Returns `None` on non-Linux or systems without CPUFreq.
#[cfg(target_os = "linux")]
#[must_use]
pub(crate) fn sysfs_cpu_freq(key: &str) -> Option<String> {
    sysfs_read_line(&format!("/sys/devices/system/cpu/cpu0/cpufreq/{key}"))
}

#[cfg(not(target_os = "linux"))]
#[must_use]
pub(crate) fn sysfs_cpu_freq(_key: &str) -> Option<String> {
    None
}

/// Read SMT active status from `/sys/devices/system/cpu/smt/active`.
///
/// Returns `"on"` or `"off"` on Linux with SMT support; `None` otherwise.
#[cfg(target_os = "linux")]
#[must_use]
pub(crate) fn sysfs_smt_active() -> Option<String> {
    let raw = sysfs_read_line("/sys/devices/system/cpu/smt/active")?;
    match raw.as_str() {
        "1" => Some("on".to_string()),
        "0" => Some("off".to_string()),
        other => Some(other.to_string()),
    }
}

#[cfg(not(target_os = "linux"))]
#[must_use]
pub(crate) fn sysfs_smt_active() -> Option<String> {
    None
}

/// Parse a specific field value from a Linux `/proc` file by line prefix.
///
/// Given a file path and a line prefix (e.g. `"VmRSS:"`), reads the file,
/// finds the first line starting with `prefix`, and returns the trimmed
/// value after the prefix. Returns `None` if the file can't be read or
/// the prefix isn't found.
///
/// This replaces the duplicated "open /proc file → scan for prefix →
/// parse value" pattern used in `memstat.rs`, `interactive/intro.rs`,
/// and `diagnostics.rs`.
#[cfg(target_os = "linux")]
#[must_use]
pub(crate) fn procfs_field(path: &str, prefix: &str) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.split('\n') {
        if let Some(rest) = line.strip_prefix(prefix) {
            let trimmed = rest.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
#[must_use]
#[allow(dead_code)]
pub(crate) fn procfs_field(_path: &str, _prefix: &str) -> Option<String> {
    None
}

/// Parse a numeric field value from a Linux `/proc` file by line prefix.
///
/// Like `procfs_field()` but returns the leading numeric portion as `u64`.
/// Skips non-digit trailing characters (e.g. `" kB"` suffix in VmRSS).
#[cfg(target_os = "linux")]
#[must_use]
pub(crate) fn procfs_field_u64(path: &str, prefix: &str) -> Option<u64> {
    let value = procfs_field(path, prefix)?;
    let end = value
        .bytes()
        .position(|b| !b.is_ascii_digit())
        .unwrap_or(value.len());
    if end == 0 {
        return None;
    }
    value[..end].parse().ok()
}

#[cfg(not(target_os = "linux"))]
#[must_use]
#[allow(dead_code)]
pub(crate) fn procfs_field_u64(_path: &str, _prefix: &str) -> Option<u64> {
    None
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_term_reinit_works() {
        // Just verify it compiles and doesn't panic on all platforms.
        let _ = default_term_reinit();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn sysfs_read_line_nonexistent_returns_none() {
        assert!(sysfs_read_line("/sys/nonexistent_xyz123").is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn procfs_field_u64_vm_peak() {
        // /proc/self/status always exists on Linux.
        let val = procfs_field_u64("/proc/self/status", "VmPeak:");
        // May be Some in a normal process, but we just verify it doesn't panic.
        let _ = val;
    }
}
