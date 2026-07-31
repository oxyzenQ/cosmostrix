// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Cosmic Dragon Egg — Experimental Benchmarks
//!
//! Standalone benchmarks that probe the cost of low-level operations
//! (syscalls, allocator behavior, cache effects) to inform future
//! optimization decisions. Each module is `#[cfg(test)]`-gated and
//! compiles only under `cargo test`.
//!
//! ## Inhabitants
//!
//! - [`io_uring_rejected`] — measures `write()` syscall overhead and computes
//!   what io_uring would theoretically save at cosmostrix's 60 FPS write rate.
//!   Verdict: io_uring is **NOT worth it** at 60 writes/sec — the overhead of
//!   adding the `io_uring` crate + async runtime exceeds the ~54µs/sec savings.
//!   Renamed in v30 from `io_uring.rs` to make the conclusion visible in the
//!   file name itself. (Unix-only — uses `libc::write` to `/dev/null`.)
//!
//! ## Policy
//!
//! Cosmic Dragon-egg benchmarks are **honest experiments**: they measure, report
//! findings, and inform decisions. They do NOT become production code paths.
//! When an experiment concludes, its findings are documented in
//! `docs/COSMIC_DRAGON_FINDINGS.md` and the benchmark itself stays here as a
//! reproducible record.

#[cfg(all(test, unix))]
pub mod io_uring_rejected;
