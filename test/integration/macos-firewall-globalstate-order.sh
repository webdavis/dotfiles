#!/usr/bin/env bash
# macos-firewall-globalstate-order.sh -- the firewall baseline declared in
# .chezmoidata/macos_system_setup.yaml renders through the Tier 2 runner as
# four sudo-prefixed, idempotent socketfilterfw commands with the global-state
# command STRICTLY first, and the logging control renders as a runbook
# pointer, never a command.
#
# Why the order is a correctness property, not cosmetics: stealth mode and the
# signed-software policies are preferences that are inert while the firewall's
# global state is off. On a fresh or drifted machine a record that renders
# before the global-state record writes a setting with no protection behind
# it: it reads back as set and nothing enforces it.
#
# The properties pinned, one per acceptance criterion:
#   1. Global state renders strictly before stealth (and before both
#      signed-software commands, the same inert-preference class), compared by
#      LINE NUMBER in the render; a presence-only assertion would pass with
#      the order reversed. Each command is pinned sudo-prefixed as a whole
#      line.
#   2. BOTH signed-software commands render, each asserted individually:
#      --setallowsigned (built-in) and --setallowsignedapp (downloaded) are
#      independent policies, and a single "a signed-app command is present"
#      assertion passes with one of the two missing.
#   3. The logging record renders NO command and DOES render its runbook
#      pointer; a completeness diff pins the render's socketfilterfw lines to
#      exactly the four declared commands, so the logging record cannot
#      contribute one.
#   4. Every socketfilterfw command in the data is an idempotent set-to-state
#      form with an explicit trailing state, never a toggle: the runner
#      re-runs the whole list on any data change, so a toggle would flip
#      protection OFF on the second run. Asserted over EVERY declared firewall
#      command, not a count.
#   5. Every manual record's runbook field resolves to a real markdown heading
#      in the runbook, so the logging pointer cannot dangle.
#
# Real chezmoi against the REAL .chezmoidata; nothing is executed, only
# rendered.
set -euo pipefail

# Scrubbed at SCRIPT scope, before any chezmoi call. Git exports GIT_DIR to
# every hook it runs and this suite runs from the pre-push hook.
unset MACOS_DEFAULTS_SOURCE_DIR GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
TEMPLATE="$REPO_ROOT/.chezmoiscripts/run_onchange_after_41-macos-system-setup.sh.tmpl"
SETUP_YAML="$REPO_ROOT/.chezmoidata/macos_system_setup.yaml"
RUNBOOK="$REPO_ROOT/docs/runbooks/macos-fresh-machine-quickstart.md"

# The tool is not on PATH; every declared command carries the absolute path.
SOCKETFILTERFW="/usr/libexec/ApplicationFirewall/socketfilterfw"
GLOBAL_STATE_COMMAND="$SOCKETFILTERFW --setglobalstate on"
STEALTH_COMMAND="$SOCKETFILTERFW --setstealthmode on"
ALLOW_BUILTIN_SIGNED_COMMAND="$SOCKETFILTERFW --setallowsigned on"
ALLOW_DOWNLOADED_SIGNED_COMMAND="$SOCKETFILTERFW --setallowsignedapp on"

# The logging record's identity in the data, and the pointer line the manual
# tier renders for it (byte-exact, shellSingleQuoted by the runner).
LOGGING_DESCRIPTION="Firewall: logging (no logging flag on macOS 26.2)"
LOGGING_RUNBOOK_SECTION="Firewall logging"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

for tool in chezmoi yq; do
  command -v "$tool" >/dev/null 2>&1 || {
    printf 'SKIP: %s not on PATH; cannot exercise the firewall baseline\n' "$tool"
    exit 0
  }
done
for required_file in "$TEMPLATE" "$SETUP_YAML" "$RUNBOOK"; do
  [[ -f $required_file ]] || fail "missing file: $required_file"
done

work="$(cd "$(mktemp -d)" && pwd -P)"
trap 'rm -rf "$work"' EXIT

# ---- data-level properties (criteria 4 and 5; OS-independent) ---------------

# Criteria 2 and 4, data half: the declared socketfilterfw commands are
# EXACTLY the four baseline commands. Set equality via a sorted byte compare
# asserts each command individually and reports a missing or an extra one by
# name, where a count or a single-presence grep would stay green.
declared_firewall_commands="$work/declared-firewall-commands"
yq eval -r \
  '[.macos.system_setup[] | select(has("command")) | .command | select(test("socketfilterfw"))] | .[]' \
  "$SETUP_YAML" | LC_ALL=C sort >"$declared_firewall_commands"

