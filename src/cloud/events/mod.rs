// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Atmospheric event implementations for the Atmospheric Event Engine.
//!
//! Each event type is a struct implementing the `AtmosphericEvent` trait
//! defined in `super::atmospheric_events`. New event types are added here
//! without modifying the renderer or event manager.

pub(crate) mod ghost;
pub(crate) mod helpers;

pub(crate) use ghost::GhostEvent;
