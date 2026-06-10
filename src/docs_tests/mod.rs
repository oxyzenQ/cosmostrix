// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: MIT

//! Documentation and metadata guard tests.
//!
//! Split into submodules to relieve file-size pressure on the original
//! single-file `docs_tests.rs` (which was at 993 LOC, dangerously close
//! to the 1000 LOC cap). Each submodule covers a logical category:
//!
//! - `assets` — demo asset existence and ordering
//! - `endurance` — resource monitor script, endurance docs, gitignore rules
//! - `metadata` — version, tagline, casing, AUR, changelog ordering
//! - `readme` — README structure guards (release notes, changelog link, etc.)
//! - `release` — release candidate doc, benchmark doc, release workflow auth
//! - `safety` — unsafe audits, debt markers, source file hygiene
//! - `zactrix` — Zactrix Engine/Cache/Core docs, planner tests, architecture

mod assets;
mod endurance;
mod metadata;
mod readme;
mod release;
mod safety;
mod zactrix;
