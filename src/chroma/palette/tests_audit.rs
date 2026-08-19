// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

use super::*;

/// A pair of schemes + their average RGB distance. Used by the audit
/// test to keep clippy's type_complexity lint happy.
type SchemePair = (ColorScheme, ColorScheme, f64);

/// A scheme + its TrueColor RGB stops. Factored out to satisfy
/// clippy's type_complexity lint on the Vec<(Scheme, Vec<...>)> type.
type SchemeStops = (ColorScheme, Vec<(u8, u8, u8)>);

/// Disposition of a known near-duplicate theme pair.
///
/// The audit test (`audit_near_duplicate_themes_act`) fails when a
/// near-duplicate pair (avg RGB distance < 30) is discovered that is
/// NOT listed in `KNOWN_NEAR_DUPLICATES`. Each listed pair must have
/// an explicit disposition + reason, so accidental near-duplicates
/// from newly added themes are caught at PR time while intentional
/// ones remain documented.
///
/// `Differentiate` and `Merge` are not currently used by any entry
/// in `KNOWN_NEAR_DUPLICATES` (all 13 pairs are `Intentional` as of
/// v25). They exist for future use — when a developer adds a new
/// theme that's too close to an existing one, they can mark the pair
/// as `Differentiate` or `Merge` to flag technical debt without
/// blocking the PR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Differentiate/Merge variants reserved for future use
enum Disposition {
    /// The two themes are intentionally similar — they belong to the
    /// same aesthetic family (e.g. "planets", "synthwave") and the
    /// subtle difference is a deliberate user-facing choice.
    Intentional,
    /// The two themes are too close and should be made more distinct.
    /// The test will still pass (the pair is allowlisted), but a
    /// follow-up issue should be filed to differentiate them.
    Differentiate,
    /// One of the two themes should be removed (merged into the
    /// other). The test will still pass, but a follow-up issue
    /// should be filed to deprecate the redundant theme.
    Merge,
}

/// A known near-duplicate pair + its disposition + a human-readable
/// reason. The reason is printed by the audit test so reviewers
/// understand why the pair was allowlisted.
struct NearDupDisposition {
    a: ColorScheme,
    b: ColorScheme,
    disposition: Disposition,
    reason: &'static str,
}

