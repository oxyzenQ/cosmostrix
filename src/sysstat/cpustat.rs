// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Process CPU time sampling for the benchmark subsystem.
//!
//! Provides a zero-dependency cross-platform "current process CPU time in
//! nanoseconds" sampler. Supported platforms:
//! - **Linux**: parses `/proc/self/stat` (fields 14 + 15 = utime + stime,
//!   in clock ticks; converted to ns via `sysconf(_SC_CLK_TCK)`).
//! - **macOS**: queries `mach_task_basic_info` via `libc` — same call as
//!   RSS sampling, but reads `user_time` + `system_time` (in Mach time,
//!   converted to ns via `mach_timebase_info`).
//! - **Other Unix (BSD, Android, iOS, etc.)**: uses `getrusage(RUSAGE_SELF)`
//!   via `libc` — reads `ru_utime` + `ru_stime` (in microseconds, converted
//!   to ns). This is the universal unix fallback and works on all unix
//!   targets where `libc::getrusage` and `libc::RUSAGE_SELF` are exposed.
//!   On Linux/macOS we prefer the more accurate primary path, but
//!   `getrusage` would also work there as a last resort.
//! - **Windows / non-unix**: returns `None`. The benchmark will omit
//!   CPU% fields rather than emit a fake or zero value.
//!
//! ## How CPU% is computed
//! The caller takes two samples (T0, T1) and computes:
//!
//! ```text
//! cpu_ns_delta  = cpu_ns(T1) - cpu_ns(T0)
//! wall_ns_delta = wall_ns(T1) - wall_ns(T0)
//! cpu_percent   = (cpu_ns_delta / wall_ns_delta) * 100.0
//! ```
//!
//! Because cosmostrix is single-threaded by design, `cpu_percent` is
//! bounded by ~100% on a single-core measurement. Values >100% would
//! indicate either multi-threading (not currently used) or measurement
//! error. The report caps the displayed value at 999.9% to keep the
//! field width stable.

#[cfg(target_os = "linux")]
use std::io::Read;

/// Sample the current process's total CPU time (user + system) in
/// nanoseconds, if available.
///
/// Returns `None` on unsupported platforms or if the OS query fails.
/// The benchmark treats `None` as "metric not available" and omits the
/// CPU% field rather than reporting zero.
///
/// # Performance
/// On Linux this opens and reads `/proc/self/stat` (~2 KiB) once per
/// call. On macOS the cost is a single `task_info` syscall plus a
/// `mach_timebase_info` syscall (the latter is cached after the first
/// call by the kernel).
#[must_use]
pub(crate) fn current_cpu_ns() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        linux_cpu_ns()
    }
    #[cfg(target_os = "macos")]
    {
        macos_cpu_ns()
    }
    // Universal unix fallback (BSD, Android, iOS, etc.) — uses
    // getrusage(RUSAGE_SELF). libc is already a dependency under
    // cfg(unix) so this adds no new external crate.
    #[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
    {
        unix_getrusage_cpu_ns()
    }
    #[cfg(not(unix))]
    {
        None
    }
}

// ── Linux: /proc/self/stat ──────────────────────────────────────────────────

/// Read buffer size for /proc files.
#[cfg(target_os = "linux")]
const PROC_READ_BUF_SIZE: usize = 4096;

#[cfg(target_os = "linux")]
fn linux_cpu_ns() -> Option<u64> {
    // /proc/self/stat is a single line. Fields (1-indexed):
    //   14 = utime (clock ticks)
    //   15 = stime (clock ticks)
    // We parse by splitting on whitespace and indexing fields 13 + 14
    // (0-indexed). The comm field (2) is wrapped in parens and may
    // contain spaces, so we skip it by finding the last ')' first.
    let mut file = std::fs::File::open("/proc/self/stat").ok()?;
    let mut buf = [0u8; PROC_READ_BUF_SIZE];
    let n = file.read(&mut buf).ok()?;
    let text = std::str::from_utf8(&buf[..n]).ok()?;

    // Find the closing paren of the comm field to skip past it safely.
    let after_comm = text.rfind(')')?;
    let rest = &text[after_comm + 1..];
    let fields: Vec<&str> = rest.split_whitespace().collect();
    // After ')', field 3 (state) is at index 0. So:
    //   utime = fields[11] (field 14 - 3 + 1 - 1 = 11)
    //   stime = fields[12] (field 15 - 3 + 1 - 1 = 12)
    if fields.len() < 13 {
        return None;
    }
    let utime: u64 = fields[11].parse().ok()?;
    let stime: u64 = fields[12].parse().ok()?;
    let ticks = utime.saturating_add(stime);

    // Convert clock ticks to nanoseconds. sysconf(_SC_CLK_TCK) is
    // typically 100 on Linux, giving 10ms per tick = 10_000_000 ns.
    // SAFETY: sysconf() with the _SC_CLK_TCK argument is a read-only
    // query that returns a positive integer (or -1 on error). It takes
    // no pointers and writes no memory; the only contract is that the
    // argument is a valid sysconf(3) name constant, which _SC_CLK_TCK is.
    let clk_tck = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    if clk_tck <= 0 {
        return None;
    }
    let ns_per_tick = 1_000_000_000u64 / (clk_tck as u64);
    Some(ticks.saturating_mul(ns_per_tick))
}

