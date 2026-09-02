// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Final-runtime-state tracker tests (S-master-LOGIC-1, v80.0.0-beta.2).
//!
//! The post-exit "final runtime state" section must disclose EVERY
//! live-reload-able field. These tests pin the v80.0.0-beta.2 additions
//! (fps / glitch_level / bold / shading / monolith / color_bg /
//! color_tune): the set_final_state -> last_* round-trips and the
//! early-exit defaults. The printer itself writes to stderr (not
//! capturable on stable Rust); the (was X) change gating is exercised
//! end-to-end by the owner's verbose sessions.

#[path = "tests_final_state_cases.rs"]
mod cases;
