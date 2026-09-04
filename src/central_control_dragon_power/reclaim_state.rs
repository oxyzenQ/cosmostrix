// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! P4: Memory Pressure Adaptive Reclaim (MPAR) + P2: Idle Phase Aggressive
//! Coalescing (IPAC).
//!
//! Two tightly-coupled idle-mode subsystems that share the same underlying
//! concern — what should cosmostrix do when nothing has changed for a long
//! time:
//!
//! - P2 (IPAC) — [`adaptive_resync_interval`] progressively stretches
//!   the idle redraw resync interval (20s → 60s → 120s) based on how long
//!   the process has been continuously idle. This reduces forced redraw
//!   CPU spikes during long idle periods (typically 13+ hours per day in
//!   long-endurance runs).
//! - P4 (MPAR) — [`hint_reclaim_pages`] tells the Linux kernel to
//!   reclaim stale file-backed frame buffer pages via `madvise(MADV_DONTNEED)`
//!   during sustained idle. [`ReclaimState`] tracks the last reclaim time
//!   to avoid hammering madvise on every idle resync — the minimum
//!   interval is 1 hour.
//!
//! Both subsystems are zero-allocation, single-threaded, and
//! backward-compatible with the existing architecture invariants.

use std::time::{Duration, Instant};

use crate::constants::*;

// ────────────────────────────────────────────────────────────────────────────
// P2: Idle Phase Aggressive Coalescing
// ────────────────────────────────────────────────────────────────────────────

/// Adaptive resync interval: progressively stretches the idle redraw resync
/// interval based on sustained idle duration.
///
/// After 1 hour of continuous idle, the interval grows from 20s → 60s.
/// After 4 hours, it grows to 120s. This reduces forced redraw CPU spikes
/// during long idle periods (typically 13+ hours per day in long-endurance runs).
///
/// # Arguments
/// - `idle_duration_secs`: How long the process has been continuously idle.
///
/// # Returns
/// The resync interval in seconds.
///
/// (perf audit): the tier thresholds and intervals are now named
/// constants in `constants.rs` (`SECS_PER_HOUR`, `SECS_PER_4_HOURS`,
/// `IDLE_RESYNC_TIER_2_SECS`, `IDLE_RESYNC_TIER_3_SECS`). Previously these
/// were four magic numbers inline.
pub(crate) fn adaptive_resync_interval(idle_duration_secs: f64) -> f64 {
    if idle_duration_secs < SECS_PER_HOUR {
        // < 1 hour idle: standard interval (20s).
        IDLE_REDRAW_RESYNC_INTERVAL_SECS
    } else if idle_duration_secs < SECS_PER_4_HOURS {
        // 1–4 hours idle: 60s interval (3× reduction).
        IDLE_RESYNC_TIER_2_SECS
    } else {
        // > 4 hours idle: 120s interval (6× reduction).
        IDLE_RESYNC_TIER_3_SECS
    }
}

// ────────────────────────────────────────────────────────────────────────────
// P4: Memory Pressure Adaptive Reclaim
// ────────────────────────────────────────────────────────────────────────────

/// Hint the Linux kernel to reclaim stale file-backed pages.
///
/// During sustained idle periods, the frame buffer's previous-generation
/// dirty regions are no longer needed. `madvise(MADV_DONTNEED)` tells the
/// kernel these pages can be reclaimed without swapping.
///
/// # Semantics (corrected 2026-08-23 — see the safety note below)
///
/// `MADV_DONTNEED` on a private anonymous mapping is NOT a
/// non-destructive hint: it discards the covered pages, and the next
/// access repopulates them as zero-filled pages (`madvise(2)`:
/// "zero-fill-on-demand pages for anonymous private mappings"). Two
/// consequences are handled explicitly:
///
/// 1. Zeroed bytes inside the caller's buffer. Both call sites set
///    `cloud.force_draw_everything()` BEFORE this call, and the next loop
///    iteration runs `rain_at()` → `Frame::clear_with_bg()` (which bumps
///    the content generation) before any cell is read. The generation
///    mismatch makes every zeroed cell read as `blank` — the discarded
///    content is never interpreted as a live `Cell` value.
/// 2. Zeroed bytes OUTSIDE the caller's buffer — the cross-object
///    hazard. `madvise` operates at PAGE granularity, while malloc
///    chunks (glibc main arena, for allocations below the dynamic mmap
///    threshold) share pages with neighboring chunks. Advising the raw
///    `[ptr, ptr+len)` range could zero the edges of adjacent heap
///    objects — arbitrary heap corruption. This function therefore
///    advises only the interior full pages of the range (start rounded
///    up, end rounded down to the page boundary), which by construction
///    contain exclusively the caller's own bytes. Edge pages that might
///    be shared are never touched.
///
/// This corrects the earlier soundness assessment (archived
/// `UNSAFE_SOUNDNESS_AUDIT.md` §2.9/§2.11) which justified the raw-range
/// call with the incorrect claim that MADV_DONTNEED is non-destructive.
///
/// # Safety
/// This function is only effective on Linux. On other platforms it's a no-op.
/// `ptr` must point to a mapped region of at least `len` bytes; only pages
/// fully interior to `[ptr, ptr+len)` are advised, so no byte outside the
/// caller's allocation is ever affected.
#[cfg(target_os = "linux")]
pub(crate) unsafe fn hint_reclaim_pages(ptr: *const u8, len: usize) {
    if len == 0 || ptr.is_null() {
        return;
    }
    let Some((start, interior_len)) = interior_page_range(ptr as usize, len, page_size()) else {
        // Allocation smaller than one page (or straddling a single page
        // boundary) — no interior full page exists, so advising anything
        // could touch neighboring objects. Skip entirely.
        return;
    };
    let ret = libc::madvise(
        start as *mut libc::c_void,
        interior_len,
        libc::MADV_DONTNEED,
    );
    // CC2-06: ignore all errors (pages not reclaimable, ENOMEM, etc.) —
    // best-effort.
    let _ = ret;
}

