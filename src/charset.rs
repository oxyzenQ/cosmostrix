// Copyright (c) 2025 rezk_nightky

use std::char;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Charset(u32);

impl Charset {
    pub const NONE: Charset = Charset(0);
    pub const ENGLISH_LETTERS: Charset = Charset(0x1);
    pub const ENGLISH_DIGITS: Charset = Charset(0x2);
    pub const ENGLISH_PUNCTUATION: Charset = Charset(0x4);
    pub const KATAKANA: Charset = Charset(0x8);
    pub const GREEK: Charset = Charset(0x10);
    pub const CYRILLIC: Charset = Charset(0x20);
    pub const ARABIC: Charset = Charset(0x40);
    pub const HEBREW: Charset = Charset(0x80);
    pub const BINARY: Charset = Charset(0x100);
    pub const HEX: Charset = Charset(0x200);
    pub const DEVANAGARI: Charset = Charset(0x400);
    pub const BRAILLE: Charset = Charset(0x800);
    pub const RUNIC: Charset = Charset(0x1000);

    pub const DEFAULT: Charset = Charset(0x7);
    pub const EXTENDED_DEFAULT: Charset = Charset(0xE);

    pub fn contains(self, other: Charset) -> bool {
        (self.0 & other.0) != 0
    }
}

pub fn parse_user_hex_chars(s: &str) -> Result<Vec<char>, String> {
    let mut out = Vec::new();
    for (i, part) in s.split(',').enumerate() {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let v = u32::from_str_radix(part, 16)
            .map_err(|_| format!("invalid hex char at index {}", i + 1))?;
        let ch = char::from_u32(v)
            .ok_or_else(|| format!("invalid unicode scalar at index {}", i + 1))?;
        out.push(ch);
    }
    Ok(out)
}

pub fn charset_from_str(spec: &str, default_to_ascii: bool) -> Result<Charset, String> {
    let spec = spec.trim().to_ascii_lowercase();
    match spec.as_str() {
        "auto" => Ok(if default_to_ascii {
            Charset::DEFAULT
        } else {
            Charset::EXTENDED_DEFAULT
        }),
        "ascii" => Ok(Charset::DEFAULT),
        "extended" => Ok(Charset::EXTENDED_DEFAULT),
        "english" => Ok(Charset::ENGLISH_LETTERS),
        "digits" | "dec" | "decimal" => Ok(Charset::ENGLISH_DIGITS),
        "punc" => Ok(Charset::ENGLISH_PUNCTUATION),
        "bin" | "binary" => Ok(Charset::BINARY),
        "hex" | "hexadecimal" => Ok(Charset::HEX),
        "katakana" => Ok(Charset::KATAKANA),
        "greek" => Ok(Charset::GREEK),
        "cyrillic" => Ok(Charset::CYRILLIC),
        "arabic" => Ok(Charset::ARABIC),
        "hebrew" => Ok(Charset::HEBREW),
        "devanagari" => Ok(Charset::DEVANAGARI),
        "braille" => Ok(Charset::BRAILLE),
        "runic" => Ok(Charset::RUNIC),
        _ => Err(format!("unsupported charset: {}", spec)),
    }
}

fn push_range(out: &mut Vec<char>, start: u32, end: u32) {
    for v in start..=end {
        if let Some(ch) = char::from_u32(v) {
            out.push(ch);
        }
    }
}

pub fn build_chars(
    mut charset: Charset,
    user_ranges: &[(char, char)],
    default_to_ascii: bool,
) -> Vec<char> {
    if charset == Charset::NONE && user_ranges.is_empty() {
        charset = if default_to_ascii {
            Charset::DEFAULT
        } else {
            Charset::EXTENDED_DEFAULT
        };
    }

    let mut out: Vec<char> = Vec::new();

    if charset.contains(Charset::BINARY) {
        push_range(&mut out, 0x30, 0x31);
    }
    if charset.contains(Charset::HEX) {
        push_range(&mut out, 0x30, 0x39);
        push_range(&mut out, 0x41, 0x46);
    }
    if charset.contains(Charset::ENGLISH_LETTERS) {
        push_range(&mut out, 0x41, 0x5A);
        push_range(&mut out, 0x61, 0x7A);
    }
    if charset.contains(Charset::ENGLISH_DIGITS) {
        push_range(&mut out, 0x30, 0x39);
    }
    if charset.contains(Charset::ENGLISH_PUNCTUATION) {
        push_range(&mut out, 0x21, 0x2F);
        push_range(&mut out, 0x3A, 0x40);
        push_range(&mut out, 0x5B, 0x60);
        push_range(&mut out, 0x7B, 0x7E);
    }
    if charset.contains(Charset::KATAKANA) {
        push_range(&mut out, 0xFF64, 0xFF9F);
    }
    if charset.contains(Charset::GREEK) {
        push_range(&mut out, 0x0370, 0x03FF);
    }
    if charset.contains(Charset::CYRILLIC) {
        push_range(&mut out, 0x0410, 0x044F);
    }
    if charset.contains(Charset::ARABIC) {
        push_range(&mut out, 0x0627, 0x0649);
    }
    if charset.contains(Charset::HEBREW) {
        push_range(&mut out, 0x0590, 0x05FF);
        push_range(&mut out, 0xFB1D, 0xFB4F);
    }
    if charset.contains(Charset::DEVANAGARI) {
        push_range(&mut out, 0x0900, 0x097F);
    }
    if charset.contains(Charset::BRAILLE) {
        push_range(&mut out, 0x2800, 0x28FF);
    }
    if charset.contains(Charset::RUNIC) {
        push_range(&mut out, 0x16A0, 0x16FF);
    }

    for &(a, b) in user_ranges {
        let start = a as u32;
        let end = b as u32;
        for v in start..=end {
            if let Some(ch) = char::from_u32(v) {
                out.push(ch);
            }
        }
    }

    if out.is_empty() {
        out.push('0');
        out.push('1');
    }

    out
}
