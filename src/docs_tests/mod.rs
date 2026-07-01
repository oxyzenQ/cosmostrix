// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

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
//! - `zactrix_integration` — v4.8 lab integration audit guards
//! - `atmosphere` — Controlled atmosphere preset doc guards (v4.6.0 Phase 3)
//! - `profile` — Profile ecosystem contract guards (v4.7.0 Phase 1)
//! - `terminal_lifecycle` — Terminal lifecycle matrix docs guards (v4.9.0 Phase 3)
//! - `doctor_report` — Doctor/report polish source and docs guards (v4.9.0 Phase 4)
//! - `v5_nightfall` — v5.0.0 Nightfall product identity foundation guards

mod assets;
mod atmosphere;
mod doctor_report;
mod endurance;
mod metadata;
mod profile;
mod readme;
mod release;
mod safety;
mod terminal_lifecycle;
mod v5_nightfall;
mod zactrix;
mod zactrix_integration;
