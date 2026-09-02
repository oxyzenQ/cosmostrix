#!/usr/bin/env bash
# Copyright (C) 2026 rezky_nightky
# SPDX-License-Identifier: GPL-3.0-only
#
# ─────────────────────────────────────────────────────────────────────────────
# PLATFORM: UNIX-only (Linux, macOS, BSD).
#   Uses `find -print0`, `grep -F`, `mktemp`, bash arrays. Not for Windows
#   cmd.exe / PowerShell — use Git Bash or WSL on Windows.
# ─────────────────────────────────────────────────────────────────────────────
#
# COSMOSTRIX SYMBOL-ONLY OUTPUT GUARD (v80.0.0-beta.2 owner rule)
#
# Owner directive (2026-09-02): diagnostic output must use ASCII SYMBOLS
# only — icon glyphs are forbidden because some OS/terminal combinations
# render them as tofu/garbage. Owner proof line that triggered the rule:
#   "! [live-reload] native watcher silent 852s — ..."
# (was an icon-glyph warning prefix before this rule).
#
# Symbol vocabulary (mirrors src/output/mod.rs print helpers):
#   "!"  warning prefix (eprintln_warn_labeled)
#   "error:"  error label (eprintln_error_labeled)
#   "OK" / "+"  pass / positive delta   "X" / "-"  fail / negative delta
#   "[INFO] [OK] [!] [X] [>]"  build.sh log badges
#
# Scope (mirrors the RULES.md enforcement scope):
#   src/**/*.rs, build.rs, scripts/*.sh, scripts/*.py,
#   benchmark/*.sh, .github/workflows/*.yml, pgo-runner/src/**/*.rs
#   (pgo-runner is a separate binary that also prints diagnostics)
#
# Forbidden classes (whole-file scan — comments included, so a comment
# can never keep showcasing a stale icon-format output):
#   U+2300..U+23FF  misc technical (clocks, media controls, key caps)
#   U+2600..U+27BF  misc symbols + dingbats (warning, check/cross, ...)
#   U+2B00..U+2BFF  stars / pictographic arrows
#   U+FE0F          emoji variation selector (byte match EF B8 8F)
#   U+200D          zero-width joiner (byte match E2 80 8D)
#   U+1F000..U+1FFFF astral emoji (byte-prefix match F0 9F)
#
# Allowed (typographic house style — text presentation, owner-accepted
# in the very proof line above: the em dash stays):
#   U+2014 em dash, U+2013 en dash, U+2022 bullet, U+2190..U+21FF prose
#   arrows ("old -> new" transitions), U+2500..U+25FF box drawing +
#   geometric (banner rules, rain art), math operators (+ - = < >).
#   The rain charset pools in src/scene/charset.rs are ART, width-filtered
#   at runtime per terminal support — not diagnostics (contains no banned
#   blocks today; if a future charset needs one, add a narrow exemption
#   with justification, do NOT widen this gate).
#
# Exemptions (file-level, each with a justification — keep this list SHORT):
#   scripts/check-symbol-only-output.sh  this file: the denylist itself
#   src/output/message.rs                sanitizer test INPUT: needs a real
#                                        emoji as data to verify replacement
#
# Companion tools:
#   scripts/emoji-audit.py  — doc-prose sweep (manual, non-blocking)
#   this gate               — output surface (hard fail, gate-keepers + CI)
#
# Usage: bash scripts/check-symbol-only-output.sh
# Exit:  0 = clean, 1 = violations found
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# ── Denylist: every ASSIGNED codepoint in the three banned BMP blocks ──────
# Matching is byte-exact (grep -F, fixed strings, LC_ALL=C) — locale-proof.
# Generated mechanically; do not hand-edit single entries (regenerate
# the block instead so the list stays complete).
# 957 assigned glyphs across 3 blocks
DENY=(
	'⌀' '⌁' '⌂' '⌃' '⌄' '⌅' '⌆' '⌇' '⌈' '⌉' '⌊' '⌋'
	'⌌' '⌍' '⌎' '⌏' '⌐' '⌑' '⌒' '⌓' '⌔' '⌕' '⌖' '⌗'
	'⌘' '⌙' '⌚' '⌛' '⌜' '⌝' '⌞' '⌟' '⌠' '⌡' '⌢' '⌣'
	'⌤' '⌥' '⌦' '⌧' '⌨' '〈' '〉' '⌫' '⌬' '⌭' '⌮' '⌯'
	'⌰' '⌱' '⌲' '⌳' '⌴' '⌵' '⌶' '⌷' '⌸' '⌹' '⌺' '⌻'
	'⌼' '⌽' '⌾' '⌿' '⍀' '⍁' '⍂' '⍃' '⍄' '⍅' '⍆' '⍇'
	'⍈' '⍉' '⍊' '⍋' '⍌' '⍍' '⍎' '⍏' '⍐' '⍑' '⍒' '⍓'
	'⍔' '⍕' '⍖' '⍗' '⍘' '⍙' '⍚' '⍛' '⍜' '⍝' '⍞' '⍟'
	'⍠' '⍡' '⍢' '⍣' '⍤' '⍥' '⍦' '⍧' '⍨' '⍩' '⍪' '⍫'
	'⍬' '⍭' '⍮' '⍯' '⍰' '⍱' '⍲' '⍳' '⍴' '⍵' '⍶' '⍷'
	'⍸' '⍹' '⍺' '⍻' '⍼' '⍽' '⍾' '⍿' '⎀' '⎁' '⎂' '⎃'
	'⎄' '⎅' '⎆' '⎇' '⎈' '⎉' '⎊' '⎋' '⎌' '⎍' '⎎' '⎏'
	'⎐' '⎑' '⎒' '⎓' '⎔' '⎕' '⎖' '⎗' '⎘' '⎙' '⎚' '⎛'
	'⎜' '⎝' '⎞' '⎟' '⎠' '⎡' '⎢' '⎣' '⎤' '⎥' '⎦' '⎧'
	'⎨' '⎩' '⎪' '⎫' '⎬' '⎭' '⎮' '⎯' '⎰' '⎱' '⎲' '⎳'
	'⎴' '⎵' '⎶' '⎷' '⎸' '⎹' '⎺' '⎻' '⎼' '⎽' '⎾' '⎿'
	'⏀' '⏁' '⏂' '⏃' '⏄' '⏅' '⏆' '⏇' '⏈' '⏉' '⏊' '⏋'
	'⏌' '⏍' '⏎' '⏏' '⏐' '⏑' '⏒' '⏓' '⏔' '⏕' '⏖' '⏗'
	'⏘' '⏙' '⏚' '⏛' '⏜' '⏝' '⏞' '⏟' '⏠' '⏡' '⏢' '⏣'
	'⏤' '⏥' '⏦' '⏧' '⏨' '⏩' '⏪' '⏫' '⏬' '⏭' '⏮' '⏯'
	'⏰' '⏱' '⏲' '⏳' '⏴' '⏵' '⏶' '⏷' '⏸' '⏹' '⏺' '⏻'
	'⏼' '⏽' '⏾' '⏿' '☀' '☁' '☂' '☃' '☄' '★' '☆' '☇'
	'☈' '☉' '☊' '☋' '☌' '☍' '☎' '☏' '☐' '☑' '☒' '☓'
	'☔' '☕' '☖' '☗' '☘' '☙' '☚' '☛' '☜' '☝' '☞' '☟'
	'☠' '☡' '☢' '☣' '☤' '☥' '☦' '☧' '☨' '☩' '☪' '☫'
	'☬' '☭' '☮' '☯' '☰' '☱' '☲' '☳' '☴' '☵' '☶' '☷'
	'☸' '☹' '☺' '☻' '☼' '☽' '☾' '☿' '♀' '♁' '♂' '♃'
	'♄' '♅' '♆' '♇' '♈' '♉' '♊' '♋' '♌' '♍' '♎' '♏'
	'♐' '♑' '♒' '♓' '♔' '♕' '♖' '♗' '♘' '♙' '♚' '♛'
	'♜' '♝' '♞' '♟' '♠' '♡' '♢' '♣' '♤' '♥' '♦' '♧'
	'♨' '♩' '♪' '♫' '♬' '♭' '♮' '♯' '♰' '♱' '♲' '♳'
	'♴' '♵' '♶' '♷' '♸' '♹' '♺' '♻' '♼' '♽' '♾' '♿'
	'⚀' '⚁' '⚂' '⚃' '⚄' '⚅' '⚆' '⚇' '⚈' '⚉' '⚊' '⚋'
	'⚌' '⚍' '⚎' '⚏' '⚐' '⚑' '⚒' '⚓' '⚔' '⚕' '⚖' '⚗'
	'⚘' '⚙' '⚚' '⚛' '⚜' '⚝' '⚞' '⚟' '⚠' '⚡' '⚢' '⚣'
	'⚤' '⚥' '⚦' '⚧' '⚨' '⚩' '⚪' '⚫' '⚬' '⚭' '⚮' '⚯'
	'⚰' '⚱' '⚲' '⚳' '⚴' '⚵' '⚶' '⚷' '⚸' '⚹' '⚺' '⚻'
	'⚼' '⚽' '⚾' '⚿' '⛀' '⛁' '⛂' '⛃' '⛄' '⛅' '⛆' '⛇'
	'⛈' '⛉' '⛊' '⛋' '⛌' '⛍' '⛎' '⛏' '⛐' '⛑' '⛒' '⛓'
	'⛔' '⛕' '⛖' '⛗' '⛘' '⛙' '⛚' '⛛' '⛜' '⛝' '⛞' '⛟'
	'⛠' '⛡' '⛢' '⛣' '⛤' '⛥' '⛦' '⛧' '⛨' '⛩' '⛪' '⛫'
	'⛬' '⛭' '⛮' '⛯' '⛰' '⛱' '⛲' '⛳' '⛴' '⛵' '⛶' '⛷'
	'⛸' '⛹' '⛺' '⛻' '⛼' '⛽' '⛾' '⛿' '✀' '✁' '✂' '✃'
	'✄' '✅' '✆' '✇' '✈' '✉' '✊' '✋' '✌' '✍' '✎' '✏'
	'✐' '✑' '✒' '✓' '✔' '✕' '✖' '✗' '✘' '✙' '✚' '✛'
	'✜' '✝' '✞' '✟' '✠' '✡' '✢' '✣' '✤' '✥' '✦' '✧'
	'✨' '✩' '✪' '✫' '✬' '✭' '✮' '✯' '✰' '✱' '✲' '✳'
	'✴' '✵' '✶' '✷' '✸' '✹' '✺' '✻' '✼' '✽' '✾' '✿'
	'❀' '❁' '❂' '❃' '❄' '❅' '❆' '❇' '❈' '❉' '❊' '❋'
	'❌' '❍' '❎' '❏' '❐' '❑' '❒' '❓' '❔' '❕' '❖' '❗'
	'❘' '❙' '❚' '❛' '❜' '❝' '❞' '❟' '❠' '❡' '❢' '❣'
	'❤' '❥' '❦' '❧' '❨' '❩' '❪' '❫' '❬' '❭' '❮' '❯'
	'❰' '❱' '❲' '❳' '❴' '❵' '❶' '❷' '❸' '❹' '❺' '❻'
	'❼' '❽' '❾' '❿' '➀' '➁' '➂' '➃' '➄' '➅' '➆' '➇'
	'➈' '➉' '➊' '➋' '➌' '➍' '➎' '➏' '➐' '➑' '➒' '➓'
	'➔' '➕' '➖' '➗' '➘' '➙' '➚' '➛' '➜' '➝' '➞' '➟'
	'➠' '➡' '➢' '➣' '➤' '➥' '➦' '➧' '➨' '➩' '➪' '➫'
	'➬' '➭' '➮' '➯' '➰' '➱' '➲' '➳' '➴' '➵' '➶' '➷'
	'➸' '➹' '➺' '➻' '➼' '➽' '➾' '➿' '⬀' '⬁' '⬂' '⬃'
	'⬄' '⬅' '⬆' '⬇' '⬈' '⬉' '⬊' '⬋' '⬌' '⬍' '⬎' '⬏'
	'⬐' '⬑' '⬒' '⬓' '⬔' '⬕' '⬖' '⬗' '⬘' '⬙' '⬚' '⬛'
	'⬜' '⬝' '⬞' '⬟' '⬠' '⬡' '⬢' '⬣' '⬤' '⬥' '⬦' '⬧'
	'⬨' '⬩' '⬪' '⬫' '⬬' '⬭' '⬮' '⬯' '⬰' '⬱' '⬲' '⬳'
	'⬴' '⬵' '⬶' '⬷' '⬸' '⬹' '⬺' '⬻' '⬼' '⬽' '⬾' '⬿'
	'⭀' '⭁' '⭂' '⭃' '⭄' '⭅' '⭆' '⭇' '⭈' '⭉' '⭊' '⭋'
	'⭌' '⭍' '⭎' '⭏' '⭐' '⭑' '⭒' '⭓' '⭔' '⭕' '⭖' '⭗'
	'⭘' '⭙' '⭚' '⭛' '⭜' '⭝' '⭞' '⭟' '⭠' '⭡' '⭢' '⭣'
	'⭤' '⭥' '⭦' '⭧' '⭨' '⭩' '⭪' '⭫' '⭬' '⭭' '⭮' '⭯'
	'⭰' '⭱' '⭲' '⭳' '⭶' '⭷' '⭸' '⭹' '⭺' '⭻' '⭼' '⭽'
	'⭾' '⭿' '⮀' '⮁' '⮂' '⮃' '⮄' '⮅' '⮆' '⮇' '⮈' '⮉'
	'⮊' '⮋' '⮌' '⮍' '⮎' '⮏' '⮐' '⮑' '⮒' '⮓' '⮔' '⮕'
	'⮗' '⮘' '⮙' '⮚' '⮛' '⮜' '⮝' '⮞' '⮟' '⮠' '⮡' '⮢'
	'⮣' '⮤' '⮥' '⮦' '⮧' '⮨' '⮩' '⮪' '⮫' '⮬' '⮭' '⮮'
	'⮯' '⮰' '⮱' '⮲' '⮳' '⮴' '⮵' '⮶' '⮷' '⮸' '⮹' '⮺'
	'⮻' '⮼' '⮽' '⮾' '⮿' '⯀' '⯁' '⯂' '⯃' '⯄' '⯅' '⯆'
	'⯇' '⯈' '⯉' '⯊' '⯋' '⯌' '⯍' '⯎' '⯏' '⯐' '⯑' '⯒'
	'⯓' '⯔' '⯕' '⯖' '⯗' '⯘' '⯙' '⯚' '⯛' '⯜' '⯝' '⯞'
	'⯟' '⯠' '⯡' '⯢' '⯣' '⯤' '⯥' '⯦' '⯧' '⯨' '⯩' '⯪'
	'⯫' '⯬' '⯭' '⯮' '⯯' '⯰' '⯱' '⯲' '⯳' '⯴' '⯵' '⯶'
	'⯷' '⯸' '⯹' '⯺' '⯻' '⯼' '⯽' '⯾' '⯿'
)

