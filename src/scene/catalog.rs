// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The built-in scene catalog (NIGHT-special-3 extraction, per the
//! RULES_LOC sibling-file policy: the catalog table outgrew
//! `scene/mod.rs`'s 800-line hard cap).
//!
//! Every entry is a complete seven-dimension profile (color,
//! charset, fps, speed, density, glitch level, rain style) — the
//! same completeness contract the scene-custom blocks enforce.
//! `SCENE_ORDER` (the interactive cycle) stays in `mod.rs` next to
//! the API; this file is the catalog the API reads. The
//! `scene_cycle_order_is_preserved` test pins the two together
//! (every catalog entry appears in the cycle exactly once), so a
//! new scene that forgets to join the cycle fails the suite.

use crate::config::GlitchLevel;
use crate::rain_style::RainStyle;

use super::{SceneConfig, SceneInfo};

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
    // owner's "sometimes circling, sometimes flying free anywhere" spec.
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
    // NIGHT-special-1: sorgonemous_intrascals — the black hole flagship
    // scene. The name is the owner's own coinage, born on a night walk
    // under the stars (kept verbatim per owner directive, joined with
    // an underscore to match the cosmic_dragon / matrix_film flagship
    // naming convention). The energy-zen palette + binary charset per
    // the owner spec: brand-crystal light wrapping an absolute void.
    // Stage 1 renders the event-horizon ball — a centered medium ball
    // with a black empty core and a photon-ring rim, dynamic for any
    // screen size. Speed 12 gives the hole a contemplative cadence;
    // density 0.55 is inert at stage 1 (no glyph rain yet) and will
    // drive the stage 3 infall when it lands. Glitch level none —
    // the void is still.
    SceneInfo {
        name: "sorgonemous_intrascals",
        description: "Sorgonemous Intrascals — black hole event horizon; a centered ball of light wrapping a black empty core, ringed by a three-tier accretion disk that pivots around the hole (glyph infall follows)",
        config: SceneConfig {
            color: Some("energy-zen"),
            charset: Some("binary"),
            fps: Some(60.0),
            speed: Some(12.0),
            density: Some(0.55),
            glitch_level: Some(GlitchLevel::None),
            rain_style: RainStyle::BlackHole,
        },
    },
    // NIGHT-special-2: aeolian — the invented string weave, the
    // project's first original-math rain style (the six laws were
    // derived in this repo — see cloud/type_rain/aeolian/mod.rs).
    // The name is the wind's own: the Aeolian harp is played by
    // moving air, this one by falling light. Aurora (557.7nm green,
    // the sky's own emission line) + runic strokes (the chime
    // marks). Speed 16 = calm weather; density 0.55 = flagship
    // parity (the calm-sky dial keeps the drizzle sparse anyway).
    SceneInfo {
        name: "aeolian",
        description: "Aeolian weave — the rain plays the instrument; glyphs fall onto invisible strings, pluck traveling light packets that sharpen as they race and knot where they cross, while the weather bends toward the resonance",
        config: SceneConfig {
            color: Some("aurora"),
            charset: Some("runic"),
            fps: Some(60.0),
            speed: Some(16.0),
            density: Some(0.55),
            glitch_level: Some(GlitchLevel::Subtle),
            rain_style: RainStyle::Aeolian,
        },
    },
    // NIGHT-special-4: solar_flare — the corona arcade, the third
    // original-math rain style (the five laws derived in this repo
    // — see cloud/type_rain/solar_flare/mod.rs; replaces the retired
    // aurora veil, NIGHT-special-3, owner-rated 5/10). The name is
    // the owner's own pick: the flare IS the magnetic arcade's
    // violent rebirth. Sun palette (the real-color golden-orange
    // photosphere ramp) + greek glyphs (the stellar-notation
    // strokes). Speed 14 = the majestic corona; density 0.50 = the
    // calm-sky dial (the arcade is the hero, the coronal rain a
    // minority layer); glitch none — the corona is clean, the
    // eruption is the drama.
    SceneInfo {
        name: "solar_flare",
        description: "Corona arcade — magnetic glyph arcs rooted on a granulated photosphere breathe and drift; coronal rain condenses at the loop tops and slides down the legs, flashing the footpoints it lands on; a flux-laden loop erupts — stretching, spraying ejecta, lifting off — and a fresh arc emerges from the surface",
        config: SceneConfig {
            color: Some("sun"),
            charset: Some("greek"),
            fps: Some(60.0),
            speed: Some(14.0),
            density: Some(0.50),
            glitch_level: Some(GlitchLevel::None),
            rain_style: RainStyle::SolarFlare,
        },
    },
    // NIGHT-research-7: dna_helix — the double helix, the owner's
    // DeepSeek-researched first pick (the second lands as the
    // murmuration). A rotating glyph ladder: two backbone strands
    // spiraling a vertical axis (one turn every 22 lines — 11 base
    // pairs per turn, B-DNA's 10.5 honored), spanned by Watson-Crick
    // rungs whose projected width breathes with the turn (the X
    // crossings drift as it rotates). The `neptune` palette (the
    // iconic deep azure — the classic DNA-illustration blue) pairs
    // with the `dna` charset (A/C/G/T bases): the rung ends show
    // real complementary pairs (A-T, G-C). The rain is the
    // nucleotide soup: falling base glyphs charged into the rungs
    // they land on (a mutation chance re-rolls the pair — the rain
    // visibly edits the genome). Periodically a replication fork
    // sweeps top-to-bottom: dissolving rungs, bowing the strands
    // apart (the Y), re-synthesizing fresh bright pairs behind
    // itself. Speed 14 = the majestic turn (a full rotation every
    // ~11 s at scene speed); density 0.50 = the calm-sky dial (the
    // molecule is the hero, the soup the minority layer); glitch
    // none — the genome is clean, the mutation is the drama.
    SceneInfo {
        name: "dna_helix",
        description: "DNA Helix — the rain writes the genome; two glyph strands spiral a rotating ladder of Watson-Crick base pairs, nucleotide rain charges the rungs it lands on, and a replication fork periodically unzips, sweeps and re-writes the molecule",
        config: SceneConfig {
            color: Some("neptune"),
            charset: Some("dna"),
            fps: Some(60.0),
            speed: Some(14.0),
            density: Some(0.50),
            glitch_level: Some(GlitchLevel::None),
            rain_style: RainStyle::DnaHelix,
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
    // --- Milestone scene: dragon_hunt (NIGHT-hunter-15) ---
    //
    // Commemorates the biggest bug hunt in cosmostrix history: the
    // "glitch rain shift" — the rain visibly shifting sideways for a
    // few seconds on real terminals (owner-reported on every terminal
    // class, first minute of a fresh session, monolith immune, the
    // first s/S/c/C shortkey re-triggering it as a left-to-right
    // lightning sweep). The hunt ran 26 rounds across HUNT-23..26
    // (output drain backoff, EMA pressure decoupling, the phosphor
    // park-epoch fix, the P2 resync full-body flash fix, the MADV NUL
    // emission fix, the amortized thaw) before the last ghost was run
    // to ground at e58f8b8.
    //
    // The scene is the visible reward: the Lorenz butterfly — the
    // engine's strange attractor, the motion the hunters chased the
    // ghost through — rendered in the `nebula` palette with the
    // `blocks` charset. Speed 22 (a hair under the lorenz flagship's
    // 24: the hunt is over, the butterfly glides). Density 0.70
    // matches the flagship. Glitch level NONE — the glitch is dead.
    SceneInfo {
        name: "dragon_hunt",
        description: "Dragon Hunt — the biggest bug hunt: the glitch rain shift, run to ground; the Lorenz butterfly glides clean through nebula where the ghost once hid",
        config: SceneConfig {
            color: Some("nebula"),
            charset: Some("blocks"),
            fps: Some(60.0),
            speed: Some(22.0),
            density: Some(0.70),
            glitch_level: Some(GlitchLevel::None),
            rain_style: RainStyle::Lorenz,
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
