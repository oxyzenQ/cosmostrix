// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! # Cosmic Dragon Engine — Diff-Based Rendering Engine
//!
//! This module holds the substantive rendering engine code, organized
//! into 4 cooperating subsystems. Together they implement the Cosmic Dragon
//! Diff-Based Rendering Engine — cosmostrix's core rendering substrate.
//!
//! ## Subsystems
//!
//! | Subsystem | Module | Role |
//! |----------|--------|------|
//! | Cloud simulation | `cloud` | Rain simulation, monolith, render pipeline, ecosystem, phosphor decay, ghost events |
//! | Frame buffer | `frame` | Differential frame buffer with double-buffered generation-based dirty tracking |
//! | Terminal output | `terminal` | Raw-mode guard, alternate screen, RLE-batched ANSI diff pipeline, `/dev/tty` fallback |
//! | Runtime types | `runtime` | Runtime type vocabulary: `ColorScheme`, `ColorMode`, `BoldMode`, `ColorPipeline` |
//!
//! ## Re-export pattern
//!
//! `main.rs` re-exports each subsystem at the crate root via
//! `pub use cosmic_dragon_engine::{cloud, frame, runtime, terminal};`
//! so all existing `crate::cloud::Foo`, `crate::frame::Foo`,
//! `crate::runtime::Foo`, and `crate::terminal::Foo` call sites continue
//! to resolve unchanged. This makes the move transparent to the ~224 call
//! sites across the codebase (54 + 44 + 17 + 109 = 224).
//!
//! ## Naming disambiguation
//!
//! - `src/engine/cosmic_dragon_engine/` (this module) — substantive rendering
//!   engine, ~15K LOC across 4 subsystems.
//! - `src/cosmic_dragon_incubator/` — separate namespace for v15+
//!   experimental / concluded work (~200 LOC). NOT a peer engine; just
//!   an incubator.
//! - `src/engine/chroma_dragon_engine/` — substantive coloring engine (~3.5K LOC).
//! - `src/engine/crystal_dragon_engine/` — substantive ambient intelligence
//!   engine (~3K LOC).
//!
//! The actual "Cosmic Dragon Diff-Based Rendering Engine" brand lives in
//! the docs, the benchmark output, the changelog, and the code's behavior
//! — and now also in this directory's name. The engine topology itself
//! (frame, terminal, runtime stay as crate-root primitives via re-export)
//! is unchanged from the prior "Flat Forever" policy.
//!
//! ## History
//!
//! This directory was created 2026-08-19 by relocating 4 modules
//! (`src/cloud/`, `src/frame.rs`, `src/terminal/`, `src/runtime.rs`) into
//! a single `cosmic_dragon_engine/` namespace. The move was owner-mandated
//! to make the engine's substantive status explicit — previously these
//! modules sat at the crate root with no branding, while a separate
//! `src/cosmic_dragon_incubator/` namespace held experimental work.
//!
//! The re-export pattern preserves all existing `crate::cloud::*`,
//! `crate::frame::`, `crate::terminal::`, and `crate::runtime::*` paths
//! — zero call-site edits needed. The 4 modules retain their internal
//! structure unchanged.

pub(crate) mod cloud;
pub(crate) mod frame;
pub(crate) mod runtime;
pub(crate) mod terminal;
