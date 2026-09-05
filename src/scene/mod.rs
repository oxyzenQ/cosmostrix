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
//! subsystem) with the five structured style flagships (`vortex`,
//! `flux`, `lorenz`, `cosmic_dragon`, `physarum` — task-18/19 +
//! NIGHT-research-4/5/6, the signature differentiators), nine curated
//! visual scenes (`classic`, `cinematic`, `calm`, `storm`, `cosmos`,
//! `neon`, `hacker`, `matrix_film`, `low-power`), the `cosmic-dragon`
//! milestone scene commemorating the temporal-prediction breakthrough
//! ( dirty_ratio 18.33% → 0.39%, FPS 7,843 → 29,773), and the tribute
//! and honor destinations (`carbonic`, `crystal-dragon`, `orange-cat`,
//! `north-stars`, `curiosity`). The interactive cycle (`SCENE_ORDER`)
//! covers all 23 built-in scenes (owner directive 2026-08-24): the
//! three core atmospheres lead (cinematic, monolith, matrix), then
//! the five style flagships, the curated classics, the atmosphere
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
//! 2010 emergent networks, NIGHT-research-6).

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

/// Ordered scene cycle — all 23 built-in scenes (owner directive
/// 2026-08-24: positions 1-3 are fixed; task-18 added the vortex
/// style flagship at 4; task-19 replaced the rejected ripple with
/// flux at 5; the NIGHT-research-4 merge added lorenz, a
/// strange-attractor masterpiece, at 6; NIGHT-research-5 added the
/// cosmic_dragon style flagship at 7; NIGHT-research-6 added the
/// physarum style flagship at 8; the rest ordered by daily-use
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
    // Classic siblings — the traditional looks users switch to often.
    "classic",     // 9 — original green-on-black
    "signal",      // 10 — digital transmission
    "hacker",      // 11 — high-contrast terminal overflow
    "matrix_film", // 12 — 1999 film homage
    // Atmosphere scenes — intensity then calm, then space and neon.
    "storm",  // 13
    "calm",   // 14
    "cosmos", // 15
    "neon",   // 16
    // Utility.
    "low-power", // 17
    // Milestone + tribute.
    "cosmic-dragon", // 18
    "carbonic",      // 19
    // Honor scenes — destinations, cycled last.
    "crystal-dragon", // 20
    "orange-cat",     // 21
    "north-stars",    // 22
    "curiosity",      // 23
];

