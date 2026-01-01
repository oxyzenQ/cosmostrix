# Cosmostrix

Cosmostrix is a terminal "Matrix rain" visualizer written in Rust.

It is a clean-room Rust migration of an older ncurses-based terminal project.

## Demo

Watch the demo on YouTube:

<div align="center">
  <a href="https://www.youtube.com/watch?v=VIDEO_ID">
    <img src="https://img.youtube.com/vi/VIDEO_ID/maxresdefault.jpg" alt="Cosmostrix demo" width="80%" />
  </a>

  <strong><a href="https://www.youtube.com/watch?v=VIDEO_ID">▶ Watch Demo on YouTube</a></strong>
</div>

## Performance & benchmarking

See `benchmark/README.md` for profiling artifacts and a reproducible benchmark script.

## Features

- Multiple built-in color schemes
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
 -c, --color <COLOR>          color scheme (default: green)
     --color-bg <MODE>        background: black, default-background, transparent (default: black)
 -d, --density <NUM>          droplet density (default: 1.0)
 -F, --fullwidth              use two columns per character
 -f, --fps <NUM>              target FPS (default: 60)
     --duration <SECONDS>      exit after N seconds (useful for benchmarks)
 -g, --glitchms <LO,HI>       glitch timing range in ms (default: 300,400)
 -G, --glitchpct <PCT>        glitch chance percent (default: 10)
 -l, --lingerms <LO,HI>       linger timing range in ms (default: 1,3000)
 -M, --shadingmode <NUM>      0=random, 1=distance-from-head (default: 0)
 -m, --message <TEXT>         overlay message
     --maxdpc <NUM>           max droplets per column (min 1 max 3, default: 3)
     --noglitch               disable glitch
 -r, --rippct <PCT>           die-early percent (default: 33.33333)
 -S, --speed <NUM>            chars per second (default: 8)
 -s, --screensaver            exit on first keypress
     --shortpct <PCT>         short droplet percent (default: 50)
     --charset <NAME>         character set (default: binary)
     --chars <HEX...>         custom unicode hex ranges (pairs)
     --colormode <MODE>       force color mode (0, 8, 24)
     --check-bitcolor          print detected terminal color capability and exit
     --info                   print version info and exit
```

## Color schemes

`--color` supports:

`green`, `green2`, `green3`, `gold`, `yellow`, `orange`, `red`, `blue`, `cyan`, `purple`, `neon`, `fire`, `ocean`, `forest`, `vaporwave`, `gray`, `snow`, `aurora`, `fancy-diamond`, `cosmos`, `nebula`, `rainbow`

`gray` also accepts `grey`.

## Charset (`--charset`) and custom ranges (`--chars`)

Built-in charsets:

`auto`, `matrix`, `ascii`, `extended`, `english`, `digits`, `punc`, `binary`, `hex`, `katakana`, `greek`, `cyrillic`, `hebrew`, `blocks`, `symbols`, `arrows`, `retro`, `cyberpunk`, `hacker`, `minimal`, `code`, `dna`, `braille`, `runic`

- `binary` also accepts `bin` and `01`.
- `auto` chooses a safe charset based on `LANG`:
  - if `LANG` does **not** contain `UTF`, it uses a safe ASCII set (letters + digits)
  - otherwise it uses `matrix`.
- `--chars` takes comma-separated *hex* unicode code points, and the list length must be even. Each pair defines an inclusive range.

Example: digits + uppercase letters

```bash
cosmostrix --chars 30,39,41,5A
```

## Color mode (`--colormode`)

If `--colormode` isn't set, Cosmostrix tries to detect terminal capabilities:

- `COLORTERM` contains `truecolor` / `24bit` -> truecolor
- `TERM` contains `256color` -> 256-color
- `TERM` equals `dumb` -> mono
- otherwise -> 256-color

To inspect what Cosmostrix detects on your system:

- `cosmostrix --check-bitcolor`

You can override with:

- `--colormode 0` (mono)
- `--colormode 8` (256-color)
- `--colormode 24` (truecolor)

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
 5              neon
 6              red
 7              blue
 8              cyan
 9              purple
 0              gray
 !              rainbow
 @              yellow
 #              orange
 $              fire
 %              vaporwave
```

## Development

```bash
cargo test --all
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
```

## Release process

Create a release by pushing a `v*` git tag (this triggers the GitHub Actions Release workflow).

```bash
# 1) Update Cargo.toml version
# 2) Commit the version bump
git commit -am "release: 1.0.1-stable.1"

# 3) Tag and push
git tag -a v1.0.1-stable.1 -m v1.0.1-stable.1
git push origin v1.0.1-stable.1
```

See `workflow/about-ci.md` for details.

## Contributing

PRs and issues are welcome. Please run `cargo fmt` and `cargo clippy` before submitting.

## License

MIT. See `LICENSE`.

## Notes

- **Terminal compatibility**: best results in modern terminals with 256-color or truecolor support.
- **UTF-8**: Cosmostrix can use Unicode character sets depending on your locale and `--charset`.
