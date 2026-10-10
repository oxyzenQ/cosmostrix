// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

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

use super::{ColorCapability, color_capability};

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
}