/// The allowlist of known near-duplicate theme pairs.
    ///
    /// Every pair in this list has avg RGB distance < 30 (the audit
    /// threshold). Pairs NOT in this list that fall below the threshold
    /// will cause `audit_near_duplicate_themes_act` to FAIL — the
    /// developer must either:
    ///   - differentiate the new theme so it's no longer a near-dup, or
    ///   - add an entry here with an explicit disposition + reason.
    ///
    /// Dispositions as of v25 (all 13 known pairs):
    ///   - All marked `Intentional` — they belong to deliberate aesthetic
    ///     families (planets, synthwave, neon variants, grayscale variants).
    ///
    /// If a pair is later marked `Differentiate` or `Merge`, the test
    /// still passes (the pair is allowlisted), but the disposition flags
    /// the need for a follow-up issue.
    #[rustfmt::skip]
    const KNOWN_NEAR_DUPLICATES: &[NearDupDisposition] = &[
        NearDupDisposition {
            a: ColorScheme::Venus, b: ColorScheme::Saturn,
            disposition: Disposition::Intentional,
            reason: "Both are warm-amber planet palettes (Venus yellow-cream, \
                     Saturn gold-amber). Part of the planets family; the subtle \
                     hue shift is the user-facing distinction.",
        },
        NearDupDisposition {
            a: ColorScheme::Neon, b: ColorScheme::Vaporwave,
            disposition: Disposition::Intentional,
            reason: "Both synthwave-inspired. Neon is magenta+cyan, Vaporwave \
                     is pink+cyan with a slightly different hue balance. Same \
                     aesthetic family, distinct enough that users prefer one.",
        },
        NearDupDisposition {
            a: ColorScheme::Mercury, b: ColorScheme::Moon,
            disposition: Disposition::Intentional,
            reason: "Both grayscale planets. Mercury is warm gray (sun-baked), \
                     Moon is cool gray (cold). Reflects actual color-temperature \
                     difference between the two bodies.",
        },
        NearDupDisposition {
            a: ColorScheme::Green, b: ColorScheme::NeonGreen,
            disposition: Disposition::Intentional,
            reason: "Both green. NeonGreen has a more saturated/neon body. \
                     Intentional variant — users requested a 'punchier' green.",
        },
        NearDupDisposition {
            a: ColorScheme::Carbon, b: ColorScheme::Gray,
            disposition: Disposition::Intentional,
            reason: "Both grayscale. Carbon has a cool blue tint (tech/industrial \
                     aesthetic), Gray is more neutral. Different aesthetic identity.",
        },
        NearDupDisposition {
            a: ColorScheme::Venus, b: ColorScheme::Jupiter,
            disposition: Disposition::Intentional,
            reason: "Both warm planet palettes. Venus is yellow-cream, Jupiter \
                     is tan-brown. Part of the planets family.",
        },
        NearDupDisposition {
            a: ColorScheme::Orange, b: ColorScheme::Fire,
            disposition: Disposition::Intentional,
            reason: "Both warm orange-red. Orange is pure orange, Fire has \
                     more red at the trail. Different aesthetic intent.",
        },
        NearDupDisposition {
            a: ColorScheme::NeonPurple, b: ColorScheme::Purple,
            disposition: Disposition::Intentional,
            reason: "Both purple. NeonPurple is more saturated/neon, Purple \
                     is more royal/lavender. Same pattern as Green/NeonGreen.",
        },
        NearDupDisposition {
            a: ColorScheme::Yellow, b: ColorScheme::Gold,
            disposition: Disposition::Intentional,
            reason: "Both yellow-gold. Yellow is pure signal yellow, Gold has \
                     a brown tint (polished metal aesthetic).",
        },
        NearDupDisposition {
            a: ColorScheme::Jupiter, b: ColorScheme::Saturn,
            disposition: Disposition::Intentional,
            reason: "Both warm planet palettes. Jupiter is tan-brown, Saturn \
                     is gold-amber. Part of the planets family.",
        },
        NearDupDisposition {
            a: ColorScheme::Purple, b: ColorScheme::Nebula,
            disposition: Disposition::Intentional,
            reason: "Both purple-ish. Purple is saturated royal, Nebula has \
                     more blue-violet (nebula gas aesthetic).",
        },
        NearDupDisposition {
            a: ColorScheme::Green, b: ColorScheme::Green2,
            disposition: Disposition::Intentional,
            reason: "Both green. Green is the original, Green2 is a slightly \
                     brighter variant added as a user-requested alternative.",
        },
        NearDupDisposition {
            a: ColorScheme::Snow, b: ColorScheme::FancyDiamond,
            disposition: Disposition::Intentional,
            reason: "Both cool-cyan-white. Snow is pure white-blue, \
                     FancyDiamond has iridescent cyan-magenta (prismatic \
                     diamond aesthetic).",
        },
        NearDupDisposition {
            a: ColorScheme::Blue, b: ColorScheme::Ocean,
            disposition: Disposition::Intentional,
            reason: "Both blue-family. Blue is pure royal blue, Ocean is \
                     blue-cyan (sea-water aesthetic). Polar gradient (sole \
                     path since v30) shifted intermediate colors so the \
                     avg RGB distance dropped to 29.9 (just below the 30 \
                     threshold). The themes are visually distinct — Blue \
                     stays royal throughout, Ocean has a visible cyan \
                     body/tail. Different aesthetic intent.",
        },
        NearDupDisposition {
            a: ColorScheme::Gray, b: ColorScheme::Mercury,
            disposition: Disposition::Intentional,
            reason: "Both grayscale-family. Gray is neutral gray, Mercury \
                     is warm-tinted gray (slight brown vs pure gray). \
                     Intermediate stop additions (gradient smoothing) \
                     reduced the avg RGB distance to 28.9. Visually \
                     distinct — Mercury has a warm tint, Gray is pure neutral.",
        },
        NearDupDisposition {
            a: ColorScheme::Stars, b: ColorScheme::Pluto,
            disposition: Disposition::Intentional,
            reason: "Both deep-space blue-white. Stars is pinprick starlight \
                     (sparse white-gold on black), Pluto is icy blue-gray \
                     (distant dwarf planet). Intermediate stop additions \
                     (gradient smoothing) reduced avg RGB distance to 29.0. \
                     Visually distinct — Stars is darker/sparser, Pluto is \
                     brighter with a blue tint.",
        },
    ];

