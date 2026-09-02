// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Final-state tracker cases. The FINAL_* statics are process-wide
//! OnceLocks shared by every test in the binary, so the defaults and the
//! round-trip are asserted in ONE deterministic sequence (defaults first
//! — a pristine binary never ran set_final_state).

use super::super::*;

#[test]
fn final_state_defaults_then_round_trip() {
    // 1) Early-exit path (set_final_state never ran): the accessors
    //    degrade to the same values a default startup would show, so
    //    the post-exit section stays honest instead of printing
    //    arbitrary zeros.
    assert_eq!(last_fps(), 60.0);
    assert_eq!(last_glitch_level(), "Subtle");
    assert_eq!(last_bold_mode(), "Random");
    assert_eq!(last_shading_mode(), "DistanceFromHead");
    assert_eq!(last_monolith_size(), "Normal");
    assert!(!last_color_bg());
    assert_eq!(
        last_color_tune(),
        "sat=1.00 bright=1.00 head=1.00 body=1.00 tail=1.00"
    );

    // 2) Round-trip: the v80.0.0-beta.2 (S-master-LOGIC-1) fields stored
    //    by set_final_state are read back verbatim (labels are the Debug
    //    formats the printer compares for the (was X) suffixes).
    set_final_state(
        "Green",
        "cinematic",
        "zen",
        9.0,
        0.75,
        true,
        Some("msg"),
        true,
        "engrave",
        true,
        false,
        true,
        None,
        Some(30.0),
        1,
        12.0,
        "None",
        "Off",
        "Random",
        "Large",
        true,
        "sat=1.20 bright=1.00 head=1.00 body=1.00 tail=1.00",
    );
    assert_eq!(last_fps(), 12.0);
    assert_eq!(last_glitch_level(), "None");
    assert_eq!(last_bold_mode(), "Off");
    assert_eq!(last_shading_mode(), "Random");
    assert_eq!(last_monolith_size(), "Large");
    assert!(last_color_bg());
    assert_eq!(
        last_color_tune(),
        "sat=1.20 bright=1.00 head=1.00 body=1.00 tail=1.00"
    );
}
