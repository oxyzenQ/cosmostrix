// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! Lorenz strange-attractor rain style (single-file module).

pub(crate) mod lorenz;

pub(crate) use lorenz::{LorenzRain, LorenzRandom, LorenzSpawnParams, LorenzStep};
