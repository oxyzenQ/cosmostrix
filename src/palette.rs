// Copyright (c) 2026 rezky_nightky

use crossterm::style::Color;

use crate::runtime::{ColorMode, ColorScheme};

#[derive(Clone, Debug)]
pub struct Palette {
    pub colors: Vec<Color>,
    pub bg: Option<Color>,
}

fn from_ansi_list(list: &[u8]) -> Vec<Color> {
    list.iter().map(|&v| Color::AnsiValue(v)).collect()
}

fn from_rgb_list(list: &[(u8, u8, u8)]) -> Vec<Color> {
    list.iter()
        .map(|&(r, g, b)| Color::Rgb { r, g, b })
        .collect()
}

fn dist2(r0: u8, g0: u8, b0: u8, r1: u8, g1: u8, b1: u8) -> i32 {
    let dr = (r0 as i32) - (r1 as i32);
    let dg = (g0 as i32) - (g1 as i32);
    let db = (b0 as i32) - (b1 as i32);
    (dr * dr) + (dg * dg) + (db * db)
}

fn rgb_to_ansi256(r: u8, g: u8, b: u8) -> u8 {
    const CUBE_LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];

    let r6 = ((r as u16 * 5) + 127) / 255;
    let g6 = ((g as u16 * 5) + 127) / 255;
    let b6 = ((b as u16 * 5) + 127) / 255;

    let cr = CUBE_LEVELS[r6 as usize];
    let cg = CUBE_LEVELS[g6 as usize];
    let cb = CUBE_LEVELS[b6 as usize];
    let cube_idx = 16 + (36 * r6 as u8) + (6 * g6 as u8) + (b6 as u8);
    let cube_dist = dist2(r, g, b, cr, cg, cb);

    let avg = ((r as u16 + g as u16 + b as u16) / 3) as u8;
    let gray_idx = if avg < 8 {
        16
    } else if avg > 238 {
        231
    } else {
        232 + ((avg - 8) / 10)
    };
    let (gr, gg, gb) = if gray_idx == 16 {
        (0, 0, 0)
    } else if gray_idx == 231 {
        (255, 255, 255)
    } else {
        let v = 8 + 10 * (gray_idx - 232);
        (v, v, v)
    };
    let gray_dist = dist2(r, g, b, gr, gg, gb);

    if gray_dist < cube_dist {
        gray_idx
    } else {
        cube_idx
    }
}

fn rgb_to_color16(r: u8, g: u8, b: u8) -> Color {
    const TABLE: [(Color, (u8, u8, u8)); 16] = [
        (Color::Black, (0, 0, 0)),
        (Color::DarkGrey, (128, 128, 128)),
        (Color::Grey, (192, 192, 192)),
        (Color::White, (255, 255, 255)),
        (Color::DarkRed, (128, 0, 0)),
        (Color::Red, (255, 0, 0)),
        (Color::DarkGreen, (0, 128, 0)),
        (Color::Green, (0, 255, 0)),
        (Color::DarkBlue, (0, 0, 128)),
        (Color::Blue, (0, 0, 255)),
        (Color::DarkCyan, (0, 128, 128)),
        (Color::Cyan, (0, 255, 255)),
        (Color::DarkMagenta, (128, 0, 128)),
        (Color::Magenta, (255, 0, 255)),
        (Color::DarkYellow, (128, 128, 0)),
        (Color::Yellow, (255, 255, 0)),
    ];

    let mut best = Color::White;
    let mut best_d = i32::MAX;
    for (c, (cr, cg, cb)) in TABLE {
        let d = dist2(r, g, b, cr, cg, cb);
        if d < best_d {
            best_d = d;
            best = c;
        }
    }
    best
}

fn colors_from_rgb(mode: ColorMode, list: &[(u8, u8, u8)]) -> Vec<Color> {
    match mode {
        ColorMode::Mono => vec![Color::White],
        ColorMode::TrueColor => from_rgb_list(list),
        ColorMode::Color256 => list
            .iter()
            .map(|&(r, g, b)| Color::AnsiValue(rgb_to_ansi256(r, g, b)))
            .collect(),
        ColorMode::Color16 => list
            .iter()
            .map(|&(r, g, b)| rgb_to_color16(r, g, b))
            .collect(),
    }
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    let a = a as f32;
    let b = b as f32;
    (a + (b - a) * t).round().clamp(0.0, 255.0) as u8
}

