<!-- SPDX-License-Identifier: GPL-3.0-only -->

# Cosmostrix Next Vision (Exploratory)

This document captures exploratory ideas for Cosmostrix's future
direction. Nothing here is committed to a release timeline. Items may
be adopted, modified, or discarded as the project evolves.

## Cosmostrix Live — Android Live Wallpaper

A potential sibling product that delivers the Cosmostrix cinematic
visual experience as a native Android live wallpaper. This is explicitly
**exploratory** and not part of any current CLI release.

### Concept

Cosmostrix Live would be a standalone Android application that renders
cinematic visual effects directly on the Android home screen via
`WallpaperService`. Unlike terminal-based Cosmostrix, this would use a
native Android rendering surface, not a terminal emulator or command-line
interface.

### Technical Direction (Exploratory)

- **Native renderer:** Android `WallpaperService` with `Canvas` or Vulkan
  for GPU-accelerated rendering. Not a terminal emulator wrapping the CLI.
- **Platform-native UX:** Settings exposed through Android's live
  wallpaper picker and a companion configuration activity, not CLI flags.
- **Performance target:** Smooth 60 FPS rendering on mid-range Android
  devices with minimal battery impact.

### Business Model (Exploratory)

- **One-time lifetime unlock** is the preferred model. Users pay once
  and receive all future updates for that major version.
- **Play Store distribution** is the anticipated channel, but no store
  listing has been created and no timeline is set.

### Relationship to CLI

- **Sibling product, not a module.** Cosmostrix Live would live in its
  own repository with its own versioning, its own release cadence, and
  its own dependency tree.
- **Shared visual identity.** The visual language (rain aesthetics,
  atmospheric effects, cinematic pacing) would be consistent between
  the CLI and Cosmostrix Live, but the implementation would be fully
  independent.
- **No code sharing requirement.** The Android renderer would be built
  from scratch for the platform. No Rust code from the CLI repository
  would be compiled for Android in this phase.

### Current Status

This is a **saved idea** only. No Android code exists in the Cosmostrix
CLI repository. No Cosmostrix Live repository has been created. No
development timeline has been set. The idea is documented here so that
the boundary is clear: Android implementation is not part of the main
CLI roadmap.

## Other Exploratory Ideas

### Web Renderer

A browser-based cinematic renderer using WebGL or WebGPU that could
serve as an interactive demo or embeddable widget. No work has started.

### Config Sharing

A community preset sharing mechanism where users can export and import
atmosphere configurations. This would likely take the form of a shared
repository of TOML config snippets. No work has started.

### macOS/iOS Live Wallpaper

Extending the Cosmostrix Live concept to Apple platforms. No work has
started and no platform-specific investigation has been done.