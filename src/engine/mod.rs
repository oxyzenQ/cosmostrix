// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Engine subsystem — groups the three dragon engines under one
//! directory for easier maintenance and a cleaner directory structure.
//!
//! v60.0.0-beta.1 (Z-master-1X): the three dragon engine modules
//! (`cosmic_dragon_engine`, `chroma_dragon_engine`,
//! `crystal_dragon_engine`) were moved from `src/` root to
//! `src/engine/` to reduce the top-level directory cost and centralize
//! all engine code under one namespace.
//!
//! The re-exports below keep the `crate::` paths stable: code that
//! references `crate::chroma_dragon_engine::palette` continues to
//! resolve through the `pub use` chain `engine → chroma_dragon_engine`.

pub(crate) mod chroma_dragon_engine;
pub(crate) mod cosmic_dragon_engine;
pub(crate) mod crystal_dragon_engine;
