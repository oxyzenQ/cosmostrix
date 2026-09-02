// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Rendering-level tests for the v80.0.0-beta.1 msg-fill-style feature.
//!
//! Each test drives `Cloud::draw_message` with a synthetic
//! `message_start_time` (instant in the past → controlled elapsed ms)
//! and inspects the resulting Frame cells — visibility, glyph position
//! (slide pass), and border reveal timelines.

use std::time::{Duration, Instant};

use super::super::Cloud;
use crate::frame::Frame;
use crate::msg_fill_style::MsgFillStyle;
use crate::rain_style::RainStyle;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode};

pub(super) fn make_cloud_colored(style: MsgFillStyle) -> Cloud {
    let mut cloud = Cloud::new(
        ColorMode::TrueColor,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        RainStyle::Glyph,
    );
    cloud.init_chars(vec!['0', '1']);
    cloud.reset(30, 12);
    cloud.set_msg_fill_style(style);
    cloud
}

/// Set the message and backdate the reveal timeline so `elapsed_ms`
/// has already passed. Shared with `tests_msg_fill_hologram.rs`.
pub(super) fn set_message_elapsed(cloud: &mut Cloud, text: &str, elapsed_ms: u64) {
    cloud.set_message_border(true);
    cloud.set_message(text);
    // set_message starts the timeline at now (the intro lead, when one
    // plays, is armed separately by event_loop_intro). Rewind it:
    // start = now - elapsed.
    cloud.message_start_time = Some(Instant::now() - Duration::from_millis(elapsed_ms));
}

/// Count visible content cells (non-space, non-border glyphs) in the
/// frame region covered by the message overlay. Shared with
/// `tests_msg_fill_hologram.rs`.
pub(super) fn visible_content_cells(frame: &Frame, cloud: &Cloud) -> Vec<(u16, u16, char)> {
    let mut cells = Vec::new();
    for mc in &cloud.message {
        // v80.0.0-alpha.1 (S-master-HUNT-3): positional classification (layout border or
        // space = not content; user text is always content).
        if mc.is_border || mc.val == ' ' {
            continue;
        }
        if let Some(cell) = frame.get(mc.col, mc.line) {
            if cell.ch == mc.val {
                cells.push((mc.col, mc.line, mc.val));
            }
        }
    }
    cells
}

/// Count visible border glyphs (box-drawing chars) in the frame.
fn visible_border_cells(frame: &Frame, cloud: &Cloud) -> usize {
    cloud
        .message
        .iter()
        .filter(|mc| mc.is_border)
        .filter(|mc| {
            frame
                .get(mc.col, mc.line)
                .is_some_and(|cell| cell.ch == mc.val)
        })
        .count()
}

fn total_border_cells(cloud: &Cloud) -> usize {
    cloud.message.iter().filter(|mc| mc.is_border).count()
}

fn total_content_cells(cloud: &Cloud) -> usize {
    cloud
        .message
        .iter()
        .filter(|mc| !mc.is_border && mc.val != ' ')
        .count()
}

#[test]
fn typewriter_reveals_progressively_like_pre_v51() {
    // "hello world" = 10 content chars. At 160 ms elapsed:
    // reveal_count = 160/80 = 2 (max(1) floor, min(total)).
    let mut cloud = make_cloud_colored(MsgFillStyle::Typewriter);
    set_message_elapsed(&mut cloud, "hello world", 160);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());

    let visible = visible_content_cells(&frame, &cloud);
    assert_eq!(
        visible.len(),
        2,
        "typewriter at 160ms must show exactly 2 chars (80ms/char), got {}",
        visible.len()
    );
    // First two chars must be h, e.
    let chars: String = visible.iter().map(|(_, _, c)| *c).collect();
    assert!(
        chars.starts_with("he"),
        "first revealed chars must be 'he', got '{chars}'"
    );
}

