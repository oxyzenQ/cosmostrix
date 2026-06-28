// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Static renderer introspection metadata.
//!
//! Provides a single source of truth for renderer capabilities,
//! used by --doctor, --benchmark, and --info outputs.

use crate::runtime::ColorMode;

/// Static renderer metadata describing the rendering pipeline.
pub struct RendererInfo {
    pub identity: &'static str,
    pub backend: &'static str,
    pub pacing: &'static str,
    pub unicode: &'static str,
    pub frame_strategy: &'static str,
    pub dirty_tracking: &'static str,
    pub color_depth: &'static str,
    pub io_strategy: &'static str,
}

/// Return the renderer info for the given effective color mode.
#[inline]
pub fn renderer_info(color_mode: ColorMode) -> RendererInfo {
    RendererInfo {
        identity:
            "production-grade cinematic Matrix rain renderer for serious terminal environments.",
        backend: "ansi-stream",
        pacing: "adaptive",
        unicode: "utf8-singlewidth",
        frame_strategy: "differential",
        dirty_tracking: "bitvec+generation",
        io_strategy: "crossterm-queue-batch",
        color_depth: match color_mode {
            ColorMode::TrueColor => "truecolor",
            ColorMode::Color256 => "256",
            ColorMode::Color16 => "16",
            ColorMode::Mono => "mono",
        },
    }
}
