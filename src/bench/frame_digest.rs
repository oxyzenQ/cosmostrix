// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! Canonical frame digest — the byte-exact serializer behind the
//! deterministic replay oracle and the embedded engine oracle
//! (night-frontier-Z1).
//!
//! A rendered frame is a `width x height` grid of [`Cell`] values. To
//! pin a frame byte-for-byte across runs, its cells are serialized
//! into one canonical byte stream and folded with blake3:
//!
//! ```text
//! width:u16le height:u16le frame_no:u64le
//! then per cell (row-major, width*height cells):
//!   ch:u32le fg:tag payload bg:tag payload bold:u8
//! ```
//!
//! The color tag byte is a PINNED ordinal over the crossterm `Color`
//! enum (documented in [`push_color`]); the match is exhaustive, so a
//! future crossterm variant is a compile error here, never a silent
//! encoding drift. Every change to this layout changes every replay
//! digest and every engine-oracle vector at once — the digests are
//! version-bound, exactly like the provenance anchor.
//!
//! Honesty notes:
//! - the digest covers the CELL GRID (the render contract), not the
//!   terminal byte stream — the diff writer's RLE batching is output
//!   formatting, not content;
//! - the digest is deterministic for the same binary and the same
//!   frame state; cross-RELEASE comparisons are expected to change
//!   when the engine legitimately changes what it draws.

use blake3::Hasher;
use crossterm::style::Color;

use crate::frame::Frame;

/// Serialize one `Option<Color>` into the canonical stream.
///
/// Tag bytes are pinned ordinals over the crossterm 0.29 `Color`
/// variants, in declaration order (Reset=1 .. Grey=17), plus
/// `Rgb=18` (three payload bytes r,g,b) and `AnsiValue=19` (one
/// payload byte). `None` encodes as tag 0 with no payload. The match
/// is exhaustive over the enum, so an upstream variant addition
/// breaks this build instead of silently reshuffling tags.
fn push_color(buf: &mut Vec<u8>, color: Option<Color>) {
    match color {
        None => buf.push(0),
        Some(Color::Reset) => buf.push(1),
        Some(Color::Black) => buf.push(2),
        Some(Color::DarkGrey) => buf.push(3),
        Some(Color::Red) => buf.push(4),
        Some(Color::DarkRed) => buf.push(5),
        Some(Color::Green) => buf.push(6),
        Some(Color::DarkGreen) => buf.push(7),
        Some(Color::Yellow) => buf.push(8),
        Some(Color::DarkYellow) => buf.push(9),
        Some(Color::Blue) => buf.push(10),
        Some(Color::DarkBlue) => buf.push(11),
        Some(Color::Magenta) => buf.push(12),
        Some(Color::DarkMagenta) => buf.push(13),
        Some(Color::Cyan) => buf.push(14),
        Some(Color::DarkCyan) => buf.push(15),
        Some(Color::White) => buf.push(16),
        Some(Color::Grey) => buf.push(17),
        Some(Color::Rgb { r, g, b }) => {
            buf.push(18);
            buf.push(r);
            buf.push(g);
            buf.push(b);
        }
        Some(Color::AnsiValue(v)) => {
            buf.push(19);
            buf.push(v);
        }
    }
}

/// Canonical bytes for one frame state.
///
/// The scratch buffer is caller-owned (passed in and cleared here) so
/// a replay loop with a pinned screen size allocates exactly once and
/// every later frame reuses the capacity — the oracle stays
/// allocation-flat in steady state.
fn serialize_frame(frame: &Frame, frame_no: u64, buf: &mut Vec<u8>) {
    buf.clear();
    buf.extend_from_slice(&frame.width.to_le_bytes());
    buf.extend_from_slice(&frame.height.to_le_bytes());
    buf.extend_from_slice(&frame_no.to_le_bytes());
    for cell in &frame.cells {
        buf.extend_from_slice(&(cell.ch as u32).to_le_bytes());
        push_color(buf, cell.fg);
        push_color(buf, cell.bg);
        buf.push(u8::from(cell.bold));
    }
}

/// blake3 digest of the canonical serialization of one frame state.
pub(crate) fn frame_digest(frame: &Frame, frame_no: u64, scratch: &mut Vec<u8>) -> [u8; 32] {
    serialize_frame(frame, frame_no, scratch);
    *blake3::hash(&scratch[..]).as_bytes()
}

/// Fold a sequence of per-frame digests into one sequence digest.
///
/// The fold is a second blake3 pass over the frame-count header and
/// the concatenated 32-byte digests, so the pinned value commits to
/// the LENGTH of the sequence as well as its content — truncating or
/// reordering frames cannot reuse a shorter/longer sequence's vector.
pub(crate) fn fold_sequence(digests: &[[u8; 32]]) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(&(digests.len() as u64).to_le_bytes());
    for d in digests {
        hasher.update(d);
    }
    *hasher.finalize().as_bytes()
}

/// Lowercase hex of a 32-byte digest (the `b3:` value prefix is the
/// caller's wording choice).
pub(crate) fn hex32(digest: &[u8; 32]) -> String {
    let mut hex = String::with_capacity(64);
    for byte in digest {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}
