// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-improve-8: modified-click selection-bypass hardening tests.
//!
//! Owner request: disable copy/paste — text/info must not be copyable,
//! including via shift+click and any other modifier combination.
//!
//! With mouse capture active, terminals reserve modified clicks
//! (shift+click above all) for their LOCAL selection engine — most never
//! deliver them to the application. For the minority that forward the
//! events, the contract pinned here is:
//! - every modifier combination on a mouse Down is classified as a
//!   modifier click (`is_modifier_click` == true) and produces zero
//!   visual acknowledgment (no click wave) plus a selection-clearing
//!   full-frame redraw (asserted at the classification level — the
//!   event-loop branch is a one-liner over this predicate);
//! - plain unmodified clicks keep the normal hover/click-wave behavior
//!   (predicate stays false);
//! - modifier bits on non-Down mouse kinds (drag/move/up/scroll) are NOT
//!   classification hits — selection attempts are Down clicks only, and
//!   scroll/drag modifiers must not trigger spurious full redraws.

#[cfg(test)]
mod cases_night_improve8 {
    use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};

    use crate::interactive::input::is_modifier_click;

    fn mouse_event(kind: MouseEventKind, modifiers: KeyModifiers) -> MouseEvent {
        MouseEvent {
            kind,
            column: 7,
            row: 3,
            modifiers,
        }
    }

    #[test]
    fn shift_down_is_modifier_click() {
        // The owner-reported bypass: shift+click.
        let e = mouse_event(
            MouseEventKind::Down(crossterm::event::MouseButton::Left),
            KeyModifiers::SHIFT,
        );
        assert!(
            is_modifier_click(&e),
            "shift+click must classify as a modifier click (zero-ack + redraw)"
        );
    }

    #[test]
    fn every_modifier_combination_on_down_is_modifier_click() {
        // "include even shift+click and any" — every modifier bit and
        // combination on a Down event is a selection-bypass attempt.
        let combos = [
            KeyModifiers::CONTROL,
            KeyModifiers::ALT,
            KeyModifiers::SUPER,
            KeyModifiers::HYPER,
            KeyModifiers::META,
            KeyModifiers::SHIFT | KeyModifiers::CONTROL,
            KeyModifiers::SHIFT | KeyModifiers::ALT,
            KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
        ];
        for mods in combos {
            let e = mouse_event(
                MouseEventKind::Down(crossterm::event::MouseButton::Left),
                mods,
            );
            assert!(
                is_modifier_click(&e),
                "down + {mods:?} must classify as a modifier click"
            );
        }
    }

    #[test]
    fn plain_down_is_not_modifier_click() {
        // Normal interaction clicks keep the click-wave behavior.
        for button in [
            crossterm::event::MouseButton::Left,
            crossterm::event::MouseButton::Right,
            crossterm::event::MouseButton::Middle,
        ] {
            let e = mouse_event(MouseEventKind::Down(button), KeyModifiers::NONE);
            assert!(
                !is_modifier_click(&e),
                "plain unmodified down must keep the normal click path"
            );
        }
    }

    #[test]
    fn non_down_kinds_with_modifiers_are_not_modifier_clicks() {
        // Selection attempts are Down clicks only. Modifier bits on
        // drag/move/up/scroll kinds must not trigger the zero-ack +
        // selection-clearing redraw path (no spurious full redraws).
        let kinds = [
            MouseEventKind::Drag(crossterm::event::MouseButton::Left),
            MouseEventKind::Moved,
            MouseEventKind::Up(crossterm::event::MouseButton::Left),
            MouseEventKind::ScrollUp,
            MouseEventKind::ScrollDown,
        ];
        for kind in kinds {
            let e = mouse_event(kind, KeyModifiers::SHIFT);
            assert!(
                !is_modifier_click(&e),
                "non-down kind with SHIFT must not classify as a modifier click"
            );
        }
    }
}
