#!/usr/bin/env bash
# report-lib-format.sh: the shared apply-output helper produces the G2 header
# the operator chose (bold 256-color-212 tool name, faint "── context"), prints
# a separating blank line first, honors REPORT_LIB_PLAIN=1, and the two
# printing run_ scripts actually source it.
#
# WHY THIS EXISTS. Operator rulings 2026-08-05: a no-op apply prints nothing,
# printed output names its owner, and decoupled sections are visually distinct.
# The G2 style was picked from live gum renders and then implemented
# dependency-free (three SGR sequences), so the exact escape codes ARE the
# contract: an edit that drops the color or the separator quietly returns the
# unowned-wall-of-text output this work removed.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LIB="$REPO_ROOT/dot_local/bin/report-lib.sh"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

[[ -r $LIB ]] || fail "$LIB is missing"
# shellcheck source=/dev/null
source "$LIB"

esc=$'\033'

# 1. Colored header: leading blank line, bold+212 name, faint context.
out="$(report_section "yt-dlp" "nightly update")"
[[ $out == $'\n'* ]] ||
  fail "report_section does not lead with a blank separator line"
[[ $out == *"${esc}[1;38;5;212myt-dlp${esc}[0m"* ]] ||
  fail "the tool name is not bold 256-color 212; got: $(printf '%q' "$out")"
[[ $out == *"${esc}[2m ── nightly update${esc}[0m"* ]] ||
  fail "the context is not faint with the ── rule; got: $(printf '%q' "$out")"

# 2. No context: name only, no dangling rule.
out="$(report_section "ssh-hardening")"
[[ $out == *"${esc}[1;38;5;212mssh-hardening${esc}[0m"* ]] ||
  fail "the no-context header lost the styled name"
[[ $out != *"──"* ]] ||
  fail "the no-context header prints a dangling ── rule"

# 3. Plain mode: byte-exact, no escapes, for report-to-log callers.
out="$(REPORT_LIB_PLAIN=1 report_section "yt-dlp" "nightly update")"
[[ $out == $'\nyt-dlp ── nightly update' ]] ||
  fail "REPORT_LIB_PLAIN=1 output is not the plain header; got: $(printf '%q' "$out")"
[[ $out != *"$esc"* ]] ||
  fail "REPORT_LIB_PLAIN=1 still emits escape sequences"

# 4. report_line is verbatim passthrough.
[[ "$(report_line "a  b  c")" == "a  b  c" ]] ||
  fail "report_line altered its input"

# 5. The printing scripts actually source the lib (an unsourced helper is the
#    mutation that reverts them to unowned output while cases 1-4 stay green).
for script in run_after_35-setup-yt-dlp.sh.tmpl run_after_59-hermes-config-migrate.sh.tmpl; do
  grep -Eq '^[[:space:]]*source "\$HOME/.local/bin/report-lib.sh"' \
    "$REPO_ROOT/.chezmoiscripts/$script" ||
    fail "$script does not source report-lib.sh; its output loses the section header"
done

printf 'report-lib-format: OK (G2 header pinned byte-for-byte, plain mode clean, both printers wired)\n'