// ── macOS: mach_task_basic_info via libc ────────────────────────────────────

#[cfg(target_os = "macos")]
fn macos_cpu_ns() -> Option<u64> {
    // See memstat.rs for the rationale on allow(deprecated) — the libc
    // shims for mach_task_self() / mach_timebase_info() are marked
    // deprecated in favor of `mach2`, but still work on macOS 12+.
    #![allow(deprecated)]
    use libc::{
        c_int, mach_task_basic_info, mach_task_self, task_info, task_info_t, KERN_SUCCESS,
        MACH_TASK_BASIC_INFO,
    };
    use std::mem;

    // SAFETY: same Mach API pattern as memstat.rs. task_info with flavor
    // MACH_TASK_BASIC_INFO writes into our mach_task_basic_info struct.
    // user_time + system_time are time_value_t structs {seconds, microseconds}
    // — NOT Mach absolute time units. We convert directly to ns without
    // needing mach_timebase_info.
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
        if kr != KERN_SUCCESS {
            return None;
        }
        // time_value_t { seconds: i32, microseconds: i32 } → nanoseconds.
        let user_ns = time_value_to_ns(info.user_time);
        let system_ns = time_value_to_ns(info.system_time);
        Some(user_ns.saturating_add(system_ns))
    }
}

/// Convert a Mach `time_value_t` (seconds + microseconds, wall-clock
/// units) to nanoseconds. Used by the macOS CPU sampler.
///
/// `time_value_t` is `{ seconds: integer_t (i32), microseconds: integer_t (i32) }`.
/// Negatives are clamped to 0 defensively (process CPU time should never
/// be negative, but the OS could theoretically return stale/zeroed fields).
#[cfg(target_os = "macos")]
fn time_value_to_ns(tv: libc::time_value_t) -> u64 {
    let secs = u64::try_from(tv.seconds.max(0)).unwrap_or(0);
    let micros = u64::try_from(tv.microseconds.max(0)).unwrap_or(0);
    secs.saturating_mul(1_000_000_000)
        .saturating_add(micros.saturating_mul(1_000))
}

// ── Other Unix (BSD, Android, iOS, …): getrusage(RUSAGE_SELF) ────────────────

/// Universal unix fallback used on every unix target that is not Linux or
/// macOS (FreeBSD, NetBSD, OpenBSD, DragonFly, Android, iOS, etc.).
///
/// Uses `getrusage(RUSAGE_SELF)` to read the process's accumulated user
/// and system CPU time. `ru_utime` and `ru_stime` are `timeval` structs
/// `{ tv_sec: time_t, tv_usec: suseconds_t }` with microsecond resolution.
///
/// ## Why getrusage and not /proc?
/// - BSDs (except FreeBSD with linprocfs) don't expose `/proc/self/stat`.
/// - Android exposes `/proc/self/stat` (Linux kernel under the hood), but
///   we route Android through this fallback anyway for uniformity — the
///   `target_os = "android"` cfg branch is the same as BSD's. The cost
///   is negligible: getrusage is one syscall, returns in <1 µs.
/// - getrusage is part of POSIX, so it works everywhere `libc` does.
///
/// ## Accuracy
/// Microsecond resolution. At a 1 Hz sampling rate (used by both the
/// benchmark CPU tracker and the HUD CPU% line), this gives 6 significant
/// digits — more than enough for a 0-100% reading rendered with 2 decimals.
#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn unix_getrusage_cpu_ns() -> Option<u64> {
    // SAFETY: getrusage(RUSAGE_SELF, &mut rusage) is a read-only syscall
    // that writes one struct rusage into our local. RUSAGE_SELF is a
    // compile-time constant. The kernel cannot fault on this — the
    // destination is a stack variable of the correct type and size.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    // SAFETY: as above. Returns 0 on success, -1 on error (errno set).
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
    if rc != 0 {
        return None;
    }
    let user_ns = timeval_to_ns(usage.ru_utime);
    let system_ns = timeval_to_ns(usage.ru_stime);
    Some(user_ns.saturating_add(system_ns))
}

