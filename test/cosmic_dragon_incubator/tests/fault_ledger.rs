// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The fault-floor ledger — the kernel-side memory mirror of the
//! zero-alloc contract (night-frontier-Z7, Kernel Floor 2c).
//!
//! INV-18's counting allocator proves what Rust ASKS for: the lean
//! frame path performs exactly zero heap allocations per frame. But
//! the allocator ledger cannot see what the KERNEL actually GRANTS:
//! stack growth (a guard page touched by deeper call chains),
//! allocator arena growth outside the counted calls, and any
//! allocation that bypasses the global allocator hook entirely. All
//! of those surface as page faults — the kernel's own memory-truth
//! counters. This module pins the same pinned steady-state window on
//! that kernel axis: across the identical 200-frame warmup + 180-frame
//! ledger window (`super::alloc_ledger`'s rig, shared verbatim), the
//! calling thread's page-fault counters must read EXACTLY ZERO:
//!
//!   one ledger frame = `Cloud::rain_at` (the sim + diff step)
//!                    + `BenchIoWriter::write_frame` (the lean
//!                      per-dirty-cell emission)
//!                    + `Frame::clear_dirty`.
//!
//! Zero minor faults says: no new pages of ANY class entered this
//! thread's address space during the window — the proof rises from
//! "Rust requested nothing" to "the kernel granted nothing". Zero
//! major faults pins the no-paging clause: not one file-backed or
//! swapped-in page was touched.
//!
//! Mechanism: `/proc/thread-self/stat` (Linux 4.1+) resolves to the
//! CALLING THREAD's task entry, so the counters are per-thread with
//! no allocator hook and no unsafe — the same isolation the FreeBSD
//! CI incident forced on INV-18 (per-thread attribution), read
//! straight from the kernel instead of reconstructed from allocator
//! calls. Concurrent test threads cannot pollute this delta: every
//! thread reads its own fault counters.
//!
//! Honest scope, stated once: the contract covers the LEAN path —
//! the same scope INV-18 covers — on Linux (the kernel-floor family
//! is Linux-anchored; the strace syscall-floor lane is Linux-only
//! too). Other hosts keep the INV-18 allocator contract only. The
//! snapshot read itself may allocate (a reused scratch buffer), so
//! the window primes the read path twice before opening: the delta
//! measures the frames, not the measuring. And the window is a
//! userspace contract on a cooperating kernel — exotic environments
//! (NUMA page balancing, memory scanners) can inject minor faults no
//! allocator can cause; the failure message names the counters and
//! the suspected environment cause, so a trip is diagnosable from
//! the message alone.
//!
//! The teeth: a positive control runs the same window with one
//! deliberate fresh-page touch and requires the counters to rise —
//! a ledger that cannot see a forced fault cannot enforce zero.

use std::io::Read;
use std::time::{Duration, Instant};

use super::alloc_ledger::{LEDGER_FRAMES, LEDGER_WARMUP_FRAMES, build_ledger_rig};

/// The probe allocation for the positive control: 2 MiB, far above
/// every allocator's mmap threshold, so the pages are fresh kernel
/// grants rather than recycled arena chunks.
const FRESH_PROBE_BYTES: usize = 2 * 1024 * 1024;

/// One thread's kernel page-fault counters, read from
/// `/proc/thread-self/stat`.
struct FaultSnapshot {
    minor: u64,
    major: u64,
}

impl FaultSnapshot {
    /// Faults between `after` and `before` (counter deltas only grow).
    fn delta(after: &FaultSnapshot, before: &FaultSnapshot) -> FaultSnapshot {
        FaultSnapshot {
            minor: after.minor - before.minor,
            major: after.major - before.major,
        }
    }
}

/// Read the calling thread's page-fault counters from the kernel.
///
/// The reader is deliberately heap-free: one `File::read` into a
/// fixed stack buffer, parsed in place. Every allocation in the
/// measuring path is a fault the window could not attribute (a
/// perturbed allocator arena — say, a concurrent test's mmap churn —
/// can push even a stable small allocation onto a fresh page), so
/// the reader spends nothing: after the stack pages prime, the read
/// path itself cannot fault, and the window measures only the
/// frames.
///
/// The stat line's layout is `pid (comm) state ppid ... flags minflt
/// cminflt majflt cmajflt ...`: the comm field is parenthesized and
/// may itself contain spaces, so the parse resumes after the LAST
/// `)`. After it: `[0]` state, `[1]` ppid, `[2]` pgrp, `[3]` session,
/// `[4]` tty_nr, `[5]` tpgid, `[6]` flags, `[7]` minflt, `[8]`
/// cminflt, `[9]` majflt. A missing file (a non-Linux kernel or a
/// pre-4.1 one without `thread-self`), a short read, or an
/// unexpected layout returns `None` — the caller fails loudly rather
/// than enforcing against a counter it does not understand.
fn read_thread_fault_counters() -> Option<FaultSnapshot> {
    // The stat line is a few hundred bytes (the comm cap is 15);
    // 1024 covers it with headroom for every kernel layout.
    let mut buf = [0u8; 1024];
    let mut file = std::fs::File::open("/proc/thread-self/stat").ok()?;
    let mut filled = 0usize;
    loop {
        let n = file.read(&mut buf[filled..]).ok()?;
        if n == 0 {
            break;
        }
        filled += n;
        if filled == buf.len() {
            break;
        }
    }
    let raw = std::str::from_utf8(&buf[..filled]).ok()?;
    let comm_close = raw.rfind(')')?;
    let rest = &raw[comm_close + 1..];
    let mut minor = None;
    let mut major = None;
    for (index, token) in rest.split_whitespace().enumerate() {
        match index {
            7 => minor = token.parse::<u64>().ok(),
            9 => major = token.parse::<u64>().ok(),
            _ => {}
        }
        if major.is_some() {
            break;
        }
    }
    Some(FaultSnapshot {
        minor: minor?,
        major: major?,
    })
}

