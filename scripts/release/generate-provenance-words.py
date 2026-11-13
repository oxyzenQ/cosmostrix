#!/usr/bin/env python3
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: AGPL-3.0-only
"""Provenance word generator for the depth-matrix signature (night-signature-advanced-2).

Why this exists: the ownership claim must NOT exist as plaintext in the
compiled binary, because a thief can run strings(1) or rg over the
artifact, find the wording, and patch it out. This tool converts the two
provenance truths (the Signature manifest and the Copyright authors
string) into XOR-masked 64-bit words. The masks come from a splitmix64
stream keyed by per-layer seeds, so the constants in
src/diagnostics/signature.rs look like opaque checksum material ("nobody
knows what this is") and no contiguous ASCII fragment of the identity
survives into .rodata. At runtime the binary unmasks the words and
materializes the plaintext on the -V/--version surface only.

This file is the committed truth source for those constants: the identity
appears here ONCE by design, exactly like NOTICE and TRADEMARK.md carry
it as the legal layer. Rotating the identity (or the seeds) is one edit
plus one paste of the emitted Rust block.

The masking/unmasking algorithm is mirrored byte-exactly in
src/diagnostics/signature.rs (const fn splitmix64_mix + mask_for). If you
change one side, change both, and re-run this tool. The emitted block is
self-checked below: it unmasks its own words and asserts the round-trip
equals the input, so a paste-mistake or an algorithm drift fails here
before it can ship.

Usage:
    python3 scripts/release/generate-provenance-words.py
"""

# The provenance truths. Byte-level truth: UTF-8 (the em dash is three
# bytes, E2 80 94, and every layer below operates on bytes, not chars).
MANIFEST = "Cosmic Dragon — Official Build by rezky_nightky (oxyzenQ) 2026"
AUTHORS = "rezky_nightky (oxyzenQ) <with dot rezky at gmail dot com>"

# Per-layer seeds. Decoys with no external meaning; rotating them is a
# supported operation (re-run this tool, paste the new block). Both must
# differ so the two layers never produce identical .rodata bytes.
SEED_MANIFEST = 0x9D3A_7C1E_44B2_6F08
SEED_AUTHORS = 0x1F4E_6B8A_C27D_53A1

MASK64 = (1 << 64) - 1
GOLDEN = 0x9E3779B97F4A7C15
FNV_OFFSET = 0xCBF29CE484222325
FNV_PRIME = 0x100000001B3


def splitmix64_mix(z: int) -> int:
    """Byte-exact mirror of signature.rs splitmix64_mix."""
    a = (z + GOLDEN) & MASK64
    b = ((a ^ (a >> 30)) * 0xBF58476D1CE4E5B9) & MASK64
    c = ((b ^ (b >> 27)) * 0x94D049BB133111EB) & MASK64
    return (c ^ (c >> 31)) & MASK64


def mask_for(seed: int, index: int) -> int:
    """Mask stream for word `index`: mix(seed + (index+1) * GOLDEN)."""
    return splitmix64_mix((seed + (index + 1) * GOLDEN) & MASK64)


def chunk_words(data: bytes) -> list:
    """Split bytes into little-endian u64 words, zero-padded."""
    words = []
    for i in range(0, len(data), 8):
        chunk = data[i : i + 8]
        words.append(int.from_bytes(chunk, "little"))
    return words


def mask_words(words: list, seed: int) -> list:
    return [w ^ mask_for(seed, i) for i, w in enumerate(words)]


def unmask_words(words: list, seed: int, byte_len: int) -> bytes:
    out = bytearray()
    for i, w in enumerate(words):
        out += ((w ^ mask_for(seed, i)) & MASK64).to_bytes(8, "little")
    return bytes(out[:byte_len])


def fnv1a64(data: bytes) -> int:
    h = FNV_OFFSET
    for byte in data:
        h ^= byte
        h = (h * FNV_PRIME) & MASK64
    return h


def longest_ascii_run(words: list) -> int:
    """Longest run of printable ASCII bytes across all masked words.

    The at-rest invisibility contract: strings(1) reports runs of four
    or more printable bytes. The emitted constants must stay below that
    floor everywhere, or the identity becomes greppable again.
    """
    longest = 0
    run = 0
    for w in words:
        for byte in (w & MASK64).to_bytes(8, "little"):
            if 0x20 <= byte <= 0x7E:
                run += 1
                longest = max(longest, run)
            else:
                run = 0
    return longest


def rust_u64(value: int) -> str:
    return f"0x{value:016X}"


def rust_array(name: str, words: list, comment: str) -> str:
    body = ", ".join(rust_u64(w) for w in words)
    return f"const {name}: [u64; {len(words)}] = [{body}]; // {comment}"


def main() -> None:
    manifest_bytes = MANIFEST.encode("utf-8")
    authors_bytes = AUTHORS.encode("utf-8")

    manifest_words = chunk_words(manifest_bytes)
    authors_words = chunk_words(authors_bytes)
    masked_manifest = mask_words(manifest_words, SEED_MANIFEST)
    masked_authors = mask_words(authors_words, SEED_AUTHORS)

    # Self-check: the block must round-trip, and it must stay invisible
    # to strings(1)-style scans (no printable run of length >= 4).
    assert unmask_words(masked_manifest, SEED_MANIFEST, len(manifest_bytes)) == manifest_bytes
    assert unmask_words(masked_authors, SEED_AUTHORS, len(authors_bytes)) == authors_bytes
    assert longest_ascii_run(masked_manifest) < 4, "manifest words leak ASCII"
    assert longest_ascii_run(masked_authors) < 4, "authors words leak ASCII"

    print("// --- Generated by scripts/release/generate-provenance-words.py ---")
    print("// Paste into src/diagnostics/signature.rs (replacing the existing")
    print("// constants of the same names). Do not hand-edit the words.")
    print()
    print(f"pub(crate) const MANIFEST_SEED: u64 = {rust_u64(SEED_MANIFEST)};")
    print(
        f"pub(crate) const MANIFEST_BYTE_LEN: usize = {len(manifest_bytes)}; "
        f"// {len(manifest_words)} words"
    )
    print(rust_array("MASKED_MANIFEST_WORDS", masked_manifest, "layer D0"))
    print()
    print(f"pub(crate) const AUTHORS_SEED: u64 = {rust_u64(SEED_AUTHORS)};")
    print(
        f"pub(crate) const AUTHORS_BYTE_LEN: usize = {len(authors_bytes)}; "
        f"// {len(authors_words)} words"
    )
    print(rust_array("MASKED_AUTHORS_WORDS", masked_authors, "layer D1"))
    print()
    print("// Cross-check values (informational; the Rust const fingerprints")
    print("// are computed independently from the words at compile time):")
    print(f"// manifest fingerprint: 0x{fnv1a64(manifest_bytes):016X}")
    print(f"// authors fingerprint:  0x{fnv1a64(authors_bytes):016X}")
    print(
        f"// longest printable run across masked words: "
        f"{max(longest_ascii_run(masked_manifest), longest_ascii_run(masked_authors))}"
    )


if __name__ == "__main__":
    main()
