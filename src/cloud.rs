// Copyright (c) 2025 rezk_nightky

use std::time::{Duration, Instant};

use crossterm::style::Color;
use rand::{
    distr::{Distribution, Uniform},
    rngs::StdRng,
    SeedableRng,
};

use crate::{
    cell::Cell,
    frame::Frame,
    palette::{build_palette, Palette},
    runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode, UserColors},
};

use crate::droplet::Droplet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CharLoc {
    Middle,
    Tail,
    Head,
}

pub struct DrawCtx<'a> {
    pub lines: u16,
    pub full_width: bool,
    pub shading_distance: bool,
    pub bg: Option<Color>,

    pub color_mode: ColorMode,
    pub bold_mode: BoldMode,
    pub glitchy: bool,

    pub last_glitch_time: Instant,
    pub next_glitch_time: Instant,

    pub palette_colors: &'a [Color],
    pub color_map: &'a [u8],
    pub glitch_map: &'a [bool],
    pub char_pool: &'a [char],
}

impl DrawCtx<'_> {
    fn is_bright(&self, now: Instant) -> bool {
        if now < self.last_glitch_time {
            return false;
        }
        let since = now
            .saturating_duration_since(self.last_glitch_time)
            .as_nanos() as f64;
        let between = self
            .next_glitch_time
            .saturating_duration_since(self.last_glitch_time)
            .as_nanos() as f64;
        if between <= 0.0 {
            return false;
        }
        (since / between) <= 0.25
    }

    fn is_dim(&self, now: Instant) -> bool {
        if now > self.next_glitch_time {
            return true;
        }
        let since = now
            .saturating_duration_since(self.last_glitch_time)
            .as_nanos() as f64;
        let between = self
            .next_glitch_time
            .saturating_duration_since(self.last_glitch_time)
            .as_nanos() as f64;
        if between <= 0.0 {
            return true;
        }
        (since / between) >= 0.75
    }

    pub fn is_glitched(&self, line: u16, col: u16) -> bool {
        if !self.glitchy {
            return false;
        }
        let idx = col as usize * self.lines as usize + line as usize;
        self.glitch_map.get(idx).copied().unwrap_or(false)
    }

    pub fn get_char(&self, line: u16, char_pool_idx: u16) -> char {
        let len = self.char_pool.len().max(1);
        let idx = ((char_pool_idx as usize) + (line as usize)) % len;
        self.char_pool.get(idx).copied().unwrap_or('0')
    }

    #[allow(clippy::too_many_arguments)]
    pub fn get_attr(
        &self,
        line: u16,
        col: u16,
        val: char,
        loc: CharLoc,
        now: Instant,
        head_put_line: u16,
        length: u16,
    ) -> (Option<Color>, bool) {
        let mut bold = false;
        if self.bold_mode == BoldMode::Random {
            bold = (((line as u32) ^ (val as u32)) % 2) == 1;
        }

        let idx = col as usize * self.lines as usize + line as usize;
        let mut color_idx = self.color_map.get(idx).copied().unwrap_or(0) as i32;

        if self.shading_distance {
            let n = self.palette_colors.len().max(1) as f32;
            let dist = (head_put_line.saturating_sub(line)) as f32;
            let len = length.max(1) as f32;
            let v = (n - 1.0) - (dist / len * (n - 1.0));
            color_idx = v.round() as i32;
        }

        if self.glitchy && self.glitch_map.get(idx).copied().unwrap_or(false) {
            if self.is_bright(now) {
                color_idx += 1;
                bold = true;
            } else if self.is_dim(now) {
                color_idx -= 1;
                bold = false;
            }
        }

        let last = self.palette_colors.len().saturating_sub(1) as i32;
        match loc {
            CharLoc::Tail => {
                color_idx = 0;
                bold = false;
            }
            CharLoc::Head => {
                color_idx = last;
                bold = true;
            }
            CharLoc::Middle => {
                color_idx = color_idx.clamp(0, last.max(0));
            }
        }

        match self.bold_mode {
            BoldMode::Off => bold = false,
            BoldMode::All => bold = true,
            BoldMode::Random => {}
        }

        let fg = if self.color_mode == ColorMode::Mono {
            None
        } else {
            self.palette_colors.get(color_idx as usize).copied()
        };

        (fg, bold)
    }
}

