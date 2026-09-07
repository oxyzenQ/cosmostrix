// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Black hole rain style (NIGHT-special-1): the ball module, the
//! stage-2 orbital ring physics module, the stage-2.1 ball rendering
//! helpers, the stage-2.2 formation phase math, and the stage-2.6
//! halo stream physics (the split mirrors the monolith/dragon family
//! helper pattern).

pub(crate) mod ball_helpers;
pub(crate) mod black_hole;
pub(crate) mod formation;
pub(crate) mod halo;
pub(crate) mod ring;

pub(crate) use black_hole::BlackHoleRain;
pub(crate) use ring::{BlackHoleRandom, BlackHoleSpawnParams, BlackHoleStep};
