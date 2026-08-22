// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Cinematic event implementations for the GhostEventScheduler.
//!
//! Each event type is a struct implementing the `GhostEvent` trait
//! defined in `super::ghost_events`. New event types are added here
//! without modifying the renderer or event manager.

pub(crate) mod ghost;

pub(crate) use ghost::GhostEvent;