#[derive(Clone, Debug)]
struct ColumnStatus {
    max_speed_pct: f32,
    num_droplets: u8,
    can_spawn: bool,
}

#[derive(Clone, Debug)]
struct MsgChr {
    line: u16,
    col: u16,
    val: char,
    draw: bool,
}

pub struct Cloud {
    pub lines: u16,
    pub cols: u16,

    pub palette: Palette,
    pub color_mode: ColorMode,

    pub full_width: bool,
    pub shading_distance: bool,
    pub bold_mode: BoldMode,

    pub async_mode: bool,
    pub raining: bool,
    pub pause: bool,

    pub droplet_density: f32,
    pub droplets_per_sec: f32,
    pub chars_per_sec: f32,

    pub glitchy: bool,
    pub glitch_pct: f32,
    pub glitch_low_ms: u16,
    pub glitch_high_ms: u16,

    pub short_pct: f32,
    pub die_early_pct: f32,
    pub linger_low_ms: u16,
    pub linger_high_ms: u16,

    pub max_droplets_per_column: u8,

    droplets: Vec<Droplet>,
    num_droplets: usize,

    chars: Vec<char>,
    char_pool: Vec<char>,
    glitch_pool: Vec<char>,
    glitch_pool_idx: usize,

    glitch_map: Vec<bool>,
    color_map: Vec<u8>,

    col_stat: Vec<ColumnStatus>,

    mt: StdRng,

    rand_chance: Uniform<f32>,
    rand_line: Uniform<u16>,
    rand_cpidx: Uniform<u16>,
    rand_len: Uniform<u16>,
    rand_col: Uniform<u16>,
    rand_glitch_ms: Uniform<u16>,
    rand_linger_ms: Uniform<u16>,
    rand_speed: Uniform<f32>,

    last_glitch_time: Instant,
    next_glitch_time: Instant,
    last_spawn_time: Instant,
    pause_time: Option<Instant>,

    force_draw_everything: bool,

    shading_mode: ShadingMode,

    message: Vec<MsgChr>,

    user_colors: Option<UserColors>,
    color_scheme: ColorScheme,
    default_background: bool,
}