/// Compute the interior full-page subrange of `[addr, addr+len)`.
///
/// Returns `Some((start, interior_len))` where `[start, start+interior_len)`
/// consists exclusively of pages fully contained inside the input range —
/// pages that (by construction of a contiguous allocation) hold only the
/// caller's own bytes. Returns `None` when no full interior page exists.
///
/// Pure function (no unsafe, no syscalls) so the confinement math is
/// unit-testable on every platform.
//
// On non-Linux targets the only callers are the `#[cfg(test)]` blocks
// below (production usage is inside the Linux-only hint_reclaim_pages).
// FreeBSD's clippy `-D warnings` gate would flag it as dead code in the
// bin build without this allow.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[must_use]
pub(crate) fn interior_page_range(addr: usize, len: usize, page: usize) -> Option<(usize, usize)> {
    if len == 0 || page == 0 {
        return None;
    }
    let end_addr = addr.checked_add(len)?;
    // Round the start UP and the end DOWN to page boundaries: any page in
    // between lies entirely within [addr, end_addr).
    let start = addr.div_ceil(page) * page;
    let end = end_addr / page * page;
    if end > start {
        Some((start, end - start))
    } else {
        None
    }
}

/// System page size, cached after the first query.
///
/// Uses `sysconf(_SC_PAGESIZE)` (handles 4 KiB x86_64, 16/64 KiB ARM,
/// 64 KiB POWER). Falls back to 4096 if sysconf fails — the fallback only
/// makes the interior range MORE conservative or empty, never wider.
///
/// Linux-only: the sole caller is the Linux `hint_reclaim_pages` above,
/// and the `libc` crate is a Unix-only dependency (Windows builds would
/// fail to resolve it — CI run #1468). `interior_page_range` stays
/// cross-platform (its tests run on every target with an explicit page
/// size parameter), so the confinement math is still verified everywhere
/// this builds.
#[cfg(target_os = "linux")]
fn page_size() -> usize {
    static PAGE_SIZE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *PAGE_SIZE.get_or_init(|| {
        // SAFETY: sysconf with a valid _SC_PAGESIZE constant is always
        // safe to call; it reads a system constant and touches no memory.
        let v = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if v > 0 {
            v as usize
        } else {
            4096
        }
    })
}

#[cfg(not(target_os = "linux"))]
pub(crate) unsafe fn hint_reclaim_pages(_ptr: *const u8, _len: usize) {
    // No-op on non-Linux platforms.
}

/// Track whether memory reclaim has been performed recently to avoid
/// hammering madvise on every idle resync.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ReclaimState {
    /// When the last reclaim hint was issued.
    last_reclaim: Option<Instant>,
    /// Minimum interval between reclaim hints (1 hour).
    min_interval: Duration,
}

impl ReclaimState {
    pub(crate) fn new() -> Self {
        Self {
            last_reclaim: None,
            min_interval: Duration::from_secs(3600),
        }
    }

    /// Returns `true` if a reclaim hint should be issued now.
    pub(crate) fn should_reclaim(&self, now: Instant) -> bool {
        match self.last_reclaim {
            None => true,
            Some(last) => now.saturating_duration_since(last) >= self.min_interval,
        }
    }

    /// Record that a reclaim hint was issued at `now`.
    pub(crate) fn mark_reclaimed(&mut self, now: Instant) {
        self.last_reclaim = Some(now);
    }
}

