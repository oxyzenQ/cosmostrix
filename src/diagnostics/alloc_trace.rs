// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Allocator tracing — global allocator wrapper that counts alloc/dealloc
//! calls, attributed to the thread that performs them.
//!
//! Phase 5 of DeepSeek benchmark restructuring plan.
//!
//! Wraps `std::alloc::System` with per-thread counters for
//! alloc/dealloc/realloc calls and bytes. Always active (a
//! const-initialized thread-local slot: no lazy-init allocation, no
//! destructor, so the allocator never re-enters itself). Stats are read
//! by the benchmark to report allocation patterns.
//!
//! ## Why per-thread counting, not process-global atomics
//!
//! The only readers are the benchmark window snapshots
//! (`AllocSnapshot::now()` on the measuring thread), and the production
//! bench binary is single-threaded — thread attribution equals process
//! totals there, byte for byte. The test harness is not: `cargo test`
//! runs every test in one process on parallel threads, and a
//! process-global counter attributes every concurrent test's allocations
//! to whatever benchmark window happens to be open. That is not a
//! theoretical concern — it is the FreeBSD CI incident of 2026-09-25:
//! `cargo test --all` (libtest, shared process) measured 16.3
//! "allocs/frame" of pure cross-thread noise on the cosmetics zero-alloc
//! tripwire while the cosmetics path itself allocated nothing; Linux CI
//! never saw it because nextest isolates every test in its own process.
//! Per-thread counters make the window mean what every consumer needs
//! it to mean: what the measured code path allocates.
//!
//! ## Why System (not mimalloc/jemalloc)
//!
//! Empirically verified on AMD Ryzen 7 5800HS (60s benchmark, 120x40): the
//! cosmostrix workload does ~2 allocs/frame against a stable ~93 KB heap.
//! At this allocation rate and heap size, glibc malloc beats mimalloc on
//! tail latency (p99 frame time +15% with mimalloc). Custom allocators
//! only win when there's heavy churn or large heap fragmentation to amortize
//! — neither applies here. Keep it simple: System is best-in-class for this
//! workload, and avoiding a C dependency keeps the build pure Rust.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

/// Per-thread allocation counters, copied through the TLS slot by the
/// allocator wrapper. All arithmetic inside the allocator uses wrapping
/// ops: the allocator must stay panic-free under dev-profile overflow
/// checks, and a u64 call counter wraps only after ~4 billion years of
/// benchmark-rate allocation.
#[derive(Clone, Copy)]
struct ThreadCounters {
    alloc_calls: u64,
    dealloc_calls: u64,
    realloc_calls: u64,
    bytes_allocated: u64,
    bytes_deallocated: u64,
}

impl ThreadCounters {
    const ZERO: Self = Self {
        alloc_calls: 0,
        dealloc_calls: 0,
        realloc_calls: 0,
        bytes_allocated: 0,
        bytes_deallocated: 0,
    };
}

thread_local! {
    /// Counters for the calling thread only. The `const` initializer is
    /// what makes this safe to touch from inside the global allocator:
    /// no lazy initialization (the first access cannot allocate, so no
    /// re-entrancy is possible) and no destructor (nothing runs at
    /// thread exit observing a torn state). Access is a plain TLS slot
    /// load/store on every supported target.
    static COUNTERS: Cell<ThreadCounters> = const { Cell::new(ThreadCounters::ZERO) };
}

/// Global allocator that wraps `std::alloc::System` and tracks allocation
/// statistics.
pub(crate) struct TraceAlloc;

static INNER: System = System;

/// # Safety
/// `TraceAlloc` is a thin wrapper around `std::alloc::System` that only
/// adds const-initialized thread-local counter updates before delegating.
/// The updates touch only the calling thread's own TLS slot — no
/// allocation, no lock, no synchronization — so no re-entrancy or
/// deadlock is possible. The underlying `System` allocator upholds the
/// `GlobalAlloc` contract — this impl forwards all `unsafe fn` arguments
/// unchanged, so the safety obligations documented on `GlobalAlloc`'s
/// methods (valid layout, valid ptr from a previous alloc, etc.) are
/// preserved by construction.
unsafe impl GlobalAlloc for TraceAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNTERS.with(|c| {
            let mut v = c.get();
            v.alloc_calls = v.alloc_calls.wrapping_add(1);
            v.bytes_allocated = v.bytes_allocated.wrapping_add(layout.size() as u64);
            c.set(v);
        });
        INNER.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        COUNTERS.with(|c| {
            let mut v = c.get();
            v.dealloc_calls = v.dealloc_calls.wrapping_add(1);
            v.bytes_deallocated = v.bytes_deallocated.wrapping_add(layout.size() as u64);
            c.set(v);
        });
        INNER.dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        COUNTERS.with(|c| {
            let mut v = c.get();
            v.realloc_calls = v.realloc_calls.wrapping_add(1);
            v.bytes_allocated = v.bytes_allocated.wrapping_add(new_size as u64);
            v.bytes_deallocated = v.bytes_deallocated.wrapping_add(layout.size() as u64);
            c.set(v);
        });
        INNER.realloc(ptr, layout, new_size)
    }
}

