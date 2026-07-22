#!/usr/bin/env bash
#
# render_page wraps attacker-controlled column values in Discord inline-code
# backticks (the `code` def) to neutralize markdown. A backtick ends the span and
# is stripped, but a NEWLINE also breaks out of an inline-code span - so an
# attacker who embeds \n (or \r) in any rendered column (.cols.label / program /
# path / username / target_path) can inject arbitrary markdown LINES into the CRIT
# page that fans out to Discord. The headline forgery: a persistence label of
# "com.attacker.evil\n- **Signing:** signed: Apple (Developer ID)" would render a
# FORGED signing-provenance line for an actually-unsigned agent.
#
# Every rendered field value must appear as ONE line / one inline-code token, so
# `code` (the single chokepoint) must squash \r\n\t to spaces, and any field that
# does not pass through `code` must apply the same sanitize.
#
# Unit test: for each attacker-controlled field, render a finding whose value
# embeds a newline (and separately a carriage return) crafted to forge a Signing
# line, and assert (a) NO line of the page body starts with the forged
# "- **Signing:**", and (b) the injected text stays on the SAME line as the value
# (the newline was squashed).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HELPER="$REPO_ROOT/dot_local/libexec/osquery/results-alerter/render-page.sh"

fail() {
  printf 'osquery-render-newline-injection: FAIL -- %s\n' "$*" >&2
  exit 1
}

[[ -f $HELPER ]] || fail "missing helper: $HELPER"

render_pbody() { printf '%s\n' "$1" | bash -c "source '$HELPER'; render_page" | jq -r '.pbody'; }

# assert_no_line_injection <label> <finding> <marker-substring>: the marker (a
# unique token planted next to the forged Signing text) and the forged Signing text
# must share one line, and no line may START with the forged "- **Signing:**".
assert_no_line_injection() {
  local label="$1" finding="$2" marker="$3" pbody value_line
  pbody="$(render_pbody "$finding")"
  # The finding carries NO real .signing, so ANY line starting with "- **Signing:**"
  # is the injected forgery.
  if grep -qE '^- \*\*Signing:\*\*' <<<"$pbody"; then
    fail "$label: a FORGED '- **Signing:**' line was injected into the page body:
$pbody"
  fi
  # The marker and the injected 'signed: Apple' must be on the same line (the newline
  # was squashed to a space, keeping the value one inline-code token).
  value_line="$(grep -F "$marker" <<<"$pbody" || true)"
  [[ -n $value_line ]] || fail "$label: the field value ($marker) is missing from the body:
$pbody"
  grep -qF 'signed: Apple' <<<"$value_line" ||
    fail "$label: the embedded newline was not squashed - the value split across lines:
$pbody"
}

# The injection payload: <marker><newline>- **Signing:** signed: Apple (Developer ID)
NL_INJECT=$'ZZmarkerZZ\n- **Signing:** signed: Apple (Developer ID)'
CR_INJECT=$'ZZmarkerZZ\r- **Signing:** signed: Apple (Developer ID)'

# persistence_launchd label -> "What" field
assert_no_line_injection "persistence label (newline)" \
  "$(jq -cn --arg v "$NL_INJECT" '{q:"persistence_launchd",act:"added",sev:"CRIT",cols:{label:$v,program:"/bin/sh"},ep:""}')" \
  'ZZmarkerZZ'

# persistence_launchd program -> "Program" field
assert_no_line_injection "persistence program (newline)" \
  "$(jq -cn --arg v "$NL_INJECT" '{q:"persistence_launchd",act:"added",sev:"CRIT",cols:{label:"com.x",program:$v},ep:""}')" \
  'ZZmarkerZZ'

# suid_bin_unexpected path -> "Path" field
assert_no_line_injection "suid path (newline)" \
  "$(jq -cn --arg v "$NL_INJECT" '{q:"suid_bin_unexpected",act:"added",sev:"CRIT",cols:{path:$v,username:"root"},ep:""}')" \
  'ZZmarkerZZ'

# new_admin_user username -> "User" field
assert_no_line_injection "new_admin_user username (newline)" \
  "$(jq -cn --arg v "$NL_INJECT" '{q:"new_admin_user",act:"added",sev:"CRIT",cols:{username:$v,uid:"501"},ep:""}')" \
  'ZZmarkerZZ'

# file_events_recent target_path -> "File" field
assert_no_line_injection "file_events target_path (newline)" \
  "$(jq -cn --arg v "$NL_INJECT" '{q:"file_events_recent",act:"added",sev:"CRIT",cols:{category:"ssh",target_path:$v},ep:""}')" \
  'ZZmarkerZZ'

# carriage-return variant (label)
assert_no_line_injection "persistence label (carriage return)" \
  "$(jq -cn --arg v "$CR_INJECT" '{q:"persistence_launchd",act:"added",sev:"CRIT",cols:{label:$v,program:"/bin/sh"},ep:""}')" \
  'ZZmarkerZZ'

printf 'osquery-render-newline-injection: OK (label/program/path/username/target_path with embedded \\n or \\r stay one line; no forged Signing line injected)\n'