# ── Byte-level patterns (invisible/zero-width + astral) ────────────────────
# All astral-plane emoji (U+1F000..U+1FFFF) share the UTF-8 lead bytes F0 9F.
# VS16 = EF B8 8F, ZWJ = E2 80 8D. Written as escapes so this script's own
# source stays glyph-free outside the exempted denylist above.
ASTRAL_BYTES=$'\xF0\x9F'
VS16_BYTES=$'\xEF\xB8\x8F'
ZWJ_BYTES=$'\xE2\x80\x8D'

# ── Exemptions ──────────────────────────────────────────────────────────────
EXEMPT=(
	"scripts/check-symbol-only-output.sh"
	"src/output/message.rs"
)

is_exempt() {
	local rel="$1"
	local e
	for e in "${EXEMPT[@]}"; do
		if [ "$rel" = "$e" ]; then
			return 0
		fi
	done
	return 1
}

# ── Temp pattern file (one glyph per line for grep -F -f) ──────────────────
PATTERN_FILE="$(mktemp)"
trap 'rm -f "$PATTERN_FILE"' EXIT
printf '%s\n' "${DENY[@]}" >"$PATTERN_FILE"

VIOLATIONS=0
FILES_CHECKED=0

check_file() {
	local file="$1"
	FILES_CHECKED=$((FILES_CHECKED + 1))
	if is_exempt "$file"; then
		return 0
	fi
	local hits
	hits="$(LC_ALL=C grep -F -n -f "$PATTERN_FILE" "$file" 2>/dev/null || true)"
	local hits2
	hits2="$(LC_ALL=C grep -F -n -e "$ASTRAL_BYTES" -e "$VS16_BYTES" -e "$ZWJ_BYTES" "$file" 2>/dev/null || true)"
	if [ -n "$hits" ] || [ -n "$hits2" ]; then
		echo "VIOLATION: $file"
		{
			printf '%s\n' "$hits"
			printf '%s\n' "$hits2"
		} | grep -v '^$' | head -5 | sed 's/^/    /'
		VIOLATIONS=$((VIOLATIONS + 1))
	fi
}

