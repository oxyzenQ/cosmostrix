// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Message overlay reset — extracted from `cloud/mod.rs` to keep that
//! file under the 800-LOC hard cap (see `src/RULES_LOC.md`).
//!
//! Owns `Cloud::reset_message()` — rebuilds the message cell grid
//! (content + border) from `message_text`, computes the clockwise
//! `border_order` (BN-01/02 Dragon Hunt), and clears stale
//! `border_pulses`.
//!
//! Implemented as a separate `impl Cloud` block (Rust allows multiple
//! impl blocks across files for the same type).

use super::border;
use super::state::MsgChr;

impl super::Cloud {
    pub(crate) fn reset_message(&mut self) {
        let Some(text) = self.message_text.as_deref() else {
            return;
        };

        let pad_x: u16 = 2;
        let pad_y: u16 = 1;

        let border: u16 = if self.message_border { 1 } else { 0 };

        let min_box_w = (2u16.saturating_mul(border))
            .saturating_add(2u16.saturating_mul(pad_x))
            .max(1);
        let min_box_h = (2u16.saturating_mul(border))
            .saturating_add(2u16.saturating_mul(pad_y))
            .max(1);
        if self.cols < min_box_w || self.lines < min_box_h {
            self.message.clear();
            self.border_order.clear();
            return;
        }

        let max_content_w = self
            .cols
            .saturating_sub(2u16.saturating_mul(border))
            .saturating_sub(2u16.saturating_mul(pad_x))
            .max(1);
        let max_content_h = self
            .lines
            .saturating_sub(2u16.saturating_mul(border))
            .saturating_sub(2u16.saturating_mul(pad_y))
            .max(1);

        let mut content_lines: Vec<Vec<char>> = Vec::new();
        for raw_line in text.split('\n') {
            if content_lines.len() as u16 >= max_content_h {
                break;
            }

            let mut buf: Vec<char> = Vec::new();
            for ch in raw_line.chars() {
                if buf.len() >= max_content_w as usize {
                    content_lines.push(std::mem::take(&mut buf));
                    if content_lines.len() as u16 >= max_content_h {
                        break;
                    }
                }
                buf.push(ch);
            }

            if content_lines.len() as u16 >= max_content_h {
                break;
            }

            if raw_line.is_empty() {
                content_lines.push(Vec::new());
            } else if !buf.is_empty() {
                content_lines.push(buf);
            }
        }

        if content_lines.is_empty() {
            content_lines.push(Vec::new());
        }

        let mut content_w: u16 = 1;
        for l in &content_lines {
            content_w = content_w.max(l.len().min(max_content_w as usize) as u16);
        }
        let content_h: u16 = (content_lines.len().min(max_content_h as usize)) as u16;

        let box_w = content_w
            .saturating_add(2u16.saturating_mul(border))
            .saturating_add(2u16.saturating_mul(pad_x));
        let box_h = content_h
            .saturating_add(2u16.saturating_mul(border))
            .saturating_add(2u16.saturating_mul(pad_y));

        // BD-01 (Border Dragon): precise centering using (cols - box_w) / 2
        // instead of cols/2 - box_w/2. The old formula lost 1px on odd-width
        // boxes due to double integer truncation, causing bottom-left corner
        // (╰) to appear 1 cell too far forward and bottom-right (╯) 1 cell
        // too far back — owner-reported visual asymmetry at round corners.
        let start_col = self.cols.saturating_sub(box_w) / 2;
        let start_line = self.lines.saturating_sub(box_h) / 2;

        self.message.clear();

        for y in 0..box_h {
            let line = start_line.saturating_add(y);
            if line >= self.lines {
                continue;
            }

            for x in 0..box_w {
                let col = start_col.saturating_add(x);
                if col >= self.cols {
                    continue;
                }

                let mut ch = ' ';
                if border == 1 {
                    let is_top = y == 0;
                    let is_bottom = y + 1 == box_h;
                    let is_left = x == 0;
                    let is_right = x + 1 == box_w;
                    // v25 cinematic border: rounded corners + smooth lines.
                    ch = if is_top && is_left {
                        '╭'
                    } else if is_top && is_right {
                        '╮'
                    } else if is_bottom && is_left {
                        '╰'
                    } else if is_bottom && is_right {
                        '╯'
                    } else if is_top || is_bottom {
                        '─'
                    } else if is_left || is_right {
                        '│'
                    } else {
                        ' '
                    };
                }

                {
                    let content_start_y = border.saturating_add(pad_y);
                    let content_start_x = border.saturating_add(pad_x);

                    if y >= content_start_y
                        && y < content_start_y.saturating_add(content_h)
                        && x >= content_start_x
                        && x < content_start_x.saturating_add(content_w)
                    {
                        let inner_y = y - content_start_y;
                        let inner_x = x - content_start_x;

                        let li = inner_y as usize;
                        if let Some(line_chars) = content_lines.get(li) {
                            let line_len = line_chars.len().min(content_w as usize);
                            let left_pad = (content_w as usize)
                                .saturating_sub(line_len)
                                .saturating_div(2);
                            let ix = inner_x as usize;
                            if ix >= left_pad && ix < left_pad + line_len {
                                ch = line_chars[ix - left_pad];
                            }
                        }
                    }
                }

                self.message.push(MsgChr { line, col, val: ch });
            }
        }

        // BN-01/02 (Dragon Hunt v3): rebuild the cached border order here
        // (rare — only fires on `--message` set or border toggle) so
        // draw_message can borrow it instead of recomputing per frame.
        self.border_order = border::build_border_order(&self.message);

        // v80.0.0-beta.1 msg-fill-style (words): rebuild the per-cell word ordinals.
        // A "word" is a maximal run of content cells (non-border-char,
        // i.e. not space and not a box-drawing glyph) between non-content
        // cells. Ordinals are 1-based; a non-content cell carries the
        // ordinal of the word that just ended (leading padding carries 0)
        // so spaces fade in together with the word they follow.
        // Z-5: hoisted buffer (clear() keeps the allocation — zero-alloc
        // after the first rebuild).
        self.message_word_ordinals.clear();
        self.message_word_ordinals.reserve(self.message.len());
        {
            let mut word_ord: u32 = 0;
            let mut in_word = false;
            for mc in &self.message {
                if border::is_border_char(mc.val) {
                    // Space / border glyph: ends the current word. The
                    // next content cell starts a new one.
                    in_word = false;
                    self.message_word_ordinals.push(word_ord);
                } else {
                    if !in_word {
                        word_ord = word_ord.saturating_add(1);
                        in_word = true;
                    }
                    self.message_word_ordinals.push(word_ord);
                }
            }
        }

        // RAIN_BORDER_TOUCH_GLOW: cache top-border geometry for the droplet
        // advance loop's touch detection. Only relevant when the overlay
        // is bordered (`-mb`); for `-m` (no border), the top edge is the
        // first content row, which is not a "border touch" event. We
        // leave `message_top_line = u16::MAX` sentinel in that case so
        // `head_put_line >= top_line` is always false.
        if self.message_border && box_h > 0 {
            self.message_top_line = start_line;
            self.message_left_col = start_col;
            // box_w is at most cols, so saturating_add won't overflow u16
            // for any practical terminal width (u16::MAX = 65535 cols).
            self.message_right_col = start_col.saturating_add(box_w);
        } else {
            self.message_top_line = u16::MAX;
            self.message_left_col = 0;
            self.message_right_col = 0;
        }
        // Pulses from the previous overlay are stale; drop them.
        self.border_pulses.clear();
        // v80.0.0-beta.1 engrave/scorch: same staleness — sparks/smoke spawned
        // against the old layout must not keep flying, and the
        // movement detector must re-arm for the fresh reveal.
        self.engrave.reset();
        self.scorch.reset();
    }
}
