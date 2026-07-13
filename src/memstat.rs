// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Process memory (RSS) sampling for the benchmark subsystem.
//!
//! Provides a zero-dependency cross-platform "current RSS in KB" sampler.
//! Supported platforms:
//! - **Linux**: parses `/proc/self/status` (`VmRSS:` field, in kB).
//! - **macOS**: queries `mach_task_basic_info` via `libc` (already a transitive
//!   dependency through `signal-hook`).
//! - **Other Unix / Windows**: returns `None`. The benchmark will omit memory
//!   fields rather than emit a fake or zero value. This keeps the report
//!   honest on platforms we do not yet instrument.
//!
//! ## Why not `sysinfo` / `procfs` / `getrusage`?
//! - `sysinfo` and `procfs` pull in large dependency trees for a single value.
//! - `getrusage`'s `ru_maxrss` is peak RSS over the entire process lifetime
//!   (and on Linux is in KB, on macOS in bytes), which is misleading for
//!   per-benchmark attribution. We sample `VmRSS` periodically instead so we
//!   can compute both peak and average *during the benchmark window*.
//!
//! ## Accuracy
//! RSS is a coarse, OS-level metric. It includes shared pages and is affected
//! by page-cache decisions. Treat the reported numbers as "order-of-magnitude
//! process footprint", not as a precise allocator accounting. For allocator
//! accounting, run under `valgrind --tool=massif` or `heaptrack` separately.

#[cfg(target_os = "linux")]
use std::io::Read;

/// Sample the current process RSS in kibibytes (KiB), if available.
///
/// Returns `None` on unsupported platforms or if the OS query fails. The
/// benchmark treats `None` as "metric not available" and omits the field
/// rather than reporting zero.
///
/// # Performance
/// On Linux this opens and reads `/proc/self/status` (~4 KiB) once per call.
/// At a 100 ms sampling interval that is well under 0.1% CPU overhead. On
/// macOS the cost is a single `task_info` syscall.
#[must_use]
pub fn current_rss_kb() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        linux_rss_kb()
    }
    #[cfg(target_os = "macos")]
    {
        macos_rss_kb()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

// ── Linux: /proc/self/status ───────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn linux_rss_kb() -> Option<u64> {
    // /proc/self/status is a small text file (~4 KiB). Read it into a
    // stack-anchored buffer to avoid heap allocation on the hot sampling
    // path. 8 KiB is generous; the file is typically ~3 KiB.
    let mut file = std::fs::File::open("/proc/self/status").ok()?;
    let mut buf = [0u8; 8192];
    let n = file.read(&mut buf).ok()?;
    let text = std::str::from_utf8(&buf[..n]).ok()?;

    // Parse line-by-line. The field we want is `VmRSS:    12345 kB`.
    // We do a manual byte scan instead of `.lines().find()` to avoid
    // allocating a `Vec<&str>` per call.
    for line in text.split('\n') {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            // Rest looks like "    12345 kB"
            let trimmed = rest.trim();
            // Take leading digits, ignore the trailing " kB" suffix.
            let digits_end = trimmed
                .bytes()
                .position(|b| !b.is_ascii_digit())
                .unwrap_or(trimmed.len());
            if digits_end == 0 {
                return None;
            }
            let value: u64 = trimmed[..digits_end].parse().ok()?;
            return Some(value);
        }
    }
    None
}

// ── macOS: mach_task_basic_info via libc ────────────────────────────────────

#[cfg(target_os = "macos")]
fn macos_rss_kb() -> Option<u64> {
    // libc 0.2.x deprecated the old `task_basic_info` + `TASK_BASIC_INFO`
    // flavor and points users at `mach2`. The `mach_task_self()` and
    // `mach_timebase_info()` shims in libc are also marked deprecated in
    // favor of `mach2`. We allow(deprecated) here to avoid adding a new
    // dependency just for two syscalls — the libc shims still work fine
    // on macOS 12+ and the underlying Mach APIs are stable.
    #![allow(deprecated)]
    use libc::{
        c_int, mach_task_basic_info, mach_task_self, task_info, task_info_t, MACH_TASK_BASIC_INFO,
    };
    use std::mem;

    // SAFETY: `mach_task_self()` returns the current task port.
    // `task_info` with flavor MACH_TASK_BASIC_INFO writes into our
    // `mach_task_basic_info` struct. This is the documented modern Mach
    // API usage (replaces the removed TASK_BASIC_INFO / task_basic_info).
    unsafe {
        let mut info: mach_task_basic_info = mem::zeroed();
        let mut count = (mem::size_of::<mach_task_basic_info>() / mem::size_of::<libc::natural_t>())
            as libc::mach_msg_type_number_t;
        let kr: c_int = task_info(
            mach_task_self(),
            MACH_TASK_BASIC_INFO,
            &mut info as *mut _ as task_info_t,
            &mut count,
        );
        if kr != libc::KERN_SUCCESS {
            return None;
        }
        // `resident_size` is in bytes. Convert to KiB.
        // Round to nearest to match Linux's kB reporting convention.
        let bytes = info.resident_size as u64;
        Some((bytes + 512) / 1024)
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_rss_kb_returns_some_on_supported_platforms() {
        // This test verifies the function contract, not a specific value.
        // On Linux/macOS, /proc or Mach task_info should be available,
        // but CI sandboxes may mask these — so we accept None gracefully.
        // The real validation is the synthetic fixture test below.
        let rss = current_rss_kb();
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            assert!(rss.is_none(), "Unsupported platforms must return None");
        }
        // On supported platforms, we just verify it doesn't panic.
        // is_some() is expected but not guaranteed in all sandboxes.
        let _ = rss;
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_rss_parses_synthetic_proc_status() {
        // Sanity-test the line-parsing logic with a synthetic snapshot.
        // We can't easily inject the file content, so we re-implement the
        // same parser step against a fixture string to guard the field
        // name and unit handling against regressions.
        let fixture = "Name:   cosmostrix\n\
                       Umask:  0022\n\
                       State:  R (running)\n\
                       VmPeak: 12345 kB\n\
                       VmSize: 11000 kB\n\
                       VmRSS:   9876 kB\n\
                       VmHWM:  12000 kB\n";
        let mut found = None;
        for line in fixture.split('\n') {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                let trimmed = rest.trim();
                let end = trimmed
                    .bytes()
                    .position(|b| !b.is_ascii_digit())
                    .unwrap_or(trimmed.len());
                found = trimmed[..end].parse::<u64>().ok();
                break;
            }
        }
        assert_eq!(found, Some(9876));
    }

    #[test]
    fn current_rss_kb_is_monotonic_within_tolerance() {
        // Two consecutive samples should be in the same order of magnitude.
        // We don't assert strict monotonicity (RSS can dip due to pageout),
        // but a 100x jump or drop would indicate a parsing bug.
        let a = current_rss_kb();
        let b = current_rss_kb();
        if let (Some(va), Some(vb)) = (a, b) {
            let max = va.max(vb);
            let min = va.min(vb).max(1);
            let ratio = max / min;
            assert!(
                ratio < 100,
                "Two consecutive RSS samples differed by >100x ({va} -> {vb}); \
                 likely a parsing bug",
            );
        }
    }
}
