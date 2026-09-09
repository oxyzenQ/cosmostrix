// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Scene catalog and scene-to-runtime mappings.
//!
//! Scenes map curated runtime knobs and internal rain style selection while
//! preserving palette override behavior.
//!
//! ## Catalog
//!
//! Built-in scenes combine the three core runtime styles (`matrix`,
//! `monolith`, `signal` — the original "rain atmospheres" naming predates
//! the v4.0.0 atmosphere engine and is unrelated to that now-eliminated
//! subsystem) with the eleven structured style flagships (`vortex`,
//! `flux`, `lorenz`, `cosmic_dragon`, `physarum`,
//! `sorgonemous_intrascals`, `aeolian`, `solar_flare`, `dna_helix`,
//! `murmuration`, `quasar` — task-18/19 +
//! NIGHT-research-4/5/6 + NIGHT-special-1/2/4 +
//! NIGHT-research-7/8, the signature
//! differentiators), nine curated
//! visual scenes (`classic`, `cinematic`, `calm`, `storm`, `cosmos`,
//! `neon`, `hacker`, `matrix_film`, `low-power`), the `cosmic-dragon`
//! milestone scene commemorating the temporal-prediction breakthrough
//! ( dirty_ratio 18.33% → 0.39%, FPS 7,843 → 29,773), the
//! `dragon_hunt` milestone scene commemorating the biggest bug hunt —
//! the "glitch rain shift" run to ground by the NIGHT hunters
//! (NIGHT-hunter-15), the tribute
//! and honor destinations (`carbonic`, `crystal-dragon`, `orange-cat`,
//! `north-stars`, `curiosity`). The interactive cycle (`SCENE_ORDER`)
//! covers all 30 built-in scenes: the
//! three core atmospheres lead (cinematic, monolith, matrix), then
//! the style flagships, the curated classics, the atmosphere
//! scenes, the power-saving utility, and the milestone/tribute/honor
//! scenes as destinations.
//!
//! task-19 + NIGHT-research-4/5/6: the rejected `ripple` style
//! (water-surface rings) was replaced by `flux` (task-19, PIC/FLIP
//! liquid matrix) at cycle position 5; `lorenz` joined at position 6
//! — a strange-attractor masterpiece (canonical Lorenz ODE
//! integrated via RK4); `cosmic_dragon` joined at position 7 — the
//! Chinese-mythology serpentine dragon (NIGHT-research-5); `physarum`
//! joined at position 8 — the bio-inspired slime mold (Jeff Jones
//! 2010 emergent networks, NIGHT-research-6);
//! `sorgonemous_intrascals` joined at position 9 — the black hole
//! (NIGHT-special-1 staged rollout: stage 1 ships the
//! event-horizon ball, the RK4 orbital ring and glyph infall follow);
//! `aeolian` joined at position 10 — the invented string weave
//! (NIGHT-special-2: the rain plays the instrument, original motion
//! math with no existing reference).

use crate::config::GlitchLevel;
use crate::rain_style::RainStyle;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SceneConfig {
    pub color: Option<&'static str>,
    pub charset: Option<&'static str>,
    pub fps: Option<f64>,
    pub speed: Option<f32>,
    pub density: Option<f32>,
    pub glitch_level: Option<GlitchLevel>,
    pub rain_style: RainStyle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SceneInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub config: SceneConfig,
}

pub(crate) const DEFAULT_SCENE: &str = "cinematic";

