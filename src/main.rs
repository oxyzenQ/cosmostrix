// Copyright (c) 2025 rezk_nightky

mod cell;
mod charset;
mod cloud;
mod config;
mod droplet;
mod frame;
mod palette;
mod runtime;
mod terminal;

use std::env;
use std::fs;
use std::time::Duration;

use clap::Parser;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

use crate::charset::{build_chars, charset_from_str, parse_user_hex_chars};
use crate::cloud::Cloud;
use crate::config::Args;
use crate::frame::Frame;
use crate::runtime::{BoldMode, ColorMode, ColorScheme, ShadingMode, UserColor, UserColors};
use crate::terminal::Terminal;

fn default_to_ascii() -> bool {
    let lang = env::var("LANG").unwrap_or_default();
    !lang.to_ascii_uppercase().contains("UTF")
}

fn detect_color_mode(args: &Args) -> ColorMode {
    if let Some(m) = args.colormode {
        return match m {
            0 => ColorMode::Mono,
            16 => ColorMode::Color16,
            32 => ColorMode::TrueColor,
            256 => ColorMode::Color256,
            _ => ColorMode::Color256,
        };
    }

    let colorterm = env::var("COLORTERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if colorterm.contains("truecolor") || colorterm.contains("24bit") {
        return ColorMode::TrueColor;
    }

    let term = env::var("TERM").unwrap_or_default().to_ascii_lowercase();
    if term.contains("256color") {
        return ColorMode::Color256;
    }

    ColorMode::Color16
}

fn parse_color_scheme(s: &str) -> Result<ColorScheme, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "user" => Ok(ColorScheme::User),
        "green" => Ok(ColorScheme::Green),
        "green2" => Ok(ColorScheme::Green2),
        "green3" => Ok(ColorScheme::Green3),
        "yellow" => Ok(ColorScheme::Yellow),
        "orange" => Ok(ColorScheme::Orange),
        "red" => Ok(ColorScheme::Red),
        "blue" => Ok(ColorScheme::Blue),
        "cyan" => Ok(ColorScheme::Cyan),
        "gold" => Ok(ColorScheme::Gold),
        "rainbow" => Ok(ColorScheme::Rainbow),
        "purple" => Ok(ColorScheme::Purple),
        "pink" => Ok(ColorScheme::Pink),
        "pink2" => Ok(ColorScheme::Pink2),
        "vaporwave" => Ok(ColorScheme::Vaporwave),
        "gray" | "grey" => Ok(ColorScheme::Gray),
        _ => Err(format!("invalid color: {}", s)),
    }
}

