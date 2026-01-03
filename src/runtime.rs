// Copyright (c) 2026 rezky_nightky

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    Mono,
    #[allow(dead_code)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorScheme {
    Green,
    Green2,
    Green3,
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
