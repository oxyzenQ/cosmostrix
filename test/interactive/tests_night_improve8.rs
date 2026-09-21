// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! NIGHT-improve-8: modified-mouse-event selection-bypass hardening tests.
//!
//! Owner request: disable copy/paste — text/info must not be copyable,
//! including via shift+click and any other modifier combination.
//!
//! With mouse capture active, terminals reserve modified mouse events
//! (shift+click above all) for their LOCAL selection engine — most never
//! deliver them to the application. For the minority that forward the
//! events, the contract pinned here is gesture-level, not click-level:
//! a selection is Down -> Drag* -> Up (with a modified Moved as the
//! pre-gesture hover), so EVERY modifier combination on EVERY
//! selection-motion kind must classify as a selection bypass
//! (`is_selection_bypass_event` == true) and produce zero visual
//! acknowledgment (hover glow frozen, no click wave) plus a
//! selection-clearing full-frame redraw (asserted at the classification
//! level — the event-loop branch is a one-liner over this predicate).
//! Plain unmodified events keep the normal hover/click-wave behavior
//! (predicate stays false). Modified scroll is excluded: the wheel is
//! not a selection primitive, and modifier bits on scroll kinds must
//! not trigger spurious full redraws.

#[cfg(test)]
mod cases_night_improve8 {
    use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

    use crate::interactive::input::is_selection_bypass_event;

    fn mouse_event(kind: MouseEventKind, modifiers: KeyModifiers) -> MouseEvent {
        MouseEvent {
            kind,
            column: 7,
            row: 3,
            modifiers,
        }
    }

    /// Every selection-motion kind: the anchor click, the drag phase,
    /// the release, and the pre-gesture hover. Returned as a runtime
    /// array because KeyModifiers bitflag OR-composition is not a
    /// const operation on this bitflags version.
    fn motion_kinds() -> [MouseEventKind; 4] {
        [
            MouseEventKind::Down(MouseButton::Left),
            MouseEventKind::Drag(MouseButton::Left),
            MouseEventKind::Up(MouseButton::Left),
            MouseEventKind::Moved,
        ]
    }

    fn modifier_combos() -> [KeyModifiers; 9] {
        [
            KeyModifiers::SHIFT,
            KeyModifiers::CONTROL,
            KeyModifiers::ALT,
            KeyModifiers::SUPER,
            KeyModifiers::HYPER,
            KeyModifiers::META,
            KeyModifiers::SHIFT | KeyModifiers::CONTROL,
            KeyModifiers::SHIFT | KeyModifiers::ALT,
            KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
        ]
    }

    #[test]
    fn shift_down_is_selection_bypass() {
        // The owner-reported bypass: shift+click.
        let e = mouse_event(MouseEventKind::Down(MouseButton::Left), KeyModifiers::SHIFT);
        assert!(
            is_selection_bypass_event(&e),
            "shift+click must classify as a selection bypass (zero-ack + redraw)"
        );
    }

    #[test]
    fn every_modifier_combination_on_every_motion_kind_is_selection_bypass() {
        // "include even shift+click and any" — every modifier bit and
        // combination on every selection-motion kind (Down, the anchor;
        // Drag, the extend; Up, the release; Moved, the pre-gesture
        // hover) is a selection-bypass attempt. The gesture-level
        // coverage is the follow-up fix: covering only the Down left
        // the drag phase visually acknowledged and the grid static
        // under the extending selection.
        for kind in motion_kinds() {
            for mods in modifier_combos() {
                let e = mouse_event(kind, mods);
                assert!(
                    is_selection_bypass_event(&e),
                    "{kind:?} + {mods:?} must classify as a selection bypass"
                );
            }
        }
    }

    #[test]
    fn full_shift_drag_gesture_sequence_is_selection_bypass() {
        // The real-world gesture a forwarding terminal delivers for a
        // shift+drag selection: anchor Down, repeated Drags, release Up
        // — every event of the sequence must stay on the bypass path.
        let gesture = [
            (MouseEventKind::Down(MouseButton::Left), KeyModifiers::SHIFT),
            (MouseEventKind::Drag(MouseButton::Left), KeyModifiers::SHIFT),
            (MouseEventKind::Drag(MouseButton::Left), KeyModifiers::SHIFT),
            (MouseEventKind::Drag(MouseButton::Left), KeyModifiers::SHIFT),
            (MouseEventKind::Up(MouseButton::Left), KeyModifiers::SHIFT),
        ];
        for (kind, mods) in gesture {
            let e = mouse_event(kind, mods);
            assert!(
                is_selection_bypass_event(&e),
                "every event of a shift+drag selection gesture must classify as a selection bypass"
            );
        }
    }

    #[test]
    fn plain_motion_events_are_not_selection_bypass() {
        // Normal interaction events keep the hover/click-wave behavior.
        for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] {
            for kind in [
                MouseEventKind::Down(button),
                MouseEventKind::Drag(button),
                MouseEventKind::Up(button),
            ] {
                let e = mouse_event(kind, KeyModifiers::NONE);
                assert!(
                    !is_selection_bypass_event(&e),
                    "plain unmodified {kind:?} must keep the normal path"
                );
            }
        }
        let moved = mouse_event(MouseEventKind::Moved, KeyModifiers::NONE);
        assert!(
            !is_selection_bypass_event(&moved),
            "plain unmodified Moved must keep the normal hover path"
        );
    }

    #[test]
    fn modified_scroll_kinds_are_not_selection_bypass() {
        // The wheel is not a selection primitive. Modifier bits on
        // scroll kinds must not trigger the zero-ack +
        // selection-clearing redraw path (no spurious full redraws).
        let scroll_kinds = [
            MouseEventKind::ScrollUp,
            MouseEventKind::ScrollDown,
            MouseEventKind::ScrollLeft,
            MouseEventKind::ScrollRight,
        ];
        for kind in scroll_kinds {
            for mods in [
                KeyModifiers::SHIFT,
                KeyModifiers::CONTROL,
                KeyModifiers::SHIFT | KeyModifiers::ALT,
            ] {
                let e = mouse_event(kind, mods);
                assert!(
                    !is_selection_bypass_event(&e),
                    "{kind:?} + {mods:?} must not classify as a selection bypass"
                );
            }
        }
    }
}