#[test]
fn instant_style_shows_all_text_immediately() {
    let mut cloud = make_cloud_colored(MsgFillStyle::Instant);
    set_message_elapsed(&mut cloud, "hello world", 0);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());

    let visible = visible_content_cells(&frame, &cloud);
    assert_eq!(
        visible.len(),
        total_content_cells(&cloud),
        "instant style must show ALL content chars at t=0"
    );
}

#[test]
fn instant_style_border_draws_over_one_second() {
    let mut cloud = make_cloud_colored(MsgFillStyle::Instant);
    set_message_elapsed(&mut cloud, "hello world", 0);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert_eq!(
        visible_border_cells(&frame, &cloud),
        0,
        "instant border must be invisible at t=0 (1s clockwise draw)"
    );

    let mut cloud = make_cloud_colored(MsgFillStyle::Instant);
    set_message_elapsed(&mut cloud, "hello world", 600);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let at_600 = visible_border_cells(&frame, &cloud);
    let total = total_border_cells(&cloud);
    assert!(
        at_600 > 0 && at_600 < total,
        "instant border must be partially drawn at t=600ms ({at_600}/{total})"
    );

    let mut cloud = make_cloud_colored(MsgFillStyle::Instant);
    set_message_elapsed(&mut cloud, "hello world", 1200);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert_eq!(
        visible_border_cells(&frame, &cloud),
        total_border_cells(&cloud),
        "instant border must be fully drawn at t=1200ms"
    );
}

#[test]
fn fade_style_reveals_all_text_but_border_ramps_with_alpha() {
    // Text: all visible from t=1ms (alpha > 0).
    let mut cloud = make_cloud_colored(MsgFillStyle::Fade);
    set_message_elapsed(&mut cloud, "hello world", 400);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert_eq!(
        visible_content_cells(&frame, &cloud).len(),
        total_content_cells(&cloud),
        "fade style must show all text once alpha > 0"
    );
    // Border at alpha=0.5: roughly half the border cells visible.
    let at_400 = visible_border_cells(&frame, &cloud);
    let total = total_border_cells(&cloud);
    assert!(
        at_400 > 0 && at_400 < total,
        "fade border must be partially revealed at t=400ms ({at_400}/{total})"
    );

    // At t=0 (alpha 0): nothing at all.
    let mut cloud = make_cloud_colored(MsgFillStyle::Fade);
    set_message_elapsed(&mut cloud, "hello world", 0);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert_eq!(
        visible_content_cells(&frame, &cloud).len(),
        0,
        "fade style must hide everything at alpha=0"
    );
    assert_eq!(visible_border_cells(&frame, &cloud), 0);
}

#[test]
fn words_style_reveals_word_by_word() {
    // "hello world": word 1 (hello) at t=0, word 2 (world) at t=200.
    let mut cloud = make_cloud_colored(MsgFillStyle::Words);
    set_message_elapsed(&mut cloud, "hello world", 100);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());

    let visible = visible_content_cells(&frame, &cloud);
    let chars: String = visible.iter().map(|(_, _, c)| *c).collect();
    assert_eq!(
        chars, "hello",
        "words style at t=100ms must show only the first word, got '{chars}'"
    );

    let mut cloud = make_cloud_colored(MsgFillStyle::Words);
    set_message_elapsed(&mut cloud, "hello world", 250);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let visible = visible_content_cells(&frame, &cloud);
    assert_eq!(
        visible.len(),
        total_content_cells(&cloud),
        "words style at t=250ms must show both words"
    );
}

#[test]
fn words_ordinals_built_by_reset_message() {
    let mut cloud = make_cloud_colored(MsgFillStyle::Words);
    set_message_elapsed(&mut cloud, "alpha beta gamma", 0);
    assert_eq!(
        cloud.message_word_ordinals.len(),
        cloud.message.len(),
        "ordinals must be parallel to the message cell vector"
    );
    let max_ord = cloud
        .message_word_ordinals
        .iter()
        .copied()
        .max()
        .unwrap_or(0);
    assert_eq!(max_ord, 3, "three words must yield ordinals 1..=3");
}