/// Extract the TrueColor RGB stops for a scheme as a Vec<(u8,u8,u8)>.
fn truecolor_stops(scheme: ColorScheme) -> Vec<(u8, u8, u8)> {
    let p = build_palette(scheme, ColorMode::TrueColor, true);
    p.colors.iter().map(|c| color_to_rgb(*c)).collect()
}

/// Average per-stop RGB Euclidean distance between two palettes.
fn palette_distance(a: &[(u8, u8, u8)], b: &[(u8, u8, u8)]) -> f64 {
    let n = a.len().min(b.len()).max(1);
    let mut sum = 0.0_f64;
    for i in 0..n {
        let (r1, g1, b1) = a[i];
        let (r2, g2, b2) = b[i];
        let dr = (i32::from(r1) - i32::from(r2)) as f64;
        let dg = (i32::from(g1) - i32::from(g2)) as f64;
        let db = (i32::from(b1) - i32::from(b2)) as f64;
        sum += (dr * dr + dg * dg + db * db).sqrt();
    }
    sum / n as f64
}

fn all_schemes() -> Vec<ColorScheme> {
    use ColorScheme::*;
    vec![
        Green,
        Green2,
        Green3,
        NeonGreen,
        NeonPurple,
        NeonWhite,
        NeonBlue,
        NeonRed,
        NeonOrange,
        NeonYellow,
        NeonCyan,
        Carbon,
        Yellow,
        Orange,
        Red,
        Blue,
        Cyan,
        Gold,
        Rainbow,
        Purple,
        Neon,
        Fire,
        Ocean,
        Forest,
        Vaporwave,
        Gray,
        Snow,
        Aurora,
        FancyDiamond,
        Cosmos,
        Nebula,
        Spectrum20,
        Stars,
        Mars,
        Venus,
        Mercury,
        Jupiter,
        Saturn,
        Uranus,
        Neptune,
        Pluto,
        Moon,
        Sun,
    ]
}

/// Audit test: identify near-duplicate themes (avg RGB distance < 30).
/// Prints findings to stderr so they're visible during `cargo test`.
/// Does NOT assert — this is an informational audit, not a pass/fail gate.
#[test]
fn audit_near_duplicate_themes() {
    let schemes = all_schemes();
    let stops: Vec<SchemeStops> = schemes.iter().map(|&s| (s, truecolor_stops(s))).collect();

    let mut near_dups: Vec<SchemePair> = Vec::new();
    for i in 0..stops.len() {
        for j in (i + 1)..stops.len() {
            let (s1, p1) = &stops[i];
            let (s2, p2) = &stops[j];
            let dist = palette_distance(p1, p2);
            if dist < 30.0 {
                near_dups.push((*s1, *s2, dist));
            }
        }
    }
    near_dups.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());

    eprintln!("\n=== Theme Audit: Near-Duplicate Pairs (avg RGB dist < 30) ===");
    if near_dups.is_empty() {
        eprintln!("  None found.");
    } else {
        for (s1, s2, dist) in &near_dups {
            eprintln!("  {:?} <-> {:?}: {:.1}", s1, s2, dist);
        }
    }

    // Also print the 5 closest pairs regardless of threshold, for context.
    eprintln!("\n=== 5 Closest Pairs (for context) ===");
    let mut all_dists: Vec<SchemePair> = Vec::new();
    for i in 0..stops.len() {
        for j in (i + 1)..stops.len() {
            let (s1, p1) = &stops[i];
            let (s2, p2) = &stops[j];
            all_dists.push((*s1, *s2, palette_distance(p1, p2)));
        }
    }
    all_dists.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());
    for (s1, s2, dist) in all_dists.iter().take(5) {
        eprintln!("  {:?} <-> {:?}: {:.1}", s1, s2, dist);
    }
}

