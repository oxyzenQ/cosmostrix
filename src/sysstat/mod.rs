// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! System statistics subsystem.
//!
//! Aggregates CPU, memory, env-level, and usage sampling helpers used by
//! the benchmark subsystem (`bench::*`), the HUD CPU/RSS lines
//! (`interactive::hud`), and the Crystal Dragon adaptive engine
//! (`crystal_dragon_engine::sensor`).
//!
//! All 4 submodules were relocated from src/ root as flat files (audit
//! M3). Re-exported as `pub(crate)` so the 15 existing
//! `crate::cpustat::Foo` / `crate::memstat::Foo` /
//! `crate::usagestat::Foo` / `crate::envstat::Foo` call sites continue
//! to resolve via the `pub(crate) use sysstat::*;` re-export in
//! main.rs.

pub(crate) mod cpustat;
pub(crate) mod envstat;
pub(crate) mod memstat;
pub(crate) mod usagestat;
