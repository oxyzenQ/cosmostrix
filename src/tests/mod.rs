// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Crate-level integration / regression tests (Pattern C unification).
//!
//! Previously these were flat files at src/ root declared via
//! `mod foo_tests;` in main.rs (Pattern A). Consolidated into a single
//! `tests/` directory (Pattern C) for navigability.
//!
//! Each submodule corresponds 1:1 to a former root test file:
//! - `loc`         <- `src/loc_tests.rs`         (Rust LOC guard)
//! - `property`     <- `src/property_tests.rs`   (proptest config parser)
//! - `terminal`     <- `src/terminal_tests.rs`   (terminal sequence tests)
//! - `width_guard`  <- `src/width_guard_tests.rs` (Bug #11 width=1 invariant)

#[cfg(test)]
mod loc;
#[cfg(test)]
mod property;
#[cfg(test)]
mod terminal;
#[cfg(test)]
mod width_guard;
