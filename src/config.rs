// Copyright (c) 2026 rezky_nightky

use std::io::IsTerminal;
use std::str::FromStr;

use clap::Parser;

pub const DEFAULT_PARAMS_USAGE: &str = "DEFAULT PARAMS USAGE:\n  cosmostrix --duration 0 --color-bg black --color green --charset binary --fps 60 --speed 8 --density 1 --maxdpc 3 --bold 1 --shadingmode 0 --glitchpct 10 --glitchms 300,400 --lingerms 1,3000 --shortpct 50 --rippct 33.33333";

pub fn color_enabled_stdout() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if matches!(std::env::var("CLICOLOR").ok().as_deref(), Some("0")) {
        return false;
    }
    std::io::stdout().is_terminal()
}

fn colorize_help_detail(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    for chunk in text.split_inclusive('\n') {
        let (line, nl) = chunk
            .strip_suffix('\n')
            .map(|l| (l, "\n"))
            .unwrap_or((chunk, ""));

        let is_heading =
            !line.starts_with(' ') && line.ends_with(':') && line == line.to_ascii_uppercase();

        if is_heading {
            out.push_str("\x1b[1;36m");
            out.push_str(line);
            out.push_str("\x1b[0m");
            out.push_str(nl);
            continue;
        }

        if let Some(rest) = line.strip_prefix("      Example:") {
            out.push_str("      \x1b[32mExample:\x1b[0m");
            out.push_str(rest);
            out.push_str(nl);
            continue;
        }

        if let Some(rest) = line.strip_prefix("  cosmostrix") {
            out.push_str("  \x1b[1;34mcosmostrix\x1b[0m");
            out.push_str(rest);
            out.push_str(nl);
            continue;
        }

        if let Some(rest) = line.strip_prefix("  -") {
            out.push_str("  \x1b[33m-");
            out.push_str(rest);
            out.push_str("\x1b[0m");
            out.push_str(nl);
            continue;
        }

        if let Some(rest) = line.strip_prefix("  --") {
            out.push_str("  \x1b[33m--");
            out.push_str(rest);
            out.push_str("\x1b[0m");
            out.push_str(nl);
            continue;
        }

        out.push_str(line);
        out.push_str(nl);
    }
    out
}

pub fn default_params_usage_for_help() -> String {
    if color_enabled_stdout() {
        colorize_help_detail(DEFAULT_PARAMS_USAGE)
    } else {
        DEFAULT_PARAMS_USAGE.to_string()
    }
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorBg {
    #[value(name = "black")]
    Black,
    #[value(name = "default-background")]
    DefaultBackground,
    #[value(name = "transparent")]
    Transparent,
}

#[derive(Clone, Copy, Debug)]
pub struct U16Range {
    pub low: u16,
    pub high: u16,
}

impl FromStr for U16Range {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (a, b) = s
            .split_once(',')
            .ok_or_else(|| "expected: NUM1,NUM2".to_string())?;
        let low: u16 = a
            .trim()
            .parse()
            .map_err(|_| "invalid low value".to_string())?;
        let high: u16 = b
            .trim()
            .parse()
            .map_err(|_| "invalid high value".to_string())?;
        if low == 0 || high == 0 || low > high {
            return Err("range must be >0 and low <= high".to_string());
        }
        Ok(Self { low, high })
    }
}

