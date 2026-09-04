// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Chroma Dragon tests (Pattern C — dedicated tests/ subdir).
//!
//! Previously co-located as `chroma/color_detection_tests.rs` and
//! `chroma/lock_tests.rs`, loaded via `#[path]` from `chroma/mod.rs`
//! (Pattern B). Moved to a dedicated `tests/` subdir (Pattern C).

#[cfg(test)]
mod color_detection;
#[cfg(test)]
mod lock;
#[cfg(test)]
mod lock_inv13_19;
#[cfg(test)]
mod night_research1;