#[test]
fn slide_style_defers_phase1_glyphs_one_row_below() {
    // Cell 0 reveals at t=0 with SLIDE_TRAVEL_MS=240: at t=60 the glyph
    // (progress 0.25 < 0.5) must be drawn ONE ROW BELOW its final cell,
    // and the final cell itself must still be blank.
    let mut cloud = make_cloud_colored(MsgFillStyle::Slide);
    set_message_elapsed(&mut cloud, "hello", 60);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());

    let first = cloud
        .message
        .iter()
        .find(|mc| mc.val == 'h')
        .expect("'h' cell must exist");
    let at_final = frame
        .get(first.col, first.line)
        .expect("final cell readable");
    assert_eq!(
        at_final.ch, ' ',
        "phase-1 slide glyph must NOT be at its final position yet"
    );
    let below = frame
        .get(first.col, first.line + 1)
        .expect("cell below readable");
    assert_eq!(
        below.ch, 'h',
        "phase-1 slide glyph must be drawn one row below its final position"
    );

    // At t=300 (> SLIDE_TRAVEL_MS for cell 0) the glyph has landed.
    let mut cloud = make_cloud_colored(MsgFillStyle::Slide);
    set_message_elapsed(&mut cloud, "hello", 300);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let first = cloud
        .message
        .iter()
        .find(|mc| mc.val == 'h')
        .expect("'h' cell must exist");
    let at_final = frame
        .get(first.col, first.line)
        .expect("final cell readable");
    assert_eq!(
        at_final.ch, 'h',
        "settled slide glyph must be at its final position"
    );
}

#[test]
fn engrave_reveals_progressively_like_typewriter_pacing() {
    // Effects OFF isolates the reveal pacing from the spark sidecar
    // (sparks overwrite the head cell glyph — see the dedicated spark
    // tests below). "hello world" = 10 content chars; 160 ms → 2.
    let mut cloud = make_cloud_colored(MsgFillStyle::Engrave);
    cloud.set_effects_enabled(false);
    set_message_elapsed(&mut cloud, "hello world", 160);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());

    let visible = visible_content_cells(&frame, &cloud);
    assert_eq!(
        visible.len(),
        2,
        "engrave at 160ms must show exactly 2 chars (80ms/char), got {}",
        visible.len()
    );
    let chars: String = visible.iter().map(|(_, _, c)| *c).collect();
    assert!(
        chars.starts_with("he"),
        "first revealed chars must be 'he', got '{chars}'"
    );
}

#[test]
fn engrave_head_burns_hot_and_cools_off() {
    // Dim custom palette so the 2x boost does not clamp at 255. "hello
    // world": at elapsed = 900 ms the last char 'd' (idx 9, revealed
    // at 720 ms, age 180 ms) is still cooling → brighter than the
    // settled 'h' (age 900 ms).
    let mut cloud = make_cloud_colored(MsgFillStyle::Engrave);
    cloud.set_effects_enabled(false);
    cloud.set_palette(
        None,
        crate::palette::Palette {
            colors: vec![
                crossterm::style::Color::Rgb {
                    r: 60,
                    g: 60,
                    b: 60,
                },
                crossterm::style::Color::Rgb {
                    r: 100,
                    g: 100,
                    b: 100,
                },
            ],
            bg: None,
        },
    );
    set_message_elapsed(&mut cloud, "hello world", 900);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());

    let sum_of = |target: char| -> Option<u32> {
        let mc = cloud.message.iter().find(|mc| mc.val == target)?;
        let cell = frame.get(mc.col, mc.line)?;
        let fg = cell.fg?;
        let (r, g, b) = crate::palette::decode_color(fg)?;
        Some(u32::from(r) + u32::from(g) + u32::from(b))
    };
    // 'h' settled at 100*3 = 300; 'd' cooling at (1 + 1.0*120/300)*300 = 420.
    let settled = sum_of('h').expect("first char must be visible with an fg");
    let cooling = sum_of('d').expect("last char must be visible with an fg");
    assert_eq!(settled, 300, "cooled char must render at base brightness");
    assert_eq!(
        cooling, 420,
        "cooling char must render at the exact heat-decay brightness"
    );
}

