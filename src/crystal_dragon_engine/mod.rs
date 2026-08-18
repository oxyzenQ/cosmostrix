// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Crystal Dragon Engine — ambient intelligence for palette drift and
//! time-of-day scene scheduling.
//!
//! ## Subsystems
//!
//! ### 1. Palette drift (CPU/CLOCK → theme)
//!
//! Maps system state (CPU usage or wall-clock time) to a **point** (1–99),
//! classifies the point into a **temperature group** (Cold / Medium / Hot),
//! and selects a color theme from that group via probabilistic weighted
//! selection (calc-v1). The selected theme is handed to the Chroma Dragon
//! engine for a smooth 300 ms OKLab wave transition.
//!
//! ### 2. Ambient scheduler (time-of-day → scene)
//!
//! Time-of-day scene switches via `ambient.HH-MM = <scene>` in config.toml.
//! Fires at scheduled times, applies scene+palette, locks Crystal Dragon
//! drift while active. User overrides (`c`/`x` keys) clear the lock;
//! auto-snapback restores it after idle.
//!
//! ## Architecture
//!
//! | File | Role |
//! |------|------|
//! | `crystal_dragon_control` | Config struct + defaults (polling interval, sensor mode, calc method) |
//! | `sensor` | CPU sampling (sysinfo/procfs) + CLOCK fallback (UTC time-based) |
//! | `palette_groups` | 44 themes partitioned into Cold(14) / Medium(14) / Hot(14) + Reserved(2) |
//! | `point_system` | calc-v1: probabilistic weighted theme selection within a group |
//! | `transition` | Hook into Chroma Dragon `set_color_scheme` for OKLab smooth fades |
//! | `ambient` | Time-of-day schedule types, parsing, validation, startup apply |
//! | `ambient_scheduler` | Background thread that fires schedule entries |
//! | `ambient_diag` | Diagnostics counters (exit summary) |
//!
//! ## Owner decisions
//!
//! - **Option A (Silent-Elegant)**: no HUD indicator, no verbose logging of drift events
//! - **calc-v1**: probabilistic weighted selection (not pattern-based calc-v2)
//! - **Polling 60 s**: sensor sampling every 60 seconds
//! - **CPU primary**, CLOCK fallback when CPU sampling is unsupported
//!
//! ## Point system
//!
//! ```text
//! Points 1–33  → Cold group  (cool, calm, serene)
//! Points 34–66 → Medium group (balanced, green/purple)
//! Points 67–99 → Hot group   (warm, fiery, energetic)
//! ```
//!
//! CPU usage maps linearly to the 1–99 point range:
//! ```text
//! point = clamp(1, 99, round(cpu_percent * 0.99))
//! ```
//!
//! When CPU is unavailable (CLOCK fallback), the point is derived from
//! UTC hour + minute so the terminal still drifts through the day:
//! ```text
//! point = clamp(1, 99, round((hour * 4.125 + minute * 0.06875)))
//! ```
//! This produces low points in early morning, high points in the afternoon.

pub(crate) mod ambient;
pub(crate) mod ambient_diag;
pub(crate) mod ambient_scheduler;
pub(crate) mod crystal_dragon_control;
pub(crate) mod palette_groups;
pub(crate) mod point_system;
pub(crate) mod sensor;
pub(crate) mod transition;

pub(crate) use crystal_dragon_control::CrystalDragonControl;
pub(crate) use sensor::CrystalDragonSensor;
