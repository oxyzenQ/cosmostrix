// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-hunter-26 resize tests: a terminal resize must NOT restart
//! the message-fill-style reveal (owner report 2026-09-10: "the mfs
//! style like want reload but just half little" on every style while
//! msg mode is active).
//!
//! Contracts:
//! 1. The resize path (`cloud.reset` -> `reset_with_bounds` ->
//!    `relayout_message`) keeps the reveal timeline, the in-flight
//!    engrave sparks / scorch smoke, and the touch pulses running.
//! 2. The movement detectors fire on FORWARD head movement only —
//!    no spurious burst at the current head after a resize remap.
//! 3. The true reveal-restart paths (`set_message`,
//!    `set_msg_fill_style`, `restart_message_typewriter` — the 'r'
//!    relaunch, owner-excluded from hunter-26) still drop the
//!    sidecars and re-arm the detectors.

use std::time::{Duration, Instant};

use super::tests_msg_fill_style::{make_cloud_colored, set_message_elapsed};
use super::Cloud;
use crate::frame::Frame;
use crate::msg_fill_style::engrave::ENGRAVE_SPARKS_PER_HEAD;
use crate::msg_fill_style::MsgFillStyle;

/// One overlay draw at a fixed wall-clock `t` (dt = 0 between calls
/// keeps spark physics frozen — no natural decay mid-test).
fn draw_at(cloud: &mut Cloud, t: Instant) -> Frame {
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, t);
    frame
}

#[test]
fn resize_keeps_engrave_sparks_and_detector_state() {
    let mut cloud = make_cloud_colored(MsgFillStyle::Engrave);
    // Head on char 2 (elapsed 240 ms -> 3 revealed cells): one burst
    // fired at the current head on the first draw.
    set_message_elapsed(&mut cloud, "hello world", 240);
    let t = Instant::now();
    draw_at(&mut cloud, t);
    assert_eq!(cloud.engrave.active_count, ENGRAVE_SPARKS_PER_HEAD);
    let last_head_before = cloud.engrave.last_head;

    // THE OWNER REPRO: resize mid-reveal. Pre-hunter-26 this wiped
    // every spark and re-armed the detector (last_head = MAX), so the
    // next frame fired a spurious burst at the unchanged head — the
    // visible "half little reload".
    cloud.reset(24, 8);

    assert_eq!(
        cloud.engrave.active_count, ENGRAVE_SPARKS_PER_HEAD,
        "resize must keep in-flight sparks flying"
    );
    assert_eq!(
        cloud.engrave.last_head, last_head_before,
        "resize must not re-arm the movement detector"
    );

    // Same elapsed bucket -> same head -> no new burst, sparks still
    // alive (frozen dt).
    draw_at(&mut cloud, t);
    assert_eq!(
        cloud.engrave.active_count, ENGRAVE_SPARKS_PER_HEAD,
        "an unchanged head must not fire a burst after resize"
    );
}

#[test]
fn resize_keeps_scorch_smoke_and_detector_state() {
    let mut cloud = make_cloud_colored(MsgFillStyle::Scorch);
    set_message_elapsed(&mut cloud, "hello world", 240);
    let t = Instant::now();
    draw_at(&mut cloud, t);
    let active_before = cloud.scorch.active_count;
    let last_head_before = cloud.scorch.last_head;
    assert!(active_before > 0, "scorch must have puffs in flight");

    cloud.reset(24, 8);

    assert_eq!(
        cloud.scorch.active_count, active_before,
        "resize must keep in-flight smoke drifting"
    );
    assert_eq!(
        cloud.scorch.last_head, last_head_before,
        "resize must not re-arm the scorch detector"
    );

    draw_at(&mut cloud, t);
    assert_eq!(
        cloud.scorch.active_count, active_before,
        "an unchanged head must not fire a puff after resize"
    );
}

#[test]
fn resize_keeps_the_reveal_timeline_running() {
    let mut cloud = make_cloud_colored(MsgFillStyle::Engrave);
    set_message_elapsed(&mut cloud, "hello world", 240);
    let start = cloud.message_start_time;

    cloud.reset(24, 8);

    assert_eq!(
        cloud.message_start_time, start,
        "resize must never touch the reveal timeline anchor"
    );
    assert!(
        !cloud.message.is_empty(),
        "the overlay must re-center its layout for the new dims"
    );
}

#[test]
fn backward_head_jump_after_narrow_resize_does_not_burst() {
    let mut cloud = make_cloud_colored(MsgFillStyle::Engrave);
    // Wide layout: full text, head parked on the last char.
    set_message_elapsed(&mut cloud, "hello world", 10_000);
    let t = Instant::now();
    draw_at(&mut cloud, t);
    let last_head = cloud.engrave.last_head;
    assert!(last_head != usize::MAX);

    // Height-truncating resize: max_content_h drops to 1 line, so the
    // wrapped second line is cut and total content shrinks — the head
    // index JUMPS BACKWARD (reveal_count clamps to the smaller
    // total). A backward jump is not a newly engraved char — no
    // burst (the other half of the owner's half-reload).
    cloud.reset(16, 5);
    draw_at(&mut cloud, t);

    // The head after the narrow layout is strictly smaller; the
    // detector must not have consumed it as movement.
    let head_now = cloud
        .message
        .iter()
        .filter(|mc| !mc.is_border && mc.val != ' ')
        .count()
        .saturating_sub(1);
    assert!(
        head_now < last_head,
        "narrow layout must shrink the head index"
    );
    assert_eq!(
        cloud.engrave.last_head, last_head,
        "a backward head jump must not re-arm or consume the detector"
    );
    assert_eq!(
        cloud.engrave.active_count, ENGRAVE_SPARKS_PER_HEAD,
        "no spurious burst on the backward jump"
    );
}

