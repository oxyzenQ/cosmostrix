// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Main render loop entry point.
//!
//! The actual `rain_at()` method was extracted to `rain_at.rs` to keep
//! this file under the 800-LOC cap. This file retains only the test-only
//! `rain()` convenience wrapper that calls `rain_at(frame, Instant::now())`.

#[cfg(test)]
use std::time::Instant;

#[cfg(test)]
use crate::frame::Frame;

use super::Cloud;

impl Cloud {
    /// No-arg convenience wrapper around `rain_at`. Test-only — production
    /// callers pass an explicit `Instant` captured before the frame work
    /// begins (so the same instant is reused for the surrounding timing
    /// measurement, see `event_loop.rs::rain_at(frame, work_start)`).
    #[cfg(test)]
    pub fn rain(&mut self, frame: &mut Frame) {
        self.rain_at(frame, Instant::now());
    }
}