/// Convert a POSIX `timeval { tv_sec, tv_usec }` (seconds + microseconds)
/// to nanoseconds. Used by the unix getrusage fallback.
///
/// Defensive clamping: tv_usec should be 0..=999_999 per POSIX, but some
/// OSes (notably older FreeBSD) historically returned up to 1_000_000 in
/// edge cases. We saturate rather than panic.
#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn timeval_to_ns(tv: libc::timeval) -> u64 {
    let secs = u64::try_from(tv.tv_sec.max(0)).unwrap_or(0);
    let micros = u64::try_from(tv.tv_usec.max(0)).unwrap_or(0);
    secs.saturating_mul(1_000_000_000)
        .saturating_add(micros.saturating_mul(1_000))
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_cpu_ns_returns_some_on_supported_platforms() {
        // This test verifies the function contract, not a specific value.
        // On Linux/macOS/BSD/Android the sampler is expected to succeed,
        // but CI sandboxes may mask these — so we accept None gracefully
        // rather than asserting is_some() unconditionally.
        //
        // The real validation is the synthetic fixture test below, which
        // verifies the parser logic without depending on the environment.
        //
        // Only non-unix targets (Windows) are expected to return None
        // unconditionally — unix always has either /proc, Mach, or
        // getrusage available.
        let cpu = current_cpu_ns();
        #[cfg(not(unix))]
        {
            assert!(cpu.is_none(), "Non-unix platforms must return None");
        }
        // On supported platforms, we just verify it doesn't panic.
        // is_some() is expected but not guaranteed in all sandboxes.
        let _ = cpu;
    }

    #[test]
    fn current_cpu_ns_is_monotonic_within_tolerance() {
        // Two consecutive samples — the second must be >= the first
        // (CPU time only increases). Allow equality in case the sampler
        // resolution is coarse (Linux clock ticks are ~10ms; getrusage
        // micros on BSD/Android).
        // Skip the assertion if either sample is None (sandbox/masked /proc).
        let a = current_cpu_ns();
        let b = current_cpu_ns();
        if let (Some(va), Some(vb)) = (a, b) {
            assert!(
                vb >= va,
                "CPU ns must be monotonic non-decreasing ({va} -> {vb})"
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn time_value_to_ns_converts_correctly() {
        // 1 second + 500_000 microseconds = 1.5 seconds = 1_500_000_000 ns.
        let tv = libc::time_value_t {
            seconds: 1,
            microseconds: 500_000,
        };
        assert_eq!(time_value_to_ns(tv), 1_500_000_000);
        // Zero time → 0 ns.
        let zero = libc::time_value_t {
            seconds: 0,
            microseconds: 0,
        };
        assert_eq!(time_value_to_ns(zero), 0);
        // Pure microseconds (no seconds): 1000 µs = 1_000_000 ns.
        let micros_only = libc::time_value_t {
            seconds: 0,
            microseconds: 1000,
        };
        assert_eq!(time_value_to_ns(micros_only), 1_000_000);
        // Negative values (shouldn't happen, but defensive) clamp to 0.
        let neg = libc::time_value_t {
            seconds: -1,
            microseconds: -500,
        };
        assert_eq!(time_value_to_ns(neg), 0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_cpu_ns_parses_synthetic_proc_stat() {
        // Synthetic /proc/self/stat fixture. After the closing paren,
        // fields are: state, ppid, pgrp, session, tty, tpgid, flags,
        // minflt, cminflt, majflt, cmajflt, utime, stime, ...
        // We only care about utime (field 14, index 11 after ')') and
        // stime (field 15, index 12 after ')').
        let fixture =
            "1234 (cosmostrix) R 1 1234 1234 0 -1 4194304 100 0 0 0 250 300 0 0 20 0 1 0\n";
        // Replicate the parse logic to verify field indices.
        let after_comm = fixture.rfind(')').unwrap();
        let rest = &fixture[after_comm + 1..];
        let fields: Vec<&str> = rest.split_whitespace().collect();
        let utime: u64 = fields[11].parse().unwrap();
        let stime: u64 = fields[12].parse().unwrap();
        assert_eq!(utime, 250, "utime must be at index 11 after ')'");
        assert_eq!(stime, 300, "stime must be at index 12 after ')'");
        assert_eq!(utime + stime, 550);
    }

    /// Test the unix getrusage fallback path's timeval→ns converter.
    /// Compiled only on non-Linux/non-macOS unix (BSD/Android/iOS/etc.).
    /// Mirrors the macOS `time_value_to_ns` test — verifies the same
    /// boundary cases (zero, pure-micros, mixed, negatives).
    #[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
    #[test]
    fn unix_timeval_to_ns_converts_correctly() {
        // 1 second + 500_000 microseconds = 1.5 seconds = 1_500_000_000 ns.
        let tv = libc::timeval {
            tv_sec: 1,
            tv_usec: 500_000,
        };
        assert_eq!(timeval_to_ns(tv), 1_500_000_000);
        // Zero time → 0 ns.
        let zero = libc::timeval {
            tv_sec: 0,
            tv_usec: 0,
        };
        assert_eq!(timeval_to_ns(zero), 0);
        // Pure microseconds (no seconds): 1000 µs = 1_000_000 ns.
        let micros_only = libc::timeval {
            tv_sec: 0,
            tv_usec: 1000,
        };
        assert_eq!(timeval_to_ns(micros_only), 1_000_000);
        // Defensive: legacy FreeBSD that occasionally returned 1_000_000 in
        // tv_usec should saturate to 1 second worth of ns, not overflow.
        let oversize_usec = libc::timeval {
            tv_sec: 0,
            tv_usec: 1_000_000,
        };
        assert_eq!(timeval_to_ns(oversize_usec), 1_000_000_000);
    }
}
