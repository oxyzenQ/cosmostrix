// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    Mono,
    Color16,
    Color256,
    TrueColor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShadingMode {
    Random,
    DistanceFromHead,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoldMode {
    Off,
    Random,
    All,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonolithSize {
    #[value(name = "small")]
    Small,
    #[value(name = "normal")]
    Normal,
    #[value(name = "large")]
    Large,
}

impl MonolithSize {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Normal => "normal",
            Self::Large => "large",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ColorScheme {
    Green,
    Green2,
    Green3,
    NeonGreen,
    NeonPurple,
    NeonWhite,
    NeonBlue,
    NeonRed,
    NeonOrange,
    NeonYellow,
    NeonCyan,
    Carbon,
    Yellow,
    Orange,
    Red,
    Blue,
    Cyan,
    Gold,
    Rainbow,
    Purple,
    Neon,
    Fire,
    Ocean,
    Forest,
    Vaporwave,
    Gray,
    Snow,
    Aurora,
    FancyDiamond,
    Cosmos,
    Nebula,
    Spectrum20,
    Stars,
    Mars,
    Venus,
    Mercury,
    Jupiter,
    Saturn,
    Uranus,
    Neptune,
    Pluto,
    Moon,
    Sun,
    Comet,
    Galaxy,
    Supernova,
    BlackHole,
    Andromeda,
    Stardust,
    Meteor,
    Eclipse,
    DeepSpace,
}