/// Ordered scene cycle — all 30 built-in scenes (owner directive
/// 2026-08-24: positions 1-3 are fixed; task-18 added the vortex
/// style flagship at 4; task-19 replaced the rejected ripple with
/// flux at 5; the NIGHT-research-4 merge added lorenz, a
/// strange-attractor masterpiece, at 6; NIGHT-research-5 added the
/// cosmic_dragon style flagship at 7; NIGHT-research-6 added the
/// physarum style flagship at 8; NIGHT-special-1 added the black
/// hole flagship sorgonemous_intrascals at 9; NIGHT-special-2 added
/// the aeolian weave flagship at 10; NIGHT-special-3 added the
/// polar-veil flagship aurora at 11; NIGHT-hunter-15 added the
/// dragon_hunt
/// milestone (the glitch-rain-shift bug-hunt reward) at 21; the rest
/// ordered by daily-use
/// likelihood so the most-switched scenes are the fewest keystrokes
/// away: core trio -> style flagships -> classic siblings ->
/// atmosphere -> power-saving utility -> milestone -> tribute ->
/// honor scenes).
pub(crate) const SCENE_ORDER: &[&str] = &[
    // Core atmospheres (owner-pinned order).
    "cinematic", // 1
    "monolith",  // 2
    "matrix",    // 3
    // Task-18/19 + NIGHT-research-4/5/6 style flagships — the
    // polar-orbit, liquid-fluid, strange-attractor, serpentine-dragon
    // and slime-mold styles are signature differentiators (no
    // competitor terminal has them; the flux style carries a real
    // incompressible Navier-Stokes projection in its critical path),
    // so they lead the cycle right after the core trio.
    "vortex", // 4
    "flux",   // 5
    "lorenz", // 6
    // NIGHT-research-5 style flagship — the Chinese-mythology
    // serpentine dragon (free flight + occasional circling), grouped
    // with the other style flagships so users cycle through all the
    // signature motion styles in one stretch.
    "cosmic_dragon", // 7
    // NIGHT-research-6 style flagship — the bio-inspired slime mold
    // (Jeff Jones 2010 emergent network patterns). The masterpiece
    // rarity (world-first in the terminal matrix rain category).
    "physarum", // 8
    // NIGHT-special-1 style flagship — the black hole. A gravitating
    // body with a black empty event-horizon core (staged rollout:
    // stage 1 ships the ball; the RK4 orbital ring and the glyph
    // infall land in stages 2 and 3). Grouped with the style
    // flagships so the cycle tours all signature motion styles.
    "sorgonemous_intrascals", // 9
    // NIGHT-special-2 style flagship — the aeolian weave. The
    // invented string instrument played by the falling rain:
    // original motion math (the six laws of the weave, derived in
    // this repo — no existing reference), the calm-sky weather
    // dial family. Grouped with the style flagships so the cycle
    // tours all signature motion styles.
    "aeolian", // 10
    // NIGHT-special-4 style flagship — the corona arcade. The third
    // invented-original-math rain style: magnetic loops fed by the
    // coronal rain until they flare (the five laws of the corona,
    // derived in this repo — see cloud/type_rain/solar_flare/mod.rs;
    // replaces the retired aurora veil, NIGHT-special-3).
    // Grouped with the style flagships so the cycle tours all
    // signature motion styles.
    "solar_flare", // 11
    // NIGHT-research-7 style flagship — the double helix, the
    // owner's DeepSeek-researched first pick. The rain writes the
    // genome: a rotating glyph ladder of Watson-Crick base pairs,
    // fed by the nucleotide soup, swept by the replication fork
    // (the five laws of the ladder, derived in this repo — see
    // cloud/type_rain/dna_helix/mod.rs). Grouped with the style
    // flagships so the cycle tours all signature motion styles.
    "dna_helix", // 12
    // NIGHT-research-7 style flagship — the murmuration, the
    // owner's DeepSeek-researched second pick. The rain is a
    // flock: Reynolds boids over a spatial hash, a roaming
    // anchor, a breathing cohesion and a clocked predator
    // startle (the five laws of the flock, derived in this repo —
    // see cloud/type_rain/murmuration/mod.rs). Grouped with the
    // style flagships so the cycle tours all signature motion
    // styles.
    "murmuration", // 13
    // NIGHT-research-8 style flagship — the quasar, the owner's
    // pick over the neural-network proposal. The rain feeds the
    // engine: cold glyph gas falls onto a Keplerian accretion
    // disk (doppler-brightened on the approaching limb), the
    // core burns white-hot and breathes, the poles fire
    // precessing jets (the five laws of the engine, derived in
    // this repo — see cloud/type_rain/quasar/mod.rs). Grouped
    // with the style flagships so the cycle tours all signature
    // motion styles.
    "quasar", // 14
    // Classic siblings — the traditional looks users switch to often.
    "classic",     // 15 — original green-on-black
    "signal",      // 16 — digital transmission
    "hacker",      // 17 — high-contrast terminal overflow
    "matrix_film", // 18 — 1999 film homage
    // Atmosphere scenes — intensity then calm, then space and neon.
    "storm",  // 19
    "calm",   // 20
    "cosmos", // 21
    "neon",   // 22
    // Utility.
    "low-power", // 23
    // Milestone + tribute.
    "cosmic-dragon", // 24
    "dragon_hunt",   // 25
    "carbonic",      // 26
    // Honor scenes — destinations, cycled last.
    "crystal-dragon", // 27
    "orange-cat",     // 28
    "north-stars",    // 29
    "curiosity",      // 30
];

/// The built-in scene catalog — extracted to `catalog.rs`
/// (NIGHT-special-3: the table outgrew this file's 800-line cap;
/// re-exported here so every `crate::scene::SCENES` consumer keeps
/// resolving — the RULES_LOC extraction recipe).
pub(crate) use catalog::SCENES;

/// All builtin scene names, alphabetically sorted.
///
/// v80.0.0 masterclass: derived from the `SCENES` catalog instead of
/// returning a hand-maintained duplicate list — a scene added to the
/// catalog can no longer be silently forgotten here (single source of
/// truth; the old hand-written array was a drift class of its own).
/// Allocation is confined to error hints and list building; the hot
/// render path never calls this.
#[must_use]
pub(crate) fn all_scene_names() -> Vec<&'static str> {
    let mut names: Vec<&'static str> = SCENES.iter().map(|s| s.name).collect();
    names.sort_unstable();
    names
}

