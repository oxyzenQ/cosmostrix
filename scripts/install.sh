#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: MIT

set -euo pipefail

PROJECT_NAME="cosmostrix"
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PREFIX="${PREFIX:-${HOME}/.local}"
BINDIR="${DESTDIR:-}${PREFIX}/bin"

cd "${REPO_ROOT}"
cargo build --release --locked
mkdir -p "${BINDIR}"
install -m 755 "target/release/${PROJECT_NAME}" "${BINDIR}/${PROJECT_NAME}"

echo "${PROJECT_NAME} installed to ${BINDIR}/${PROJECT_NAME}"
echo "Make sure ${BINDIR} is in your PATH."