/// Count spark glyphs ('·' / '*') anywhere in the frame. The overlay
/// itself never emits these glyphs (halo '·' requires an active border
/// pulse, which these clouds do not have), so any hit is an engrave
/// spark.
fn spark_glyphs(frame: &Frame) -> Vec<(u16, u16, char)> {
    let mut out = Vec::new();
    for y in 0..12u16 {
        for x in 0..30u16 {
            if let Some(cell) = frame.get(x, y) {
                if cell.ch == '·' || cell.ch == '*' {
                    out.push((x, y, cell.ch));
                }
            }
        }
    }
    out
}

#[test]
fn engrave_fires_one_spark_burst_per_newly_revealed_char() {
    let mut cloud = make_cloud_colored(MsgFillStyle::Engrave);
    set_message_elapsed(&mut cloud, "hello world", 10);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    // First draw: head at char 0 → exactly one burst of 3 sparks.
    assert_eq!(
        cloud.engrave.active_count, 3,
        "first draw must fire exactly ENGRAVE_SPARKS_PER_HEAD sparks"
    );
    assert!(!spark_glyphs(&frame).is_empty(), "sparks must render");

    // Head still on char 0 (elapsed within the same 80 ms bucket):
    // no movement → no second burst.
    cloud.message_start_time = Some(Instant::now() - Duration::from_millis(20));
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert_eq!(
        cloud.engrave.active_count, 3,
        "a stationary head must not spawn additional bursts"
    );

    // Head advanced to char 1 (elapsed ≥ 160 ms: floor(160/80) = 2
    // revealed cells): exactly one new burst.
    cloud.message_start_time = Some(Instant::now() - Duration::from_millis(200));
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert_eq!(
        cloud.engrave.active_count, 6,
        "a moved head must fire exactly one additional burst"
    );
    assert!(cloud.engrave.active_count <= crate::msg_fill_style::engrave::ENGRAVE_SPARK_POOL_SIZE);
}

#[test]
fn engrave_sparks_expire_and_stop_when_reveal_completes() {
    let mut cloud = make_cloud_colored(MsgFillStyle::Engrave);
    // Elapsed far past the reveal: head parks on the last char and
    // fires its (single) burst on the first draw, then never again.
    set_message_elapsed(&mut cloud, "hi", 10_000);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert_eq!(cloud.engrave.active_count, 3);

    // Same head again → no new burst.
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert_eq!(cloud.engrave.active_count, 3);

    // 250 ms later (past ENGRAVE_SPARK_LIFETIME_SECS = 0.20 s): the
    // burst expires AND the parked head spawns no replacement.
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now() + Duration::from_millis(250));
    assert_eq!(
        cloud.engrave.active_count, 0,
        "expired sparks must deactivate with no respawn from a parked head"
    );
    assert!(
        spark_glyphs(&frame).is_empty(),
        "no spark glyphs may remain"
    );
}

#[test]
fn engrave_sparks_respect_no_effects() {
    // PERF-4: --no-effects must suppress the spark sidecar exactly
    // like every other particle subsystem.
    let mut cloud = make_cloud_colored(MsgFillStyle::Engrave);
    cloud.set_effects_enabled(false);
    set_message_elapsed(&mut cloud, "hello world", 10);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert_eq!(
        cloud.engrave.active_count, 0,
        "--no-effects must spawn nothing"
    );
    assert!(spark_glyphs(&frame).is_empty());
    // The reveal itself is NOT an effect — text still burns in.
    assert_eq!(
        visible_content_cells(&frame, &cloud).len(),
        1,
        "head char must still be revealed under --no-effects"
    );
}