#[derive(Parser, Debug, Clone)]
#[command(name = "cosmostrix", version, disable_version_flag = true)]
pub struct Args {
    #[arg(
        short = 'a',
        long = "async",
        help_heading = "GENERAL",
        help = "Enable async rendering"
    )]
    pub async_mode: bool,

    #[arg(
        short = 'b',
        long = "bold",
        default_value_t = 1,
        help_heading = "APPEARANCE",
        help = "Bold mode (min 0 max 2): 0=off, 1=random, 2=all"
    )]
    pub bold: u8,

    #[arg(
        short = 'c',
        long = "color",
        default_value = "green",
        help_heading = "APPEARANCE",
        help = "Color theme (see --list-colors)"
    )]
    pub color: String,

    #[arg(
        long = "color-bg",
        default_value_t = ColorBg::Black,
        value_enum,
        help_heading = "APPEARANCE",
        help = "Background mode (black, default-background, transparent)"
    )]
    pub color_bg: ColorBg,

    #[arg(
        short = 'd',
        long = "density",
        default_value_t = 1.0,
        help_heading = "PERFORMANCE",
        help = "Droplet density (min 0.01 max 5.0)"
    )]
    pub density: f32,

    #[arg(
        short = 'F',
        long = "fullwidth",
        help_heading = "GENERAL",
        help = "Use full terminal width"
    )]
    pub fullwidth: bool,

    #[arg(
        short = 'f',
        long = "fps",
        default_value_t = 60.0,
        help_heading = "PERFORMANCE",
        help = "Target FPS (min 1 max 240)"
    )]
    pub fps: f64,

    #[arg(
        long = "duration",
        help_heading = "GENERAL",
        help = "Stop after N seconds (min 0.1 max 86400; <=0 disables)"
    )]
    pub duration: Option<f64>,

    #[arg(
        long = "perf-stats",
        help_heading = "PERFORMANCE",
        help = "Print performance statistics summary on exit"
    )]
    pub perf_stats: bool,

    #[arg(
        short = 'g',
        long = "glitchms",
        default_value = "300,400",
        help_heading = "GLITCH (ADVANCED)",
        help = "Glitch duration range in ms: LOW,HIGH (min 1 max 5000)"
    )]
    pub glitch_ms: U16Range,

    #[arg(
        short = 'G',
        long = "glitchpct",
        default_value_t = 10.0,
        help_heading = "GLITCH (ADVANCED)",
        help = "Glitch chance in percent (min 0 max 100)"
    )]
    pub glitch_pct: f32,

    #[arg(
        short = 'l',
        long = "lingerms",
        default_value = "1,3000",
        help_heading = "GLITCH (ADVANCED)",
        help = "Linger time range in ms: LOW,HIGH (min 1 max 60000)"
    )]
    pub linger_ms: U16Range,

    #[arg(
        short = 'M',
        long = "shadingmode",
        default_value_t = 0,
        help_heading = "APPEARANCE",
        help = "Shading mode (min 0 max 1): 0=random, 1=distance-from-head"
    )]
    pub shading_mode: u8,

    #[arg(
        short = 'm',
        long = "message",
        help_heading = "GENERAL",
        help = "Overlay message"
    )]
    pub message: Option<String>,

    #[arg(
        long = "message-no-border",
        help_heading = "GENERAL",
        help = "Draw message box without border (use with --message; shorthand: -mB)"
    )]
    pub message_no_border: bool,

    #[arg(
        long = "maxdpc",
        default_value_t = 3,
        help_heading = "PERFORMANCE",
        help = "Max droplets per column (min 1 max 3)"
    )]
    pub max_droplets_per_column: u8,

    #[arg(
        long = "noglitch",
        help_heading = "GLITCH (ADVANCED)",
        help = "Disable glitch effects"
    )]
    pub noglitch: bool,

    #[arg(
        short = 'r',
        long = "rippct",
        default_value_t = 33.33333,
        help_heading = "GLITCH (ADVANCED)",
        help = "Die-early chance in percent (min 0 max 100)"
    )]
    pub rippct: f32,

    #[arg(
        short = 'S',
        long = "speed",
        default_value_t = 8.0,
        help_heading = "PERFORMANCE",
        help = "Characters per second (min 0.001 max 1000)"
    )]
    pub speed: f32,

    #[arg(
        short = 's',
        long = "screensaver",
        help_heading = "GENERAL",
        help = "Screensaver mode (exit on keypress)"
    )]
    pub screensaver: bool,

    #[arg(
        long = "shortpct",
        default_value_t = 50.0,
        help_heading = "GLITCH (ADVANCED)",
        help = "Chance for short droplets in percent (min 0 max 100)"
    )]
    pub shortpct: f32,

    #[arg(
        long = "charset",
        default_value = "binary",
        help_heading = "CHARSET",
        help = "Charset preset (see --list-charsets)"
    )]
    pub charset: String,

    #[arg(
        long = "chars",
        help_heading = "CHARSET",
        help = "Custom characters override"
    )]
    pub chars: Option<String>,

    #[arg(
        long = "colormode",
        help_heading = "APPEARANCE",
        help = "Force color mode (allowed: 0,16,8/256,24/32). Default: 24-bit if supported (COLORTERM), else 8-bit (TERM=...256color), else 16-color"
    )]
    pub colormode: Option<u16>,

    #[arg(
        long = "check-bitcolor",
        help_heading = "HELP",
        help = "Print detected terminal color capability and exit"
    )]
    pub check_bitcolor: bool,

    #[arg(
        long = "help-detail",
        help_heading = "HELP",
        help = "Show detailed help for all parameters and exit"
    )]
    pub help_detail: bool,

    #[arg(
        long = "list-charsets",
        help_heading = "HELP",
        help = "List available charset presets and exit"
    )]
    pub list_charsets: bool,

    #[arg(
        long = "list-colors",
        help_heading = "HELP",
        help = "List available color themes and exit"
    )]
    pub list_colors: bool,

    #[arg(
        long = "info",
        short = 'i',
        help_heading = "HELP",
        help = "Print version info and exit"
    )]
    pub info: bool,

    #[arg(
        long = "version",
        short = 'v',
        help_heading = "HELP",
        help = "Print version and exit"
    )]
    pub version: bool,
}