/// Snapshot of allocator statistics at a point in time.
#[derive(Debug, Clone, Default)]
pub(crate) struct AllocSnapshot {
    pub alloc_calls: u64,
    pub dealloc_calls: u64,
    pub realloc_calls: u64,
    pub bytes_allocated: u64,
    pub bytes_deallocated: u64,
}

impl AllocSnapshot {
    /// Take a snapshot of the calling thread's allocator counters.
    ///
    /// Thread attribution is the contract: a window delta covers the
    /// allocations the measuring thread itself performed, never the
    /// concurrent activity of other threads (see the module docs for
    /// the CI incident that pinned this).
    pub(crate) fn now() -> Self {
        COUNTERS.with(|c| {
            let v = c.get();
            Self {
                alloc_calls: v.alloc_calls,
                dealloc_calls: v.dealloc_calls,
                realloc_calls: v.realloc_calls,
                bytes_allocated: v.bytes_allocated,
                bytes_deallocated: v.bytes_deallocated,
            }
        })
    }

    /// Compute delta between two snapshots (after - before).
    pub(crate) fn delta(&self, before: &Self) -> AllocMetrics {
        let alloc = self.alloc_calls - before.alloc_calls;
        let dealloc = self.dealloc_calls - before.dealloc_calls;
        let realloc = self.realloc_calls - before.realloc_calls;
        let bytes_alloc = self.bytes_allocated - before.bytes_allocated;
        let bytes_dealloc = self.bytes_deallocated - before.bytes_deallocated;
        AllocMetrics {
            alloc_calls: alloc,
            dealloc_calls: dealloc,
            realloc_calls: realloc,
            bytes_allocated_total: bytes_alloc,
            bytes_deallocated_total: bytes_dealloc,
            heap_retained_bytes: bytes_alloc.saturating_sub(bytes_dealloc),
            alloc_calls_per_frame: 0.0, // computed by bench.rs
            dealloc_calls_per_frame: 0.0,
            heap_virtual_kib: 0, // filled from /proc on Linux
        }
    }
}

/// Allocator metrics computed from snapshot delta.
#[derive(Debug, Clone, Default)]
pub(crate) struct AllocMetrics {
    pub alloc_calls: u64,
    pub dealloc_calls: u64,
    pub realloc_calls: u64,
    pub bytes_allocated_total: u64,
    pub bytes_deallocated_total: u64,
    pub heap_retained_bytes: u64,
    pub alloc_calls_per_frame: f64,
    pub dealloc_calls_per_frame: f64,
    pub heap_virtual_kib: u64,
}

impl AllocMetrics {
    /// Read heap virtual size from /proc/self/status (Linux only).
    pub(crate) fn read_proc_heap(&mut self) {
        #[cfg(target_os = "linux")]
        {
            if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                for line in status.lines() {
                    if line.starts_with("VmData:") {
                        if let Some(kib) = line.split_whitespace().nth(1) {
                            self.heap_virtual_kib = kib.parse().unwrap_or(0);
                        }
                    }
                }
            }
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use std::hint::black_box;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    use super::*;

    /// The counting contract, unit level: allocations performed on
    /// OTHER threads must not move this thread's snapshot. This pins
    /// the FreeBSD CI incident of 2026-09-25 — under `cargo test`'s
    /// parallel shared-process execution, process-global counters
    /// attributed concurrent tests' allocations to the open benchmark
    /// window and tripped the cosmetics zero-alloc assertion with
    /// 16.3 "allocs per frame" of pure cross-thread noise.
    #[test]
    fn other_thread_allocations_do_not_move_this_threads_snapshot() {
        let before = AllocSnapshot::now();

        // A child thread performs a large, counted allocation churn.
        let child_allocs = Arc::new(AtomicU64::new(0));
        let counter = Arc::clone(&child_allocs);
        let handle = std::thread::spawn(move || {
            let mut done = 0u64;
            for _ in 0..20_000 {
                let v = vec![0u8; 256];
                black_box(&v);
                done += 1;
            }
            counter.store(done, Ordering::Relaxed);
        });
        handle.join().expect("noise thread must join");

        let after = AllocSnapshot::now();
        let child = child_allocs.load(Ordering::Relaxed);
        assert!(child >= 20_000, "child performed only {child} allocations");

        // The spawn/join machinery performs a handful of allocations on
        // THIS thread (thread packet, Arc); anything beyond that would
        // mean cross-thread attribution leaked back in.
        let delta = after.alloc_calls - before.alloc_calls;
        assert!(
            delta < 64,
            "main-thread snapshot moved by {delta} allocs while a child \
             thread performed {child} — counting must stay thread-attributed"
        );
    }

    /// The counting contract, positive side: the measured thread's own
    /// allocations DO move its snapshot (attribution is per-thread, not
    /// disabled).
    #[test]
    fn snapshot_counts_this_threads_own_allocations() {
        let before = AllocSnapshot::now();
        let owned: Vec<u8> = vec![7; 128];
        black_box(&owned);
        let after = AllocSnapshot::now();
        assert!(
            after.alloc_calls > before.alloc_calls,
            "an allocation on the measured thread must be counted"
        );
        assert!(
            after.bytes_allocated > before.bytes_allocated,
            "allocated bytes must be counted on the measured thread"
        );
    }
}
