#!/usr/bin/env bash
#
# update-agent-plugins-sanitizer.sh: __agent_plugins_code renders third-party CLI
# error text as a Discord inline code span on the phone-push (alert) path. It is
# deliberately SEPARATE from the library's own quoting helper, because the alert
# path fires whether or not the library loaded, and a sanitiser absent exactly
# when the library is missing would leave the one message pushed to a phone
# unquoted. This pins the sanitiser against hostile input so a future
# dedup-into-the-library cleanup cannot silently resurrect an unquoted push.
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HELPER="$REPO_ROOT/dot_local/bin/executable_update-agent-plugins.sh"

fail() {
  printf 'update-agent-plugins-sanitizer: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $HELPER ]] || fail "helper not found: $HELPER"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
HOME="$tmp/home"
export HOME
mkdir -p "$HOME/.local/bin"

# Source the helper for its functions only, before its main flow runs. `set --`
# clears positional args so the helper's argument parser sees none.
set --
# shellcheck source=/dev/null
UPDATE_AGENT_PLUGINS_LIB_ONLY=1 source "$HELPER"

command -v __agent_plugins_code >/dev/null 2>&1 ||
  fail "LIB_ONLY source did not define __agent_plugins_code"

# check_code <input> <description> -- the rendered text must be ONE literal code
# span: wrapped in backticks, carrying no INNER backtick (which would close the
# span early and let the rest render as markdown, e.g. a clickable link) and no
# control character (which can also break the span or move the cursor).
check_code() {
  local input="$1" desc="$2" out ticks
  out="$(__agent_plugins_code "$input")"
  [[ $out == \`*\` ]] || fail "[$desc] output is not a code span: [$out]"
  ticks="${out//[^\`]/}"
  [[ ${#ticks} -eq 2 ]] ||
    fail "[$desc] output carries an inner backtick that would break the span: [$out]"
  [[ "$(printf '%s' "$out" | tr -d '[:cntrl:]')" == "$out" ]] ||
    fail "[$desc] a control character survived into the rendered span: [$out]"
}

check_code '[urgent: click here](https://evil.example)' "markdown link"
# The backticks below are LITERAL test input; single quotes keep them from
# starting a command substitution, which is exactly the hostile case under test.
# shellcheck disable=SC2016
check_code 'oops `injected` span closer' "embedded backticks"
# shellcheck disable=SC2016
check_code 'both `a](http://x) and `b' "backticks around a link"
printf -v with_cr 'line1\rline2'
check_code "$with_cr" "carriage return"
printf -v with_bel 'ding\ading'
check_code "$with_bel" "bell control char"
# OSC-8 terminal hyperlink: ESC ] 8 ;; URL ESC \ TEXT ESC ] 8 ;; ESC \ -- the
# trailing \\ is an escaped backslash for printf, not an escaped quote.
# shellcheck disable=SC1003
printf -v osc8 '\033]8;;https://evil.example\033\\click me\033]8;;\033\\'
check_code "$osc8" "OSC-8 hyperlink escape"
check_code 'plain@mkt-a: update failed: could not reach marketplace' "benign passthrough"

# The length cap still applies and still ends the span cleanly.
long="$(printf 'x%.0s' {1..500})"
capped="$(__agent_plugins_code "$long" 40)"
[[ $capped == \`*...\` ]] ||
  fail "a capped value does not end in an ellipsis inside the span: [$capped]"
[[ ${#capped} -le 46 ]] || fail "the cap did not bound the rendered length: [${#capped}]"

printf 'update-agent-plugins-sanitizer: OK (a markdown link, embedded backticks, a CR, a BEL and an OSC-8 hyperlink all render as ONE literal code span with no inner backtick and no control character; the length cap ends the span cleanly; benign text passes through)\n'
