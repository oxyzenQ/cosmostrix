#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# NIGHT-research-3 probe: replicates the OKLab polar gradient math from
# src/engine/chroma_dragon_engine/gradient/mod.rs in Python and measures
# how often interpolated samples exit the sRGB gamut (per-channel clamp
# territory), for real catalog theme stops and worst-case synthetic
# opposing-hue pairs. For each clipped sample it also computes what a
# CSS-style chroma-reduction gamut map (binary search on C at fixed L, h)
# would have produced instead, and the perceptual hue/L distance between
# the clamped result and the gamut-mapped result.
# Usage: python3 benchmark/research/oklab_gamut_probe.py

import math

# --- sRGB transfer + OKLab matrices, verbatim from gradient/mod.rs ---


def srgb_to_linear(c):
    cs = c / 255.0
    if cs <= 0.04045:
        return cs / 12.92
    return ((cs + 0.055) / 1.055) ** 2.4


def linear_to_srgb(c):
    if c <= 0.0031308:
        cs = 12.92 * c
    else:
        cs = 1.055 * c ** (1.0 / 2.4) - 0.055
    return cs * 255.0


def linear_to_oklab(r, g, b):
    l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b
    m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b
    s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b
    l_, m_, s_ = l ** (1 / 3), m ** (1 / 3), s ** (1 / 3)
    return (
        0.2104542553 * l_ + 0.7936177850 * m_ - 0.0040720468 * s_,
        1.9779984951 * l_ - 2.4285922050 * m_ + 0.4505937099 * s_,
        0.0259040371 * l_ + 0.7827717662 * m_ - 0.8086757660 * s_,
    )


def oklab_to_linear(L, a, b):
    l_ = L + 0.3963377774 * a + 0.2158037573 * b
    m_ = L - 0.1055613458 * a - 0.0638541728 * b
    s_ = L - 0.0894841775 * a - 1.2914855480 * b
    li, mi, si = l_**3, m_**3, s_**3
    return (
        4.0767416621 * li - 3.3077115913 * mi + 0.2309699292 * si,
        -1.2684380046 * li + 2.6097574011 * mi - 0.3413193965 * si,
        -0.0041960863 * li - 0.7034186147 * mi + 1.7076147010 * si,
    )


def srgb_to_oklab(rgb):
    return linear_to_oklab(*(srgb_to_linear(c) for c in rgb))


def polar_chroma_lerp(a0, b0, a1, b1, t):
    """Shortest-arc chroma rotation, exactly as gradient/mod.rs."""
    c0 = math.hypot(a0, b0)
    c1 = math.hypot(a1, b1)
    if c0 < 1e-6 or c1 < 1e-6:
        return (a0 + (a1 - a0) * t, b0 + (b1 - b0) * t)
    h0 = math.atan2(b0, a0)
    h1 = math.atan2(b1, a1)
    delta = h1 - h0
    if delta > math.pi:
        delta -= 2 * math.pi
    elif delta < -math.pi:
        delta += 2 * math.pi
    c = c0 + (c1 - c0) * t
    h = h0 + delta * t
    return (c * math.cos(h), c * math.sin(h))


def oklch_to_oklab(L, C, h):
    return (L, C * math.cos(h), C * math.sin(h))


def in_gamut(lin):
    return all(-1e-9 <= v <= 1.0 + 1e-9 for v in lin)


def chroma_reduce(L, C, h):
    """CSS-style gamut map: keep L and hue, binary-search max in-gamut C."""
    lo, hi = 0.0, C
    if in_gamut(oklab_to_linear(*oklch_to_oklab(L, hi, h))):
        return hi
    for _ in range(24):
        mid = (lo + hi) / 2
        if in_gamut(oklab_to_linear(*oklch_to_oklab(L, mid, h))):
            lo = mid
        else:
            hi = mid
    return lo


def probe(name, stops, steps=9):
    """Sample a stops list like gradient_from_stops_oklab, count clips."""
    ok = [srgb_to_oklab(s) for s in stops]
    segs = len(stops) - 1
    clipped = 0
    max_ov = 0.0
    max_hue = 0.0
    max_l = 0.0
    for i in range(steps):
        t = i / (steps - 1)
        pos = t * segs
        seg = min(int(pos), segs - 1)
        lt = pos - seg
        l0, a0, b0 = ok[seg]
        l1, a1, b1 = ok[seg + 1]
        L = l0 + (l1 - l0) * lt
        a, b = polar_chroma_lerp(a0, b0, a1, b1, lt)
        lin = oklab_to_linear(L, a, b)
        if not in_gamut(lin):
            clipped += 1
            max_ov = max(max_ov, max(-v for v in lin), max(v - 1.0 for v in lin))
            C = math.hypot(a, b)
            h = math.atan2(b, a)
            Cg = chroma_reduce(L, C, h)
            rgb_c = [min(1.0, max(0.0, v)) for v in lin]
            oc = linear_to_oklab(*rgb_c)
            og = linear_to_oklab(*oklab_to_linear(*oklch_to_oklab(L, Cg, h)))
            dh = abs(
                (math.atan2(oc[2], oc[1]) - math.atan2(og[2], og[1]) + math.pi)
                % (2 * math.pi)
                - math.pi
            )
            max_hue = max(max_hue, math.degrees(dh))
            max_l = max(max_l, abs(oc[0] - og[0]))
    print(
        f"{name:12s} clipped {clipped}/{steps}  maxOvershoot {max_ov:.4f}  "
        f"hueShift(clamp vs reduce) {max_hue:5.2f} deg  Lshift {max_l:.4f}"
    )
    return clipped, max_ov, max_hue


# Real stop sets copied from src/engine/chroma_dragon_engine/catalog/themes.rs
THEMES = {
    "Blue": [
        (0, 5, 22),
        (0, 28, 95),
        (21, 83, 159),
        (56, 142, 227),
        (80, 175, 255),
        (118, 192, 255),
        (155, 210, 255),
        (190, 223, 242),
    ],
    "Ocean": [
        (0, 5, 18),
        (0, 28, 65),
        (13, 85, 118),
        (42, 148, 175),
        (60, 185, 210),
        (98, 205, 228),
        (135, 225, 245),
        (180, 233, 242),
    ],
    "Rainbow": [
        (232, 89, 74),
        (219, 109, 0),
        (170, 143, 0),
        (64, 169, 55),
        (0, 173, 186),
        (15, 146, 247),
        (161, 112, 235),
    ],
    "Cosmos": [
        (3, 3, 18),
        (15, 18, 60),
        (52, 47, 136),
        (94, 80, 221),
        (120, 100, 255),
        (150, 125, 255),
        (180, 150, 255),
        (213, 194, 248),
    ],
}

if __name__ == "__main__":
    print("== real catalog themes, 9-step polar OKLab (production path) ==")
    for theme_name, stops in THEMES.items():
        probe(theme_name, stops)
    print()
    print("== worst-case synthetic opposing pairs (not a cosmostrix pattern) ==")
    probe("red-cyan", [(255, 0, 0), (0, 255, 255)])
    probe("blue-yellow", [(0, 0, 255), (255, 255, 0)])