pub(crate) const SCENES: &[SceneInfo] = &[
    // --- Original runtime scenes (interactive cycle entries) ---
    SceneInfo {
        name: "matrix",
        description: "Classic Matrix glyph rain — organic cascade with katakana flow",
        config: SceneConfig {
            color: Some("neon-green"),
            charset: Some("matrix"),
            fps: Some(60.0),
            speed: Some(18.0),
            density: Some(0.65),
            glitch_level: Some(GlitchLevel::Subtle),
            rain_style: RainStyle::Glyph,
        },
    },
    // --- Task-18/19 + NIGHT-research-4/5/6 style flagships (rain styles 3 through 7) ---
    SceneInfo {
        name: "vortex",
        description: "Polar-orbit galaxy drain — glyphs spiral inward on Keplerian orbits toward a glowing core",
        config: SceneConfig {
            color: Some("cosmos"),
            charset: Some("zen"),
            fps: Some(60.0),
            speed: Some(24.0),
            density: Some(0.70),
            glitch_level: Some(GlitchLevel::Subtle),
            rain_style: RainStyle::Vortex,
        },
    },
    SceneInfo {
        name: "flux",
        description: "Liquid matrix — code rain falling through a living incompressible fluid; jets shear into emergent eddies",
        config: SceneConfig {
            color: Some("ocean"),
            charset: Some("minimal"),
            fps: Some(60.0),
            speed: Some(18.0),
            density: Some(0.70),
            glitch_level: Some(GlitchLevel::Subtle),
            rain_style: RainStyle::Flux,
        },
    },
    // NIGHT-research-4: the lorenz scene is the project's flagship
    // masterpiece — the only terminal rain that renders a real
    // strange attractor (canonical Lorenz ODE, sigma=10, rho=28,
    // beta=8/3, RK4-integrated). Two-lobe butterfly projected to 2D
    // with z-as-depth brightness. The `cosmos` palette + `binary`
    // charset evoke the deep-space + mathematical-purity aesthetic;
    // speed 24 (same as vortex) gives the butterfly a majestic
    // wingbeat cadence (one lobe traversal every ~3-5 s). Density
    // 0.70 matches vortex so the two scenes cycle-read as siblings.
    // Catalog history: ripple (water-surface rings) was
    // owner-rejected and removed by task-19's flux; this scene
    // joined at cycle position 6 via the NIGHT-research-4 merge as
    // the fifth rain style.
    SceneInfo {
        name: "lorenz",
        description: "Lorenz strange attractor — glyphs ride the canonical chaotic butterfly (RK4-integrated 3D ODE, two-lobe projection)",
        config: SceneConfig {
            color: Some("cosmos"),
            charset: Some("binary"),
            fps: Some(60.0),
            speed: Some(24.0),
            density: Some(0.70),
            glitch_level: Some(GlitchLevel::Subtle),
            rain_style: RainStyle::Lorenz,
        },
    },
    // NIGHT-research-5: cosmic_dragon — Chinese-mythology serpentine
    // dragon. Distinct from the existing `cosmic-dragon` (hyphen)
    // milestone scene: cosmic-dragon is a Glyph-style tribute to the
    // temporal-prediction breakthrough; cosmic_dragon (underscore) is
    // a new rain STYLE — a structured-family chain renderer with
    // serpentine motion DNA. The `nebula` palette evokes the cosmic
    // sky the dragon flies through; `zen` charset keeps the body
    // clean and Asian-feel. Speed 18 = majestic flight cadence.
    // Density no longer affects dragon count — owner directive
    // fixes the count at 3 dragons to match the three dragon engines
    // in cosmostrix (cosmic_dragon_engine, crystal_dragon_engine,
    // chroma_dragon_engine). The 0.55 value is kept for spawn-timing
    // parity with the other style flagships. The head state machine
    // alternates Soar (free flight) and Circle (orbital) per the
    // owner's "kadang melingkar, terbang bebas kemana aja" spec.
    SceneInfo {
        name: "cosmic_dragon",
        description: "Cosmic Dragon — Chinese-mythology serpentine dragon; free flight with occasional circling, body trails the head in a living chain",
        config: SceneConfig {
            color: Some("nebula"),
            charset: Some("zen"),
            fps: Some(60.0),
            speed: Some(18.0),
            density: Some(0.55),
            glitch_level: Some(GlitchLevel::Subtle),
            rain_style: RainStyle::Dragon,
        },
    },
    // NIGHT-research-6: physarum — bio-inspired slime mold (Jeff
    // Jones 2010 model). Particles sense / decide / move / deposit
    // on a stigmergic trail field, producing emergent network
    // patterns. The terminal's discrete cell grid IS the substrate
    // — a 1:1 medium match (masterpiece contract: terminal
    // limitations BECOME the simulation substrate).
    // The `cosmos` palette + `binary` charset evoke the deep-space
    // petri dish aesthetic; speed 18 = steady exploration cadence;
    // density 0.55 = 30-40 particles (enough for visible networks,
    // sparse enough for the trail decay to keep patterns alive).
    // No other terminal matrix rain project ships physarum — this
    // is the project's first bio-inspired renderer, the rarest
    // style engineering in the category.
    SceneInfo {
        name: "physarum",
        description: "Physarum slime mold — bio-inspired emergent network patterns; particles sense / decide / move / deposit on a stigmergic trail field (Jeff Jones 2010 model)",
        config: SceneConfig {
            color: Some("cosmos"),
            charset: Some("binary"),
            fps: Some(60.0),
            speed: Some(18.0),
            density: Some(0.55),
            glitch_level: Some(GlitchLevel::Subtle),
            rain_style: RainStyle::Physarum,
        },
    },
    SceneInfo {
        name: "monolith",
        description:
            "Signature structured segmented rain — dense zen pillars with premium pacing",
        config: SceneConfig {
            color: Some("energy-zen"),
            charset: Some("zen"),
            fps: Some(60.0),
            speed: Some(30.0),
            density: Some(0.85),
            glitch_level: Some(GlitchLevel::Subtle),
            rain_style: RainStyle::Monolith,
        },
    },
    SceneInfo {
        name: "signal",
        // v80.0.0 sync: "cyan aurora glyphs" -> "aurora glyphs" — the
        // aurora palette was retuned to real 557.7nm green (earth-
        // element real-color masterclass), so the palette is now
        // green-dominant with cyan fringes, not cyan-led.
        description: "Digital transmission — aurora glyphs in box-draw frames",
        config: SceneConfig {
            color: Some("aurora"),
            charset: Some("retro"),
            fps: Some(60.0),
            speed: Some(14.0),
            density: Some(0.55),
            glitch_level: Some(GlitchLevel::Default),
            rain_style: RainStyle::Glyph,
        },
    },
    // --- Curated visual scenes ---
    SceneInfo {
        name: "classic",
        description: "Original green-on-black — slow contemplative katakana cascade",
        config: SceneConfig {
            color: Some("green"),
            charset: Some("matrix"),
            fps: Some(60.0),
            speed: Some(12.0),
            density: Some(0.70),
            glitch_level: Some(GlitchLevel::Subtle),
            rain_style: RainStyle::Glyph,
        },
    },
    SceneInfo {
        name: "cinematic",
        description: "Cosmic zen — slow vast pacing with deep-space breathing room",
        config: SceneConfig {
            color: Some("energy-zen"),
            charset: Some("zen"),
            fps: Some(60.0),
            speed: Some(9.0),
            density: Some(0.75),
            glitch_level: Some(GlitchLevel::Subtle),
            rain_style: RainStyle::Glyph,
        },
    },
    SceneInfo {
        name: "calm",
        description: "Gentle ocean — zen minimal density, slow meditative flow",
        config: SceneConfig {
            color: Some("ocean"),
            charset: Some("minimal"),
            fps: Some(60.0),
            speed: Some(6.0),
            density: Some(0.40),
            glitch_level: Some(GlitchLevel::None),
            rain_style: RainStyle::Glyph,
        },
    },
    SceneInfo {
        name: "storm",
        description: "Fast intense purple cyberpunk — dense aggressive neon chaos",
        config: SceneConfig {
            color: Some("purple"),
            charset: Some("cyberpunk"),
            fps: Some(120.0),
            speed: Some(28.0),
            density: Some(1.10),
            glitch_level: Some(GlitchLevel::Intense),
            rain_style: RainStyle::Glyph,
        },
    },
    SceneInfo {
        name: "cosmos",
        description: "Deep-space cosmos — nebula gradient with spacious starlit drift",
        config: SceneConfig {
            color: Some("nebula"),
            charset: Some("binary"),
            fps: Some(60.0),
            speed: Some(11.0),
            // v80.0.0 masterclass tune: density 0.80 -> 0.70. The
            // description promises "spacious starlit drift", but 0.80
            // sat dead-on the catalog median (~0.78) — a median value
            // is not spacious. 0.70 gives the deep-sky scene genuine
            // room while keeping the nebula visibly fuller than its
            // milestone sibling cosmic-dragon (0.65, deliberate kin)
            // and far airier than the overflow scenes (hacker and
            // carbonic at 0.95). Speed 11 "drift" and the rest were
            // audited peak — unchanged.
            density: Some(0.70),
            glitch_level: Some(GlitchLevel::Subtle),
            rain_style: RainStyle::Glyph,
        },
    },
    SceneInfo {
        name: "neon",
        description: "Vibrant cyberpunk — neon pop with breathing room and medium flow",
        config: SceneConfig {
            color: Some("neon"),
            charset: Some("cyberpunk"),
            fps: Some(60.0),
            speed: Some(16.0),
            // v80.0.0 masterclass tune: density 0.90 -> 0.78. The
            // description promises "breathing room", but 0.90 sat 5%
            // under hacker's 0.95 — an imperceptible gap that read as
            // the same soup with a different palette. 0.78 puts real
            // air between the two cyberpunk scenes (hacker 0.95 =
            // dense terminal overflow, neon 0.78 = pop with room)
            // while staying above matrix's 0.65 so the neon signage
            // still pops. Speed 16 "medium flow" sits on the catalog
            // median — audited peak, unchanged.
            density: Some(0.78),
            glitch_level: Some(GlitchLevel::Default),
            rain_style: RainStyle::Glyph,
        },
    },
    SceneInfo {
        name: "hacker",
        description: "Green hacker aesthetic — dense high-speed terminal overflow",
        config: SceneConfig {
            color: Some("green"),
            charset: Some("hacker"),
            fps: Some(60.0),
            speed: Some(24.0),
            density: Some(0.95),
            glitch_level: Some(GlitchLevel::Default),
            rain_style: RainStyle::Glyph,
        },
    },
    // --- Film homage scene: matrix_film ---
    //
    // Tuned to capture the visual fingerprint of the Matrix 1999 code rain:
    // dense phosphor-green katakana+digit columns falling at cinematic pace.
    // This is not a 1:1 reproduction — cosmostrix remains its own frontier —
    // but a scene that lets the engine's parallax depth, phosphor decay, and
    // head-bloom layer onto the film's foundational look.
    //
    // Distinct from the `matrix` scene (the modern organic cascade, density
    // 0.65, speed 18.0): matrix_film pushes density to 0.85 and speed to 22.0
    // to match the film's packed-column, steady-fall rhythm. Charset `matrix`
    // (katakana + Latin digits + letters) is literally the film's glyph set.
    // Palette `neon-green` keeps the canonical Matrix green. Glitch stays
    // Subtle — the film has occasional flickers but is mostly clean. Rain
    // style is Glyph (Monolith is cosmostrix's own invention, not
    // film-accurate). FPS 60 keeps motion smooth; the film's 24fps cadence
    // would look choppy against cosmostrix's frontier pacing.
    SceneInfo {
        name: "matrix_film",
        description: "Matrix Film — 1999 cinematic homage; dense phosphor-green katakana rain with cosmostrix frontier depth",
        config: SceneConfig {
            color: Some("neon-green"),
            charset: Some("matrix"),
            fps: Some(60.0),
            speed: Some(22.0),
            density: Some(0.85),
            glitch_level: Some(GlitchLevel::Subtle),
            rain_style: RainStyle::Glyph,
        },
    },
    SceneInfo {
        name: "low-power",
        description: "Ultra power-saving — 30 FPS, minimal density, no glitch",
        config: SceneConfig {
            color: Some("green"),
            charset: Some("binary"),
            fps: Some(30.0),
            speed: Some(5.0),
            density: Some(0.45),
            glitch_level: Some(GlitchLevel::None),
            rain_style: RainStyle::Glyph,
        },
    },
    // --- Milestone scene (commemorates the temporal-prediction breakthrough) ---
    //  horizon=12 + skip-draw + persistent cells slashed dirty_ratio
    // from 18.33% to 0.39% and boosted avg_fps from 7,843 to 29,773 — a 280%
    // speedup with 99.6% fewer drawn cells. This scene is the visible reward
    // for that achievement: a deep-space binary rain that, like the Cosmic Dragon,
    // sees its own future. Palette `cosmos` + charset `binary` mirror the
    // cinematic base; speed 12 + density 0.65 give it room to breathe.
    SceneInfo {
        name: "cosmic-dragon",
        description: "Cosmic Dragon — temporal-prediction milestone; deep-space binary rain that sees its own future",
        config: SceneConfig {
            color: Some("cosmos"),
            charset: Some("binary"),
            fps: Some(60.0),
            speed: Some(12.0),
            density: Some(0.65),
            glitch_level: Some(GlitchLevel::Subtle),
            rain_style: RainStyle::Glyph,
        },
    },
    // --- Tribute scene: carbonic ---
    //
    // Honors the +280% FPS achievement of the temporal-prediction
    // experiment ( 7,843 → 29,773 FPS, dirty_ratio 18.33% →
    // 0.39%). The experiment was ultimately reverted in v25 because
    // it compromised the cinematic visual quality, but the lessons
    // learned — about prediction, drift tolerance, and the
    // tension between performance and beauty — remain invaluable.
    //
    // `carbonic` evokes the aesthetic of carbon fiber: dark, dense,
    // futuristic, and resilient. The `carbon` palette (dark-grey-to-
    // silver ramp, head RGB 230/240/250 — compliant with the
    // head-not-pure-white invariant) gives a sleek metallic feel.
    // `binary` charset keeps the visual high-tech and minimal.
    // Speed 18 + density 0.95 produce a dense, energetic rain that
    // showcases the engine's throughput. Subtle glitch hints at the
    // controlled chaos of the prediction experiment.
    SceneInfo {
        name: "carbonic",
        description: "Carbonic — tribute to the temporal-prediction experiment; dense metallic carbon-fiber binary rain",
        config: SceneConfig {
            color: Some("carbon"),
            charset: Some("binary"),
            fps: Some(60.0),
            speed: Some(18.0),
            density: Some(0.95),
            glitch_level: Some(GlitchLevel::Subtle),
            rain_style: RainStyle::Glyph,
        },
    },
    // ── Honor scenes ──────────────────────────────────────────────
    //
    // crystal-dragon: honors the cosmostrix + oxyzenQ journey and the
    // hardthinking-mode reward. The energy-zen palette's crystal-edge
    // magenta stop inspires the name — a dragon carved from living
    // crystal, breathing violet fire. v80.0.0 masterclass tune: speed
    // raised 10 -> 30 (owner directive — the honor scene must move with
    // living-crystal energy, not crawl); vast pacing is preserved by the
    // Monolith segmented structure, so the reward scene stays meditative
    // in texture while finally flowing at premium pace.
    SceneInfo {
        name: "crystal-dragon",
        description: "Dragon Crystal — honors the cosmostrix + oxyzenQ journey; living crystal violet rain, the hardthinking-mode reward",
        config: SceneConfig {
            color: Some("energy-zen"),
            charset: Some("zen"),
            fps: Some(60.0),
            speed: Some(30.0),
            density: Some(0.78),
            glitch_level: Some(GlitchLevel::Subtle),
            rain_style: RainStyle::Monolith,
        },
    },
    // orange-cat: honors the owner's orange cat, who passed on 2 Aug 2026.
    // A warm amber-gold rain, gentle and contemplative — like afternoon
    // sunlight through a window where a cat used to sleep. Slow pace,
    // minimal density, no glitch. The `orange` palette ranges from
    // deep ember to bright honey, mirroring a tabby's coat. The
    // `minimal` charset (single nabla glyph since the 2026-08-30 owner
    // pick) keeps the visual quiet and meditative.
    SceneInfo {
        name: "orange-cat",
        description: "Orange Cat — in memory of the owner's orange cat (2 Aug 2026); warm amber-gold gentle contemplative rain",
        config: SceneConfig {
            color: Some("orange"),
            charset: Some("minimal"),
            fps: Some(60.0),
            speed: Some(7.0),
            density: Some(0.45),
            glitch_level: Some(GlitchLevel::None),
            rain_style: RainStyle::Glyph,
        },
    },
    // north-stars: honors the owner's habit of watching stars at 3 AM.
    // A cool white-gold rain on deep space, sparse and distant — like
    // looking up at a winter sky. The `stars` palette (white-gold
    // gradient) + `binary` charset evokes pinprick starlight. Very low
    // density (0.35) + slow speed (5.0) for the quiet vastness of 3 AM.
    // No glitch — the night sky is still.
    SceneInfo {
        name: "north-stars",
        description: "North Stars — honors 3 AM stargazing; sparse white-gold pinprick starlight on deep space",
        config: SceneConfig {
            color: Some("stars"),
            charset: Some("binary"),
            fps: Some(60.0),
            speed: Some(5.0),
            density: Some(0.35),
            glitch_level: Some(GlitchLevel::None),
            rain_style: RainStyle::Glyph,
        },
    },
    // curiosity: honors the owner's curiosity — the engine that built
    // cosmostrix. A vibrant spectrum rain, ever-shifting, dense and
    // inquisitive. The `rainbow` palette cycles through the full hue
    // range, mirroring the restless color of wonder. `cyberpunk` charset
    // + speed 20 + density 0.90 produce a dense, energetic flow that
    // showcases the engine's full chroma range. Default glitch hints at
    // the creative chaos of exploration.
    SceneInfo {
        name: "curiosity",
        description: "Curiosity — honors the owner's wonder; vibrant spectrum rainbow rain, the engine that built cosmostrix",
        config: SceneConfig {
            color: Some("rainbow"),
            charset: Some("cyberpunk"),
            fps: Some(60.0),
            speed: Some(20.0),
            density: Some(0.90),
            glitch_level: Some(GlitchLevel::Default),
            rain_style: RainStyle::Glyph,
        },
    },
];

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

pub(crate) mod charset;
pub(crate) mod charset_custom;

#[cfg(test)]
#[path = "../../test/scene/tests.rs"]
mod tests;
