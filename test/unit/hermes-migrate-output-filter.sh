#!/usr/bin/env bash
# hermes-migrate-output-filter.sh: the awk filter in run_after_59 drops the
# optional-API-key notice (header + its bullet list + the "checking for
# updates" banner) while keeping everything else, including the REQUIRED-key
# warning and its bullets.
#
# WHY THIS EXISTS. The filter is applied to REAL hermes output, but no test
# ever fed it a realistic transcript, so a rewrite that swapped the literal
# bullet `•` for `\xe2\x80\xa2` hex escapes (unsupported by macOS awk) shipped
# and every optional bullet leaked onto the operator's apply, 180 lines. This
# extracts the awk program from the rendered script and drives it with a fixture
# that contains bullets, the banner, and both notice blocks, so the pattern is
# exercised rather than merely present.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_after_59-hermes-config-migrate.sh.tmpl"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -r $TEMPLATE ]] || fail "$TEMPLATE is missing"
command -v chezmoi >/dev/null 2>&1 || fail "chezmoi is not on PATH"

# Render, then lift the awk program out of the rendered script (between the
# `| awk '` and the closing `'` that precedes `|| true`). Testing the RENDERED
# awk, not a copy, is the point: a copy would not have caught the hex-escape
# regression.
rendered="$work/r59.sh"
mkdir -p "$work/home" # chezmoi's read-source-state pre hook chdirs into HOME
HOME="$work/home" CI=1 chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$TEMPLATE" >"$rendered" 2>"$work/err" ||
  fail "template render failed:"$'\n'"$(cat "$work/err")"

# The awk body is every line strictly between the `| awk '` opener and the
# `' || true` closer. sed by that pair, then drop those two boundary lines, so
# the extracted program is exactly what the shell hands awk.
start="$(grep -n "| awk '" "$rendered" | head -1 | cut -d: -f1)"
end="$(grep -n "' || true" "$rendered" | head -1 | cut -d: -f1)"
[[ -n $start && -n $end && $end -gt $start ]] ||
  fail "could not locate the awk filter boundaries in the rendered script"
awk_prog="$(sed -n "$((start + 1)),$((end - 1))p" "$rendered")"
[[ -n $awk_prog ]] || fail "the extracted awk filter is empty"

# A transcript shaped like real `hermes config migrate` output: a normal line,
# the update banner, the OPTIONAL block with bullets (some with "enables:"
# suffixes), padding blanks, then the REQUIRED block with a bullet.
fixture="$work/transcript"
{
  printf 'Config is current.\n'
  printf '\n'
  printf '\xf0\x9f\x94\x84 Checking configuration for updates...\n'
  printf '\n'
  printf '  \xe2\x84\xb9\xef\xb8\x8f  180 optional API key(s) not configured:\n'
  printf '     \xe2\x80\xa2 DEEPSEEK_API_KEY\n'
  printf '     \xe2\x80\xa2 EXA_API_KEY (enables: web_search)\n'
  printf '\n\n'
  printf '  \xe2\x9a\xa0\xef\xb8\x8f  1 required API key(s) missing:\n'
  printf '     \xe2\x80\xa2 DISCORD_BOT_TOKEN\n'
} >"$fixture"

out="$(awk "$awk_prog" <"$fixture")"

# 1. Every optional bullet is gone. This is the regression that shipped.
printf '%s\n' "$out" | grep -q 'DEEPSEEK_API_KEY' &&
  fail "an optional-key bullet leaked through the filter (the hex-escape regression):"$'\n'"$out"
printf '%s\n' "$out" | grep -q 'EXA_API_KEY' &&
  fail "an optional-key bullet with an 'enables:' suffix leaked through:"$'\n'"$out"
printf '%s\n' "$out" | grep -q 'optional API key' &&
  fail "the optional-key notice header leaked through:"$'\n'"$out"

# 2. The update banner is gone.
printf '%s\n' "$out" | grep -q 'Checking configuration for updates' &&
  fail "the 'checking for updates' banner leaked through:"$'\n'"$out"

# 3. The required-key warning and ITS bullet survive: that is the whole reason
#    the operator wants any output at all.
printf '%s\n' "$out" | grep -q 'required API key' ||
  fail "the REQUIRED-key warning was dropped; the filter over-reached:"$'\n'"$out"
printf '%s\n' "$out" | grep -q 'DISCORD_BOT_TOKEN' ||
  fail "the required-key bullet was dropped; only the OPTIONAL block should lose its bullets:"$'\n'"$out"

# 4. The ordinary line survives.
printf '%s\n' "$out" | grep -q 'Config is current' ||
  fail "an ordinary output line was dropped:"$'\n'"$out"

printf 'hermes-migrate-output-filter: OK (optional block + banner dropped, required warning and its bullet kept)\n'
