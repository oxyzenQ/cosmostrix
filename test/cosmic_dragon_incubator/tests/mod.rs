// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! Cosmic Dragon Engine tests (Pattern C — dedicated tests/ subdir).
//!
//! Previously co-located as `cosmic_dragon/lock_tests.rs`, loaded via
//! `#[path]` from `cosmic_dragon/mod.rs` (Pattern B). Moved to a
//! dedicated `tests/` subdir (Pattern C).

#[cfg(test)]
mod alloc_ledger;
// The fault-floor ledger reads /proc/thread-self/stat - a Linux 4.1+
// contract surface (the kernel-floor family is Linux-anchored; see
// the module docs). The gate IS the honest scope: other hosts keep
// the INV-18 allocator ledger only.
#[cfg(all(test, target_os = "linux"))]
mod fault_ledger;
// The information ledger (INV-21, night-frontier-Z9): the field's
// per-style entropy and Gini bands, cross-architecture-safe by
// construction (no OS surface, pure frame math) — compiled on every
// host.
#[cfg(test)]
mod info_ledger;
#[cfg(test)]
mod lock;