fn gradient_from_stops(stops: &[(u8, u8, u8)], steps: usize) -> Vec<(u8, u8, u8)> {
    if steps == 0 || stops.is_empty() {
        return Vec::new();
    }
    if stops.len() == 1 {
        return vec![stops[0]; steps];
    }
    if steps == 1 {
        return vec![stops[0]];
    }

    let segs = stops.len().saturating_sub(1);
    let mut out = Vec::with_capacity(steps);
    for i in 0..steps {
        let t = (i as f32) / ((steps - 1) as f32);
        let pos = t * (segs as f32);
        let mut seg = pos.floor() as usize;
        if seg >= segs {
            seg = segs.saturating_sub(1);
        }
        let lt = pos - (seg as f32);
        let (r0, g0, b0) = stops[seg];
        let (r1, g1, b1) = stops[seg + 1];
        out.push((
            lerp_u8(r0, r1, lt),
            lerp_u8(g0, g1, lt),
            lerp_u8(b0, b1, lt),
        ));
    }
    out
}

fn colors_from_stops(mode: ColorMode, stops: &[(u8, u8, u8)], steps: usize) -> Vec<Color> {
    if matches!(mode, ColorMode::Mono) {
        return vec![Color::White];
    }
    let rgb = gradient_from_stops(stops, steps);
    colors_from_rgb(mode, &rgb)
}