expected_firewall_commands="$work/expected-firewall-commands"
LC_ALL=C sort >"$expected_firewall_commands" <<EOF
$GLOBAL_STATE_COMMAND
$STEALTH_COMMAND
$ALLOW_BUILTIN_SIGNED_COMMAND
$ALLOW_DOWNLOADED_SIGNED_COMMAND
EOF

cmp -s "$expected_firewall_commands" "$declared_firewall_commands" ||
  fail "the declared socketfilterfw commands must be exactly the four baseline commands (diff: $(diff "$expected_firewall_commands" "$declared_firewall_commands" || true))"

# Criterion 4: EVERY declared firewall command is a set-to-state form with an
# explicit trailing state. A completeness loop, not a count: a fifth command
# sneaking in as a toggle must fail here by name. Against TODAY'S data this
# loop is shadowed: the exact-command set compare above fires first on any
# deviation from the four baseline commands. It is a forward-looking layer,
# not dead code: it fails when a future toggle command lands alongside a
# co-evolved whitelist, so keep it.
set_to_state_pattern='^/usr/libexec/ApplicationFirewall/socketfilterfw --set[a-z]+ (on|off)$'
while IFS= read -r firewall_command; do
  [[ $firewall_command =~ $set_to_state_pattern ]] ||
    fail "firewall command is not an idempotent set-to-state form: $firewall_command"
done <"$declared_firewall_commands"

# The logging record exists exactly once, is tier manual, and carries no
# mutating payload (no command, no sudo). Its runbook field names the
# expected section.
logging_record_count="$(LOGGING_DESCRIPTION="$LOGGING_DESCRIPTION" yq eval -r \
  '[.macos.system_setup[] | select(.description == strenv(LOGGING_DESCRIPTION))] | length' \
  "$SETUP_YAML")"
[[ $logging_record_count -eq 1 ]] ||
  fail "expected exactly one logging record (description: $LOGGING_DESCRIPTION); found $logging_record_count"

logging_tier="$(LOGGING_DESCRIPTION="$LOGGING_DESCRIPTION" yq eval -r \
  '.macos.system_setup[] | select(.description == strenv(LOGGING_DESCRIPTION)) | .tier' \
  "$SETUP_YAML")"
[[ $logging_tier == "manual" ]] ||
  fail "the logging record must be tier: manual (socketfilterfw on 26.2 has no logging flag); found tier: $logging_tier"

logging_payload_keys="$(LOGGING_DESCRIPTION="$LOGGING_DESCRIPTION" yq eval -r \
  '[.macos.system_setup[] | select(.description == strenv(LOGGING_DESCRIPTION)) | keys | .[] | select(. == "command" or . == "sudo")] | join(",")' \
  "$SETUP_YAML")"
[[ -z $logging_payload_keys ]] ||
  fail "the logging record must carry no mutating payload; found: $logging_payload_keys"

logging_runbook="$(LOGGING_DESCRIPTION="$LOGGING_DESCRIPTION" yq eval -r \
  '.macos.system_setup[] | select(.description == strenv(LOGGING_DESCRIPTION)) | .runbook' \
  "$SETUP_YAML")"
[[ $logging_runbook == "$LOGGING_RUNBOOK_SECTION" ]] ||
  fail "the logging record must point at the runbook section \"$LOGGING_RUNBOOK_SECTION\"; found: $logging_runbook"

# Criterion 5: EVERY manual record's runbook field resolves to a real markdown
# heading in the runbook. An absent runbook field surfaces as the literal text
# null and fails the heading lookup, so a dangling or missing pointer cannot
# pass.
runbook_headings="$work/runbook-headings"
sed -E -n 's/^#{1,6} //p' "$RUNBOOK" >"$runbook_headings"
manual_runbook_sections="$work/manual-runbook-sections"
yq eval -r '[.macos.system_setup[] | select(.tier == "manual") | .runbook] | .[]' \
  "$SETUP_YAML" >"$manual_runbook_sections"
while IFS= read -r runbook_section; do
  grep -qxF -- "$runbook_section" "$runbook_headings" ||
    fail "manual record points at runbook section \"$runbook_section\", which is not a heading in $RUNBOOK"
done <"$manual_runbook_sections"

# ---- render properties (criteria 1, 2, 3; darwin render only) ---------------

