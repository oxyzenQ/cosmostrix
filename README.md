# Cosmostrix

Cosmostrix is a terminal "Matrix rain" visualizer written in Rust.

It is a clean-room Rust migration of an older ncurses-based terminal project.

## Features

- Multiple built-in color schemes + user-defined palettes (`--colorfile`)
- Configurable speed, density, FPS, glitching, shading, and boldness
- Unicode character sets (`--charset`) and custom ranges (`--chars`)
- Screensaver mode (`--screensaver`)
- Runs in **alternate screen** and **raw mode** (no scrollback spam)

## Requirements

- Rust toolchain (stable) to build from source
- A terminal that supports ANSI escape sequences, alternate screen, and raw mode
- Best results with 256-color or truecolor terminals

Cosmostrix is intended for Unix-like systems (Linux, BSD, macOS, and similar). It uses `crossterm`, so it can also be built on Windows in many setups, but Windows is not the primary target.

## Quickstart

Run directly from source:

```bash
cargo run -- --help
cargo run -- --color green --fps 60 --speed 10
```

Build a release binary:

```bash
cargo build --release
```

Run the built binary:

```bash
# Linux/macOS
./target/release/cosmostrix --help

# Windows (PowerShell)
.\target\release\cosmostrix.exe --help
```

## Installation

### From GitHub Releases

Download the `.tar.xz` archive for your OS/arch from Releases, extract it, and place `cosmostrix` somewhere in your `PATH`.

### From source (recommended)

```bash
cargo install --path .
cosmostrix --help
```

### Manual install (Linux example)

```bash
cargo build --release
install -Dm755 ./target/release/cosmostrix ~/.local/bin/cosmostrix
```

## Usage

Common examples:

```bash
# default settings
cosmostrix

# color + speed
cosmostrix --color rainbow --speed 12

# tune visuals
cosmostrix --density 1.5 --fps 30 --shadingmode 1 --bold 2

# disable glitching
cosmostrix --noglitch

# screensaver: exit on first keypress
cosmostrix --screensaver

# overlay message
cosmostrix --message "wake up, neo"

# character sets
cosmostrix --charset katakana
cosmostrix --charset braille

# custom unicode ranges (hex code points, pairs define inclusive ranges)
cosmostrix --chars 30,39,41,5A
```

## CLI options

These flags match the current Rust implementation (`src/config.rs`).

```text
 -a, --async                  enable async column speeds
 -b, --bold <NUM>             0=off, 1=random, 2=all
 -C, --colorfile <FILE>       load user colors from file (legacy-compatible format)
 -c, --color <COLOR>          color scheme (default: green)
 -D, --defaultbg              use terminal default background color
 -d, --density <NUM>          droplet density (default: 1.0)
 -F, --fullwidth              use two columns per character
 -f, --fps <NUM>              target FPS (default: 60)
 -g, --glitchms <LO,HI>       glitch timing range in ms (default: 300,400)
 -G, --glitchpct <PCT>        glitch chance percent (default: 10)
 -l, --lingerms <LO,HI>       linger timing range in ms (default: 1,3000)
 -M, --shadingmode <NUM>      0=random, 1=distance-from-head (default: 0)
 -m, --message <TEXT>         overlay message
     --maxdpc <NUM>           max droplets per column (clamped to 1..3, default: 3)
     --noglitch               disable glitch
 -r, --rippct <PCT>           die-early percent (default: 33.33333)
 -S, --speed <NUM>            chars per second (default: 8)
 -s, --screensaver            exit on first keypress
     --shortpct <PCT>         short droplet percent (default: 50)
     --charset <NAME>         character set (default: auto)
     --chars <HEX...>         custom unicode hex ranges (pairs)
     --colormode <MODE>       force color mode (0, 16, 256, 32)
     --info                   print build info and exit
```

## Color schemes

`--color` supports:

`user`, `green`, `green2`, `green3`, `gold`, `yellow`, `orange`, `red`, `blue`, `cyan`, `purple`, `pink`, `pink2`, `vaporwave`, `gray`, `rainbow`

`gray` also accepts `grey`.

If `--colorfile` is provided, Cosmostrix automatically switches to `user` color scheme.

## User color file (`--colorfile`)

- File is parsed line-by-line; empty lines are ignored.
- Lines starting with `;`, `#`, `/`, `*`, `@` are treated as comments.
- Each non-comment line is a comma-separated record:
  - `INDEX` (0-255) OR
  - `INDEX, R, G, B` where `R/G/B` are 0..1000 (used for truecolor)
- The first record is used as the background color (unless `--defaultbg` is set).
- At least 2 colors are required.

Example:

```text
# background
16, 0, 0, 0
# droplets
46, 0, 1000, 0
82, 200, 1000, 200
231, 1000, 1000, 1000
```

## Charset (`--charset`) and custom ranges (`--chars`)

Built-in charsets:

`auto`, `ascii`, `extended`, `english`, `digits`, `punc`, `bin`, `hex`, `katakana`, `greek`, `cyrillic`, `arabic`, `hebrew`, `devanagari`, `braille`, `runic`

- `auto` chooses between `ascii` and `extended` using the `LANG` environment variable (falls back to ASCII when `LANG` is unset).
- `--chars` takes comma-separated *hex* unicode code points, and the list length must be even. Each pair defines an inclusive range.

Example: digits + uppercase letters

```bash
cosmostrix --chars 30,39,41,5A
```

## Color mode (`--colormode`)

If `--colormode` isn't set, Cosmostrix tries to detect terminal capabilities:

- `COLORTERM` contains `truecolor` / `24bit` -> truecolor
- `TERM` contains `256color` -> 256-color
- otherwise -> 16-color

You can override with:

- `--colormode 0` (mono)
- `--colormode 16`
- `--colormode 256`
- `--colormode 32` (truecolor)

## Runtime controls (keys)

Controls are handled in `src/main.rs`:

```text
 Esc / q        quit
 Space          reset
 a              toggle async mode
 p              pause/unpause
 Up/Down        change speed
 Left/Right     change glitch percent
 Tab            toggle shading mode
 -              decrease density
 + / =          increase density

 1              green
 2              green2
 3              green3
 4              gold
 5              pink2
 6              red
 7              blue
 8              cyan
 9              purple
 0              gray
 !              rainbow
 @              yellow
 #              orange
 $              pink
 %              vaporwave
```

## Development

```bash
cargo test --all
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
```

## Release process

This repo includes a GitHub Actions workflow that can build `.tar.xz` release packages for Linux/macOS and publish a GitHub Release.

- Triggered by either:
  - a manual workflow dispatch with a `version`, or
  - a commit to `main` whose message starts with `release: X.Y.Z`

The workflow will tag `vX.Y.Z` and upload `dist/cosmostrix-<version>-<target>.tar.xz`.

## Contributing

PRs and issues are welcome. Please run `cargo fmt` and `cargo clippy` before submitting.

## License

No license file is currently included in this repository.

## Notes

- **Terminal compatibility**: best results in modern terminals with 256-color or truecolor support.
- **UTF-8**: Cosmostrix can use Unicode character sets depending on your locale and `--charset`.
