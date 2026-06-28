// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-or-later

//! Scene cycle tests — forward/backward cycling, roundtrips, scene order.

use super::make_monolith_cloud;
use crate::rain_style::RainStyle;

#[test]
fn scene_cycle_forward_updates_cloud_scene() {
    let mut cloud = make_monolith_cloud();
    let charset = "binary".to_string();
    let new_charset = cloud.apply_scene_runtime("matrix", &charset, &[], false);
    assert_eq!(cloud.active_scene(), "matrix");
    assert_eq!(cloud.rain_style(), RainStyle::Glyph);
    assert_eq!(new_charset, "binary"); // matrix has no charset override
}

#[test]
fn scene_cycle_to_signal_updates_cloud_scene() {
    let mut cloud = make_monolith_cloud();
    let charset = "binary".to_string();
    let new_charset = cloud.apply_scene_runtime("signal", &charset, &[], false);
    assert_eq!(cloud.active_scene(), "signal");
    assert_eq!(cloud.rain_style(), RainStyle::Glyph);
    assert_eq!(new_charset, "code"); // signal overrides charset to code
}

#[test]
fn cycle_monolith_signal_monolith_roundtrip() {
    let mut cloud = make_monolith_cloud();
    let mut charset = "binary".to_string();
    // monolith -> signal
    let c = cloud.apply_scene_runtime("signal", &charset, &[], false);
    charset = c.to_string();
    assert_eq!(cloud.rain_style(), RainStyle::Glyph);
    assert_eq!(cloud.active_scene(), "signal");
    // signal -> monolith
    let c = cloud.apply_scene_runtime("monolith", &charset, &[], false);
    charset = c.to_string();
    assert_eq!(cloud.rain_style(), RainStyle::Monolith);
    assert_eq!(cloud.active_scene(), "monolith");
    assert_eq!(charset, "binary");
}
