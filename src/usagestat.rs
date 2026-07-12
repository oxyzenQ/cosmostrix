// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Process resource usage sampling via `getrusage(RUSAGE_SELF)`.
//!
//! Provides zero-dependency cross-platform sampling of cumulative process
//! resource counters: minor page faults, major page faults, voluntary
//! context switches, and involuntary context switches.
//!
//! ## Why getrusage (not perf_event_open)?
//!
//! `perf_event_open` gives hardware counters (instructions, cycles, cache
//! misses, branch misses, IPC) but is Linux-only and permission-gated
//! (`/proc/sys/kernel/perf_event_paranoid`). `getrusage` is a POSIX
//! syscall available on all Unix systems with no permissions required.
//! It does not give hardware counters, but it does give:
//!
//! - **Minor page faults** (`ru_minflt`): page reclaims from the page
//!   cache (no disk I/O). High values indicate memory pressure or
//!   frequent allocation patterns.
//! - **Major page faults** (`ru_majflt`): page faults requiring disk
//!   I/O. Non-zero indicates the process is touching memory not in RAM
//!   (swap-in, cold-start file mapping, etc.).
//! - **Voluntary context switches** (`ru_nvcsw`): the process yielded
//!   the CPU voluntarily (blocking syscall like `read`, `sleep`).
//!   High values indicate IO-bound behavior.
//! - **Involuntary context switches** (`ru_nivcsw`): the process was
//!   preempted by the scheduler (time slice expired). High values
//!   indicate CPU contention.
//!
//! These counters are cumulative since process start. For benchmark
//! window attribution, we sample at start and end, then compute deltas.
//!
//! ## Platform support
//! - **Linux + macOS**: via `libc::getrusage(RUSAGE_SELF, ...)`.
//! - **Windows / other**: returns `None` for all fields. The benchmark
//!   report emits "unsupported" with a reason field.

/// Snapshot of process resource counters at a point in time.
#[derive(Clone, Copy, Debug, Default)]
pub struct ResourceSnapshot {
    pub minor_faults: u64,
    pub major_faults: u64,
    pub voluntary_ctxt: u64,
    pub involuntary_ctxt: u64,
}

impl ResourceSnapshot {
    /// Take a snapshot of the current process's cumulative resource
    /// counters. Returns `None` on unsupported platforms or if the
    /// syscall fails.
    #[must_use]
    pub fn now() -> Option<Self> {
        #[cfg(unix)]
        {
            unix_snapshot()
        }
        #[cfg(not(unix))]
        {
            None
        }
    }

    /// Compute the delta between two snapshots. All fields are
    /// saturating-subtracted (clamped to 0) to guard against counter
    /// resets on some platforms.
    #[must_use]
    pub fn delta_since(&self, earlier: &Self) -> Self {
        Self {
            minor_faults: self.minor_faults.saturating_sub(earlier.minor_faults),
            major_faults: self.major_faults.saturating_sub(earlier.major_faults),
            voluntary_ctxt: self.voluntary_ctxt.saturating_sub(earlier.voluntary_ctxt),
            involuntary_ctxt: self
                .involuntary_ctxt
                .saturating_sub(earlier.involuntary_ctxt),
        }
    }
}

#[cfg(unix)]
fn unix_snapshot() -> Option<ResourceSnapshot> {
    // SAFETY: getrusage with RUSAGE_SELF writes into our rusage struct.
    // This is the documented POSIX API. The struct is zeroed first so
    // any field not filled by the kernel is 0.
    unsafe {
        let mut ru: libc::rusage = std::mem::zeroed();
        let rc = libc::getrusage(libc::RUSAGE_SELF, &mut ru);
        if rc != 0 {
            return None;
        }
        // On Linux, ru_maxrss is in KiB; on macOS it's in bytes. We don't
        // use ru_maxrss here (memstat.rs does windowed RSS sampling instead),
        // so we just read the fault + context-switch counters which have
        // the same semantics on both platforms.
        Some(ResourceSnapshot {
            minor_faults: ru.ru_minflt as u64,
            major_faults: ru.ru_majflt as u64,
            voluntary_ctxt: ru.ru_nvcsw as u64,
            involuntary_ctxt: ru.ru_nivcsw as u64,
        })
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_now_returns_some_on_unix() {
        let snap = ResourceSnapshot::now();
        #[cfg(unix)]
        {
            assert!(snap.is_some(), "getrusage must succeed on Unix");
            let s = snap.unwrap();
            // A running test process should have at least some minor faults
            // (page reclaims are ubiquitous). Major faults may be 0 if the
            // process is fully resident. Context switches should be >= 0.
            assert!(s.minor_faults > 0, "minor_faults should be non-zero");
        }
        #[cfg(not(unix))]
        {
            assert!(snap.is_none(), "Unsupported platforms must return None");
        }
    }

    #[test]
    fn delta_since_computes_correctly() {
        let earlier = ResourceSnapshot {
            minor_faults: 100,
            major_faults: 5,
            voluntary_ctxt: 10,
            involuntary_ctxt: 3,
        };
        let later = ResourceSnapshot {
            minor_faults: 150,
            major_faults: 5,
            voluntary_ctxt: 12,
            involuntary_ctxt: 4,
        };
        let delta = later.delta_since(&earlier);
        assert_eq!(delta.minor_faults, 50);
        assert_eq!(delta.major_faults, 0, "no change in major faults");
        assert_eq!(delta.voluntary_ctxt, 2);
        assert_eq!(delta.involuntary_ctxt, 1);
    }

    #[test]
    fn delta_since_saturates_on_counter_reset() {
        let earlier = ResourceSnapshot {
            minor_faults: 200,
            major_faults: 10,
            voluntary_ctxt: 20,
            involuntary_ctxt: 5,
        };
        let later = ResourceSnapshot {
            minor_faults: 100, // "decreased" — should clamp to 0
            major_faults: 10,
            voluntary_ctxt: 20,
            involuntary_ctxt: 5,
        };
        let delta = later.delta_since(&earlier);
        assert_eq!(delta.minor_faults, 0, "negative delta must clamp to 0");
        assert_eq!(delta.major_faults, 0);
        assert_eq!(delta.voluntary_ctxt, 0);
        assert_eq!(delta.involuntary_ctxt, 0);
    }

    #[test]
    fn snapshot_is_monotonic_non_decreasing() {
        // Two consecutive snapshots — the second must be >= the first
        // for all counters (they only increase over a process lifetime).
        let a = ResourceSnapshot::now();
        let b = ResourceSnapshot::now();
        if let (Some(sa), Some(sb)) = (a, b) {
            assert!(
                sb.minor_faults >= sa.minor_faults,
                "minor_faults must be monotonic"
            );
            assert!(
                sb.major_faults >= sa.major_faults,
                "major_faults must be monotonic"
            );
            assert!(
                sb.voluntary_ctxt >= sa.voluntary_ctxt,
                "voluntary_ctxt must be monotonic"
            );
            assert!(
                sb.involuntary_ctxt >= sa.involuntary_ctxt,
                "involuntary_ctxt must be monotonic"
            );
        }
    }
}