/// Cycle to the next or previous scene in the ordered cycle.
/// Returns the next scene name.
/// Forward:  cinematic -> monolith -> matrix -> classic -> ... -> curiosity
/// Backward: the reverse. Unknown names fall back to DEFAULT_SCENE.
#[must_use]
pub(crate) fn cycle_scene(current: &str, dir: i32) -> &'static str {
    let Some(pos) = SCENE_ORDER.iter().position(|&n| n == current) else {
        return DEFAULT_SCENE;
    };
    let n = SCENE_ORDER.len() as i32;
    let mut idx = pos as i32 + dir;
    idx = ((idx % n) + n) % n;
    SCENE_ORDER[idx as usize]
}

#[must_use]
pub(crate) fn get_scene(name: &str) -> Option<&'static SceneInfo> {
    let normalized = name.trim().to_ascii_lowercase();
    SCENES.iter().find(|scene| scene.name == normalized)
}

#[must_use]
pub(crate) fn rain_style_for_scene(name: &str) -> Option<RainStyle> {
    get_scene(name).map(|scene| scene.config.rain_style)
}

/// Validate a scene name against the builtin scene list.
///
/// v50.0.0-beta.6 Option D: this function is no longer used in the
/// production resolution path (custom scenes are now accepted alongside
/// builtins — see `config_apply.rs`). Kept for test coverage and future
/// strict-validation use cases. The production path uses `get_scene()`
/// directly (returns None for custom-only names, which is handled gracefully).
#[cfg(test)]
pub(crate) fn validate_scene_name(name: &str) -> Result<String, String> {
    let normalized = name.trim().to_ascii_lowercase();
    if get_scene(&normalized).is_some() {
        Ok(normalized)
    } else {
        Err(format!(
            "error: unknown scene '{name}'\n\n  Use --list-scenes to see available scenes."
        ))
    }
}

#[must_use]
pub(crate) fn list_scenes_text() -> String {
    let mut out = String::new();
    for scene in SCENES {
        out.push_str(&format!("  {:10} {}\n", scene.name, scene.description));
    }
    out
}

/// Render a detailed, human-readable description of a single built-in scene.
///
/// Output is intended for `--show-scene <name>` when `<name>` matches a
/// built-in scene. Each field line is only printed when the scene actually
/// sets that field (i.e. it is `Some(_)`), so partial scenes do not show
/// misleading "default" placeholders.
#[must_use]
pub(crate) fn show_scene_text(info: &SceneInfo) -> String {
    let mut out = String::new();
    out.push_str(&format!("SCENE: {}\n\n", info.name));
    out.push_str(&format!("  Description: {}\n\n", info.description));
    out.push_str("  Configuration:\n");

    let cfg = info.config;
    if let Some(color) = cfg.color {
        out.push_str(&format!("    color        = {color}\n"));
    }
    if let Some(charset) = cfg.charset {
        out.push_str(&format!("    charset      = {charset}\n"));
    }
    if let Some(fps) = cfg.fps {
        out.push_str(&format!("    fps          = {fps}\n"));
    }
    if let Some(speed) = cfg.speed {
        out.push_str(&format!("    speed        = {speed}\n"));
    }
    if let Some(density) = cfg.density {
        out.push_str(&format!("    density      = {density}\n"));
    }
    if let Some(glitch) = cfg.glitch_level {
        out.push_str(&format!("    glitch-level = {}\n", glitch_label(glitch)));
    }
    // rain_style is always set (it's not an Option), so always show it.
    out.push_str(&format!("    rain-style   = {}\n", cfg.rain_style.as_str()));

    // v50.0.0-beta.7: note that CLI flags override scene values at runtime.
    // --show-scene shows the scene's builtin defaults; actual runtime values
    // may differ when CLI flags like --color, --speed, --charset-custom etc.
    // are passed alongside --scene.
    out.push_str("\n  Note: CLI flags (--color, --speed, --charset, etc.) override\n");
    out.push_str("  scene values at runtime. This output shows the scene's builtin\n");
    out.push_str("  defaults only.\n");

    out.push_str("\n  Use: cosmostrix --scene ");
    out.push_str(info.name);
    out.push('\n');
    out
}

/// Map a `GlitchLevel` to its lowercase CLI string label.
fn glitch_label(level: crate::config::GlitchLevel) -> &'static str {
    use crate::config::GlitchLevel;
    match level {
        GlitchLevel::None => "none",
        GlitchLevel::Subtle => "subtle",
        GlitchLevel::Default => "default",
        GlitchLevel::Intense => "intense",
    }
}

pub(crate) mod catalog;
pub(crate) mod charset;
pub(crate) mod charset_custom;

#[cfg(test)]
#[path = "../../test/scene/tests.rs"]
mod tests;