pub fn build_palette(scheme: ColorScheme, mode: ColorMode, default_background: bool) -> Palette {
    let mut bg = if default_background {
        None
    } else {
        Some(match mode {
            ColorMode::Color16 => Color::Black,
            ColorMode::TrueColor => Color::Rgb { r: 0, g: 0, b: 0 },
            _ => Color::AnsiValue(16),
        })
    };

    let colors: Vec<Color> = match scheme {
        ColorScheme::Green => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkGreen, Color::Green],
            _ => from_ansi_list(&[234, 22, 28, 35, 78, 84, 159]),
        },
        ColorScheme::Green2 => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![
                Color::DarkGrey,
                Color::DarkGreen,
                Color::Green,
                Color::White,
            ],
            _ => from_ansi_list(&[28, 34, 76, 84, 120, 157, 231]),
        },
        ColorScheme::Green3 => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkGreen, Color::White],
            _ => from_ansi_list(&[22, 28, 34, 70, 76, 82, 157]),
        },
        ColorScheme::Gold => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![
                Color::DarkGrey,
                Color::DarkYellow,
                Color::Yellow,
                Color::White,
            ],
            _ => from_ansi_list(&[58, 94, 172, 178, 228, 230, 231]),
        },
        ColorScheme::Yellow => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkGrey, Color::Yellow, Color::White],
            _ => from_ansi_list(&[100, 142, 184, 226, 227, 229, 230]),
        },
        ColorScheme::Orange => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::Red, Color::Grey],
            _ => from_ansi_list(&[52, 94, 130, 166, 202, 208, 231]),
        },
        ColorScheme::Red => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkRed, Color::Red, Color::White],
            _ => from_ansi_list(&[234, 52, 88, 124, 160, 196, 217]),
        },
        ColorScheme::Blue => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkBlue, Color::Blue, Color::White],
            _ => from_ansi_list(&[234, 17, 18, 19, 20, 21, 75, 159]),
        },
        ColorScheme::Cyan => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkCyan, Color::Cyan, Color::White],
            _ => from_ansi_list(&[24, 25, 31, 32, 38, 45, 159]),
        },
        ColorScheme::Purple => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::Magenta, Color::Grey],
            _ => from_ansi_list(&[60, 61, 62, 63, 69, 111, 225]),
        },
        ColorScheme::Neon => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::Blue, Color::Magenta, Color::Cyan, Color::White],
            _ => from_ansi_list(&[17, 18, 19, 54, 93, 129, 201, 51, 231]),
        },
        ColorScheme::Fire => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![
                Color::DarkRed,
                Color::Red,
                Color::DarkYellow,
                Color::Yellow,
                Color::White,
            ],
            _ => from_ansi_list(&[52, 88, 124, 160, 196, 202, 208, 214, 226, 231]),
        },
        ColorScheme::Ocean => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![
                Color::DarkBlue,
                Color::Blue,
                Color::DarkCyan,
                Color::Cyan,
                Color::White,
            ],
            _ => from_ansi_list(&[17, 18, 19, 24, 30, 37, 44, 51, 87, 159, 231]),
        },
        ColorScheme::Forest => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkGreen, Color::Green, Color::Yellow, Color::White],
            _ => from_ansi_list(&[22, 28, 34, 40, 46, 82, 118, 154, 190, 229, 231]),
        },
        ColorScheme::Vaporwave => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![
                Color::Magenta,
                Color::Magenta,
                Color::Yellow,
                Color::Cyan,
                Color::White,
            ],
            _ => from_ansi_list(&[
                53, 54, 55, 134, 177, 219, 214, 220, 227, 229, 87, 123, 159, 195, 231,
            ]),
        },
        ColorScheme::Gray => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkGrey, Color::Grey, Color::White],
            _ => from_ansi_list(&[234, 237, 240, 243, 246, 249, 251, 252, 231]),
        },
        ColorScheme::Rainbow => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![
                Color::Red,
                Color::Blue,
                Color::Yellow,
                Color::Green,
                Color::Cyan,
                Color::Magenta,
            ],
            _ => from_ansi_list(&[196, 208, 226, 46, 21, 93, 201]),
        },
        ColorScheme::Snow => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkGrey, Color::Grey, Color::White, Color::Cyan],
            _ => from_ansi_list(&[234, 240, 250, 252, 231, 117, 159]),
        },
        ColorScheme::Aurora => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkGreen, Color::Green, Color::Cyan, Color::Magenta],
            _ => from_ansi_list(&[22, 28, 34, 40, 45, 51, 93, 129, 159]),
        },
        ColorScheme::FancyDiamond => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::Cyan, Color::White, Color::Magenta],
            _ => from_ansi_list(&[45, 51, 87, 123, 159, 195, 231, 225]),
        },
        ColorScheme::Cosmos => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::DarkBlue, Color::Blue, Color::Magenta, Color::White],
            _ => from_ansi_list(&[17, 18, 19, 54, 55, 56, 57, 93, 129, 189, 225]),
        },
        ColorScheme::Nebula => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::Magenta, Color::Red, Color::Blue, Color::White],
            _ => from_ansi_list(&[53, 54, 90, 126, 162, 198, 201, 207, 213, 219, 225]),
        },
        ColorScheme::Spectrum20 => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![
                Color::DarkGrey,
                Color::DarkRed,
                Color::Red,
                Color::DarkYellow,
                Color::Yellow,
                Color::DarkGreen,
                Color::Green,
                Color::DarkCyan,
                Color::Cyan,
                Color::DarkBlue,
                Color::Blue,
                Color::DarkMagenta,
                Color::Magenta,
                Color::DarkGrey,
                Color::Grey,
                Color::White,
                Color::Cyan,
                Color::Yellow,
                Color::Magenta,
                Color::White,
            ],
            ColorMode::TrueColor => from_rgb_list(&[
                (0, 0, 0),
                (128, 0, 0),
                (255, 0, 0),
                (255, 64, 0),
                (255, 128, 0),
                (255, 191, 0),
                (255, 255, 0),
                (191, 255, 0),
                (128, 255, 0),
                (0, 255, 0),
                (0, 255, 128),
                (0, 255, 191),
                (0, 255, 255),
                (0, 191, 255),
                (0, 128, 255),
                (0, 0, 255),
                (128, 0, 255),
                (191, 0, 255),
                (255, 0, 255),
                (255, 255, 255),
            ]),
            _ => from_ansi_list(&[
                234, 52, 88, 124, 160, 196, 202, 208, 214, 226, 190, 154, 118, 82, 51, 39, 27, 93,
                201, 231,
            ]),
        },
        ColorScheme::Stars => colors_from_stops(
            mode,
            &[(0, 0, 0), (10, 10, 40), (80, 160, 255), (255, 255, 255)],
            9,
        ),
        ColorScheme::Mars => colors_from_stops(
            mode,
            &[(20, 0, 0), (120, 10, 10), (220, 60, 20), (255, 235, 220)],
            9,
        ),
        ColorScheme::Venus => colors_from_stops(
            mode,
            &[(10, 10, 0), (120, 90, 30), (255, 220, 120), (255, 255, 255)],
            9,
        ),
        ColorScheme::Mercury => colors_from_stops(
            mode,
            &[(0, 0, 0), (64, 64, 64), (160, 160, 160), (255, 255, 255)],
            9,
        ),
        ColorScheme::Jupiter => colors_from_stops(
            mode,
            &[(20, 10, 0), (120, 60, 20), (200, 140, 90), (255, 255, 255)],
            9,
        ),
        ColorScheme::Saturn => colors_from_stops(
            mode,
            &[
                (20, 20, 10),
                (140, 120, 60),
                (230, 210, 150),
                (255, 255, 255),
            ],
            9,
        ),
        ColorScheme::Uranus => colors_from_stops(
            mode,
            &[(0, 10, 10), (0, 120, 130), (120, 255, 255), (255, 255, 255)],
            9,
        ),
        ColorScheme::Neptune => colors_from_stops(
            mode,
            &[(0, 0, 20), (0, 40, 140), (0, 140, 255), (240, 255, 255)],
            9,
        ),
        ColorScheme::Pluto => colors_from_stops(
            mode,
            &[(10, 5, 0), (90, 60, 40), (180, 190, 210), (255, 255, 255)],
            9,
        ),
        ColorScheme::Moon => colors_from_stops(
            mode,
            &[(0, 0, 0), (90, 100, 120), (200, 210, 220), (255, 255, 255)],
            9,
        ),
        ColorScheme::Sun => colors_from_stops(
            mode,
            &[(40, 0, 0), (200, 60, 0), (255, 200, 0), (255, 255, 255)],
            9,
        ),
        ColorScheme::Comet => colors_from_stops(
            mode,
            &[(0, 0, 20), (0, 100, 160), (180, 255, 255), (255, 255, 255)],
            9,
        ),
        ColorScheme::Galaxy => colors_from_stops(
            mode,
            &[(10, 0, 20), (60, 0, 120), (180, 60, 255), (255, 255, 255)],
            9,
        ),
        ColorScheme::Supernova => colors_from_stops(
            mode,
            &[(20, 0, 40), (180, 0, 60), (255, 120, 0), (255, 255, 255)],
            9,
        ),
        ColorScheme::BlackHole => colors_from_stops(
            mode,
            &[(0, 0, 0), (20, 0, 40), (40, 0, 80), (200, 120, 255)],
            9,
        ),
        ColorScheme::Andromeda => colors_from_stops(
            mode,
            &[(0, 0, 20), (50, 0, 120), (255, 80, 200), (255, 255, 255)],
            9,
        ),
        ColorScheme::Stardust => colors_from_stops(
            mode,
            &[(10, 0, 20), (120, 60, 200), (80, 200, 255), (255, 255, 255)],
            9,
        ),
        ColorScheme::Meteor => colors_from_stops(
            mode,
            &[(20, 10, 0), (180, 60, 0), (255, 170, 0), (255, 255, 255)],
            9,
        ),
        ColorScheme::Eclipse => colors_from_stops(
            mode,
            &[(0, 0, 0), (40, 0, 60), (255, 120, 0), (255, 255, 255)],
            9,
        ),
        ColorScheme::DeepSpace => colors_from_stops(
            mode,
            &[(0, 0, 0), (0, 10, 40), (0, 80, 160), (200, 120, 255)],
            9,
        ),
    };

    if default_background {
        bg = None;
    }

    Palette { colors, bg }
}