pub fn print_list_charsets() {
    if color_enabled_stdout() {
        println!("\x1b[1;36mAVAILABLE CHARSET PRESETS:\x1b[0m");
        println!("\x1b[2mNOTE: Use only the VALUE (left side) with --charset.\x1b[0m");
    } else {
        println!("AVAILABLE CHARSET PRESETS:");
        println!("NOTE: Use only the VALUE (left side) with --charset.");
    }
    println!();
    println!("VALUE        DESCRIPTION");
    println!("auto         Auto-select (ASCII_SAFE when non-UTF, otherwise matrix)");
    println!("matrix       Letters + digits + katakana (no punctuation)");
    println!("ascii        Letters + digits + punctuation");
    println!("extended     Digits + punctuation + katakana");
    println!("english      Letters only");
    println!("digits       Digits only (aliases: dec, decimal)");
    println!("punc         Punctuation only");
    println!("binary       0 and 1 (aliases: bin, 01)");
    println!("hex          0-9 and A-F (alias: hexadecimal)");
    println!("katakana     Katakana");
    println!("greek        Greek");
    println!("cyrillic     Cyrillic");
    println!("hebrew       Hebrew");
    println!("blocks       Block elements (shading blocks)");
    println!("symbols      Math/technical symbols");
    println!("arrows       Arrow symbols");
    println!("retro        Box-drawing characters");
    println!("cyberpunk    Katakana + hex + symbols (combo)");
    println!("hacker       Letters + hex + punc + symbols (combo)");
    println!("minimal      Dots and simple shapes");
    println!("code         Letters + digits + punc + symbols (combo)");
    println!("dna          DNA bases (ACGT)");
    println!("braille      Braille");
    println!("runic        Runic");
}

pub fn print_list_colors() {
    if color_enabled_stdout() {
        println!("\x1b[1;36mAVAILABLE COLOR THEMES:\x1b[0m");
        println!("\x1b[2mNOTE: Use only the VALUE (left side) with --color.\x1b[0m");
    } else {
        println!("AVAILABLE COLOR THEMES:");
        println!("NOTE: Use only the VALUE (left side) with --color.");
    }
    println!();
    println!("VALUE        DESCRIPTION");
    println!("green        Green theme");
    println!("green2       Green variant");
    println!("green3       Green variant");
    println!("yellow       Yellow theme");
    println!("orange       Orange theme");
    println!("red          Red theme");
    println!("blue         Blue theme");
    println!("cyan         Cyan theme");
    println!("gold         Gold theme");
    println!("rainbow      Rainbow theme");
    println!("purple       Purple theme");
    println!("neon         Neon theme (alias: synthwave)");
    println!("fire         Fire theme (alias: inferno)");
    println!("ocean        Ocean theme (alias: deep-sea)");
    println!("forest       Forest theme (alias: jungle)");
    println!("vaporwave    Vaporwave theme");
    println!("spectrum20   Spectrum 20-color theme (aliases: theme20, spectrum-20)");
    println!("gray         Gray theme (alias: grey)");
    println!("snow         Snow / ice theme");
    println!("aurora       Aurora theme");
    println!("fancy-diamond Fancy diamond theme");
    println!("cosmos       Cosmos theme");
    println!("nebula       Nebula theme");
    println!("stars        Stars theme");
    println!("mars         Mars theme");
    println!("venus        Venus theme");
    println!("mercury      Mercury theme");
    println!("jupiter      Jupiter theme");
    println!("saturn       Saturn theme");
    println!("uranus       Uranus theme");
    println!("neptune      Neptune theme");
    println!("pluto        Pluto theme");
    println!("moon         Moon theme");
    println!("sun          Sun theme");
    println!("comet        Comet theme");
    println!("galaxy       Galaxy theme");
    println!("supernova    Supernova theme");
    println!("blackhole    Black hole theme");
    println!("andromeda    Andromeda theme");
    println!("stardust     Stardust theme");
    println!("meteor       Meteor theme");
    println!("eclipse      Eclipse theme");
    println!("deepspace    Deep space theme");
}