impl Cloud {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        color_mode: ColorMode,
        full_width: bool,
        shading_mode: ShadingMode,
        bold_mode: BoldMode,
        async_mode: bool,
        default_background: bool,
        color_scheme: ColorScheme,
        user_colors: Option<UserColors>,
    ) -> Self {
        let now = Instant::now();
        let mt = StdRng::seed_from_u64(0x1234567);

        let cloud = Self {
            lines: 25,
            cols: 80,
            palette: build_palette(
                color_scheme,
                color_mode,
                default_background,
                user_colors.as_ref(),
            ),
            color_mode,
            full_width,
            shading_distance: matches!(shading_mode, ShadingMode::DistanceFromHead),
            bold_mode,
            async_mode,
            raining: true,
            pause: false,
            droplet_density: 1.0,
            droplets_per_sec: 5.0,
            chars_per_sec: 8.0,
            glitchy: true,
            glitch_pct: 0.1,
            glitch_low_ms: 300,
            glitch_high_ms: 400,
            short_pct: 0.5,
            die_early_pct: 0.3333333,
            linger_low_ms: 1,
            linger_high_ms: 3000,
            max_droplets_per_column: 3,
            droplets: Vec::new(),
            num_droplets: 0,
            chars: Vec::new(),
            char_pool: Vec::new(),
            glitch_pool: Vec::new(),
            glitch_pool_idx: 0,
            glitch_map: Vec::new(),
            color_map: Vec::new(),
            col_stat: Vec::new(),
            mt,
            rand_chance: Uniform::new(0.0, 1.0).expect("valid range"),
            rand_line: Uniform::new_inclusive(0, 23).expect("valid range"),
            rand_cpidx: Uniform::new_inclusive(0, 2047).expect("valid range"),
            rand_len: Uniform::new_inclusive(1, 23).expect("valid range"),
            rand_col: Uniform::new_inclusive(0, 79).expect("valid range"),
            rand_glitch_ms: Uniform::new_inclusive(300, 400).expect("valid range"),
            rand_linger_ms: Uniform::new_inclusive(1, 3000).expect("valid range"),
            rand_speed: Uniform::new_inclusive(0.3333333, 1.0).expect("valid range"),
            last_glitch_time: now,
            next_glitch_time: now + Duration::from_millis(300),
            last_spawn_time: now,
            pause_time: None,
            force_draw_everything: false,
            shading_mode,
            message: Vec::new(),
            user_colors,
            color_scheme,
            default_background,
        };

        cloud
    }

    pub fn set_message(&mut self, msg: &str) {
        self.message.clear();
        for ch in msg.chars() {
            self.message.push(MsgChr {
                line: 0,
                col: 0,
                val: ch,
                draw: false,
            });
        }
        self.reset_message();
    }

    pub fn set_color_scheme(&mut self, scheme: ColorScheme) {
        self.color_scheme = scheme;
        self.palette = build_palette(
            scheme,
            self.color_mode,
            self.default_background,
            self.user_colors.as_ref(),
        );
        self.fill_color_map();
        self.force_draw_everything = true;
    }

    pub fn set_async(&mut self, on: bool) {
        self.async_mode = on;
        self.set_column_speeds();
        self.update_droplet_speeds();
    }

    pub fn set_chars_per_sec(&mut self, cps: f32) {
        self.chars_per_sec = cps;
        self.recalc_droplets_per_sec();
        self.set_column_speeds();
        self.update_droplet_speeds();
    }

    pub fn set_droplet_density(&mut self, density: f32) {
        self.droplet_density = density;
        self.recalc_droplets_per_sec();
    }

    pub fn set_glitch_pct(&mut self, pct: f32) {
        self.glitch_pct = pct;
        self.fill_glitch_map();
    }

    pub fn set_glitch_times(&mut self, low_ms: u16, high_ms: u16) {
        self.glitch_low_ms = low_ms;
        self.glitch_high_ms = high_ms;
        let (lo, hi) = if low_ms <= high_ms {
            (low_ms, high_ms)
        } else {
            (high_ms, low_ms)
        };
        self.rand_glitch_ms = Uniform::new_inclusive(lo, hi).expect("valid range");
    }

    pub fn set_linger_times(&mut self, low_ms: u16, high_ms: u16) {
        self.linger_low_ms = low_ms;
        self.linger_high_ms = high_ms;
        let (lo, hi) = if low_ms <= high_ms {
            (low_ms, high_ms)
        } else {
            (high_ms, low_ms)
        };
        self.rand_linger_ms = Uniform::new_inclusive(lo, hi).expect("valid range");
    }

    pub fn set_max_droplets_per_column(&mut self, v: u8) {
        self.max_droplets_per_column = v;
    }

    pub fn toggle_pause(&mut self) {
        self.pause = !self.pause;
        if self.pause {
            self.pause_time = Some(Instant::now());
        } else if let Some(pt) = self.pause_time.take() {
            let elapsed = Instant::now().saturating_duration_since(pt);
            self.last_spawn_time += elapsed;
            for d in &mut self.droplets {
                if d.is_alive {
                    d.increment_time(elapsed);
                }
            }
        }
    }

    pub fn reset(&mut self, cols: u16, lines: u16) {
        self.cols = cols;
        self.lines = lines;

        self.num_droplets = (1.5 * self.cols as f32).round() as usize;
        self.droplets.clear();
        self.droplets.resize_with(self.num_droplets, Droplet::new);

        let max_line = lines.saturating_sub(2);
        let max_len = max_line.max(1);
        self.rand_line = Uniform::new_inclusive(0, max_line).expect("valid range");
        self.rand_len = Uniform::new_inclusive(1, max_len).expect("valid range");
        self.rand_col = Uniform::new_inclusive(0, cols.saturating_sub(1)).expect("valid range");
        self.rand_cpidx = Uniform::new_inclusive(0, 2047).expect("valid range");

        self.recalc_droplets_per_sec();

        self.col_stat.clear();
        self.col_stat.resize(
            cols as usize,
            ColumnStatus {
                max_speed_pct: 1.0,
                num_droplets: 0,
                can_spawn: true,
            },
        );

        self.fill_glitch_map();
        self.fill_color_map();
        self.set_column_speeds();
        self.update_droplet_speeds();

        if !self.message.is_empty() {
            self.reset_message();
        }

        let now = Instant::now();
        self.last_glitch_time = now;
        self.next_glitch_time =
            now + Duration::from_millis(self.rand_glitch_ms.sample(&mut self.mt) as u64);
        self.last_spawn_time = now;
        self.force_draw_everything = true;
    }

    pub fn init_chars(&mut self, chars: Vec<char>) {
        self.chars = chars;
        if self.chars.is_empty() {
            self.chars.push('0');
            self.chars.push('1');
        }

        self.char_pool.resize(2048, '0');
        self.glitch_pool.resize(1024, '0');
        self.glitch_pool_idx = 0;

        let dist = Uniform::new_inclusive(0usize, self.chars.len().saturating_sub(1))
            .expect("valid range");
        for i in 0..self.char_pool.len() {
            let idx = dist.sample(&mut self.mt);
            self.char_pool[i] = self.chars[idx];
        }
        for i in 0..self.glitch_pool.len() {
            let idx = dist.sample(&mut self.mt);
            self.glitch_pool[i] = self.chars[idx];
        }
    }

    fn recalc_droplets_per_sec(&mut self) {
        let droplet_seconds = (self.lines as f32) / self.chars_per_sec.max(0.001);
        self.droplets_per_sec = (self.cols as f32) * self.droplet_density / droplet_seconds;
    }

    fn fill_glitch_map(&mut self) {
        if !self.glitchy {
            self.glitch_map.clear();
            return;
        }
        let size = self.lines as usize * self.cols as usize;
        self.glitch_map.resize(size, false);
        for v in &mut self.glitch_map {
            *v = self.rand_chance.sample(&mut self.mt) <= self.glitch_pct;
        }
    }

    fn fill_color_map(&mut self) {
        let size = self.lines as usize * self.cols as usize;
        self.color_map.resize(size, 0);

        let n = self.palette.colors.len().max(1);
        let (low, high) = if n < 3 {
            (0, 0)
        } else if n == 3 {
            (1, 1)
        } else {
            (1, (n - 2) as u8)
        };
        let dist = Uniform::new_inclusive(low, high).expect("valid range");

        for v in &mut self.color_map {
            *v = dist.sample(&mut self.mt);
        }
    }

    pub fn set_column_spawn(&mut self, col: u16, b: bool) {
        if let Some(cs) = self.col_stat.get_mut(col as usize) {
            cs.can_spawn = b;
        }
    }

    fn set_column_speeds(&mut self) {
        for cs in &mut self.col_stat {
            cs.max_speed_pct = if self.async_mode {
                self.rand_speed.sample(&mut self.mt)
            } else {
                1.0
            };
        }
    }

    fn update_droplet_speeds(&mut self) {
        for d in &mut self.droplets {
            if !d.is_alive {
                continue;
            }
            if let Some(cs) = self.col_stat.get(d.bound_col as usize) {
                d.chars_per_sec = cs.max_speed_pct * self.chars_per_sec;
            }
        }
    }

    fn time_for_glitch(&self, now: Instant) -> bool {
        self.glitchy && now >= self.next_glitch_time
    }

    pub fn is_glitched(&self, line: u16, col: u16) -> bool {
        if !self.glitchy {
            return false;
        }
        let idx = col as usize * self.lines as usize + line as usize;
        self.glitch_map.get(idx).copied().unwrap_or(false)
    }

    fn do_glitch_span(&mut self, start_line: u16, hp: u16, col: u16, cp_idx: u16) {
        if !self.glitchy {
            return;
        }

        for line in start_line..=hp {
            if line >= self.lines {
                break;
            }
            if self.is_glitched(line, col) {
                let char_idx = ((cp_idx as usize) + (line as usize)) % self.char_pool.len();
                let repl = self.glitch_pool[self.glitch_pool_idx % self.glitch_pool.len()];
                self.char_pool[char_idx] = repl;
                self.glitch_pool_idx = (self.glitch_pool_idx + 1) % self.glitch_pool.len();
            }
        }
    }

    fn fill_droplet(&mut self, d: &mut Droplet, col: u16) {
        let mut end_line = self.lines.saturating_sub(1);
        if self.rand_chance.sample(&mut self.mt) <= self.die_early_pct {
            end_line = self.rand_line.sample(&mut self.mt);
        }
        let cp_idx = self.rand_cpidx.sample(&mut self.mt);

        let mut len = self.lines;
        if self.rand_chance.sample(&mut self.mt) <= self.short_pct {
            len = self.rand_len.sample(&mut self.mt);
        }

        let mut ttl = Duration::from_millis(1);
        if end_line <= len {
            let ms = self.rand_linger_ms.sample(&mut self.mt) as u64;
            ttl = Duration::from_millis(ms);
        }

        let speed = self
            .col_stat
            .get(col as usize)
            .map(|cs| cs.max_speed_pct)
            .unwrap_or(1.0)
            * self.chars_per_sec;

        d.bound_col = col;
        d.end_line = end_line;
        d.char_pool_idx = cp_idx;
        d.length = len;
        d.chars_per_sec = speed;
        d.time_to_linger = ttl;
        d.head_put_line = 0;
        d.head_cur_line = 0;
        d.tail_put_line = None;
        d.tail_cur_line = 0;
        d.head_stop_time = None;
    }

    fn spawn_droplets(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.last_spawn_time);
        let elapsed_sec = elapsed.as_secs_f32();
        let to_spawn = ((elapsed_sec * self.droplets_per_sec) as usize).min(self.num_droplets);
        if to_spawn == 0 {
            return;
        }

        let mut idx = 0usize;
        let mut spawned = 0usize;

        for _ in 0..to_spawn {
            let mut col = self.rand_col.sample(&mut self.mt);
            if self.full_width {
                col &= 0xFFFE;
            }

            if col as usize >= self.col_stat.len() {
                continue;
            }

            if !self.col_stat[col as usize].can_spawn
                || self.col_stat[col as usize].num_droplets >= self.max_droplets_per_column
            {
                continue;
            }

            let mut found = None;
            while idx < self.droplets.len() {
                if !self.droplets[idx].is_alive {
                    found = Some(idx);
                    break;
                }
                idx += 1;
            }
            let Some(di) = found else {
                break;
            };

            let mut d = std::mem::replace(&mut self.droplets[di], Droplet::new());
            self.fill_droplet(&mut d, col);
            d.activate(now);
            self.droplets[di] = d;

            self.col_stat[col as usize].can_spawn = false;
            self.col_stat[col as usize].num_droplets += 1;

            spawned += 1;
        }

        if spawned > 0 {
            self.last_spawn_time = now;
        }
    }

    pub fn force_draw_everything(&mut self) {
        self.force_draw_everything = true;
    }

    pub fn set_shading_mode(&mut self, sm: ShadingMode) {
        self.shading_mode = sm;
        self.shading_distance = matches!(sm, ShadingMode::DistanceFromHead);
        self.force_draw_everything = true;
    }

    fn reset_message(&mut self) {
        if self.message.is_empty() {
            return;
        }

        let first_col = self.cols / 4;
        let last_col = (3 * self.cols) / 4;
        let chars_per_col = last_col.saturating_sub(first_col) + 1;
        let msg_lines = (self.message.len() as u16 / chars_per_col).saturating_add(1);
        let first_line = self.lines / 2 - msg_lines / 2;

        let mut remaining = self.message.len() as u16;
        let mut line = first_line;
        let mut col = first_col;
        if remaining < chars_per_col {
            col += (chars_per_col - remaining) / 2;
        }

        for mc in &mut self.message {
            mc.draw = false;
            if line < self.lines {
                mc.line = line;
                mc.col = col;
            } else {
                mc.line = u16::MAX;
                mc.col = u16::MAX;
            }

            if col == last_col {
                line = line.saturating_add(1);
                col = first_col;
                if remaining < chars_per_col {
                    col += (chars_per_col - remaining) / 2;
                }
            } else {
                col = col.saturating_add(1);
            }
            remaining = remaining.saturating_sub(1);
        }
    }

    fn calc_message(&mut self, frame: &Frame) {
        for mc in &mut self.message {
            if mc.line == u16::MAX || mc.col == u16::MAX {
                break;
            }
            if let Some(c) = frame.get(mc.col, mc.line) {
                if c.ch != ' ' {
                    mc.draw = true;
                }
            }
        }
    }

    fn draw_message(&self, frame: &mut Frame) {
        let bg = self.palette.bg;
        for mc in &self.message {
            if !mc.draw {
                continue;
            }
            if mc.line == u16::MAX || mc.col == u16::MAX {
                continue;
            }
            frame.set(
                mc.col,
                mc.line,
                Cell {
                    ch: mc.val,
                    fg: if self.color_mode == ColorMode::Mono {
                        None
                    } else {
                        self.palette.colors.last().copied()
                    },
                    bg,
                    bold: self.bold_mode != BoldMode::Off,
                },
            );
        }
    }

    pub fn rain(&mut self, frame: &mut Frame) {
        if self.pause {
            return;
        }

        let now = Instant::now();
        self.spawn_droplets(now);

        if self.force_draw_everything {
            frame.clear_with_bg(self.palette.bg);
        }

        let time_for_glitch = self.time_for_glitch(now);

        // Update pass (mut self)
        for i in 0..self.droplets.len() {
            if !self.droplets[i].is_alive {
                continue;
            }

            let (col, start_line, hp, cp_idx, free_col) = {
                let d = &mut self.droplets[i];
                let free_col = d.advance(now, self.lines);
                let col = d.bound_col;
                let start_line = d.tail_put_line.map(|v| v + 1).unwrap_or(0);
                let hp = d.head_put_line;
                let cp_idx = d.char_pool_idx;
                (col, start_line, hp, cp_idx, free_col)
            };

            if free_col {
                self.set_column_spawn(col, true);
            }

            if time_for_glitch {
                self.do_glitch_span(start_line, hp, col, cp_idx);
            }
        }

        // Draw pass (split-borrows via DrawCtx)
        let draw_everything = self.force_draw_everything;
        let ctx = DrawCtx {
            lines: self.lines,
            full_width: self.full_width,
            shading_distance: self.shading_distance,
            bg: self.palette.bg,
            color_mode: self.color_mode,
            bold_mode: self.bold_mode,
            glitchy: self.glitchy,
            last_glitch_time: self.last_glitch_time,
            next_glitch_time: self.next_glitch_time,
            palette_colors: &self.palette.colors,
            color_map: &self.color_map,
            glitch_map: &self.glitch_map,
            char_pool: &self.char_pool,
        };

        for d in &mut self.droplets {
            if !d.is_alive {
                continue;
            }
            d.draw(&ctx, frame, now, draw_everything);

            if !d.is_alive {
                if let Some(cs) = self.col_stat.get_mut(d.bound_col as usize) {
                    cs.num_droplets = cs.num_droplets.saturating_sub(1);
                    if d.tail_put_line.unwrap_or(0) <= self.lines / 4 {
                        cs.can_spawn = true;
                    }
                }
            }
        }

        if !self.message.is_empty() {
            self.calc_message(frame);
            self.draw_message(frame);
        }

        if time_for_glitch {
            self.last_glitch_time = now;
            let ms = self.rand_glitch_ms.sample(&mut self.mt) as u64;
            self.next_glitch_time = self.last_glitch_time + Duration::from_millis(ms);
        }

        self.force_draw_everything = false;
    }
}
