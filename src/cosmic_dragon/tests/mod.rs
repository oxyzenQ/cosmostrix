// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cosmic Dragon Engine tests (Pattern C — dedicated tests/ subdir).
//!
//! Previously co-located as `cosmic_dragon/lock_tests.rs`, loaded via
//! `#[path]` from `cosmic_dragon/mod.rs` (Pattern B). Moved to a
//! dedicated `tests/` subdir (Pattern C).

#[cfg(test)]
mod lock;
