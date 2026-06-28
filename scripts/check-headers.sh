#!/usr/bin/env bash
# =============================================================================
# COSMOSTRIX SPDX HEADER CHECK
# =============================================================================
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-or-later
# =============================================================================
# Scans all core/code/config/script files for required SPDX-License-Identifier
# headers. Fails if any included file is missing the header or has the wrong
# license identifier.
#
# Required: SPDX-License-Identifier: GPL-3.0-or-later
# Rejected: MIT (project is GPL-3.0-or-later licensed)
#
# Included file types: *.rs, *.sh, *.toml, *.yml, *.yaml
# Excluded: target/, .git/, Cargo.lock, *.md, *.txt, assets, media
#
# Usage: bash scripts/check-headers.sh
# =============================================================================
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Colors
RED='\033[0;31m'
NC='\033[0m'

MISSING=0
WRONG=0
CHECKED=0

EXPECTED_LICENSE="GPL-3.0-or-later"
REJECTED_LICENSE="MIT"

while IFS= read -r -d '' file; do
    CHECKED=$((CHECKED + 1))

    # Check if the file has any SPDX-License-Identifier at all
    if ! head -10 "$file" | grep -q "SPDX-License-Identifier"; then
        echo -e "${RED}MISSING SPDX header: ${file}${NC}"
        MISSING=$((MISSING + 1))
        continue
    fi

    # Check for rejected license identifiers
    if head -10 "$file" | grep -q "SPDX-License-Identifier: ${REJECTED_LICENSE}"; then
        echo -e "${RED}WRONG LICENSE (${REJECTED_LICENSE}): ${file}${NC}"
        WRONG=$((WRONG + 1))
        continue
    fi

    # Check for the correct license identifier
    if ! head -10 "$file" | grep -q "SPDX-License-Identifier: ${EXPECTED_LICENSE}"; then
        echo -e "${RED}WRONG LICENSE (expected ${EXPECTED_LICENSE}): ${file}${NC}"
        WRONG=$((WRONG + 1))
    fi
done < <(
    find "$REPO_ROOT" \
        \( -name '*.rs' -o -name '*.sh' -o -name '*.toml' -o -name '*.yml' -o -name '*.yaml' \) \
        -not -path '*/target/*' \
        -not -path '*/.git/*' \
        -not -name 'Cargo.lock' \
        -print0 2>/dev/null
)

TOTAL_FAIL=$((MISSING + WRONG))

if [[ "$TOTAL_FAIL" -eq 0 ]]; then
    echo "OK: $CHECKED files checked, all have SPDX-License-Identifier: $EXPECTED_LICENSE"
    exit 0
else
    echo -e "${RED}FAIL: $MISSING missing, $WRONG wrong license (of $CHECKED checked)${NC}"
    exit 1
fi