pub fn print_help_detail() {
    let block = format!(
        "{}\n\nUSAGE:\n  cosmostrix [OPTIONS]\n\nGENERAL:\n  -a, --async\n      Enable async rendering.\n      Example: cosmostrix -a\n\n  -s, --screensaver\n      Screensaver mode (exit on keypress).\n      Example: cosmostrix -s\n\n  -F, --fullwidth\n      Use full terminal width.\n      Example: cosmostrix -F\n\n  --duration <seconds>\n      Stop after N seconds (min 0.1 max 86400).\n      Example: cosmostrix --duration 10\n\n  --check-bitcolor\n      Print detected terminal color capability and exit.\n      Example: cosmostrix --check-bitcolor\n\n  -m, --message <text>\n      Overlay message.\n      Example: cosmostrix -m \"hello\"\n\nAPPEARANCE:\n  -c, --color <name>\n      Set theme (see --list-colors).\n      Example: cosmostrix --color rainbow\n\n  --colormode <0|8|24>\n      Force color mode; otherwise auto-detected from COLORTERM/TERM.\n      Example: cosmostrix --colormode 24\n\n  -b, --bold <0|1|2>\n      Bold style (0 off, 1 random, 2 all).\n      Example: cosmostrix --bold 2\n\n  -M, --shadingmode <0|1>\n      Shading (0 random, 1 distance-from-head).\n      Example: cosmostrix -M 1\n\n  --color-bg <black|default-background|transparent>\n      Background mode.\n      Example: cosmostrix --color-bg transparent\n\nPERFORMANCE:\n  -f, --fps <number>\n      Target FPS (min 1 max 240).\n      Example: cosmostrix --fps 30\n\n  -S, --speed <number>\n      Characters per second (rain speed) (min 0.001 max 1000).\n      Example: cosmostrix --speed 12\n\n  -d, --density <number>\n      Droplet density (min 0.01 max 5.0).\n      Example: cosmostrix --density 1.25\n\n  --maxdpc <number>\n      Max droplets per column (min 1 max 3).\n      Example: cosmostrix --maxdpc 2\n\n  --perf-stats\n      Print performance statistics summary on exit.\n      Example: cosmostrix --duration 10 --perf-stats\n\nCHARSET:\n  --charset <name>\n      Charset preset (see --list-charsets).\n      Example: cosmostrix --charset binary\n\n  --chars <string>\n      Custom character override (advanced).\n      Example: cosmostrix --chars \"01\"\n\nGLITCH (ADVANCED):\n  --noglitch\n      Disable glitch effects.\n      Example: cosmostrix --noglitch\n\n  -G, --glitchpct <number>\n      Glitch chance in percent (min 0 max 100).\n      Example: cosmostrix --glitchpct 5\n\n  -g, --glitchms <low,high>\n      Glitch duration range in ms (min 1 max 5000).\n      Example: cosmostrix --glitchms 200,500\n\n  -l, --lingerms <low,high>\n      Linger duration range in ms (min 1 max 60000).\n      Example: cosmostrix --lingerms 1,3000\n\n  --shortpct <number>\n      Short droplet chance in percent (min 0 max 100).\n      Example: cosmostrix --shortpct 40\n\n  -r, --rippct <number>\n      Die-early chance in percent (min 0 max 100).\n      Example: cosmostrix --rippct 20\n\nHELP:\n  --check-bitcolor\n      Print detected terminal color capability and exit.\n\n  --help\n      Show short help.\n\n  --help-detail\n      Show this detailed help.\n\n  --list-charsets\n      List available charset presets and exit.\n\n  --list-colors\n      List available color themes and exit.\n\n  -v, --version\n      Print version and exit.\n\n  -i, --info\n      Print version info and exit.\n",
        DEFAULT_PARAMS_USAGE
    );

    if color_enabled_stdout() {
        print!("{}", colorize_help_detail(&block));
    } else {
        print!("{}", block);
    }

    let tail = "\nVALUE LISTS:\n  cosmostrix --list-charsets\n  cosmostrix --list-colors\n\nMESSAGE BOX:\n  --message-no-border, -mB\n      Draw filled box without border characters\n\nLIMITS / VALID RANGES:\n";
    if color_enabled_stdout() {
        print!("{}", colorize_help_detail(tail));
    } else {
        print!("{}", tail);
    }
    println!("  --duration <seconds>     min 0.1 max 86400 (<=0 disables)");
    println!("  --perf-stats             print performance summary on exit");
    println!("  --fps <number>           min 1 max 240");
    println!("  --speed <number>         min 0.001 max 1000");
    println!("  --density <number>       min 0.01 max 5.0");
    println!("  --maxdpc <number>        min 1 max 3");
    println!("  --glitchpct <number>     min 0 max 100");
    println!("  --shortpct <number>      min 0 max 100");
    println!("  --rippct <number>        min 0 max 100");
    println!("  --glitchms <low,high>    min 1 max 5000 (each)");
    println!("  --lingerms <low,high>    min 1 max 60000 (each)");
    println!("  --bold <0|1|2>           min 0 max 2");
    println!("  --shadingmode <0|1>      min 0 max 1");
    println!("  --colormode <0|16|8|24>  allowed values only (8==256, 24==32)");
    println!();
    print_list_charsets();
    println!();
    print_list_colors();
}