impl Default for ReclaimState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── P2: adaptive_resync_interval ────────────────────────────────────────

    #[test]
    fn resync_interval_standard_under_1h() {
        assert_eq!(
            adaptive_resync_interval(0.0),
            IDLE_REDRAW_RESYNC_INTERVAL_SECS
        );
        assert_eq!(
            adaptive_resync_interval(1800.0),
            IDLE_REDRAW_RESYNC_INTERVAL_SECS
        );
        assert_eq!(
            adaptive_resync_interval(3599.0),
            IDLE_REDRAW_RESYNC_INTERVAL_SECS
        );
    }

    #[test]
    fn resync_interval_60s_after_1h() {
        assert_eq!(adaptive_resync_interval(3600.0), 60.0);
        assert_eq!(adaptive_resync_interval(7200.0), 60.0);
        assert_eq!(adaptive_resync_interval(14399.0), 60.0);
    }

    #[test]
    fn resync_interval_120s_after_4h() {
        assert_eq!(adaptive_resync_interval(14400.0), 120.0);
        assert_eq!(adaptive_resync_interval(86400.0), 120.0);
    }

    // ── P4: ReclaimState ────────────────────────────────────────────────────

    #[test]
    fn reclaim_state_initial_should_reclaim() {
        let s = ReclaimState::new();
        assert!(s.should_reclaim(Instant::now()));
    }

    #[test]
    fn reclaim_state_respects_min_interval() {
        let mut s = ReclaimState::new();
        let t0 = Instant::now();
        s.mark_reclaimed(t0);
        let t1 = t0 + Duration::from_secs(100);
        assert!(!s.should_reclaim(t1));
        let t2 = t0 + Duration::from_secs(3700);
        assert!(s.should_reclaim(t2));
    }

    // ── interior_page_range (confinement math) ───────────────────────
    //
    // The interior-page confinement is the load-bearing safety property
    // of hint_reclaim_pages: MADV_DONTNEED is page-granular and
    // zero-fills on next access, so only pages fully inside the caller's
    // allocation may ever be advised.

    #[test]
    fn interior_range_excludes_partial_edge_pages() {
        // Unaligned start (offset 100) and unaligned end: the range
        // [100, 12389) spans partial page 0, full pages 1-2, and a partial
        // page 3 — so the interior is exactly pages 1-2.
        let (start, len) =
            interior_page_range(100, 3 * 4096 + 1, 4096).expect("2 interior pages exist");
        assert_eq!(start, 4096);
        assert_eq!(len, 2 * 4096);
        // The excluded head [100, 4096) and tail [12288, 12389)
        // are exactly the partial pages that may be shared with neighbors.
    }

    #[test]
    fn interior_range_fully_aligned_keeps_whole_range() {
        // Page-aligned start, exact multiple of the page size — the whole
        // range is interior.
        let (start, len) = interior_page_range(8192, 2 * 4096, 4096).expect("full range");
        assert_eq!((start, len), (8192, 2 * 4096));
    }

    #[test]
    fn interior_range_sub_page_allocation_is_none() {
        // A 300-byte allocation starting mid-page: no full interior page.
        assert_eq!(interior_page_range(1234, 300, 4096), None);
        // Even a full-page-size allocation starting mid-page has no
        // interior page (it touches two partial pages).
        assert_eq!(interior_page_range(1234, 4096, 4096), None);
    }

    #[test]
    fn interior_range_spans_page_boundary_with_alignment() {
        // Start mid-page, length reaches exactly the next page boundary:
        // one interior page exists.
        let (start, len) =
            interior_page_range(1234, 2 * 4096 - 1234, 4096).expect("one interior page");
        assert_eq!((start, len), (4096, 4096));
    }

    #[test]
    fn interior_range_never_exceeds_input_bounds() {
        // Property check across a sweep of unaligned starts and lengths:
        // the interior range must be a subset of [addr, addr+len).
        let page = 4096usize;
        for addr in [0usize, 1, 100, 4095, 4096, 4097, 12_345] {
            for len in [0usize, 1, 4095, 4096, 4097, 12_288, 1 << 20] {
                if let Some((start, ilen)) = interior_page_range(addr, len, page) {
                    assert!(start >= addr, "start {start} < addr {addr}");
                    assert!(
                        start + ilen <= addr + len,
                        "interior end {} > input end {}",
                        start + ilen,
                        addr + len
                    );
                    assert_eq!(start % page, 0, "start must be page-aligned");
                    assert_eq!(ilen % page, 0, "interior length must be page multiple");
                    assert!(ilen > 0);
                }
            }
        }
    }

    #[test]
    fn interior_range_rejects_degenerate_inputs() {
        assert_eq!(interior_page_range(0, 0, 4096), None);
        assert_eq!(interior_page_range(4096, 0, 4096), None);
        // page=0 would divide by zero — must be rejected, not panic.
        assert_eq!(interior_page_range(4096, 8192, 0), None);
        // usize overflow in addr+len must yield None, not wrap.
        assert_eq!(interior_page_range(usize::MAX, 2, 4096), None);
    }

    #[test]
    fn interior_range_frame_sized_allocations_have_interior_pages() {
        // Typical frame.cells allocations (Cell = 24 bytes):
        // 80x24  = 45 KiB, 120x40 = 112 KiB, 200x60 = 281 KiB.
        // All must yield a non-empty interior regardless of alignment.
        for cells in [1920usize, 4800, 12_000] {
            let len = cells * 24;
            for addr in [16usize, 4096 + 16, 2 * 4096 + 123] {
                let r = interior_page_range(addr, len, 4096);
                assert!(
                    r.is_some_and(|(_, ilen)| ilen >= len.saturating_sub(2 * 4096)),
                    "cells={cells} addr={addr}: interior too small: {r:?}"
                );
            }
        }
    }
}