render_home="$work/render-home"
mkdir -p "$render_home"
rendered="$work/rendered"
render_error="$work/render.err"
HOME="$render_home" chezmoi --source "$REPO_ROOT" execute-template --no-tty \
  <"$TEMPLATE" >"$rendered" 2>"$render_error" ||
  fail "the Tier 2 runner must render against the real data (stderr: $(cat "$render_error"))"
# The skip is gated on the ACTUAL host OS, never on the render coming out
# empty: emptiness conflates "non-darwin host" (skip, by design) with "the
# template's OS guard is broken on darwin" (a failure this test exists to
# catch). An empty render on darwin must fail loudly, not skip.
[[ "$(uname)" == Darwin ]] || {
  printf 'SKIP: non-darwin host; render assertions not exercised\n'
  exit 0
}
[[ -n "$(tr -d '[:space:]' <"$rendered")" ]] ||
  fail "the Tier 2 runner rendered EMPTY on a darwin host; its OS guard is broken (template: $TEMPLATE; stderr: $(cat "$render_error"))"

# Whole-line match (-x) pins the sudo prefix; uniqueness makes the returned
# line number meaningful for the order comparison.
line_number_of_unique_line() { # <exact-line> <label>
  local hits
  hits="$(grep -nxF -- "$1" "$rendered")" ||
    fail "$2 must render exactly once; not found: $1"
  [[ $(printf '%s\n' "$hits" | wc -l) -eq 1 ]] ||
    fail "$2 must render exactly once; found multiple: $hits"
  printf '%s' "${hits%%:*}"
}

# Criterion 1: global state strictly before stealth, by line number. The two
# signed-software policies are held behind the global-state line too: they are
# the same inert-while-off preference class.
global_state_line="$(line_number_of_unique_line "sudo $GLOBAL_STATE_COMMAND" "the sudo-prefixed global-state command")"
stealth_line="$(line_number_of_unique_line "sudo $STEALTH_COMMAND" "the sudo-prefixed stealth command")"
((global_state_line < stealth_line)) ||
  fail "global state (line $global_state_line) must render STRICTLY BEFORE stealth (line $stealth_line); stealth is inert while the firewall is off"

# Criterion 2: each signed-software command individually, sudo-prefixed.
allow_builtin_signed_line="$(line_number_of_unique_line "sudo $ALLOW_BUILTIN_SIGNED_COMMAND" "the sudo-prefixed built-in signed-software command")"
allow_downloaded_signed_line="$(line_number_of_unique_line "sudo $ALLOW_DOWNLOADED_SIGNED_COMMAND" "the sudo-prefixed downloaded signed-software command")"
((global_state_line < allow_builtin_signed_line)) ||
  fail "global state (line $global_state_line) must render before the built-in signed-software policy (line $allow_builtin_signed_line)"
((global_state_line < allow_downloaded_signed_line)) ||
  fail "global state (line $global_state_line) must render before the downloaded signed-software policy (line $allow_downloaded_signed_line)"

# Criterion 3: the logging record renders its runbook pointer...
manual_pointer_line="echo '→ MANUAL ${LOGGING_DESCRIPTION}: see the runbook section ${LOGGING_RUNBOOK_SECTION}'"
grep -qxF -- "$manual_pointer_line" "$rendered" ||
  fail "the logging record must render its runbook pointer (expected: $manual_pointer_line)"

# ...and NO command: every socketfilterfw line in the render is one of the
# four declared sudo-prefixed commands, byte for byte. A completeness diff,
# not a count, so an extra or altered line is reported by name.
rendered_firewall_lines="$work/rendered-firewall-lines"
{ grep -F 'socketfilterfw' "$rendered" || true; } | LC_ALL=C sort >"$rendered_firewall_lines"
expected_rendered_firewall_lines="$work/expected-rendered-firewall-lines"
LC_ALL=C sort >"$expected_rendered_firewall_lines" <<EOF
sudo $GLOBAL_STATE_COMMAND
sudo $STEALTH_COMMAND
sudo $ALLOW_BUILTIN_SIGNED_COMMAND
sudo $ALLOW_DOWNLOADED_SIGNED_COMMAND
EOF
cmp -s "$expected_rendered_firewall_lines" "$rendered_firewall_lines" ||
  fail "the render must contain exactly the four declared socketfilterfw command lines (diff: $(diff "$expected_rendered_firewall_lines" "$rendered_firewall_lines" || true))"

printf 'macos-firewall-globalstate-order: OK (global state renders first; both signed-software policies render sudo-prefixed; the logging record renders a pointer and no command; every declared firewall command is set-to-state; every manual runbook pointer resolves)\n'