fn parse_user_colors(path: &std::path::Path) -> std::result::Result<UserColors, String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut colors: Vec<UserColor> = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let first = line.chars().next().unwrap_or(' ');
        if first == ';' || first == '#' || first == '/' || first == '*' || first == '@' {
            continue;
        }
        if line.contains("neo_color_version") {
            continue;
        }

        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if parts.is_empty() {
            continue;
        }
        let idx: u8 = parts[0]
            .parse::<u16>()
            .map_err(|_| "invalid color index".to_string())?
            .min(255) as u8;

        let rgb_1000 = if parts.len() >= 4 {
            let r: u16 = parts[1].parse().map_err(|_| "invalid r".to_string())?;
            let g: u16 = parts[2].parse().map_err(|_| "invalid g".to_string())?;
            let b: u16 = parts[3].parse().map_err(|_| "invalid b".to_string())?;
            Some((r, g, b))
        } else {
            None
        };

        colors.push(UserColor {
            index: idx,
            rgb_1000,
        });
    }

    if colors.len() < 2 {
        return Err("color file must contain at least two colors".to_string());
    }

    Ok(UserColors { colors })
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    if args.info {
        println!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
        println!("author: {}", env!("CARGO_PKG_AUTHORS"));
        println!("{}", env!("CARGO_PKG_DESCRIPTION"));
        return Ok(());
    }

    let def_ascii = default_to_ascii();
    let color_mode = detect_color_mode(&args);

    let shading_mode = match args.shading_mode {
        1 => ShadingMode::DistanceFromHead,
        _ => ShadingMode::Random,
    };

    let bold_mode = match args.bold {
        0 => BoldMode::Off,
        2 => BoldMode::All,
        _ => BoldMode::Random,
    };

    let mut user_colors: Option<UserColors> = None;
    if let Some(path) = &args.colorfile {
        match parse_user_colors(path) {
            Ok(uc) => user_colors = Some(uc),
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    }

    let mut color_scheme = match parse_color_scheme(&args.color) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    if user_colors.is_some() {
        color_scheme = ColorScheme::User;
    }

    let mut term = Terminal::new()?;
    let (w, h) = term.size()?;

    let mut cloud = Cloud::new(
        color_mode,
        args.fullwidth,
        shading_mode,
        bold_mode,
        args.async_mode,
        args.defaultbg,
        color_scheme,
        user_colors,
    );

    cloud.glitchy = !args.noglitch;
    cloud.set_glitch_pct((args.glitch_pct / 100.0).clamp(0.0, 1.0));
    cloud.set_glitch_times(args.glitch_ms.low, args.glitch_ms.high);
    cloud.set_linger_times(args.linger_ms.low, args.linger_ms.high);
    cloud.short_pct = (args.shortpct / 100.0).clamp(0.0, 1.0);
    cloud.die_early_pct = (args.rippct / 100.0).clamp(0.0, 1.0);
    cloud.set_max_droplets_per_column(args.max_droplets_per_column.clamp(1, 3));

    cloud.set_droplet_density(args.density.clamp(0.01, 5.0));
    cloud.set_chars_per_sec(args.speed.clamp(0.001, 1_000_000.0));

    let mut user_ranges: Vec<(char, char)> = Vec::new();
    if let Some(spec) = &args.chars {
        match parse_user_hex_chars(spec) {
            Ok(list) => {
                if list.len() % 2 != 0 {
                    eprintln!("--chars: odd number of unicode chars given (must be even)");
                    std::process::exit(1);
                }
                for pair in list.chunks(2) {
                    let a = pair[0];
                    let b = pair[1];
                    user_ranges.push((a, b));
                }
            }
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    }

    let charset = match charset_from_str(&args.charset, def_ascii) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    let chars = build_chars(charset, &user_ranges, def_ascii);
    cloud.init_chars(chars);
    cloud.reset(w, h);

    if let Some(msg) = &args.message {
        cloud.set_message(msg);
    }

    let mut frame = Frame::new(w, h, cloud.palette.bg);

    let target_fps = args.fps.max(1.0);
    let target_period = Duration::from_secs_f64(1.0 / target_fps);
    let mut prev = std::time::Instant::now();
    let mut prev_delay = Duration::from_millis(5);

    while cloud.raining {
        while Terminal::poll_event(Duration::from_millis(0))? {
            let ev = Terminal::read_event()?;
            match ev {
                Event::Resize(nw, nh) => {
                    cloud.reset(nw, nh);
                    frame = Frame::new(nw, nh, cloud.palette.bg);
                    cloud.force_draw_everything();
                }
                Event::Key(k) if k.kind == KeyEventKind::Press => {
                    if args.screensaver {
                        cloud.raining = false;
                        break;
                    }

                    match (k.code, k.modifiers) {
                        (KeyCode::Esc, _) => cloud.raining = false,
                        (KeyCode::Char('q'), _) => cloud.raining = false,
                        (KeyCode::Char(' '), _) => {
                            cloud.reset(frame.width, frame.height);
                            cloud.force_draw_everything();
                        }
                        (KeyCode::Char('a'), _) => {
                            cloud.set_async(!cloud.async_mode);
                        }
                        (KeyCode::Char('p'), _) => {
                            cloud.toggle_pause();
                        }
                        (KeyCode::Up, _) => {
                            let mut cps = cloud.chars_per_sec;
                            if cps <= 0.5 {
                                cps *= 2.0;
                            } else {
                                cps += 1.0;
                            }
                            cloud.set_chars_per_sec(cps.min(1000.0));
                        }
                        (KeyCode::Down, _) => {
                            let mut cps = cloud.chars_per_sec;
                            if cps <= 1.0 {
                                cps /= 2.0;
                            } else {
                                cps -= 1.0;
                            }
                            cloud.set_chars_per_sec(cps.max(0.001));
                        }
                        (KeyCode::Left, _) => {
                            if cloud.glitchy {
                                let gp = (cloud.glitch_pct - 0.05).max(0.0);
                                cloud.set_glitch_pct(gp);
                            }
                        }
                        (KeyCode::Right, _) => {
                            if cloud.glitchy {
                                let gp = (cloud.glitch_pct + 0.05).min(1.0);
                                cloud.set_glitch_pct(gp);
                            }
                        }
                        (KeyCode::Tab, _) => {
                            let sm = if cloud.shading_distance {
                                ShadingMode::Random
                            } else {
                                ShadingMode::DistanceFromHead
                            };
                            cloud.set_shading_mode(sm);
                        }
                        (KeyCode::Char('-'), _) => {
                            let d = (cloud.droplet_density - 0.25).max(0.01);
                            cloud.set_droplet_density(d);
                        }
                        (KeyCode::Char('+'), _) | (KeyCode::Char('='), KeyModifiers::SHIFT) => {
                            let d = (cloud.droplet_density + 0.25).min(5.0);
                            cloud.set_droplet_density(d);
                        }
                        (KeyCode::Char('1'), _) => cloud.set_color_scheme(ColorScheme::Green),
                        (KeyCode::Char('2'), _) => cloud.set_color_scheme(ColorScheme::Green2),
                        (KeyCode::Char('3'), _) => cloud.set_color_scheme(ColorScheme::Green3),
                        (KeyCode::Char('4'), _) => cloud.set_color_scheme(ColorScheme::Gold),
                        (KeyCode::Char('5'), _) => cloud.set_color_scheme(ColorScheme::Pink2),
                        (KeyCode::Char('6'), _) => cloud.set_color_scheme(ColorScheme::Red),
                        (KeyCode::Char('7'), _) => cloud.set_color_scheme(ColorScheme::Blue),
                        (KeyCode::Char('8'), _) => cloud.set_color_scheme(ColorScheme::Cyan),
                        (KeyCode::Char('9'), _) => cloud.set_color_scheme(ColorScheme::Purple),
                        (KeyCode::Char('0'), _) => cloud.set_color_scheme(ColorScheme::Gray),
                        (KeyCode::Char('!'), _) => cloud.set_color_scheme(ColorScheme::Rainbow),
                        (KeyCode::Char('@'), _) => cloud.set_color_scheme(ColorScheme::Yellow),
                        (KeyCode::Char('#'), _) => cloud.set_color_scheme(ColorScheme::Orange),
                        (KeyCode::Char('$'), _) => cloud.set_color_scheme(ColorScheme::Pink),
                        (KeyCode::Char('%'), _) => cloud.set_color_scheme(ColorScheme::Vaporwave),
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        cloud.rain(&mut frame);
        term.draw(&frame)?;

        let cur = std::time::Instant::now();
        let elapsed = cur.duration_since(prev);
        let calc_delay = if elapsed >= target_period {
            Duration::from_nanos(0)
        } else {
            target_period - elapsed
        };

        let cur_delay = (prev_delay.mul_f32(7.0) + calc_delay).div_f32(8.0);
        std::thread::sleep(cur_delay);
        prev = cur;
        prev_delay = cur_delay;
    }

    Ok(())
}
