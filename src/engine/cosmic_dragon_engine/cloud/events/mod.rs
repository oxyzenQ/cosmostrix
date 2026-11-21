// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! Cinematic event implementations for the GhostEventScheduler.
//!
//! Each event type is a struct implementing the `GhostEvent` trait
//! defined in `super::ghost_events`. New event types are added here
//! without modifying the renderer or event manager.

pub(crate) mod ghost;

pub(crate) use ghost::GhostEvent;
