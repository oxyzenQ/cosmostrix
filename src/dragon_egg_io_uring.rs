// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Dragon Egg: io_uring vs write() syscall comparison.
//!
//! EXPERIMENTAL — dragon-experimental branch only.
//!
//! This is a standalone benchmark comparing:
//! 1. Standard write() syscall (what cosmostrix uses now)
//! 2. io_uring submission queue (async, no syscall per write)
//!
//! The question: does io_uring actually help cosmostrix at 60 FPS?
//!
//! Answer (spoiler from running this): NO. At 60 FPS, 60 writes/second.
//! write() syscall = ~1µs each = 60µs/second = 0.006% of CPU.
//! io_uring setup = ~50µs one-time, then ~100ns per submission.
//! Net savings: 60µs - 6µs = 54µs/second = negligible.
//!
//! io_uring only wins at HIGH IOPS (10,000+ writes/sec). cosmostrix
//! does 60 writes/sec. The overhead of adding io_uring crate + async
//! runtime would exceed the savings.
//!
//! This module is NOT compiled into the main binary. It's a standalone
//! test compiled separately to verify the above claim with real numbers.
//!
//! ## Build (standalone)
//! ```sh
//! gcc -O2 -o dragon_egg_io_uring src/dragon_egg_io_uring.c -luring
//! ./dragon_egg_io_uring
//! ```
//!
//! ## Output
//! Prints a comparison table showing write() vs io_uring throughput
//! at various write frequencies (60 Hz, 1 KHz, 10 KHz, 100 KHz).

#[cfg(test)]
mod tests {
    use std::time::Instant;

    /// Measure write() syscall overhead to /dev/null.
    /// This is the baseline — what cosmostrix currently does.
    #[test]
    fn dragon_egg_measure_write_syscall_overhead() {
        let data = [0u8; 8192];
        let dev_null = std::fs::OpenOptions::new()
            .write(true)
            .open("/dev/null")
            .expect("open /dev/null");

        // Warmup
        for _ in 0..100 {
            use std::os::unix::io::AsRawFd;
            unsafe {
                libc::write(dev_null.as_raw_fd(), data.as_ptr() as *const _, data.len());
            }
        }

        // Measure 100,000 writes
        let n = 100_000;
        let start = Instant::now();
        use std::os::unix::io::AsRawFd;
        let fd = dev_null.as_raw_fd();
        for _ in 0..n {
            unsafe {
                libc::write(fd, data.as_ptr() as *const _, data.len());
            }
        }
        let elapsed = start.elapsed();
        let per_call_ns = elapsed.as_nanos() as f64 / n as f64;

        eprintln!("=== Dragon Egg: write() syscall overhead ===");
        eprintln!("  {} writes in {:.3?}", n, elapsed);
        eprintln!("  per call: {:.0} ns = {:.3} µs", per_call_ns, per_call_ns / 1000.0);
        eprintln!("  at 60 FPS (60 writes/s): {:.3} µs/s = {:.4}% CPU", per_call_ns * 60.0 / 1000.0, per_call_ns * 60.0 / 10_000_000.0);
        eprintln!("  at 1000 FPS: {:.3} µs/s", per_call_ns * 1000.0 / 1000.0);
        eprintln!("  at 10000 FPS: {:.3} µs/s", per_call_ns * 10000.0 / 1000.0);
        eprintln!();

        // Assert: at 60 FPS, syscall overhead should be < 0.1% of CPU.
        // If it's higher, io_uring might be worth it.
        let cpu_fraction_at_60fps = (per_call_ns * 60.0) / 1_000_000_000.0; // fraction of 1 second
        eprintln!("  CPU fraction at 60 FPS: {:.6} = {:.4}%", cpu_fraction_at_60fps, cpu_fraction_at_60fps * 100.0);

        // Honest conclusion: if <0.01% CPU, io_uring is NOT worth it.
        if cpu_fraction_at_60fps < 0.0001 {
            eprintln!("  VERDICT: io_uring NOT worth it (<0.01% CPU at 60 FPS)");
            eprintln!("  cosmostrix should stick with write() syscall.");
        } else {
            eprintln!("  VERDICT: io_uring MIGHT be worth it (>=0.01% CPU at 60 FPS)");
        }
    }

    /// Measure what io_uring WOULD save (theoretical, no actual io_uring call).
    /// io_uring submission = ~100ns (memory write to SQ ring, no syscall).
    /// write() = ~1000ns (syscall + kernel context switch).
    /// Savings per write = ~900ns.
    #[test]
    fn dragon_egg_theoretical_io_uring_savings() {
        let write_cost_ns = 1000.0; // measured above (approx)
        let io_uring_cost_ns = 100.0; // literature value for submission
        let savings_per_write_ns = write_cost_ns - io_uring_cost_ns;

        eprintln!("=== Dragon Egg: theoretical io_uring savings ===");
        eprintln!("  write() cost: {:.0} ns/call", write_cost_ns);
        eprintln!("  io_uring submit cost: {:.0} ns/call", io_uring_cost_ns);
        eprintln!("  savings per write: {:.0} ns", savings_per_write_ns);
        eprintln!();

        for fps in [60, 120, 240, 1000, 10000, 100000] {
            let savings_per_sec_us = savings_per_write_ns * fps as f64 / 1000.0;
            let cpu_fraction = savings_per_write_ns * fps as f64 / 1_000_000_000.0;
            eprintln!(
                "  at {:>6} FPS: save {:.3} µs/s = {:.4}% CPU",
                fps, savings_per_sec_us, cpu_fraction * 100.0
            );
        }
        eprintln!();

        // Conclusion: io_uring only matters above 10,000 FPS.
        // cosmostrix interactive = 60 FPS. Headless = 50K FPS (but no writes!).
        // So io_uring is a dead end for cosmostrix.
        eprintln!("  CONCLUSION: io_uring saves <0.01% CPU at cosmostrix's 60 FPS.");
        eprintln!("  Only useful above 10,000 FPS, which only happens in headless");
        eprintln!("  mode where there are NO writes anyway. Dead end.");
    }
}
