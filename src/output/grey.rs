// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! The calm grey grammar tier (night-improve-2).
//!
//! One concern, one module: every flag spelling the help renders —
//! definition rows and inline prose mentions alike — rides this tier,
//! the zelynic flag-row law ("flags are grammar; grey says context,
//! not content"). The span covers the spelling exactly (a value hint
//! like `<name>` rides inside it; the indent, padding, and surrounding
//! prose never do), so piped output keeps its plain bytes. Runnable
//! example lines stay the bold Matrix green tier (NIGHT-boost-2), and
//! section headings stay the bold brand purple tier.

use super::{ColorCapability, color_capability, reset};

/// Calm grey open sequence, capability-parameterized (unit-testable
/// without env sniffing — the same four-rung ladder as
/// `brand_bold_open_for`).
///
/// Color provenance (night-improve-2, zelynic lineage): the calm grey
/// #8B8B8B zelynic's help paints every flag spelling with, quantized
/// per capability — 256-color index 245 (the same rung zelynic pins
/// for its grey tier), ANSI bright black (90) as the classic 16-color
/// grey anchor, and plain text on mono.
fn grey_open_for(cap: ColorCapability) -> &'static str {
    match cap {
        ColorCapability::TrueColor => "\x1b[38;2;139;139;139m",
        ColorCapability::Color256 => "\x1b[38;5;245m",
        ColorCapability::Color16 => "\x1b[90m",
        ColorCapability::Mono => "",
    }
}

/// Calm grey open sequence for the active capability.
pub(crate) fn grey_open() -> &'static str {
    grey_open_for(color_capability())
}

/// Wrap `msg` in the calm grey tier for the STDERR lane - the
/// loading lane's seat (night-ux-orion-1): a narration is secondary
/// content, dimmer than every report byte it annotates. Plain text
/// when color is off, so piped output keeps its plain bytes.
pub(crate) fn grey_err(msg: &str) -> String {
    let cap = color_capability();
    match cap {
        ColorCapability::Mono => msg.to_owned(),
        _ => format!("{}{msg}{}", grey_open_for(cap), reset()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The grey ladder must match the provenance contract exactly:
    /// truecolor #8B8B8B, the 256-color 245 rung, the classic bright
    /// black 16-color anchor, and nothing on mono.
    #[test]
    fn grey_open_is_capability_laddered() {
        assert_eq!(
            grey_open_for(ColorCapability::TrueColor),
            "\x1b[38;2;139;139;139m"
        );
        assert_eq!(grey_open_for(ColorCapability::Color256), "\x1b[38;5;245m");
        assert_eq!(grey_open_for(ColorCapability::Color16), "\x1b[90m");
        assert_eq!(grey_open_for(ColorCapability::Mono), "");
    }

    /// The stderr-lane wrapper: painted grey on every capable rung,
    /// plain bytes on mono - the piped run's output law.
    #[test]
    fn grey_err_wraps_the_ladder_and_stays_plain_on_mono() {
        assert_eq!(grey_err("[waiting... 1s]"), "[waiting... 1s]");
        assert!(grey_err("[waiting... 1s]").contains("[waiting... 1s]"));
        assert!(!grey_err("[waiting... 1s]").starts_with('\x1b'));
    }
}
