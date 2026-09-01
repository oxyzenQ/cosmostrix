// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Type definitions + constants subsystem.
//!
//! Consolidated from src/ root flat files into src/types/ for a clean
//! src/ root (only main.rs at root). Re-exported at crate root via
//! `pub(crate) use types::{constants, cell, rain_style, renderer_info};`
//! in main.rs so all existing call sites resolve unchanged.
//!
//! The v80.0.0-beta.1 msg-fill-style subsystem graduated to its own crate-root
//! directory (`src/msg_fill_style/`, one file per style) in the
//! owner-mandated plug-and-play refactor.

pub(crate) mod cell;
pub(crate) mod constants;
pub(crate) mod rain_style;
pub(crate) mod renderer_info;
