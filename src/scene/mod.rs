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
//! subsystem) with nine curated visual scenes (`classic`,
//! `cinematic`, `calm`, `storm`, `cosmos`, `neon`, `hacker`, `matrix_film`,
//! `low-power`) plus the `cosmic-dragon` milestone scene commemorating the
//! temporal-prediction breakthrough ( dirty_ratio 18.33% → 0.39%,
//! FPS 7,843 → 29,773). The interactive cycle (`SCENE_ORDER`) covers all
//! 18 built-in scenes (owner directive 2026-08-24): the three core
//! atmospheres lead (cinematic, monolith, matrix), followed by the
//! curated classics, atmosphere scenes, the power-saving utility, and
//! the milestone/tribute/honor scenes as destinations.

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

/// Ordered scene cycle — all 18 built-in scenes (owner directive
/// 2026-08-24: positions 1-3 are fixed; the rest ordered by daily-use
/// likelihood so the most-switched scenes are the fewest keystrokes
/// away: core trio -> classic siblings -> atmosphere -> power-saving
/// utility -> milestone -> tribute -> honor scenes).
pub(crate) const SCENE_ORDER: &[&str] = &[
    // Core atmospheres (owner-pinned order).
    "cinematic", // 1
    "monolith",  // 2
    "matrix",    // 3
    // Classic siblings — the traditional looks users switch to often.
    "classic",     // 4 — original green-on-black
    "signal",      // 5 — digital transmission
    "hacker",      // 6 — high-contrast terminal overflow
    "matrix_film", // 7 — 1999 film homage
    // Atmosphere scenes — intensity then calm, then space and neon.
    "storm",  // 8
    "calm",   // 9
    "cosmos", // 10
    "neon",   // 11
    // Utility.
    "low-power", // 12
    // Milestone + tribute.
    "cosmic-dragon", // 13
    "carbonic",      // 14
    // Honor scenes — destinations, cycled last.
    "dragon-crystal", // 15
    "orange-cat",     // 16
    "north-stars",    // 17
    "curiosity",      // 18
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
        description: "Digital transmission — cyan aurora glyphs in box-draw frames",
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
            density: Some(0.80),
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
            density: Some(0.90),
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
    // dragon-crystal: honors the cosmostrix + oxyzenQ journey and the
    // hardthinking-mode reward. The energy-zen palette's crystal-edge
    // magenta stop inspires the name — a dragon carved from living
    // crystal, breathing violet fire. Slow vast pacing with deep-space
    // breathing room, matching the meditative focus of hardthinking mode.
    SceneInfo {
        name: "dragon-crystal",
        description: "Dragon Crystal — honors the cosmostrix + oxyzenQ journey; living crystal violet rain, the hardthinking-mode reward",
        config: SceneConfig {
            color: Some("energy-zen"),
            charset: Some("zen"),
            fps: Some(60.0),
            speed: Some(10.0),
            density: Some(0.78),
            glitch_level: Some(GlitchLevel::Subtle),
            rain_style: RainStyle::Monolith,
        },
    },
    // orange-cat: honors the owner's orange cat, who passed on 2 Aug 2026.
    // A warm amber-gold rain, gentle and contemplative — like afternoon
    // sunlight through a window where a cat used to sleep. Slow pace,
    // minimal density, no glitch. The `orange` palette ranges from
    // deep ember to bright honey, mirroring a tabby's coat. The `minimal`
    // charset keeps the visual quiet and meditative.
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

#[must_use]
pub(crate) fn all_scene_names() -> &'static [&'static str] {
    &[
        "calm",
        "carbonic",
        "cinematic",
        "classic",
        "cosmic-dragon",
        "cosmos",
        "curiosity",
        "dragon-crystal",
        "hacker",
        "low-power",
        "matrix",
        "matrix_film",
        "monolith",
        "neon",
        "north-stars",
        "orange-cat",
        "signal",
        "storm",
    ]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_scene_forward_order() {
        // Owner-pinned core trio: cinematic -> monolith -> matrix.
        assert_eq!(cycle_scene("cinematic", 1), "monolith");
        assert_eq!(cycle_scene("monolith", 1), "matrix");
        assert_eq!(cycle_scene("matrix", 1), "classic");
        // Tail of the cycle wraps back to the head.
        assert_eq!(cycle_scene("curiosity", 1), "cinematic");
    }

    #[test]
    fn cycle_scene_backward_order() {
        // Core trio backward: matrix -> monolith -> cinematic.
        assert_eq!(cycle_scene("matrix", -1), "monolith");
        assert_eq!(cycle_scene("monolith", -1), "cinematic");
        // Head wraps backward to the tail.
        assert_eq!(cycle_scene("cinematic", -1), "curiosity");
    }

    #[test]
    fn cycle_scene_unknown_returns_default() {
        assert_eq!(cycle_scene("nonexistent", 1), DEFAULT_SCENE);
        assert_eq!(cycle_scene("nonexistent", -1), DEFAULT_SCENE);
    }

    #[test]
    fn cycle_scene_wraps_around() {
        // Double forward from matrix: matrix -> classic -> signal.
        assert_eq!(cycle_scene(cycle_scene("matrix", 1), 1), "signal");
        // Double backward from matrix: matrix -> monolith -> cinematic.
        assert_eq!(cycle_scene(cycle_scene("matrix", -1), -1), "cinematic");
        // Full lap forward returns to start.
        let mut cur = "cinematic";
        for _ in 0..SCENE_ORDER.len() {
            cur = cycle_scene(cur, 1);
        }
        assert_eq!(cur, "cinematic");
    }

    #[test]
    fn scene_names_are_present() {
        assert_eq!(DEFAULT_SCENE, "cinematic");
        assert_eq!(
            all_scene_names(),
            &[
                "calm",
                "carbonic",
                "cinematic",
                "classic",
                "cosmic-dragon",
                "cosmos",
                "curiosity",
                "dragon-crystal",
                "hacker",
                "low-power",
                "matrix",
                "matrix_film",
                "monolith",
                "neon",
                "north-stars",
                "orange-cat",
                "signal",
                "storm",
            ]
        );
        for name in all_scene_names() {
            assert!(get_scene(name).is_some(), "missing scene {name}");
        }
    }

    #[test]
    fn scene_catalog_has_eighteen_entries() {
        assert_eq!(SCENES.len(), 18, "catalog must contain 18 built-in scenes");
    }

    #[test]
    fn matrix_film_scene_uses_film_accurate_values() {
        let s = get_scene("matrix_film").expect("matrix_film scene");
        assert_eq!(s.config.color, Some("neon-green"));
        assert_eq!(s.config.charset, Some("matrix"));
        assert_eq!(s.config.fps, Some(60.0));
        assert_eq!(s.config.speed, Some(22.0));
        assert_eq!(s.config.density, Some(0.85));
        assert_eq!(s.config.glitch_level, Some(GlitchLevel::Subtle));
        assert_eq!(s.config.rain_style, RainStyle::Glyph);
        // description must reference the 1999 cinematic homage so the
        // scene's purpose is self-documenting via --list-scenes / --show-scene.
        assert!(
            s.description.contains("1999"),
            "matrix_film description must reference 1999: {}",
            s.description
        );
    }

    #[test]
    fn matrix_film_distinct_from_matrix_scene() {
        // matrix_film is the film-faithful homage; matrix is the modern
        // organic cascade. They must differ on density and speed so that
        // cycling into matrix_film is visually distinct from matrix.
        let matrix = get_scene("matrix").expect("matrix scene");
        let film = get_scene("matrix_film").expect("matrix_film scene");
        assert_ne!(
            matrix.config.density, film.config.density,
            "matrix_film must have different density than matrix"
        );
        assert_ne!(
            matrix.config.speed, film.config.speed,
            "matrix_film must have different speed than matrix"
        );
    }

    #[test]
    fn cosmic_dragon_scene_marks_temporal_prediction_milestone() {
        let s = get_scene("cosmic-dragon").expect("cosmic-dragon scene");
        assert_eq!(s.config.color, Some("cosmos"));
        assert_eq!(s.config.charset, Some("binary"));
        assert_eq!(s.config.fps, Some(60.0));
        assert_eq!(s.config.speed, Some(12.0));
        assert_eq!(s.config.density, Some(0.65));
        assert_eq!(s.config.glitch_level, Some(GlitchLevel::Subtle));
        assert_eq!(s.config.rain_style, RainStyle::Glyph);
        // description must mention temporal-prediction milestone so the
        // scene's purpose is self-documenting via --list-scenes / --show-scene.
        assert!(
            s.description.contains("temporal-prediction"),
            "cosmic-dragon description must reference temporal-prediction: {}",
            s.description
        );
    }

    #[test]
    fn scene_cycle_order_is_preserved() {
        // Owner-pinned first three (2026-08-24 directive) + full coverage.
        assert_eq!(&SCENE_ORDER[..3], &["cinematic", "monolith", "matrix"]);
        assert_eq!(
            SCENE_ORDER.len(),
            18,
            "all built-in scenes must be cyclable"
        );
        // Every SCENES entry must appear in SCENE_ORDER exactly once —
        // a new scene that forgets to join the cycle fails here.
        for s in SCENES {
            assert!(
                SCENE_ORDER.contains(&s.name),
                "scene '{}' missing from SCENE_ORDER",
                s.name
            );
        }
        let mut sorted = SCENE_ORDER.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            SCENE_ORDER.len(),
            "SCENE_ORDER has duplicates"
        );
    }

    #[test]
    fn classic_scene_uses_cinematic_sparse_values() {
        let s = get_scene("classic").expect("classic scene");
        assert_eq!(s.config.color, Some("green"));
        assert_eq!(s.config.charset, Some("matrix"));
        assert_eq!(s.config.fps, Some(60.0));
        assert_eq!(s.config.speed, Some(12.0));
        assert_eq!(s.config.density, Some(0.70));
        assert_eq!(s.config.glitch_level, Some(GlitchLevel::Subtle));
        assert_eq!(s.config.rain_style, RainStyle::Glyph);
    }

    #[test]
    fn storm_scene_uses_intense_cyberpunk_values() {
        let s = get_scene("storm").expect("storm scene");
        assert_eq!(s.config.color, Some("purple"));
        assert_eq!(s.config.charset, Some("cyberpunk"));
        assert_eq!(s.config.fps, Some(120.0));
        assert_eq!(s.config.speed, Some(28.0));
        assert_eq!(s.config.density, Some(1.10));
        assert_eq!(s.config.glitch_level, Some(GlitchLevel::Intense));
    }

    #[test]
    fn low_power_scene_caps_fps_at_thirty() {
        let s = get_scene("low-power").expect("low-power scene");
        assert_eq!(s.config.fps, Some(30.0));
        assert_eq!(s.config.speed, Some(5.0));
        assert_eq!(s.config.density, Some(0.45));
        assert_eq!(s.config.color, Some("green"));
        assert_eq!(s.config.charset, Some("binary"));
        assert_eq!(s.config.glitch_level, Some(GlitchLevel::None));
    }

    #[test]
    fn hacker_scene_uses_hacker_charset() {
        let s = get_scene("hacker").expect("hacker scene");
        assert_eq!(s.config.charset, Some("hacker"));
        assert_eq!(s.config.speed, Some(24.0));
        assert_eq!(s.config.density, Some(0.95));
    }

    #[test]
    fn calm_scene_uses_ocean_zen_density() {
        let s = get_scene("calm").expect("calm scene");
        assert_eq!(s.config.color, Some("ocean"));
        assert_eq!(s.config.charset, Some("minimal"));
        assert_eq!(s.config.speed, Some(6.0));
        assert_eq!(s.config.density, Some(0.40));
        assert_eq!(s.config.glitch_level, Some(GlitchLevel::None));
    }

    #[test]
    fn scenes_select_expected_rain_style() {
        assert_eq!(rain_style_for_scene("matrix"), Some(RainStyle::Glyph));
        assert_eq!(rain_style_for_scene("signal"), Some(RainStyle::Glyph));
        assert_eq!(rain_style_for_scene("monolith"), Some(RainStyle::Monolith));
    }

    #[test]
    fn monolith_scene_uses_premium_motion_defaults() {
        let monolith = get_scene("monolith").expect("monolith scene");
        assert_eq!(monolith.config.speed, Some(30.0));
        assert_eq!(monolith.config.density, Some(0.85));
        assert_eq!(monolith.config.charset, Some("zen"));
    }

    #[test]
    fn scene_parser_is_case_insensitive() {
        assert_eq!(validate_scene_name("SIGNAL").unwrap(), "signal");
        assert_eq!(validate_scene_name(" Monolith ").unwrap(), "monolith");
    }

    #[test]
    fn invalid_scene_error_mentions_discovery() {
        let err = validate_scene_name("nonexistent").unwrap_err();
        assert!(
            err.contains("error: unknown scene"),
            "scene error must use 'unknown' terminology: {err}"
        );
        assert!(
            err.contains("--list-scenes"),
            "scene error must reference --list-scenes: {err}"
        );
    }

    #[test]
    fn list_scenes_output_includes_all_scenes() {
        let text = list_scenes_text();
        for name in all_scene_names() {
            assert!(text.contains(name), "missing scene {name}");
        }
        assert!(
            text.contains("low-power"),
            "list must include low-power scene"
        );
        assert!(text.contains("storm"), "list must include storm scene");
    }

    #[test]
    fn show_scene_text_includes_header_and_usage() {
        let info = get_scene("storm").expect("storm scene");
        let text = show_scene_text(info);
        assert!(text.starts_with("SCENE: storm"), "header missing: {text}");
        assert!(
            text.contains("Description:"),
            "description label missing: {text}"
        );
        assert!(
            text.contains("Configuration:"),
            "config label missing: {text}"
        );
        assert!(
            text.contains("color        = purple"),
            "color field missing: {text}"
        );
        assert!(
            text.contains("fps          = 120"),
            "fps field missing: {text}"
        );
        assert!(
            text.contains("rain-style   = glyph"),
            "rain-style missing: {text}"
        );
        assert!(
            text.contains("cosmostrix --scene storm"),
            "usage hint missing: {text}"
        );
    }

    #[test]
    fn show_scene_text_handles_partial_scene() {
        // v18: all scenes now set all fields. This test verifies that
        // show_scene_text correctly renders a fully-populated scene.
        let info = get_scene("matrix").expect("matrix scene");
        let text = show_scene_text(info);
        assert!(text.contains("SCENE: matrix"), "header missing: {text}");
        assert!(
            text.contains("rain-style   = glyph"),
            "rain-style missing: {text}"
        );
        // v18: matrix now sets color, charset, fps, speed, density
        assert!(
            text.contains("color        = neon-green"),
            "color field missing: {text}"
        );
        assert!(
            text.contains("charset      = matrix"),
            "charset field missing: {text}"
        );
        assert!(
            text.contains("fps          = 60"),
            "fps field missing: {text}"
        );
    }
}

// Submodules (moved from src/ root for clean src/ layout)
pub(crate) mod charset;
pub(crate) mod charset_custom;
