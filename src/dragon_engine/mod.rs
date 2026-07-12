// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Dragon Engine — namespace wrapper for cosmostrix's rendering engine.
//!
//! This module provides a clean, logical grouping of cosmostrix's
//! internal modules under the `dragon_engine` namespace. It is a
//! **pure re-export wrapper** — no code is moved or duplicated.
//!
//! ## Structure
//!
//! - [`core`] — terminal I/O, frame buffer, cell representation, color cache
//! - [`cloud`] — rain simulation (droplets, phosphor, atmosphere, monolith)
//! - [`cli`] — CLI parsing, config, validation
//! - [`bench`] — benchmark runner, report formatting, JSON output
//! - [`interactive`] — event loop, HUD overlay, input handling, adaptive pacing
//! - [`diagnostics`] — verbose output, system detection, memory/CPU stats
//!
//! ## Usage
//!
//! Existing code continues to use flat `crate::terminal::Terminal` paths.
//! The dragon_engine namespace is additive — it provides an alternative
//! organized path for external consumers and documentation:
//!
//! ```ignore
//! use crate::dragon_engine::core::Terminal;
//! use crate::dragon_engine::cloud::Cloud;
//! ```

pub mod bench;
pub mod cli;
pub mod cloud;
pub mod core;
pub mod diagnostics;
pub mod interactive;
