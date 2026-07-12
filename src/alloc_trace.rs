// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Allocator tracing — global allocator wrapper that counts alloc/dealloc calls.
//!
//! Phase 5 of DeepSeek benchmark restructuring plan.
//!
//! Wraps `std::alloc::System` with atomic counters for alloc/dealloc/realloc
//! calls and bytes. Always active (overhead = ~2ns per call from atomic increment).
//! Stats are read by the benchmark to report allocation patterns.
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
use std::sync::atomic::{AtomicU64, Ordering};

static ALLOC_CALLS: AtomicU64 = AtomicU64::new(0);
static DEALLOC_CALLS: AtomicU64 = AtomicU64::new(0);
static REALLOC_CALLS: AtomicU64 = AtomicU64::new(0);
static BYTES_ALLOCATED: AtomicU64 = AtomicU64::new(0);
static BYTES_DEALLOCATED: AtomicU64 = AtomicU64::new(0);

/// Global allocator that wraps `std::alloc::System` and tracks allocation
/// statistics.
pub struct TraceAlloc;

static INNER: System = System;

unsafe impl GlobalAlloc for TraceAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
        BYTES_ALLOCATED.fetch_add(layout.size() as u64, Ordering::Relaxed);
        INNER.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        DEALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
        BYTES_DEALLOCATED.fetch_add(layout.size() as u64, Ordering::Relaxed);
        INNER.dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        REALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
        BYTES_ALLOCATED.fetch_add(new_size as u64, Ordering::Relaxed);
        BYTES_DEALLOCATED.fetch_add(layout.size() as u64, Ordering::Relaxed);
        INNER.realloc(ptr, layout, new_size)
    }
}

/// Snapshot of allocator statistics at a point in time.
#[derive(Debug, Clone, Default)]
pub struct AllocSnapshot {
    pub alloc_calls: u64,
    pub dealloc_calls: u64,
    pub realloc_calls: u64,
    pub bytes_allocated: u64,
    pub bytes_deallocated: u64,
}

impl AllocSnapshot {
    /// Take a snapshot of current allocator counters.
    pub fn now() -> Self {
        Self {
            alloc_calls: ALLOC_CALLS.load(Ordering::Relaxed),
            dealloc_calls: DEALLOC_CALLS.load(Ordering::Relaxed),
            realloc_calls: REALLOC_CALLS.load(Ordering::Relaxed),
            bytes_allocated: BYTES_ALLOCATED.load(Ordering::Relaxed),
            bytes_deallocated: BYTES_DEALLOCATED.load(Ordering::Relaxed),
        }
    }

    /// Compute delta between two snapshots (after - before).
    pub fn delta(&self, before: &Self) -> AllocMetrics {
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
pub struct AllocMetrics {
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
    pub fn read_proc_heap(&mut self) {
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
