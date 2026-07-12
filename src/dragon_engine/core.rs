// Copyright (C) 2026 rezky_nightky
#![allow(unused_imports)]
// SPDX-License-Identifier: GPL-3.0-only

//! Core rendering primitives: terminal I/O, frame buffer, cells, colors.

pub(crate) use crate::cell::Cell;
pub(crate) use crate::color_cache::ColorCache;
pub(crate) use crate::frame::Frame;
pub(crate) use crate::sgr_format;
pub(crate) use crate::termdetect::TerminalCaps;
pub(crate) use crate::terminal::Terminal;
