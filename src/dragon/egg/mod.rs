// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Dragon Egg — Experimental Benchmarks
//!
//! Standalone benchmarks that probe the cost of low-level operations
//! (syscalls, allocator behavior, cache effects) to inform future
//! optimization decisions. Each module is `#[cfg(test)]`-gated and
//! compiles only under `cargo test`.
//!
//! ## Inhabitants
//!
//! - [`io_uring`] — compares `write()` syscall overhead vs theoretical
//!   io_uring savings. Verdict: io_uring is NOT worth it at cosmostrix's
//!   60 FPS write rate (60 writes/sec). The overhead of adding the
//!   `io_uring` crate + async runtime exceeds the ~54µs/sec savings.
//!
//! ## Policy
//!
//! Dragon-egg benchmarks are **honest experiments**: they measure, report
//! findings, and inform decisions. They do NOT become production code paths.
//! When an experiment concludes, its findings are documented in
//! `docs/DRAGON_FINDINGS.md` and the benchmark itself stays here as a
//! reproducible record.

#[cfg(test)]
pub mod io_uring;