/// Audit test (actionable): every near-duplicate pair (avg RGB
/// distance < 30) MUST be listed in `KNOWN_NEAR_DUPLICATES` with an
/// explicit disposition + reason.
///
/// This is the "actionable" successor to `audit_near_duplicate_themes`
/// (which only prints). It catches accidental near-duplicates from
/// newly added themes at PR time — the developer must either
/// differentiate the new theme or add an explicit disposition entry.
///
/// Pairs already in `KNOWN_NEAR_DUPLICATES` are allowed to pass; the
/// test prints their disposition + reason for reviewer visibility.
/// Pairs NOT in the allowlist cause the test to FAIL with a helpful
/// message naming the offending pair and its distance.
///
/// ## Adding a new theme
///
/// If you add a new `ColorScheme` variant and this test fails:
///   1. Look at the printed near-dup pair — is the new theme too
///      close to an existing one?
///   2. If yes, decide:
///      - Differentiate the new theme (adjust stops until distance
///        >= 30). Re-run the test — it should pass.
///      - OR add an entry to `KNOWN_NEAR_DUPLICATES` with
///        `Disposition::Intentional` (or `Differentiate`/`Merge` if
///        the similarity is a problem to fix later) and a reason.
///   3. Commit the change.
///
/// ## Disposition hygiene
///
/// Pairs marked `Differentiate` or `Merge` indicate technical debt —
/// the test still passes, but a follow-up issue should be filed to
/// either differentiate or remove the redundant theme. The
/// disposition serves as the issue's justification.
#[test]
fn audit_near_duplicate_themes_act() {
    let schemes = all_schemes();
    let stops: Vec<SchemeStops> = schemes.iter().map(|&s| (s, truecolor_stops(s))).collect();

    // Build the list of currently-near-duplicate pairs.
    let mut near_dups: Vec<SchemePair> = Vec::new();
    for i in 0..stops.len() {
        for j in (i + 1)..stops.len() {
            let (s1, p1) = &stops[i];
            let (s2, p2) = &stops[j];
            let dist = palette_distance(p1, p2);
            if dist < 30.0 {
                near_dups.push((*s1, *s2, dist));
            }
        }
    }
    near_dups.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());

    eprintln!("\n=== Actionable Near-Duplicate Audit ===");
    eprintln!("Threshold: avg RGB dist < 30.0");
    eprintln!("Allowlist size: {} pairs", KNOWN_NEAR_DUPLICATES.len());
    eprintln!();

    let mut unlisted: Vec<SchemePair> = Vec::new();
    for (a, b, dist) in &near_dups {
        // Look up the pair in KNOWN_NEAR_DUPLICATES. The pair may be
        // listed in either order (a,b) or (b,a), so check both.
        let found = KNOWN_NEAR_DUPLICATES
            .iter()
            .find(|d| (d.a == *a && d.b == *b) || (d.a == *b && d.b == *a));

        match found {
            Some(d) => {
                eprintln!(
                    "  [OK] {:?} <-> {:?} ({:.1}): {:?} — {}",
                    a, b, dist, d.disposition, d.reason
                );
            }
            None => {
                eprintln!(
                    "  [MISSING] {:?} <-> {:?} ({:.1}): NOT in KNOWN_NEAR_DUPLICATES",
                    a, b, dist
                );
                unlisted.push((*a, *b, *dist));
            }
        }
    }

    // Also check for stale allowlist entries — pairs that ARE in
    // KNOWN_NEAR_DUPLICATES but no longer near-duplicate (distance
    // >= 30). These should be removed from the allowlist.
    let mut stale: Vec<&NearDupDisposition> = Vec::new();
    for d in KNOWN_NEAR_DUPLICATES {
        let still_near = near_dups
            .iter()
            .any(|(a, b, _)| (*a == d.a && *b == d.b) || (*a == d.b && *b == d.a));
        if !still_near {
            stale.push(d);
        }
    }
    if !stale.is_empty() {
        eprintln!("\n=== Stale Allowlist Entries (no longer near-duplicate) ===");
        for d in &stale {
            eprintln!(
                "  {:?} <-> {:?}: listed but distance >= 30; remove from KNOWN_NEAR_DUPLICATES",
                d.a, d.b
            );
        }
    }

    // The actionable assertion: every near-dup must be allowlisted.
    assert!(
        unlisted.is_empty(),
        "Found {} near-duplicate pair(s) NOT in KNOWN_NEAR_DUPLICATES.\n\
             Either differentiate the themes (adjust stops until avg RGB dist >= 30)\n\
             or add explicit disposition entries to KNOWN_NEAR_DUPLICATES in\n\
             src/chroma/palette.rs.\n\
             Unlisted pairs:\n{}",
        unlisted.len(),
        unlisted
            .iter()
            .map(|(a, b, d)| format!("  - {:?} <-> {:?} (dist {:.1})", a, b, d))
            .collect::<Vec<_>>()
            .join("\n")
    );

    // Stale entries don't fail the test (they're harmless), but
    // the printed message above flags them for cleanup.
    let _ = stale; // silence unused warning if empty
}
