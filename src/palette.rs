// Copyright (c) 2025 rezk_nightky

use crossterm::style::Color;

use crate::runtime::{ColorMode, ColorScheme, UserColors};

#[derive(Clone, Debug)]
pub struct Palette {
    pub colors: Vec<Color>,
    pub bg: Option<Color>,
}

fn rgb_from_1000(r: u16, g: u16, b: u16) -> Color {
    let rr = ((r as u32).saturating_mul(255) / 1000) as u8;
    let gg = ((g as u32).saturating_mul(255) / 1000) as u8;
    let bb = ((b as u32).saturating_mul(255) / 1000) as u8;
    Color::Rgb {
        r: rr,
        g: gg,
        b: bb,
    }
}

fn from_ansi_list(list: &[u8]) -> Vec<Color> {
    list.iter().map(|&v| Color::AnsiValue(v)).collect()
}

pub fn build_palette(
    scheme: ColorScheme,
    mode: ColorMode,
    default_background: bool,
    user: Option<&UserColors>,
) -> Palette {
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
        ColorScheme::User => {
            if let Some(u) = user {
                if !u.colors.is_empty() {
                    if !default_background {
                        bg = Some(match mode {
                            ColorMode::TrueColor => {
                                if let Some((r, g, b)) = u.colors[0].rgb_1000 {
                                    rgb_from_1000(r, g, b)
                                } else {
                                    Color::AnsiValue(u.colors[0].index)
                                }
                            }
                            _ => Color::AnsiValue(u.colors[0].index),
                        });
                    }

                    let mut out = Vec::new();
                    for (i, c) in u.colors.iter().enumerate() {
                        if i == 0 {
                            continue;
                        }
                        out.push(match mode {
                            ColorMode::TrueColor => {
                                if let Some((r, g, b)) = c.rgb_1000 {
                                    rgb_from_1000(r, g, b)
                                } else {
                                    Color::AnsiValue(c.index)
                                }
                            }
                            _ => Color::AnsiValue(c.index),
                        });
                    }

                    if out.is_empty() {
                        out.push(Color::Green);
                    }
                    out
                } else {
                    vec![Color::Green]
                }
            } else {
                vec![Color::Green]
            }
        }
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
        ColorScheme::Pink => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::Magenta, Color::White],
            _ => from_ansi_list(&[133, 139, 176, 212, 218, 224, 231]),
        },
        ColorScheme::Pink2 => match mode {
            ColorMode::Mono => vec![Color::White],
            ColorMode::Color16 => vec![Color::Magenta, Color::Magenta, Color::White],
            _ => from_ansi_list(&[145, 181, 217, 218, 224, 225, 231]),
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
    };

    if default_background {
        bg = None;
    }

    Palette { colors, bg }
}