#[test]
fn engrave_reveal_restarts_fresh_burst_after_typewriter_restart() {
    // r-restart (restart_message_typewriter) rewinds the timeline;
    // the movement detector must re-arm so the fresh reveal's first
    // char fires its burst again.
    let mut cloud = make_cloud_colored(MsgFillStyle::Engrave);
    set_message_elapsed(&mut cloud, "hi", 10_000);
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert_eq!(cloud.engrave.active_count, 3);

    cloud.restart_message_typewriter();
    // The restart sets start = now (immediate — Space replays carry no
    // intro lead): elapsed ~ 0 → head at char 0 → fresh burst on the
    // next draw.
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert_eq!(
        cloud.engrave.active_count, 3,
        "restarted reveal must re-fire the first-char burst"
    );
}

#[test]
fn set_msg_fill_style_restarts_reveal_on_change() {
    let mut cloud = make_cloud_colored(MsgFillStyle::Typewriter);
    set_message_elapsed(&mut cloud, "hello world", 10_000);
    // Sanity: fully revealed under typewriter.
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    assert_eq!(
        visible_content_cells(&frame, &cloud).len(),
        total_content_cells(&cloud)
    );

    // Switch style mid-session: reveal must restart (elapsed ~ 0 under
    // the new style — first cell visible via the .max(1) floor, later
    // cells hidden).
    cloud.set_msg_fill_style(MsgFillStyle::Fade);
    assert!(
        cloud.message_start_time.is_some(),
        "style change must restart the reveal timeline"
    );
    let elapsed = cloud
        .message_start_time
        .unwrap()
        .elapsed()
        .as_millis()
        .min(50) as usize; // generous bound: restart happened "now"
    let mut frame = Frame::new(30, 12, cloud.palette.bg);
    cloud.draw_message(&mut frame, Instant::now());
    let alpha = (elapsed as f32 / 800.0).min(1.0);
    if alpha <= 0.0 {
        assert_eq!(visible_content_cells(&frame, &cloud).len(), 0);
    }
    // Same-style set must NOT restart (no-op guard).
    let before = cloud.message_start_time;
    cloud.set_msg_fill_style(MsgFillStyle::Fade);
    assert_eq!(
        cloud.message_start_time, before,
        "setting the SAME style must not restart the timeline"
    );
}