#[test]
fn resize_keeps_border_pulses_alive_with_bounds_safety() {
    let mut cloud = make_cloud_colored(MsgFillStyle::Engrave);
    set_message_elapsed(&mut cloud, "hi", 240);
    let t = Instant::now();
    draw_at(&mut cloud, t);

    // Simulate an in-flight touch pulse (border_touch pushes these on
    // droplet contact; constructing one directly keeps the test
    // independent of the spawn path).
    cloud.border_pulses.push(crate::cloud::BorderPulse {
        msg_idx: 0,
        col: 3,
        head_rgb: (255, 255, 255),
        birth: t,
    });

    cloud.reset(24, 8);

    assert_eq!(
        cloud.border_pulses.len(),
        1,
        "resize must not wipe in-flight touch pulses"
    );
    // Draw on the rebuilt grid: the stale msg_idx must be
    // bounds-checked (not panic), and the fresh pulse stays alive.
    draw_at(&mut cloud, t);
    assert_eq!(
        cloud.border_pulses.len(),
        1,
        "a live pulse must survive the draw pass on the rebuilt grid"
    );
}

#[test]
fn border_toggle_keeps_sidecars_and_timeline() {
    let mut cloud = make_cloud_colored(MsgFillStyle::Engrave);
    set_message_elapsed(&mut cloud, "hello world", 240);
    let t = Instant::now();
    draw_at(&mut cloud, t);
    let start = cloud.message_start_time;
    assert_eq!(cloud.engrave.active_count, ENGRAVE_SPARKS_PER_HEAD);

    // The border toggle is a layout change (the overlay gains/loses a
    // ring), not a new message: the reveal, the sparks, and the
    // timeline all continue.
    cloud.set_message_border(false);

    assert_eq!(
        cloud.message_start_time, start,
        "border toggle must not restart the reveal timeline"
    );
    assert_eq!(
        cloud.engrave.active_count, ENGRAVE_SPARKS_PER_HEAD,
        "border toggle must keep in-flight sparks"
    );
    draw_at(&mut cloud, t);
    assert_eq!(
        cloud.engrave.active_count, ENGRAVE_SPARKS_PER_HEAD,
        "no spurious burst after the border toggle"
    );
}

#[test]
fn set_message_still_fully_restarts_the_reveal() {
    let mut cloud = make_cloud_colored(MsgFillStyle::Engrave);
    set_message_elapsed(&mut cloud, "hello world", 240);
    let t = Instant::now();
    draw_at(&mut cloud, t);
    assert_eq!(cloud.engrave.active_count, ENGRAVE_SPARKS_PER_HEAD);

    // A NEW message is a fresh reveal: sidecars drop, detector
    // re-arms, the timeline restarts (unchanged pre-hunter-26
    // contract — hunter-26 only changed the LAYOUT paths).
    let start_before = cloud.message_start_time;
    cloud.set_message("brand new message");
    std::thread::sleep(Duration::from_millis(2));

    assert_eq!(cloud.engrave.active_count, 0, "new message drops sparks");
    assert_eq!(cloud.engrave.last_head, usize::MAX, "detector re-arms");
    assert!(cloud.message_start_time > start_before);
    assert!(cloud.border_pulses.is_empty(), "pulses drop");
}

#[test]
fn style_change_still_fully_restarts_the_reveal() {
    let mut cloud = make_cloud_colored(MsgFillStyle::Engrave);
    set_message_elapsed(&mut cloud, "hello world", 240);
    let t = Instant::now();
    draw_at(&mut cloud, t);
    assert_eq!(cloud.engrave.active_count, ENGRAVE_SPARKS_PER_HEAD);

    cloud.set_msg_fill_style(MsgFillStyle::Typewriter);

    assert_eq!(cloud.engrave.active_count, 0, "style change drops sparks");
    assert_eq!(cloud.engrave.last_head, usize::MAX, "detector re-arms");
    assert!(cloud.border_pulses.is_empty(), "pulses drop");
}

#[test]
fn r_restart_still_fully_restarts_the_reveal() {
    // The 'r' shortkey is owner-EXCLUDED from hunter-26: a restart is
    // a relaunch (NIGHT-lts-3), and the message reveal replays with
    // it. The handler calls restart_message_typewriter after
    // restart_from_zero; this pins the message half of that contract.
    let mut cloud = make_cloud_colored(MsgFillStyle::Engrave);
    set_message_elapsed(&mut cloud, "hello world", 10_000);
    let t = Instant::now();
    draw_at(&mut cloud, t);
    assert_eq!(cloud.engrave.active_count, ENGRAVE_SPARKS_PER_HEAD);
    let start_before = cloud.message_start_time;

    cloud.restart_from_zero(30, 12);
    cloud.restart_message_typewriter();
    std::thread::sleep(Duration::from_millis(2));

    assert!(cloud.message_start_time > start_before, "timeline re-arms");
    assert_eq!(cloud.engrave.active_count, 0, "sparks drop on restart");
    assert_eq!(cloud.engrave.last_head, usize::MAX, "detector re-arms");

    // And the fresh reveal fires its first burst again.
    draw_at(&mut cloud, t);
    assert_eq!(
        cloud.engrave.active_count, ENGRAVE_SPARKS_PER_HEAD,
        "the fresh reveal must fire its first burst"
    );
}