/// Snapshot with a loud failure: the contract must never pass
/// silently because the kernel counters went unread.
fn fault_snapshot_or_panic(stage: &str) -> FaultSnapshot {
    read_thread_fault_counters().unwrap_or_else(|| {
        panic!(
            "the fault-floor ledger cannot read /proc/thread-self/stat ({stage}) \
             — the kernel-side contract is unenforceable here; this is a \
             Linux 4.1+ contract surface and the parse must match the \
             documented stat layout"
        )
    })
}

/// INV-19: the lean frame path takes EXACTLY ZERO page faults per
/// window in steady state — the kernel grants no new pages.
///
/// The window is INV-18's shape read on the kernel axis: prime the
/// snapshot read path (twice, so the reader's own buffers reach their
/// steady-state footprint), open the ledger window, run the identical
/// 180 lean frames, read again. Any new page — heap growth the
/// allocator hook cannot see, stack growth past the warmup's
/// high-water mark, a paged-in file-backed page — trips this test.
#[test]
fn lock_inv19_lean_frame_path_takes_zero_page_faults() {
    let (mut cloud, mut frame, mut writer) = build_ledger_rig();
    let target_period = Duration::from_secs_f64(1.0 / 60.0);

    let mut sim_now = Instant::now();
    for _ in 0..LEDGER_WARMUP_FRAMES {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        writer.write_frame(&frame);
        frame.clear_dirty();
    }

    // Prime the read path twice: the first reads size the reader's
    // scratch buffers (allocations of their own), the second confirms
    // the steady-state footprint. The window measures the frames, not
    // the measuring.
    let _prime_a = fault_snapshot_or_panic("prime read a");
    let _prime_b = fault_snapshot_or_panic("prime read b");

    // The ledger window opens: per-thread counters from
    // /proc/thread-self, so concurrent test threads cannot pollute
    // this delta (the FreeBSD CI incident design, kernel-side).
    let before = fault_snapshot_or_panic("window open");
    for _ in 0..LEDGER_FRAMES {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        writer.write_frame(&frame);
        frame.clear_dirty();
    }
    let after = fault_snapshot_or_panic("window close");

    let delta = FaultSnapshot::delta(&after, &before);
    assert_eq!(
        delta.minor, 0,
        "the lean frame path took {} minor page faults across {LEDGER_FRAMES} \
         frames — the kernel granted a page the zero-alloc ledger could not \
         see (stack growth, allocator arena growth, or a hook-bypassing \
         allocation); on an exotic environment suspect kernel page \
         balancing first, but verify against this delta, not against hope",
        delta.minor
    );
    assert_eq!(
        delta.major, 0,
        "the lean frame path took {} major page faults across {LEDGER_FRAMES} \
         frames — a file-backed or swapped-in page entered the window; \
         the no-paging clause is part of the floor",
        delta.major
    );
}

/// The ledger's teeth: the SAME window shape with one deliberate
/// fresh-page touch makes the kernel counters rise.
///
/// A tripwire that cannot fire is decoration. This positive control
/// touches 2 MiB of brand-new pages mid-window (far above the mmap
/// threshold, written page by page so each page must fault in) and
/// requires the minor-fault delta to be at least one — if this fires
/// zero, the ledger is not reading the calling thread's kernel truth
/// and the zero-fault contract is unenforceable. The bound is
/// deliberately a floor, not an exact count: the exact page count of
/// one allocation is the kernel's and the allocator's to choose (page
/// size, THP, arena reuse), and a contract must not pretend to rule
/// what it does not own.
#[test]
fn lock_inv19_fault_ledger_counts_deliberate_fresh_pages() {
    let (mut cloud, mut frame, mut writer) = build_ledger_rig();
    let target_period = Duration::from_secs_f64(1.0 / 60.0);

    let mut sim_now = Instant::now();
    for _ in 0..LEDGER_WARMUP_FRAMES {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        writer.write_frame(&frame);
        frame.clear_dirty();
    }

    let _prime_a = fault_snapshot_or_panic("prime read a");
    let _prime_b = fault_snapshot_or_panic("prime read b");

    let before = fault_snapshot_or_panic("window open");
    for i in 0..LEDGER_FRAMES {
        sim_now += target_period;
        cloud.rain_at(&mut frame, sim_now);
        writer.write_frame(&frame);
        frame.clear_dirty();
        if i == LEDGER_FRAMES / 2 {
            // The deliberate kernel-visible event: fresh pages, each
            // written so the kernel must grant them (lazily mapped
            // anonymous memory faults on first write).
            let mut fresh = vec![0u8; FRESH_PROBE_BYTES];
            for page in fresh.chunks_mut(4096) {
                page[0] = 1;
            }
            std::hint::black_box(&fresh);
            drop(fresh);
        }
    }
    let after = fault_snapshot_or_panic("window close");

    let delta = FaultSnapshot::delta(&after, &before);
    assert!(
        delta.minor >= 1,
        "the positive control touched {FRESH_PROBE_BYTES} fresh bytes and the \
         fault ledger counted {} minor page faults — if this fires zero, \
         /proc/thread-self/stat is not reaching this thread's counters and \
         the zero-fault contract is unenforceable",
        delta.minor
    );
}
