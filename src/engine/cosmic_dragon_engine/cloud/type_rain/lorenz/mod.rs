// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Lorenz strange-attractor rain style (single-file module).

pub(crate) mod lorenz;

pub(crate) use lorenz::{LorenzRain, LorenzRandom, LorenzSpawnParams, LorenzStep};
