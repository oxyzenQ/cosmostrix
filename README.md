# Cosmostrix

A cosmic take on the classic "Matrix rain" effect — rewritten for modern terminals in Rust.

> A lightweight, configurable terminal visualizer that paints cascading characters across your terminal like cosmic rain.

[![Crates.io](https://img.shields.io/badge/crates.io-none-lightgrey)](https://crates.io/) <!-- replace when published -->
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE) <!-- update if different -->
[![Rust](https://img.shields.io/badge/built%20with-rust-000000.svg)](https://www.rust-lang.org)

Table of contents
- What is Cosmostrix?
- Demo / Screenshots
- Features
- Installation
- Usage
- Configuration & Examples
- Tips & Troubleshooting
- Contributing
- License
- Acknowledgements

What is Cosmostrix?
-------------------
Cosmostrix is a small Rust-powered terminal program that recreates the cascading "Matrix" rain effect with a modern, customizable twist: color palettes, speed/density control, and terminal-friendly performance.

It's ideal for:
- Terminal backgrounds for streaming/screenshots
- Ambient terminal visuals
- Learning a bit of Rust + terminal rendering

Demo / Screenshots
------------------
Include an animated GIF or a short video here for the best first impression.

Example (static ASCII snapshot)

    █▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒█
    █ █ █ █ █ █ █ █ █ █ █ █ █
      ░░░░░░░░░░░░░░░░░░░░░

(Replace the above with an actual GIF or link to demo in the repo's `assets/` directory or Releases.)

Features
--------
- Minimal, fast, and terminal-native (no GUI).
- Rust-based for safety and performance.
- Configurable speed, density, and palettes.
- Works in most modern terminals that support ANSI colors.
- Low CPU usage — suitable for background visuals.

Installation
------------
Binary builds may be available under Releases. If a release is not present, you can build from source.

From a release (recommended when available)
1. Download the appropriate release for your OS from the Releases page.
2. Extract and move the binary to a directory in your PATH, for example `/usr/local/bin/`.

Build from source (requires Rust toolchain)
```bash
# Install Rust toolchain if you don't have it:
# https://rustup.rs
git clone https://github.com/oxyzenQ/cosmostrix.git
cd cosmostrix
cargo build --release
# Binary will be at:
# target/release/cosmostrix
```

Install via cargo (if published to crates.io)
```bash
cargo install --git https://github.com/oxyzenQ/cosmostrix.git --bin cosmostrix
```

Usage
-----
Run the binary from your terminal:

```bash
cosmostrix
```

Most terminal programs support a `--help` flag. If available, run:

```bash
cosmostrix --help
```

Common usage patterns
- Run in full-screen terminal for maximum effect.
- Pipe into a multiplexer (tmux, screen) session to keep it running.
- Use `ctrl+c` to stop (or the terminal's normal interrupt).

Configuration & Examples
------------------------
Cosmostrix aims to be simple and unobtrusive. You can customize how the rain looks and behaves.

Suggested configuration options (check `--help` for exact flags / names):
- speed: How fast the characters fall.
- density: How many falling columns are active.
- palette/theme: Choose colors (classic green, neon, pastel, etc).
- charset: Which characters to display (ASCII, Unicode, custom string).

Example (hypothetical; check the real CLI flags):
```bash
# Faster, denser, classic green palette
cosmostrix --speed 1.5 --density 0.9 --palette classic
```

Example config file (TOML)
```toml
# ~/.config/cosmostrix/config.toml
speed = 1.0
density = 0.6
palette = "classic"
charset = "abcdefghijklmnopqrstuvwxyz0123456789"
```

If configuration file support is available, place it in:
- Linux/macOS: `~/.config/cosmostrix/config.toml`
- Windows: `%APPDATA%\cosmostrix\config.toml`

Tips & Troubleshooting
----------------------
- If colors look off, ensure your terminal supports at least 256 colors or truecolor.
- If performance is poor, try reducing density and/or speed.
- Use a compositor or terminal emulator that supports hardware-accelerated rendering for smoother visuals.
- If the program doesn't run: make sure the binary is executable (`chmod +x cosmostrix`) and that dependencies were built successfully.

Contributing
------------
Contributions are welcome!

If you'd like to help:
1. Open an issue to discuss major changes or features.
2. Fork the repo and create a branch for your feature/fix.
3. Make small, focused pull requests and include tests where appropriate.
4. Follow the existing coding style and add documentation for new features.

Suggested areas to help:
- Add more palettes/themes.
- Add unit/integration tests for rendering logic.
- Improve CLI ergonomics and add config-file support (if missing).
- Add Windows terminal-specific fixes or enhancements.

License
-------
This project is provided under the MIT license. See the LICENSE file for details. (Update this section if a different license applies.)

Acknowledgements
----------------
- Inspiration: The classic "Matrix" rain screensavers and terminal art.
- Built with Rust and terminal libraries — thank you to the maintainers of those crates.

Contact / Maintainer
--------------------
- Maintained by oxyzenQ
- Repo: https://github.com/oxyzenQ/cosmostrix

If you'd like, I can:
- Flesh out examples using the actual CLI flags (run `cosmostrix --help` in your repo or tell me the exact options).
- Add a copy-ready GIF or screenshot section if you provide an image or release URL.
- Draft a CONTRIBUTING.md with more specific development and testing steps.
