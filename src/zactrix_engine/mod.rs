// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Zactrix Engine — Architecture facade for Cosmostrix.
//!
//! This module is the single entry point for all Zactrix subsystems. It
//! re-exports types from submodules so that existing `crate::zactrix_engine::*`
//! import paths continue to work without modification.
//!
//! ## Architecture — Single-Thread by Design
//!
//! ```text
//! src/zactrix_engine/
//!   mod.rs        — this facade (re-exports)
//!   core.rs       — deterministic helpers (frame jitter, monolith depth)
//!   cache.rs      — bounded generation-aware cache policy
//!   scheduler.rs  — deterministic planner (always SingleCore mode)
//!   system.rs     — Zactrix System diagnostic model (RuntimeMode, IdlePolicy)
//!   render.rs     — render planning boundary (TerminalWriterPolicy)
//!   metrics.rs    — diagnostic labels
//! ```
//!
//! ## Invariant
//!
//! Cosmostrix is a single-thread, single-core renderer. Terminal writing is
//! **single-owner** at all times. No worker threads are spawned. No
//! `available_parallelism()` syscall is made. This is an immutable
//! architectural invariant.
//!
//! ## Backward Compatibility
//!
//! Existing import paths are preserved via re-exports:
//! - `crate::zactrix_engine::{EngineMode, EnginePlan, EngineProbe}` — from scheduler
//! - `crate::zactrix_cache::{CachePolicy, InvalidationEvent}` — via wrapper mod in main.rs
//! - `crate::zactrix_core::{classify_frame_jitter, ...}` — via wrapper mod in main.rs

pub(crate) mod cache;
pub(crate) mod core;
pub(crate) mod metrics;
pub(crate) mod render;
pub(crate) mod scheduler;
pub(crate) mod system;

// ── Facade re-exports: scheduler types ─────────────────────────────────────

#[allow(unused_imports)]
pub(crate) use scheduler::EngineMode;
#[allow(unused_imports)]
pub(crate) use scheduler::EnginePlan;
pub(crate) use scheduler::EngineProbe;

// ── Facade re-exports: system types ────────────────────────────────────────

#[allow(unused_imports)]
pub(crate) use system::IdlePolicy;
#[allow(unused_imports)]
pub(crate) use system::RuntimeMode;
#[allow(unused_imports)]
pub(crate) use system::ZactrixSystemConfig;

// ── Facade re-exports: render types ───────────────────────────────────────

#[allow(unused_imports)]
pub(crate) use render::RenderPlan;
#[allow(unused_imports)]
pub(crate) use render::TerminalWriterPolicy;