#[test]
fn default_style_is_engrave_champion_contract() {
    // v80.0.0-beta.2: the default msg-fill-style is now Engrave (owner
    // champion winner). The pre-beta.2 default was Typewriter for LTS
    // bit-identical parity. A fresh Cloud (no explicit style) must render
    // exactly like an Engrave-configured cloud — the champion contract.
    let mut plain = Cloud::new(
        ColorMode::TrueColor,
        ShadingMode::Random,
        BoldMode::Off,
        false,
        true,
        ColorScheme::Green,
        RainStyle::Glyph,
    );
    plain.init_chars(vec!['0', '1']);
    plain.reset(30, 12);
    assert_eq!(plain.msg_fill_style, MsgFillStyle::Engrave);

    let mut engraved = make_cloud_colored(MsgFillStyle::Engrave);
    for cloud in [&mut plain, &mut engraved] {
        set_message_elapsed(cloud, "hello world", 320);
    }
    let mut f1 = Frame::new(30, 12, plain.palette.bg);
    plain.draw_message(&mut f1, Instant::now());
    let mut f2 = Frame::new(30, 12, engraved.palette.bg);
    engraved.draw_message(&mut f2, Instant::now());

    let v1 = visible_content_cells(&f1, &plain);
    let v2 = visible_content_cells(&f2, &engraved);
    assert_eq!(
        v1, v2,
        "default cloud and explicit engrave cloud must reveal identically"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// v52 message-intro lead (owner bug report: premature 6 s delay)
// ─────────────────────────────────────────────────────────────────────────────

/// `set_message` starts the reveal timeline immediately — the intro lead
/// is armed by the intro runner, never here. Pre-v52 this armed
/// `now + 6 s` unconditionally, which dead-aired every `--intro none`
/// start (owner bug report).
#[test]
fn set_message_starts_timeline_immediately() {
    let mut cloud = make_cloud_colored(MsgFillStyle::Typewriter);
    let before = Instant::now();
    cloud.set_message("hello");
    let after = Instant::now();
    let start = cloud.message_start_time.expect("timeline must be armed");
    // The start must fall inside the set_message call window — i.e. NOW,
    // not now + 6 s and not in the past.
    assert!(
        start >= before && start <= after,
        "set_message must arm the reveal at now (start={start:?}, window=[{before:?},{after:?}])"
    );
}

/// The intro runner arms the full 6 s lead only through
/// `hold_message_behind_intro` — and only when a message exists.
#[test]
fn hold_message_behind_intro_arms_full_lead() {
    let mut cloud = make_cloud_colored(MsgFillStyle::Typewriter);
    cloud.set_message("hello");
    let before = Instant::now();
    cloud.hold_message_behind_intro();
    let after = Instant::now();
    let start = cloud.message_start_time.unwrap();
    let lead = crate::cloud::MESSAGE_INTRO_LEAD;
    assert_eq!(
        lead,
        Duration::from_secs(6),
        "lead constant is the tuned 6 s"
    );
    assert!(
        start >= before + lead && start <= after + lead,
        "hold must arm now + MESSAGE_INTRO_LEAD (start={start:?})"
    );

    // No message → no-op (start stays unset).
    let mut bare = make_cloud_colored(MsgFillStyle::Typewriter);
    bare.hold_message_behind_intro();
    assert!(bare.message_start_time.is_none());
}

/// `cut_message_intro_lead` pulls a still-future start to now and leaves
/// an already-started timeline untouched.
#[test]
fn cut_message_intro_lead_clamps_only_future_starts() {
    let mut cloud = make_cloud_colored(MsgFillStyle::Typewriter);
    cloud.set_message("hello");

    // Future start (intro skipped at t~0): clamp to now.
    cloud.hold_message_behind_intro();
    assert!(cloud.message_start_time.unwrap() > Instant::now());
    cloud.cut_message_intro_lead();
    assert!(
        cloud.message_start_time.unwrap() <= Instant::now(),
        "a future start must be pulled to now"
    );

    // Past start (lead already expired / fully played intro): untouched.
    let past = Instant::now() - Duration::from_secs(30);
    cloud.message_start_time = Some(past);
    cloud.cut_message_intro_lead();
    assert_eq!(
        cloud.message_start_time,
        Some(past),
        "an already-started timeline must not move"
    );

    // No timeline at all: no-op, no panic.
    let mut bare = make_cloud_colored(MsgFillStyle::Typewriter);
    bare.cut_message_intro_lead();
    assert!(bare.message_start_time.is_none());
}

/// r restart (`restart_message_typewriter`) replays the reveal
/// immediately — pre-v52 it re-armed `now + 6 s`, dead air after reset
/// (owner bug report).
#[test]
fn restart_message_typewriter_is_immediate() {
    let mut cloud = make_cloud_colored(MsgFillStyle::Typewriter);
    cloud.set_message("hello");
    let before = Instant::now();
    cloud.restart_message_typewriter();
    let after = Instant::now();
    let start = cloud.message_start_time.unwrap();
    assert!(
        start >= before && start <= after,
        "r restart must re-arm at now, not now + 6 s (start={start:?})"
    );

    // Without a message it is a no-op.
    let mut bare = make_cloud_colored(MsgFillStyle::Typewriter);
    bare.restart_message_typewriter();
    assert!(bare.message_start_time.is_none());
}