# ── File discovery (relative paths from repo root) ──────────────────────────
while IFS= read -r -d '' file; do
	check_file "$file"
done < <(find src -name '*.rs' -not -path '*/target/*' -print0 2>/dev/null)

[ -f build.rs ] && check_file build.rs

if [ -d pgo-runner/src ]; then
	while IFS= read -r -d '' file; do
		check_file "$file"
	done < <(find pgo-runner/src -name '*.rs' -print0 2>/dev/null)
fi

while IFS= read -r -d '' file; do
	check_file "$file"
done < <(find scripts \( -name '*.sh' -o -name '*.py' \) -print0 2>/dev/null)

if [ -d benchmark ]; then
	while IFS= read -r -d '' file; do
		check_file "$file"
	done < <(find benchmark -name '*.sh' -print0 2>/dev/null)
fi

while IFS= read -r -d '' file; do
	check_file "$file"
done < <(find .github \( -name '*.yml' -o -name '*.yaml' \) -print0 2>/dev/null)

# ── Summary ─────────────────────────────────────────────────────────────────
if [ "$VIOLATIONS" -eq 0 ]; then
	echo "OK: $FILES_CHECKED files checked, no icon glyphs (symbol-only output rule v80.0.0-beta.2)"
	exit 0
else
	echo ""
	echo "FAIL: $VIOLATIONS file(s) contain icon glyphs (symbol-only output rule v80.0.0-beta.2)"
	echo ""
	echo "Fix: replace icon glyphs with ASCII symbols —"
	echo "  warning -> \"!\"   pass -> \"OK\" or \"+\"   fail -> \"X\" or \"-\""
	echo "Typical offenders: warning prefixes, status marks, log badges."
	echo "See docs/RULES.md (Output Glyph Policy) for the full vocabulary."
	echo "Genuine art/test-data need? Add a narrow EXEMPT entry WITH justification."
	exit 1
fi
